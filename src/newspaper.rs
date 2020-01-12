use std::borrow::{Borrow, Cow};
use std::collections::hash_map::IntoIter;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Output;
use std::time::Duration;

use bytes::Bytes;
use fnv::{FnvHashMap, FnvHashSet};
use futures::future::{join_all, select_all, SelectAll};
use futures::stream::{FuturesUnordered, Stream};
use futures::task::Poll;
use futures::{Future, FutureExt, StreamExt, TryFutureExt};
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::{Body, Error, Response};
use reqwest::{Client, ClientBuilder, IntoUrl, StatusCode, Url};
use select::document::Document;
use wasm_timer::Instant;

use anyhow::{anyhow, Context, Result};

use crate::article::{Article, ArticleContent, ArticleUrl};
use crate::error::ExtrablattError;
use crate::extract::{DefaultExtractor, Extractor};
use crate::language::Language;
use crate::storage::ArticleStore;

#[derive(Debug)]
pub struct Newspaper<TExtractor: Extractor = DefaultExtractor> {
    /// The [`reqwest::Client`] that drives requests.
    client: Client,
    /// The expected language of this newspaper.
    language: Language,
    /// The parsed main page.
    main_page: Document,
    /// Url of the main page.
    base_url: Url,
    /// The [`extrablatt::Extractor`] used for content retrieval.
    ///
    /// Default is [`extrablatt::DefaultExtractor`].
    extractor: TExtractor,
    /// Cache for retrieved articles.
    articles: FnvHashMap<ArticleUrl, DocumentDownloadState>,
    /// All known categories for this newspaper.
    categories: FnvHashMap<Category, DocumentDownloadState>,
    /// Configuration for article extraction.
    config: Config,
}

impl Newspaper<DefaultExtractor> {
    /// Convenience method for creating a new [`NewspaperBuilder`]
    ///
    /// Same as calling [`NewspaperBuilder::new`]
    #[inline]
    pub fn builder<T: IntoUrl>(url: T) -> Result<NewspaperBuilder> {
        NewspaperBuilder::new(url)
    }
}

impl<TExtractor: Extractor> Newspaper<TExtractor> {
    #[inline]
    pub fn clear(&mut self) {
        self.articles.clear();
        self.categories.clear()
    }

    #[inline]
    pub fn language(&self) -> &Language {
        &self.language
    }

    /// The used to store and detect valid articles.
    #[inline]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[inline]
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// The extractor used to retrieve content for an article.
    #[inline]
    pub fn extractor(&self) -> &TExtractor {
        &self.extractor
    }

    /// Iterator over all available categories.
    #[inline]
    pub fn categories(&self) -> impl Iterator<Item = &Category> {
        self.categories.keys()
    }

    /// Insert all categories extracted from the main page.
    fn insert_new_categories(&mut self) {
        for category in self.extractor.categories(&self.main_page, &self.base_url) {
            self.categories
                .entry(category)
                .or_insert(DocumentDownloadState::NotRequested);
        }
    }

    // TODO check cache restrictions
    fn insert_new_articles(&mut self) {
        for category_doc in self.categories.values().filter_map(|doc| match doc {
            DocumentDownloadState::Success { doc, .. } => Some(doc),
            _ => None,
        }) {
            for article in self
                .extractor
                .article_urls(&category_doc, Some(&self.base_url))
            {
                self.articles
                    .entry(article)
                    .or_insert(DocumentDownloadState::NotRequested);
            }
        }
    }

    pub async fn download_articles(&mut self) -> ArticleDownloadIter<'_, TExtractor> {
        let results = join_all(
            self.articles
                .iter()
                .filter_map(|(article, state)| {
                    if state.is_not_requested() {
                        Some(article.url.clone())
                    } else {
                        None
                    }
                })
                .map(|url| {
                    self.client.get(url.clone()).send().then(|res| async {
                        (url, DocumentDownloadState::from_response(res).await)
                    })
                }),
        )
        .await;

        for (url, doc) in results {
            *self.articles.get_mut(&url).unwrap() = doc.unwrap();
        }

        ArticleDownloadIter {
            inner: self.articles.iter(),
            extractor: &self.extractor,
            language: self.language.clone(),
            base_url: &self.base_url,
        }
    }

    pub fn iter_articles(&self) -> impl Iterator<Item = (&ArticleUrl, ArticleContent<'_>)> {
        self.articles.iter().filter_map(move |(url, doc)| {
            if let DocumentDownloadState::Success { doc, .. } = doc {
                Some((
                    url,
                    self.extractor.article_content(
                        doc,
                        Some(&self.base_url),
                        Some(self.language.clone()),
                    ),
                ))
            } else {
                None
            }
        })
    }

    pub async fn retry_download_categories(&mut self) {
        unimplemented!()
    }

    pub async fn download_categories(
        &mut self,
    ) -> Vec<std::result::Result<Category, (Category, ExtrablattError)>> {
        // join all futures
        let requests = join_all(
            self.categories
                .iter()
                .filter_map(|(cat, state)| {
                    if state.is_not_requested() {
                        Some(cat)
                    } else {
                        None
                    }
                })
                .map(|cat| {
                    let cat = cat.clone();
                    self.client.get(cat.url.clone()).send().then(|res| async {
                        (cat, DocumentDownloadState::from_response(res).await)
                    })
                }),
        )
        .await;

        let mut results = Vec::with_capacity(requests.len());

        // save to unwrap since all responses are wrapped in an `Ok`.
        for (cat, res) in requests {
            match res {
                Ok(res) => {
                    if let DocumentDownloadState::Success { doc, .. } = &res {
                        for url in self.extractor.article_urls(doc, Some(&self.base_url)) {
                            self.articles
                                .entry(url)
                                .or_insert(DocumentDownloadState::NotRequested);
                        }
                    }
                    *self.categories.get_mut(&cat).unwrap() = res;
                    results.push(Ok(cat));
                }
                Err((doc, err)) => {
                    *self.categories.get_mut(&cat).unwrap() = doc;
                    results.push(Err((cat, err)));
                }
            }
        }
        results
    }

    /// Refresh the main page for the newspaper and returns the old document.
    pub async fn refresh_homepage(&mut self) -> Result<Document> {
        let main_page = self
            .get_document(self.base_url.clone())
            .await
            .map_err(|(_, err)| err)?
            .success_document()?;

        // extract all categories
        self.insert_new_categories();

        Ok(std::mem::replace(&mut self.main_page, main_page))
    }

    #[cfg(feature = "archive")]
    pub fn archive(&self) {
        unimplemented!("coming as soon as reqwest 0.10 is stabilized.")
    }

    async fn get_document(
        &self,
        url: Url,
    ) -> std::result::Result<DocumentDownloadState, (DocumentDownloadState, ExtrablattError)> {
        let resp = self.client.get(url).send().await;
        DocumentDownloadState::from_response(resp).await
    }
}

//impl<TExtractor: Extractor> IntoIterator for Newspaper<TExtractor> {
//    type Item = std::result::Result<Article, ExtrablattError>;
//    type IntoIter = std::slice::Iter<'static, std::result::Result<Article,
// ExtrablattError>>;
//
//    fn into_iter(self) -> Self::IntoIter {
//        unimplemented!()
//    }
//}

impl<TExtractor: Extractor + Unpin> Newspaper<TExtractor> {
    /// Converts the newspaper into a stream, yielding all available
    /// [`extrablatt::Article`]s.
    ///
    /// All available categories will be fetched first, hence the `async`.
    pub async fn into_stream(
        mut self,
    ) -> impl Stream<Item = std::result::Result<Article, ExtrablattError>> {
        let _ = self.download_categories().await;

        let mut articles = Vec::new();
        let mut responses = Vec::new();

        let mut extracted = FnvHashMap::default();
        std::mem::swap(&mut extracted, &mut self.articles);

        for (article_url, doc) in extracted.into_iter() {
            match doc {
                DocumentDownloadState::NotRequested => {
                    let x: Pin<Box<dyn Future<Output = _>>> = self
                        .client
                        .get(article_url.url.clone())
                        .send()
                        .and_then(|resp| async {
                            resp.bytes().await.map(|bytes| (article_url.url, bytes))
                        })
                        .boxed();

                    responses.push(x);
                }
                DocumentDownloadState::Success { doc, .. } => {
                    let article = Article {
                        content: self
                            .extractor
                            .article_content(
                                &doc,
                                Some(&self.base_url),
                                Some(self.language.clone()),
                            )
                            .into_owned(),
                        url: article_url.url,
                        language: self
                            .extractor
                            .meta_language(&doc)
                            .unwrap_or_else(|| self.language.clone()),
                        doc,
                    };
                    articles.push(article);
                }
                _ => {}
            }
        }
        ArticleStream {
            paper: self,
            responses,
            articles,
        }
    }
}

type ArticleResponse =
    Pin<Box<dyn Future<Output = std::result::Result<(Url, Bytes), reqwest::Error>>>>;

pub struct ArticleStream<TExtractor: Extractor> {
    paper: Newspaper<TExtractor>,
    responses: Vec<ArticleResponse>,
    articles: Vec<Article>,
}

impl<TExtractor: Extractor + Unpin> Stream for ArticleStream<TExtractor> {
    type Item = std::result::Result<Article, ExtrablattError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if let Some(ready) = self.articles.pop() {
            return Poll::Ready(Some(Ok(ready)));
        }
        if self.responses.is_empty() {
            return Poll::Ready(None);
        }

        let item =
            self.responses
                .iter_mut()
                .enumerate()
                .find_map(|(i, f)| match f.as_mut().poll(cx) {
                    Poll::Pending => None,
                    Poll::Ready(resp) => Some((i, resp)),
                });

        match item {
            Some((idx, resp)) => {
                let _ = self.responses.swap_remove(idx);
                let article = match resp {
                    Ok((url, body)) => {
                        if let Ok(doc) = Document::from_read(&*body) {
                            let content = self
                                .paper
                                .extractor
                                .article_content(
                                    &doc,
                                    Some(&self.paper.base_url),
                                    Some(self.paper.language.clone()),
                                )
                                .into_owned();
                            let language = self
                                .paper
                                .extractor
                                .meta_language(&doc)
                                .unwrap_or_else(|| self.paper.language.clone());
                            Ok(Article {
                                url,
                                doc,
                                content,
                                language,
                            })
                        } else {
                            Err(ExtrablattError::ReadDocumentError { body })
                        }
                    }
                    Err(error) => Err(ExtrablattError::HttpRequestFailure { error }),
                };

                Poll::Ready(Some(article))
            }
            None => Poll::Pending,
        }
    }
}

#[derive(Debug)]
pub struct NewspaperBuilder {
    base_url: Option<Url>,
    config: Option<Config>,
    language: Option<Language>,
}

impl NewspaperBuilder {
    pub fn new<T: IntoUrl>(base_url: T) -> Result<Self> {
        Ok(Self {
            base_url: Some(base_url.into_url()?),
            config: None,
            language: None,
        })
    }

    pub fn language(mut self, language: Language) -> Self {
        self.language = Some(language);
        self
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub async fn build_with_extractor<TExtractor: Extractor>(
        self,
        extractor: TExtractor,
    ) -> Result<Newspaper<TExtractor>> {
        let base_url = self
            .base_url
            .context("Url of the article must be initialized.")?;

        if base_url.cannot_be_a_base() {
            return Err(anyhow!("url {:?} can not be a base url", base_url));
        }

        let config = self.config.unwrap_or_default();

        let mut headers = HeaderMap::with_capacity(1);

        headers.insert(
            USER_AGENT,
            config.browser_user_agent.parse().context(format!(
                "Failed to parse user agent header name: {}",
                config.browser_user_agent
            ))?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(config.request_timeout)
            .build()?;

        let resp = client.get(base_url.clone()).send().await;

        let main_page = DocumentDownloadState::from_response(resp)
            .await
            .map_err(|(_, err)| err)?
            .success_document()?;

        let mut paper = Newspaper {
            client,
            language: self.language.unwrap_or_default(),
            main_page,
            base_url,
            extractor,
            categories: Default::default(),
            articles: FnvHashMap::with_capacity_and_hasher(
                config.max_doc_cache,
                Default::default(),
            ),
            config,
        };

        paper.insert_new_categories();

        Ok(paper)
    }

    pub async fn build(self) -> Result<Newspaper> {
        let base_url = self
            .base_url
            .clone()
            .context("Url of the article must be initialized.")?;
        self.build_with_extractor(Default::default()).await
    }
}

// TODO refactor Error State with `ExtrablattError`

#[derive(Debug)]
pub enum DocumentDownloadState {
    /// No request sent yet.
    NotRequested,
    /// Received a success response at `received` and successfully parsed the
    /// html document.
    Success {
        /// Timestamp the response was received.
        received: Instant,
        /// The parsed html body.
        doc: Document,
    },
    NoHttpSuccessResponse {
        /// Timestamp the response was received.
        received: Instant,
    },
    /// Received an error response.
    HttpRequestFailure {
        /// Timestamp the response was received.
        received: Instant,
    },
    /// Received a success response at `received` but failed to parse the `body`
    /// into a [`select::Document`]
    DocumentReadFailure {
        /// Timestamp the response was received.
        received: Instant,
    },
}

impl DocumentDownloadState {
    pub(crate) async fn from_response(
        response: std::result::Result<Response, reqwest::Error>,
    ) -> std::result::Result<Self, (Self, ExtrablattError)> {
        match response {
            Ok(response) => {
                if response.status().is_success() {
                    match response.bytes().await {
                        Ok(body) => {
                            if let Ok(doc) = Document::from_read(&*body) {
                                Ok(DocumentDownloadState::Success {
                                    received: Instant::now(),
                                    doc,
                                })
                            } else {
                                Err((
                                    DocumentDownloadState::DocumentReadFailure {
                                        received: Instant::now(),
                                    },
                                    ExtrablattError::ReadDocumentError { body },
                                ))
                            }
                        }
                        Err(error) => Err((
                            DocumentDownloadState::HttpRequestFailure {
                                received: Instant::now(),
                            },
                            ExtrablattError::HttpRequestFailure { error },
                        )),
                    }
                } else {
                    Err((
                        DocumentDownloadState::NoHttpSuccessResponse {
                            received: Instant::now(),
                        },
                        ExtrablattError::NoHttpSuccessResponse { response },
                    ))
                }
            }
            Err(error) => Err((
                DocumentDownloadState::HttpRequestFailure {
                    received: Instant::now(),
                },
                ExtrablattError::HttpRequestFailure { error },
            )),
        }
    }

    pub fn success_document(self) -> Result<Document> {
        match self {
            DocumentDownloadState::NotRequested => Err(anyhow!("Not requested yet")),
            DocumentDownloadState::Success { doc, .. } => Ok(doc),
            DocumentDownloadState::NoHttpSuccessResponse { .. } => {
                Err(anyhow!("Unsuccessful response."))
            }
            DocumentDownloadState::HttpRequestFailure { .. } => Err(anyhow!("Failed request.")),
            DocumentDownloadState::DocumentReadFailure { .. } => {
                Err(anyhow!("Failed to parse document."))
            }
        }
    }

    pub fn is_http_failure(&self) -> bool {
        match self {
            DocumentDownloadState::HttpRequestFailure { .. } => true,
            _ => false,
        }
    }

    pub fn is_no_http_success_response(&self) -> bool {
        match self {
            DocumentDownloadState::NoHttpSuccessResponse { .. } => true,
            _ => false,
        }
    }

    pub fn is_doc_parsing_failure(&self) -> bool {
        match self {
            DocumentDownloadState::DocumentReadFailure { .. } => true,
            _ => false,
        }
    }

    pub fn is_not_requested(&self) -> bool {
        match self {
            DocumentDownloadState::NotRequested { .. } => true,
            _ => false,
        }
    }

    pub fn is_success(&self) -> bool {
        match self {
            DocumentDownloadState::Success { .. } => true,
            _ => false,
        }
    }
}

impl Default for DocumentDownloadState {
    fn default() -> Self {
        DocumentDownloadState::NotRequested
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Category {
    /// The address for the category website.
    pub url: Url,
}

impl Category {
    pub fn new(url: Url) -> Self {
        Self { url }
    }

    pub fn language_hint(&self) -> Option<Language> {
        for lang in Language::known_languages() {
            let full_name = lang.full_name().to_lowercase();
            let id = lang.identifier();
            if let Some(domain) = &self.url.domain() {
                if domain.ends_with(&format!(".{}", id))
                    || domain.starts_with(&format!("{}.", id))
                    || domain.starts_with(&format!("{}.", full_name))
                {
                    return Some(lang.clone());
                }
            }
            if let Some(mut seg) = self.url.path_segments() {
                if seg.next().map(str::to_lowercase) == Some(full_name) {
                    return Some(lang.clone());
                }
            }
        }

        None
    }
}

impl Borrow<str> for Category {
    fn borrow(&self) -> &str {
        self.url.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Number of word tokens in the text.
    min_word_count: Option<usize>,
    /// Number of sentence tokens.
    min_sentence_count: Option<usize>,
    /// Number of chars for the text's title.
    max_title_len: Option<usize>,
    /// Number of chars for the text.
    max_text_len: Option<usize>,
    /// Number of keywords for the text.
    max_keywords: Option<usize>,
    /// Number of Authors.
    max_authors: Option<usize>,
    /// Max. number of urls to cache for a news source.
    max_doc_cache: usize,
    /// Save articles.
    article_storage: Option<ArticleStore>,
    /// Whether to also capture non 2XX responses.
    http_success_only: bool,
    /// The user-agent used for requests.
    browser_user_agent: String,
    /// Timeout for requests.
    request_timeout: Duration,
}

impl Config {
    /// Default timeout for requests made inside `extrablatt`.
    pub const DEFAULT_REQUEST_TIMEOUT_SEC: u64 = 30;

    /// Default user agent for `extrablatt`.
    #[inline]
    pub(crate) fn user_agent() -> String {
        format!("extrablatt/{}", env!("CARGO_PKG_VERSION"))
    }

    /// Convenience method to create a [`ConfigBuilder`]
    #[inline]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Config::builder().build()
    }
}

#[derive(Debug, Default)]
pub struct ConfigBuilder {
    /// Number of word tokens in the text.
    min_word_count: Option<usize>,
    /// Number of sentence tokens.
    min_sentence_count: Option<usize>,
    /// Number of chars for the text's title.
    max_title_len: Option<usize>,
    /// Number of chars for the text.
    max_text_len: Option<usize>,
    /// Number of keywords for the text.
    max_keywords: Option<usize>,
    /// Number of Authors.
    max_authors: Option<usize>,
    /// Max. number of urls to cache for each news source.
    max_doc_cache: Option<usize>,
    /// Storage for articles.
    article_storage: Option<ArticleStore>,
    /// Whether to also capture non 2XX responses.
    http_success_only: Option<bool>,
    /// The user-agent used for requests.
    browser_user_agent: Option<String>,
    /// Timeout for requests.
    request_timeout: Option<Duration>,
}

impl ConfigBuilder {
    pub fn min_word_count(mut self, min_word_count: usize) -> Self {
        self.min_word_count = Some(min_word_count);
        self
    }
    pub fn min_sentence_count(mut self, min_sentence_count: usize) -> Self {
        self.min_sentence_count = Some(min_sentence_count);
        self
    }

    pub fn max_title_len(mut self, max_title_len: usize) -> Self {
        self.max_title_len = Some(max_title_len);
        self
    }

    pub fn max_text_len(mut self, max_text_len: usize) -> Self {
        self.max_text_len = Some(max_text_len);
        self
    }

    pub fn max_keywords(mut self, max_keywords: usize) -> Self {
        self.max_keywords = Some(max_keywords);
        self
    }

    pub fn max_authors(mut self, max_authors: usize) -> Self {
        self.max_authors = Some(max_authors);
        self
    }

    pub fn max_doc_cache(mut self, max_doc_cache: usize) -> Self {
        self.max_doc_cache = Some(max_doc_cache);
        self
    }

    pub fn article_storage(mut self, article_storage: ArticleStore) -> Self {
        self.article_storage = Some(article_storage);
        self
    }

    pub fn http_success_only(mut self, keep_article_html: bool) -> Self {
        self.http_success_only = Some(keep_article_html);
        self
    }

    pub fn browser_user_agent<T: ToString>(mut self, browser_user_agent: T) -> Self {
        self.browser_user_agent = Some(browser_user_agent.to_string());
        self
    }

    pub fn request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = Some(request_timeout);
        self
    }

    pub fn build(self) -> Config {
        Config {
            min_word_count: self.min_word_count,
            min_sentence_count: self.min_sentence_count,
            max_title_len: self.max_title_len,
            max_text_len: self.max_text_len,
            max_keywords: self.max_keywords,
            max_authors: self.max_authors,
            max_doc_cache: self.max_doc_cache.unwrap_or(2_0000),
            article_storage: self.article_storage,
            http_success_only: self.http_success_only.unwrap_or(true),
            browser_user_agent: self.browser_user_agent.unwrap_or_else(Config::user_agent),
            request_timeout: self
                .request_timeout
                .unwrap_or_else(|| Duration::from_secs(Config::DEFAULT_REQUEST_TIMEOUT_SEC)),
        }
    }

    /// Pre sets some restrictions regarding the article content:
    ///     * Word Count >= 300
    ///     * Sentences >= 7
    ///     * Text Length >= 100_000
    pub fn with_restrictions() -> Self {
        Self {
            min_word_count: Some(300),
            min_sentence_count: Some(7),
            max_title_len: Some(200),
            max_text_len: Some(100_000),
            max_keywords: None,
            max_authors: None,
            max_doc_cache: Some(2_0000),
            article_storage: None,
            http_success_only: None,
            browser_user_agent: None,
            request_timeout: None,
        }
    }
}

/// Iterator over the downloaded articles.
pub struct ArticleDownloadIter<'a, T: Extractor> {
    /// Each found url for an article paired with the result of it's request.
    inner: std::collections::hash_map::Iter<'a, ArticleUrl, DocumentDownloadState>,
    /// The `Extractor` used to get the article's content.
    extractor: &'a T,
    /// Language of the news source.
    language: Language,
    /// Base url of the news source.
    base_url: &'a Url,
}

impl<'a, T: Extractor> ArticleDownloadIter<'a, T> {
    /// All successfully retrieved Articles.
    pub fn successes(self) -> impl Iterator<Item = (&'a ArticleUrl, ArticleContent<'a>)> + 'a {
        let extractor = self.extractor;
        let language = self.language;
        let base_url = self.base_url;
        self.inner.filter_map(move |(url, doc)| {
            if let DocumentDownloadState::Success { doc, .. } = doc {
                Some((
                    url,
                    extractor.article_content(doc, Some(base_url), Some(language.clone())),
                ))
            } else {
                None
            }
        })
    }
}

impl<'a, T: Extractor> Deref for ArticleDownloadIter<'a, T> {
    type Target = std::collections::hash_map::Iter<'a, ArticleUrl, DocumentDownloadState>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T: Extractor> DerefMut for ArticleDownloadIter<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_lang_hint() {
        let category = Category::new(Url::parse("https://arabic.cnn.com/").unwrap());
        assert_eq!(category.language_hint(), Some(Language::Arabic));

        let category = Category::new(Url::parse("https://cnn.com/Arabic/").unwrap());
        assert_eq!(category.language_hint(), Some(Language::Arabic));

        let category = Category::new(Url::parse("https://cnn.com/Europe").unwrap());
        assert_eq!(category.language_hint(), None);
    }
}
