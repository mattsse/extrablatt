#![allow(unused)]

use crate::language::Language;
use std::time::Duration;

pub mod article;
pub mod category;
pub mod date;
mod error;
pub mod extract;
pub mod fulltext;
pub mod image;
pub mod language;
pub mod stopwords;
pub mod summarize;
pub mod video;

#[derive(Debug)]
pub struct Newspaper {}

impl Newspaper {
    pub fn builder<T: ToString>(url: T) -> NewspaperBuilder {
        NewspaperBuilder::new(url)
    }
}

#[derive(Debug)]
pub struct NewspaperBuilder {
    language: Option<Language>,
}

impl NewspaperBuilder {
    pub fn new<T: ToString>(url: T) -> Self {
        unimplemented!()
    }

    pub fn build(self) -> anyhow::Result<Newspaper> {
        unimplemented!()
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
    /// Max. number of urls to cache for each news source.
    max_file_cache: usize,
    /// Cache and save articles run after run.
    memoize_articles: bool,
    /// Whether to extract images from the site.
    fetch_images: bool,
    /// Max ratio for height/width to extract. Ignore if greater.
    max_image_dimension_ration: (usize, usize),
    /// Follow meta refresh redirect when downloading
    follow_meta_refresh: bool,
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
    pub const DEFAULT_TIMEOUT_SEC: u64 = 7;

    /// Default user agent for `extrablatt`.
    pub(crate) fn user_agent() -> String {
        format!("extrablatt/{}", env!("CARGO_PKG_VERSION"))
    }

    /// Convenience method to create a [`ConfigBuilder`]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
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
    max_file_cache: Option<usize>,
    /// Cache and save articles run after run.
    memoize_articles: Option<bool>,
    /// Whether to extract images from the site.
    fetch_images: Option<bool>,
    /// Max ratio for width/height to extract. Ignore if greater.
    max_image_dimension_ration: Option<(usize, usize)>,
    /// Follow meta refresh redirect when downloading
    follow_meta_refresh: Option<bool>,
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

    pub fn max_file_cache(mut self, max_file_cache: usize) -> Self {
        self.max_file_cache = Some(max_file_cache);
        self
    }

    pub fn memoize_articles(mut self, memoize_articles: bool) -> Self {
        self.memoize_articles = Some(memoize_articles);
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

    pub fn follow_meta_refresh(mut self, follow_meta_refresh: bool) -> Self {
        self.follow_meta_refresh = Some(follow_meta_refresh);
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
            max_file_cache: self.max_file_cache.unwrap_or(2_0000),
            memoize_articles: self.memoize_articles.unwrap_or(true),
            fetch_images: self.fetch_images.unwrap_or(true),
            max_image_dimension_ration: self.max_image_dimension_ration.unwrap_or((16, 9)),
            follow_meta_refresh: self.follow_meta_refresh.unwrap_or_default(),
            keep_article_html: self.keep_article_html.unwrap_or_default(),
            browser_user_agent: self
                .browser_user_agent
                .unwrap_or_else(|| Config::user_agent()),
            request_timeout: self
                .request_timeout
                .unwrap_or_else(|| Duration::from_secs(Config::DEFAULT_TIMEOUT_SEC)),
            http_success_only: self.http_success_only.unwrap_or(true),
        }
    }
}
