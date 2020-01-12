use std::borrow::{Borrow, Cow};
use std::collections::hash_map::IntoIter;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Output;
use std::time::Duration;

use bytes::Bytes;
use fnv::{FnvHashMap, FnvHashSet};
use futures::future::SelectAll;
use futures::stream::{self, BufferUnordered, FuturesUnordered, Stream};
use futures::task::{Poll, Spawn};
use futures::{Future, FutureExt, StreamExt, TryFutureExt, TryStreamExt};
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
    /// Clear all cached articles and categories.
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

    /// For each successfully downloaded category document, insert their article
    /// urls as unrequested.
    fn insert_new_outstanding_articles(&mut self) {
        for category_doc in self.categories.values().filter_map(|doc| match doc {
            DocumentDownloadState::Success { doc, .. } => Some(doc),
            _ => None,
        }) {
            for article in self
                .extractor
                .article_urls(&category_doc, Some(&self.base_url))
                .into_iter()
                .take(self.config.max_doc_cache - self.articles.len())
            {
                self.articles
                    .entry(article)
                    .or_insert(DocumentDownloadState::NotRequested);
            }
        }
    }

    /// Download and store all outstanding articles and returns an iterator over
    /// their results.
    ///
    /// # Example
    ///
    /// Loop over all downloaded articles.
    ///
    /// ```edition2018
    /// # use extrablatt::Newspaper;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut newspaper = Newspaper::builder("https://cnn.com/")?.build().await?;
    ///     newspaper.download_outstanding_categories().await?;
    ///     for(url, content) in newspaper.download_articles().await.successes() {
    ///         // ...
    ///     }
    /// #   Ok(())
    /// # }
    /// ```
    pub async fn download_articles(&mut self) -> ArticleDownloadIter<'_, TExtractor> {
        let results = stream::iter(
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
        .buffer_unordered(10)
        .collect::<Vec<_>>()
        .await;

        for (url, doc) in results {
            let state = match doc {
                Ok((doc, received)) => DocumentDownloadState::Success { received, doc },
                Err((state, err)) => {
                    if !self.config.http_success_only {
                        if let Ok((doc, received)) =
                            DocumentDownloadState::advance_non_http_success(err).await
                        {
                            DocumentDownloadState::Success { doc, received }
                        } else {
                            state
                        }
                    } else {
                        state
                    }
                }
            };

            *self.articles.get_mut(&url).unwrap() = state;
        }

        ArticleDownloadIter {
            inner: self.articles.iter(),
            extractor: &self.extractor,
            language: self.language.clone(),
            base_url: &self.base_url,
        }
    }

    /// Iterator over all known articles.
    pub fn iter_articles(&self) -> ArticleDownloadIter<'_, TExtractor> {
        ArticleDownloadIter {
            inner: self.articles.iter(),
            extractor: &self.extractor,
            language: self.language.clone(),
            base_url: &self.base_url,
        }
    }

    pub async fn retry_download_categories(&mut self) {
        unimplemented!()
    }

    fn insert_article_urls(&mut self, doc: &Document) {
        for url in self.extractor.article_urls(doc, Some(&self.base_url)) {
            self.articles
                .entry(url)
                .or_insert(DocumentDownloadState::NotRequested);
        }
    }

    ///
    pub async fn download_category(
        &mut self,
        category: Category,
    ) -> std::result::Result<Category, (Category, ExtrablattError)> {
        if let Some(state) = self.categories.get(&category) {
            if state.is_success() {
                return Ok(category);
            }
        }
        let (state, err) = match self.get_document(category.url.clone()).await {
            Ok((doc, received)) => {
                self.insert_article_urls(&doc);
                (DocumentDownloadState::Success { doc, received }, None)
            }
            Err((state, err)) => {
                if !self.config.http_success_only {
                    match DocumentDownloadState::advance_non_http_success(err).await {
                        Ok((doc, received)) => {
                            self.insert_article_urls(&doc);
                            (DocumentDownloadState::Success { doc, received }, None)
                        }
                        Err(err) => (state, Some(err)),
                    }
                } else {
                    (state, Some(err))
                }
            }
        };
        self.categories.insert(category.clone(), state);
        if let Some(err) = err {
            Err((category, err))
        } else {
            Ok(category)
        }
    }

    /// Download and store all categories that haven't been requested yet.
    pub async fn download_all_outstanding_categories(
        &mut self,
    ) -> Vec<std::result::Result<Category, (Category, ExtrablattError)>> {
        // join all futures
        let requests = stream::iter(
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
        .buffer_unordered(10)
        .collect::<Vec<_>>()
        .await;

        let mut results = Vec::with_capacity(requests.len());

        for (cat, res) in requests {
            let res = match res {
                Ok((doc, received)) => {
                    self.insert_article_urls(&doc);
                    *self.categories.get_mut(&cat).unwrap() =
                        DocumentDownloadState::Success { doc, received };
                    Ok(cat)
                }
                Err((state, err)) => {
                    if !self.config.http_success_only {
                        match DocumentDownloadState::advance_non_http_success(err).await {
                            Ok((doc, received)) => {
                                *self.categories.get_mut(&cat).unwrap() =
                                    DocumentDownloadState::Success { doc, received };
                                Ok(cat)
                            }
                            Err(err) => {
                                *self.categories.get_mut(&cat).unwrap() = state;
                                Err((cat, err))
                            }
                        }
                    } else {
                        *self.categories.get_mut(&cat).unwrap() = state;
                        Err((cat, err))
                    }
                }
            };
            results.push(res);
        }
        results
    }

    /// Refresh the main page for the newspaper and returns the old document.
    pub async fn refresh_homepage(&mut self) -> std::result::Result<Document, ExtrablattError> {
        let (main_page, _) = self
            .get_document(self.base_url.clone())
            .await
            .map_err(|(_, err)| err)?;

        // extract all available categories
        self.insert_new_categories();

        Ok(std::mem::replace(&mut self.main_page, main_page))
    }

    #[cfg(feature = "archive")]
    pub fn archive(&self) {
        unimplemented!("coming as soon as reqwest 0.10 is stabilized and archiveis crate is updated to async/await")
    }

    /// Execute a GET request and return the response wrapped in
    /// [`DocumentDownloadState`].
    async fn get_document(
        &self,
        url: Url,
    ) -> std::result::Result<(Document, Instant), (DocumentDownloadState, ExtrablattError)> {
        let resp = self.client.get(url).send().await;
        DocumentDownloadState::from_response(resp).await
    }
}

impl<TExtractor: Extractor + Unpin> Newspaper<TExtractor> {
    /// Converts the newspaper into a stream, yielding all available
    /// [`extrablatt::Article`]s.
    pub fn into_stream(
        mut self,
    ) -> impl Stream<Item = std::result::Result<Article, ExtrablattError>> {
        let mut articles = Vec::new();
        let mut article_responses = Vec::new();

        let mut extracted = FnvHashMap::default();
        std::mem::swap(&mut extracted, &mut self.articles);

        for (article_url, doc) in extracted.into_iter() {
            match doc {
                DocumentDownloadState::NotRequested => {
                    article_responses.push(self.get_response(article_url.url));
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

        let mut categories = Vec::new();
        let mut category_responses = Vec::new();
        let mut extracted = FnvHashMap::default();
        std::mem::swap(&mut extracted, &mut self.categories);

        for (cat, doc) in extracted {
            match doc {
                DocumentDownloadState::NotRequested => {
                    category_responses.push(self.get_response(cat.url));
                }
                DocumentDownloadState::Success { doc, .. } => {
                    categories.push((cat, doc));
                }
                _ => {}
            }
        }

        ArticleStream {
            paper: self,
            article_responses,
            articles,
            categories,
            category_responses,
        }
    }

    fn get_response(&self, url: Url) -> PaperResponse {
        self.client
            .get(url.clone())
            .send()
            .map_err(|error| ExtrablattError::HttpRequestFailure { error })
            .and_then(|response| async {
                if !response.status().is_success() {
                    Err(ExtrablattError::NoHttpSuccessResponse { response })
                } else {
                    response
                        .bytes()
                        .await
                        .map(|bytes| (url, bytes))
                        .map_err(|error| ExtrablattError::HttpRequestFailure { error })
                }
            })
            .boxed()
    }
}

type PaperResponse =
    Pin<Box<dyn Future<Output = std::result::Result<(Url, Bytes), ExtrablattError>>>>;

/// Stream for getting a `Article` each at a time.
#[must_use = "streams do nothing unless polled"]
pub struct ArticleStream<TExtractor: Extractor> {
    /// The origin newspaper.
    paper: Newspaper<TExtractor>,
    /// Pending responses for an Article html.
    article_responses: Vec<PaperResponse>,
    /// Pending responses for Category html.
    category_responses: Vec<PaperResponse>,
    /// Articles already available.
    articles: Vec<Article>,
    /// Categories already available.
    categories: Vec<(Category, Document)>,
}

impl<TExtractor: Extractor + Unpin> ArticleStream<TExtractor> {
    /// Queue in new requests for articles.
    fn queue_category_articles(&mut self, doc: &Document) {
        for article_url in self
            .paper
            .extractor
            .article_urls(&doc, Some(&self.paper.base_url))
            .into_iter()
        {
            self.article_responses
                .push(self.paper.get_response(article_url.url));
        }
    }

    /// Poll each item and return the index together with the response of first
    /// ready future.
    fn find_ready_response(
        items: &mut [PaperResponse],
        cx: &mut core::task::Context<'_>,
    ) -> Option<(usize, std::result::Result<(Url, Bytes), ExtrablattError>)> {
        items
            .iter_mut()
            .enumerate()
            .find_map(|(i, f)| match f.as_mut().poll(cx) {
                Poll::Pending => None,
                Poll::Ready(resp) => Some((i, resp)),
            })
    }
}

impl<TExtractor: Extractor + Unpin> Stream for ArticleStream<TExtractor> {
    type Item = std::result::Result<Article, ExtrablattError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if let Some(article) = self.articles.pop() {
            return Poll::Ready(Some(Ok(article)));
        }
        if self.article_responses.is_empty() {
            if let Some((_, doc)) = self.categories.pop() {
                // add futures to article_response
                self.queue_category_articles(&doc);
            }

            if self.category_responses.is_empty() {
                // nothing do anymore
                return Poll::Ready(None);
            }

            // poll pending category futures to get new article futures
            let item = Self::find_ready_response(&mut self.category_responses, cx);

            match item {
                Some((idx, resp)) => {
                    let _ = self.category_responses.swap_remove(idx);
                    match resp {
                        Ok((url, body)) => {
                            if let Ok(doc) = Document::from_read(&*body) {
                                self.queue_category_articles(&doc);
                            } else {
                                return Poll::Ready(Some(Err(
                                    ExtrablattError::ReadDocumentError { body },
                                )));
                            }
                        }
                        Err(e) => {
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                }
                None => return Poll::Pending,
            }
        }

        let item = Self::find_ready_response(&mut self.article_responses, cx);

        match item {
            Some((idx, resp)) => {
                let _ = self.article_responses.swap_remove(idx);
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

                            // TODO check completeness
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
                    Err(error) => Err(error),
                };
                Poll::Ready(Some(article))
            }
            None => Poll::Pending,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            self.articles
                .len()
                .saturating_add(self.article_responses.len()),
            None,
        )
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

    /// Create a new builder with a specific extractor.
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

        let (main_page, _) = DocumentDownloadState::from_response(resp)
            .await
            .map_err(|(_, err)| err)?;

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
    /// Wraps the [`hyper::Response`] into the proper state.
    pub(crate) async fn from_response(
        response: std::result::Result<Response, reqwest::Error>,
    ) -> std::result::Result<(Document, Instant), (Self, ExtrablattError)> {
        match response {
            Ok(response) => {
                if response.status().is_success() {
                    Self::read_response(response).await
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

    async fn read_response(
        response: Response,
    ) -> std::result::Result<(Document, Instant), (DocumentDownloadState, ExtrablattError)> {
        match response.bytes().await {
            Ok(body) => {
                if let Ok(doc) = Document::from_read(&*body) {
                    Ok((doc, Instant::now()))
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
    }

    /// If the error is due to an non 2xx response, try to read it into an
    /// `[select::Document]` anyway.
    async fn advance_non_http_success(
        err: ExtrablattError,
    ) -> std::result::Result<(Document, Instant), ExtrablattError> {
        if let ExtrablattError::NoHttpSuccessResponse { response } = err {
            match DocumentDownloadState::read_response(response).await {
                Ok((doc, received)) => Ok((doc, received)),
                Err((_, err)) => Err(err),
            }
        } else {
            Err(err)
        }
    }

    pub fn success_document(&self) -> Option<&Document> {
        match self {
            DocumentDownloadState::Success { doc, .. } => Some(doc),
            _ => None,
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
