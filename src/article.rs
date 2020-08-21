use std::borrow::{Borrow, Cow};
use std::hash::{Hash, Hasher};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::{Client, IntoUrl, Url};
use select::document::Document;
#[cfg(feature = "serde0")]
use serde::{Deserialize, Serialize};

use crate::date::ArticleDate;
use crate::error::ExtrablattError;
#[cfg(not(target_arch = "wasm32"))]
use crate::extrablatt::Config;
use crate::extract::{DefaultExtractor, Extractor};
use crate::language::Language;

/// Extension for documents that are considered valid sources for articles.
pub const ALLOWED_FILE_EXT: [&str; 12] = [
    "html", "htm", "md", "rst", "aspx", "jsp", "rhtml", "cgi", "xhtml", "jhtml", "asp", "shtml",
];

/// Segments in an url path that usually point to valid articles.
pub const GOOD_SEGMENTS: [&str; 15] = [
    "story",
    "article",
    "feature",
    "featured",
    "slides",
    "slideshow",
    "gallery",
    "news",
    "video",
    "media",
    "v",
    "radio",
    "press",
    "graphics",
    "investigations",
];

/// Segments in an url path that usually point to invalid articles.
pub const BAD_SEGMENTS: [&str; 17] = [
    "careers",
    "contact",
    "about",
    "faq",
    "terms",
    "privacy",
    "advert",
    "preferences",
    "feedback",
    "info",
    "browse",
    "howto",
    "account",
    "subscribe",
    "donate",
    "shop",
    "admin",
];

/// Domain names that are treated as bad sources for articles.
pub const BAD_DOMAINS: [&str; 4] = ["amazon", "doubleclick", "twitter", "outbrain"];

/// An identified url to an article and it's title.
#[derive(Debug, Clone)]
pub struct ArticleUrl {
    /// The url of the article.
    pub url: Url,
    /// The title of the article.
    pub title: Option<String>,
}

impl ArticleUrl {
    pub fn new(url: Url) -> Self {
        Self { url, title: None }
    }

    pub fn new_with_title<T: ToString>(url: Url, title: Option<T>) -> Self {
        Self {
            url,
            title: title.map(|s| s.to_string()),
        }
    }

    /// If the article is related heavily to media: gallery, video, big
    /// pictures, etc
    pub fn is_media_news(&self) -> bool {
        if let Some(segments) = self.url.path_segments() {
            let media_segemnts = &[
                "video",
                "slide",
                "gallery",
                "powerpoint",
                "fashion",
                "glamour",
                "cloth",
                "graphics",
            ];
            for segment in segments.filter(|s| s.len() < 11) {
                if media_segemnts.contains(&segment.to_lowercase().as_str()) {
                    return true;
                }
            }
        }
        false
    }
}

impl PartialEq for ArticleUrl {
    fn eq(&self, other: &Self) -> bool {
        self.url.eq(&other.url)
    }
}

impl Hash for ArticleUrl {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.hash(state)
    }
}

impl Eq for ArticleUrl {}

impl Borrow<Url> for ArticleUrl {
    fn borrow(&self) -> &Url {
        &self.url
    }
}

/// A full representation of a news article.
///
/// # Example
///
/// Download a single article directly
///
/// ```edition2018
/// # use extrablatt::Article;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let article = Article::builder("https:://example.com/some/article.html")?.get()?;
/// #   Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Article {
    /// The url of the article.
    pub url: Url,
    /// The parsed response html `Document`.
    pub doc: Document,
    /// The extracted content of the article.
    pub content: ArticleContent<'static>,
    /// The expected language of the article.
    pub language: Language,
}

impl Article {
    /// Retrieves the [`ArticleContent`] from the `url`
    ///
    /// Convenience method for:
    ///
    /// ```no_run
    ///  Article::builder(url)?.get().await?.content
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn content<T: IntoUrl>(url: T) -> Result<ArticleContent<'static>> {
        let article = Self::get(url).await?;
        Ok(article.content)
    }

    /// Get the [`Article`] for the `url`
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn get<T: IntoUrl>(url: T) -> Result<Article> {
        Self::builder(url)?.get().await
    }

    /// Convenience method for creating a new [`ArticleBuilder`]
    ///
    /// Same as calling [`ArticleBuilder::new`]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn builder<T: IntoUrl>(url: T) -> Result<ArticleBuilder> {
        ArticleBuilder::new(url)
    }

    /// Drops the article's html [`select::Document`].
    pub fn drop_document(self) -> PureArticle {
        PureArticle {
            url: self.url,
            content: self.content,
            language: self.language,
        }
    }
}

/// An [`extrablatt::Article`] without the [`select::Document`], mainly to use
/// serde.
#[derive(Debug)]
#[cfg_attr(feature = "serde0", derive(Serialize, Deserialize))]
pub struct PureArticle {
    /// The url of the article.
    pub url: Url,
    /// The extracted content of the article.
    pub content: ArticleContent<'static>,
    /// The expected language of the article.
    pub language: Language,
}

pub struct ArticleBuilder {
    url: Option<Url>,
    timeout: Option<Duration>,
    language: Option<Language>,
    browser_user_agent: Option<String>,
}

impl ArticleBuilder {
    pub fn new<T: IntoUrl>(url: T) -> Result<Self> {
        let url = url.into_url()?;

        Ok(ArticleBuilder {
            url: Some(url),
            timeout: None,
            language: None,
            browser_user_agent: None,
        })
    }

    pub fn browser_user_agent<T: ToString>(mut self, browser_user_agent: T) -> Self {
        self.browser_user_agent = Some(browser_user_agent.to_string());
        self
    }

    pub fn timeout(mut self, dur: Duration) -> Self {
        self.timeout = Some(dur);
        self
    }

    pub fn language(mut self, language: Language) -> Self {
        self.language = Some(language);
        self
    }

    /// Downloads the article and extract it's content using the
    /// [`extrablatt::DefaultExtractor`].
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn get(self) -> Result<Article> {
        self.get_with_extractor(&DefaultExtractor::default()).await
    }

    /// Downloads the article and extracts it's content using the provided
    /// [`extrablatt::Extractor`].
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn get_with_extractor<TExtract: Extractor>(
        self,
        extractor: &TExtract,
    ) -> Result<Article> {
        let url = self
            .url
            .context("Url of the article must be initialized.")?;

        let timeout = self
            .timeout
            .unwrap_or_else(|| Duration::from_secs(Config::DEFAULT_REQUEST_TIMEOUT_SEC));

        let mut headers = HeaderMap::with_capacity(1);

        headers.insert(
            USER_AGENT,
            self.browser_user_agent
                .map(|x| x.parse())
                .unwrap_or_else(|| Config::user_agent().parse())
                .context("Failed to parse user agent header.")?,
        );

        let resp = Client::builder()
            .timeout(timeout)
            .default_headers(headers)
            .build()?
            .get(url)
            .send()
            .await?;

        if !resp.status().is_success() {
            let msg = format!("Unsuccessful request to {:?}", resp.url());
            return Err(ExtrablattError::NoHttpSuccessResponse { response: resp }).context(msg);
        }

        let url = resp.url().to_owned();
        let doc = Document::from_read(&*resp.bytes().await?)
            .context(format!("Failed to read {:?} html as document.", url))?;

        let content = extractor
            .article_content(
                &doc,
                extractor.base_url(&doc).as_ref(),
                self.language.clone(),
            )
            .into_owned();

        Ok(Article {
            url,
            doc,
            content,
            language: self.language.unwrap_or_default(),
        })
    }
}

/// Bundles all the content found for an article.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde0", derive(Serialize, Deserialize))]
pub struct ArticleContent<'a> {
    pub authors: Vec<Cow<'a, str>>,
    pub title: Option<Cow<'a, str>>,
    pub publishing_date: Option<ArticleDate>,
    pub keywords: Vec<Cow<'a, str>>,
    pub description: Option<Cow<'a, str>>,
    pub text: Option<Cow<'a, str>>,
    pub language: Option<Language>,
    pub thumbnail: Option<Url>,
    pub top_image: Option<Url>,
    pub references: Vec<Url>,
    pub images: Vec<Url>,
    pub videos: Vec<Url>,
}

impl<'a> ArticleContent<'a> {
    /// Convenience method to create a  [`ArticleContentBuilder`]
    pub fn builder() -> ArticleContentBuilder<'a> {
        ArticleContentBuilder::default()
    }

    /// Transfers ownership of the content directly to this `ArticleContent`.
    pub fn into_owned(self) -> ArticleContent<'static> {
        ArticleContent {
            authors: self
                .authors
                .into_iter()
                .map(Cow::into_owned)
                .map(Cow::Owned)
                .collect(),
            title: self.title.map(Cow::into_owned).map(Cow::Owned),
            publishing_date: self.publishing_date,
            keywords: self
                .keywords
                .into_iter()
                .map(Cow::into_owned)
                .map(Cow::Owned)
                .collect(),
            description: self.description.map(Cow::into_owned).map(Cow::Owned),
            text: self.text.map(Cow::into_owned).map(Cow::Owned),
            language: self.language,
            thumbnail: self.thumbnail,
            top_image: self.top_image,
            references: self.references,
            images: self.images,
            videos: self.videos,
        }
    }
}

#[derive(Debug, Default)]
pub struct ArticleContentBuilder<'a> {
    pub authors: Option<Vec<Cow<'a, str>>>,
    pub title: Option<Cow<'a, str>>,
    pub publishing_date: Option<ArticleDate>,
    pub keywords: Option<Vec<Cow<'a, str>>>,
    pub description: Option<Cow<'a, str>>,
    pub text: Option<Cow<'a, str>>,
    pub language: Option<Language>,
    pub thumbnail: Option<Url>,
    pub top_image: Option<Url>,
    pub references: Option<Vec<Url>>,
    pub images: Option<Vec<Url>>,
    pub videos: Option<Vec<Url>>,
}

impl<'a> ArticleContentBuilder<'a> {
    pub fn authors(mut self, authors: Vec<Cow<'a, str>>) -> Self {
        self.authors = Some(authors);
        self
    }

    pub fn title(mut self, title: Cow<'a, str>) -> Self {
        self.title = Some(title);
        self
    }

    pub fn publishing_date(mut self, date: ArticleDate) -> Self {
        self.publishing_date = Some(date);
        self
    }

    pub fn keywords(mut self, keywords: Vec<Cow<'a, str>>) -> Self {
        self.keywords = Some(keywords);
        self
    }

    pub fn description(mut self, description: Cow<'a, str>) -> Self {
        self.description = Some(description);
        self
    }

    pub fn text(mut self, text: Cow<'a, str>) -> Self {
        self.text = Some(text);
        self
    }

    pub fn language(mut self, language: Language) -> Self {
        self.language = Some(language);
        self
    }

    pub fn thumbnail(mut self, thumbnail: Url) -> Self {
        self.thumbnail = Some(thumbnail);
        self
    }

    pub fn top_image(mut self, top_image: Url) -> Self {
        self.top_image = Some(top_image);
        self
    }

    pub fn references(mut self, references: Vec<Url>) -> Self {
        self.references = Some(references);
        self
    }

    pub fn images(mut self, images: Vec<Url>) -> Self {
        self.images = Some(images);
        self
    }

    pub fn videos(mut self, videos: Vec<Url>) -> Self {
        self.videos = Some(videos);
        self
    }

    pub fn build(self) -> ArticleContent<'a> {
        ArticleContent {
            authors: self.authors.unwrap_or_default(),
            title: self.title,
            publishing_date: self.publishing_date,
            keywords: self.keywords.unwrap_or_default(),
            description: self.description,
            text: self.text,
            language: self.language,
            thumbnail: self.thumbnail,
            top_image: self.top_image,
            references: self.references.unwrap_or_default(),
            images: self.images.unwrap_or_default(),
            videos: self.videos.unwrap_or_default(),
        }
    }
}
