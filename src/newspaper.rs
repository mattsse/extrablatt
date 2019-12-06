use std::borrow::Borrow;
use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use fnv::{FnvHashMap, FnvHashSet};
use futures::future::join_all;
use futures::FutureExt;
use futures::stream::{FuturesUnordered, Stream};
use reqwest::{Client, ClientBuilder, IntoUrl, StatusCode, Url};
use reqwest::Body;
use select::document::Document;
use wasm_timer::Instant;

use anyhow::{anyhow, Context, Result};

use crate::article::Article;
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
    article_cache: FnvHashMap<Url, DocumentDownloadState>,
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
        self.article_cache.clear();
        self.categories.clear()
    }

    #[inline]
    pub fn categories(&self) -> impl Iterator<Item = &Category> {
        self.categories.keys()
    }

    fn insert_new_categories(&mut self) {
        for category in self.extractor.categories(&self.main_page, &self.base_url) {
            if !self.categories.contains_key(&category) {
                self.categories
                    .insert(category, DocumentDownloadState::NotRequested);
            }
        }
    }

    async fn extract_articles(&mut self) -> Result<Vec<()>> {
        // join all futures
        let futures: Vec<_> = self
            .categories
            .iter()
            .filter(|(_, state)| state.is_not_requested())
            .map(|(cat, _)| {
                self.client
                    .get(cat.url.clone())
                    .send()
                    .then(move |res| futures::future::ok::<_, ()>((cat, res, Instant::now())))
            })
            .collect();

        for (cat, resp, time) in join_all(futures).await.into_iter().filter_map(|res| {
            if let Ok(r) = res {
                Some(r)
            } else {
                None
            }
        }) {
            // TODO update the documentstate based in response
            // 1. match resp --> insert in categories
        }

        // TODO what to return here? just the &Category as Result or also the Document
        // state which would require additional hashmap lookups
        Ok(vec![])
    }

    /// Refresh the main page for the newspaper and returns the old document.
    pub async fn refresh_homepage(&mut self) -> Result<Document> {
        let main_page = Newspaper::get_document(
            self.base_url.clone(),
            &self.client,
            self.config.http_success_only,
        )
        .await?;

        // extract all categories
        self.insert_new_categories();

        Ok(std::mem::replace(&mut self.main_page, main_page))
    }

    #[cfg(feature = "archive")]
    pub fn archive(&self) {
        unimplemented!()
    }
}

impl Newspaper {
    /// TODO should return DocumentState
    pub(crate) async fn get_document(
        url: Url,
        client: &Client,
        http_success_only: bool,
    ) -> Result<Document> {
        let resp = client.get(url).send().await?;

        if http_success_only {
            if !resp.status().is_success() {
                return Err(ExtrablattError::NoHttpSuccess {
                    status: resp.status(),
                })
                .context(format!("Unsuccessful request to {:?}", resp.url()));
            }
        }

        Ok(Document::from_read(&*resp.bytes().await?)
            .context(format!("Failed to read html as document."))?)
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
        let client = Client::builder().timeout(config.request_timeout).build()?;

        let main_page =
            Newspaper::get_document(base_url.clone(), &client, config.http_success_only).await?;

        let mut paper = Newspaper {
            client,
            language: self.language.unwrap_or_default(),
            main_page,
            base_url,
            extractor,
            categories: Default::default(),
            article_cache: FnvHashMap::with_capacity_and_hasher(
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

#[derive(Debug, Clone)]
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
    /// Received an error response.
    HttpFailure {
        /// Timestamp the response was received.
        received: Instant,
        /// Statuscode of the error response
        status: StatusCode,
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
    pub fn is_http_failure(&self) -> bool {
        match self {
            DocumentDownloadState::HttpFailure { .. } => true,
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
            browser_user_agent: self
                .browser_user_agent
                .unwrap_or_else(|| Config::user_agent()),
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
