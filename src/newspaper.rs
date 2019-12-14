use std::borrow::Borrow;
use std::ops::DerefMut;
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
    pub main_page: Document,
    /// Url of the main page.
    pub base_url: Url,
    /// The [`extrablatt::Extractor`] used for content retrieval.
    ///
    /// Default is [`extrablatt::DefaultExtractor`].
    pub extractor: TExtractor,
    /// Cache for retrieved articles.
    articles: FnvHashMap<ArticleUrl, DocumentDownloadState>,
    /// All known categories for this newspaper.
    categories: FnvHashMap<Category, DocumentDownloadState>,
    /// Configuration for article extraction.
    config: Config,
}

impl<TExtractor: Extractor> Newspaper<TExtractor> {
    /// Convenience method for creating a new [`NewspaperBuilder`]
    ///
    /// Same as calling [`NewspaperBuilder::new`]
    #[inline]
    pub fn builder<T: IntoUrl>(url: T) -> Result<NewspaperBuilder> {
        NewspaperBuilder::new(url)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.articles.clear();
        self.categories.clear()
    }

    #[inline]
    pub fn categories(&self) -> impl Iterator<Item = &Category> {
        self.categories.keys()
    }

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
            for article in self.extractor.article_urls(&category_doc, &self.base_url) {
                self.articles
                    .entry(article)
                    .or_insert(DocumentDownloadState::NotRequested);
            }
        }
    }

    pub async fn get_all_articles(&mut self) -> Result<Vec<ArticleContent<'_>>> {
        // join all futures
        let futures: Vec<_> = self
            .articles
            .iter()
            .filter_map(|(article, state)| {
                if state.is_not_requested() {
                    Some(article.url.clone())
                } else {
                    None
                }
            })
            .map(|url| {
                self.client.get(url.clone()).send().then(|res| {
                    futures::future::ok::<_, ()>((url, DocumentDownloadState::from_response(res)))
                })
            })
            .collect();

        Ok(vec![])
    }

    pub async fn get_all_categories(&mut self) -> Result<Vec<(Category, &DocumentDownloadState)>> {
        // join all futures
        let futures: Vec<_> = self
            .categories
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
                self.client.get(cat.url.clone()).send().then(|res| {
                    futures::future::ok::<_, ()>((cat, DocumentDownloadState::from_response(res)))
                })
            })
            .collect();

        let mut requested_categories = Vec::with_capacity(futures.len());

        // save to unwrap since all responses are wrapped in an `Ok`.
        for (cat, res) in join_all(futures)
            .await
            .into_iter()
            .map(std::result::Result::unwrap)
        {
            let res = res.await;

            if let DocumentDownloadState::Success { doc, .. } = &res {
                for url in self.extractor.article_urls(doc, &self.base_url) {
                    self.articles
                        .entry(url)
                        .or_insert(DocumentDownloadState::NotRequested);
                }
            } else {
                // don't insert if only successful responses are allowed
                if self.config.http_success_only {
                    continue;
                }
            }

            let state = self.categories.get_mut(&cat).unwrap();
            *state = res;

            requested_categories.push(cat);
        }

        // TODO extract articles

        let mut res = Vec::with_capacity(requested_categories.len());
        for cat in requested_categories {
            let state = self.categories.get(&cat).unwrap();
            res.push((cat, state))
        }

        Ok(res)
    }

    /// Refresh the main page for the newspaper and returns the old document.
    pub async fn refresh_homepage(&mut self) -> Result<Document> {
        let main_page = self
            .get_document(self.base_url.clone())
            .await
            .success_document()?;

        // extract all categories
        self.insert_new_categories();

        Ok(std::mem::replace(&mut self.main_page, main_page))
    }

    #[cfg(feature = "archive")]
    pub fn archive(&self) {
        unimplemented!()
    }

    async fn get_document(&self, url: Url) -> DocumentDownloadState {
        let resp = self.client.get(url).send().await;
        DocumentDownloadState::from_response(resp).await
    }
}

impl<TExtractor: Extractor + Unpin> Newspaper<TExtractor> {
    /// Converts the newspaper into a stream, yielding all available
    /// [`extrablatt::Article`]s.
    ///
    /// Fetches all available categories first, hence the `async`.
    pub async fn into_stream(
        mut self,
    ) -> impl Stream<Item = std::result::Result<Article, ExtrablattError>> {
        let _ = self.get_all_categories().await;

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
                        content: self.extractor.article_content(&doc).into_owned(),
                        url: article_url.url,
                        language: self
                            .extractor
                            .meta_language(&doc)
                            .unwrap_or_else(|| self.language.clone()),
                        doc,
                    };
                    articles.push(Ok(article));
                }
                DocumentDownloadState::NoHttpSuccessResponse { response, .. } => {
                    articles.push(Err(ExtrablattError::NoHttpSuccessResponse { response }));
                }
                DocumentDownloadState::HttpRequestFailure { error, .. } => {
                    articles.push(Err(ExtrablattError::HttpRequestFailure { error }));
                }
                DocumentDownloadState::DocumentReadFailure { body, .. } => {
                    articles.push(Err(ExtrablattError::ReadDocumentError { body }));
                }
            }
        }
        ArticleStream {
            paper: self,
            responses,
            articles,
        }
    }
}

pub struct ArticleStream<TExtractor: Extractor> {
    paper: Newspaper<TExtractor>,
    responses:
        Vec<Pin<Box<dyn Future<Output = std::result::Result<(Url, Bytes), reqwest::Error>>>>>,
    articles: Vec<std::result::Result<Article, ExtrablattError>>,
}

impl<TExtractor: Extractor + Unpin> Stream for ArticleStream<TExtractor> {
    type Item = std::result::Result<Article, ExtrablattError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if let Some(ready) = self.articles.pop() {
            return Poll::Ready(Some(ready));
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
                            let content = self.paper.extractor.article_content(&doc).into_owned();
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

#[derive(Debug)]
pub enum DocumentDownloadState {
    /// No request sent yet.
    NotRequested,
    //    /// Waiting for a response for request sent at `started`
    //    PendingResponse {
    //        /// Timestamp the request was sent.
    //        started: Instant,
    //    },
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
        /// The response received.
        response: reqwest::Response,
    },
    /// Received an error response.
    HttpRequestFailure {
        /// Timestamp the response was received.
        received: Instant,
        /// The encountered [`reqwest::Error`].
        error: reqwest::Error,
    },
    /// Received a success response at `received` but failed to parse the `body`
    /// into a [`select::Document`]
    DocumentReadFailure {
        /// Timestamp the response was received.
        received: Instant,
        /// Payload of the response.
        body: Bytes,
    },
}

impl DocumentDownloadState {
    pub(crate) async fn from_response(
        response: std::result::Result<Response, reqwest::Error>,
    ) -> Self {
        match response {
            Ok(response) => {
                if response.status().is_success() {
                    match response.bytes().await {
                        Ok(body) => {
                            if let Ok(doc) = Document::from_read(&*body) {
                                DocumentDownloadState::Success {
                                    received: Instant::now(),
                                    doc,
                                }
                            } else {
                                DocumentDownloadState::DocumentReadFailure {
                                    received: Instant::now(),
                                    body,
                                }
                            }
                        }
                        Err(error) => DocumentDownloadState::HttpRequestFailure {
                            received: Instant::now(),
                            error,
                        },
                    }
                } else {
                    DocumentDownloadState::NoHttpSuccessResponse {
                        received: Instant::now(),
                        response,
                    }
                }
            }
            Err(error) => DocumentDownloadState::HttpRequestFailure {
                received: Instant::now(),
                error,
            },
        }
    }

    pub fn success_document(self) -> std::result::Result<Document, ExtrablattError> {
        match self {
            DocumentDownloadState::NotRequested => Err(ExtrablattError::UrlError {
                msg: "Not requested yet".to_string(),
            }),
            DocumentDownloadState::Success { doc, .. } => Ok(doc),
            DocumentDownloadState::NoHttpSuccessResponse { response, .. } => {
                Err(ExtrablattError::NoHttpSuccessResponse { response })
            }
            DocumentDownloadState::HttpRequestFailure { error, .. } => {
                Err(ExtrablattError::HttpRequestFailure { error })
            }
            DocumentDownloadState::DocumentReadFailure { body, .. } => {
                Err(ExtrablattError::ReadDocumentError { body })
            }
        }
    }

    pub fn is_http_failure(&self) -> bool {
        match self {
            DocumentDownloadState::HttpRequestFailure { .. } => true,
            _ => false,
        }
    }

    pub fn is_failure_response(&self) -> bool {
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
    min_word_count: usize,
    /// Number of sentence tokens.
    min_sentence_count: usize,
    /// Number of chars for the text's title.
    max_title_len: usize,
    /// Number of chars for the text.
    max_text_len: usize,
    /// Number of keywords for the text.
    max_keywords: usize,
    /// Number of Authors.
    max_authors: usize,
    /// Number of chars of the summary.
    max_summary_len: usize,
    /// Number of sentences.
    max_summary_sentences: usize,
    /// Max. number of urls to cache for a news source.
    max_doc_cache: usize,
    /// Save articles.
    article_storage: Option<ArticleStore>,
    /// Whether to extract images from the site.
    fetch_images: bool,
    /// Max ratio for height/width to extract. Ignore if greater.
    max_image_dimension_ration: (usize, usize),
    /// Whether to kee the html of the article.
    keep_article_html: bool,
    /// The user-agent used for requests.
    browser_user_agent: String,
    /// Timeout for requests.
    request_timeout: Duration,
    /// Whether to capture only 2XX responses or failures as well.
    http_success_only: bool,
}

impl Config {
    /// Default timeout for requests made inside `extrablatt`.
    pub const DEFAULT_REQ_TIMEOUT_SEC: u64 = 7;

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
    /// Number of chars of the summary.
    max_summary_len: Option<usize>,
    /// Number of sentences.
    max_summary_sentences: Option<usize>,
    /// Max. number of urls to cache for each news source.
    max_doc_cache: Option<usize>,
    /// Storage for articles.
    article_storage: Option<ArticleStore>,
    /// Whether to extract images from the site.
    fetch_images: Option<bool>,
    /// Max ratio for width/height to extract. Ignore if greater.
    max_image_dimension_ration: Option<(usize, usize)>,
    /// Whether to kee the html of the article.
    keep_article_html: Option<bool>,
    /// The user-agent used for requests.
    browser_user_agent: Option<String>,
    /// Timeout for requests.
    request_timeout: Option<Duration>,
    /// Whether to capture only 2XX responses or failures as well.
    http_success_only: Option<bool>,
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

    pub fn max_summary_len(mut self, max_summary_len: usize) -> Self {
        self.max_summary_len = Some(max_summary_len);
        self
    }

    pub fn max_summary_sentences(mut self, max_summary_sentences: usize) -> Self {
        self.max_summary_sentences = Some(max_summary_sentences);
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

    pub fn fetch_images(mut self, fetch_images: bool) -> Self {
        self.fetch_images = Some(fetch_images);
        self
    }

    pub fn max_image_dimension_ration(mut self, width: usize, height: usize) -> Self {
        self.max_image_dimension_ration = Some((width, height));
        self
    }

    pub fn keep_article_html(mut self, keep_article_html: bool) -> Self {
        self.keep_article_html = Some(keep_article_html);
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

    pub fn http_success_only(mut self, http_success_only: bool) -> Self {
        self.http_success_only = Some(http_success_only);
        self
    }

    pub fn build(self) -> Config {
        Config {
            min_word_count: self.min_word_count.unwrap_or(300),
            min_sentence_count: self.min_sentence_count.unwrap_or(7),
            max_title_len: self.max_title_len.unwrap_or(200),
            max_text_len: self.max_text_len.unwrap_or(100_000),
            max_keywords: self.max_keywords.unwrap_or(35),
            max_authors: self.max_authors.unwrap_or(10),
            max_summary_len: self.max_summary_len.unwrap_or(5_000),
            max_summary_sentences: self.max_summary_sentences.unwrap_or(5),
            max_doc_cache: self.max_doc_cache.unwrap_or(2_0000),
            article_storage: self.article_storage,
            fetch_images: self.fetch_images.unwrap_or(true),
            max_image_dimension_ration: self.max_image_dimension_ration.unwrap_or((16, 9)),
            keep_article_html: self.keep_article_html.unwrap_or_default(),
            browser_user_agent: self.browser_user_agent.unwrap_or_else(Config::user_agent),
            request_timeout: self
                .request_timeout
                .unwrap_or_else(|| Duration::from_secs(Config::DEFAULT_REQ_TIMEOUT_SEC)),
            http_success_only: self.http_success_only.unwrap_or(true),
        }
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
