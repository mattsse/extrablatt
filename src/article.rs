use std::borrow::{Borrow, Cow};
use std::hash::{Hash, Hasher};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, USER_AGENT};
use reqwest::{Client, IntoUrl, RequestBuilder, Url};
use select::document::Document;
use tokio::future::FutureExt;

use crate::date::ArticleDate;
use crate::error::ExtrablattError;
use crate::extract::{DefaultExtractor, Extractor};
use crate::language::Language;
use crate::newspaper::Config;

pub const ALLOWED_FILE_EXT: [&str; 12] = [
    "html", "htm", "md", "rst", "aspx", "jsp", "rhtml", "cgi", "xhtml", "jhtml", "asp", "shtml",
];

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

pub const BAD_DOMAINS: [&str; 4] = ["amazon", "doubleclick", "twitter", "outbrain"];

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

#[derive(Debug)]
pub struct Article {
    /// The url of the article.
    pub url: Url,
    /// The parsed response html `Document`.
    pub doc: Document,
    /// The content of the article.
    pub content: ArticleContent<'static>,
    /// The expected language of the article.
    pub language: Language,
}

impl Article {
    /// Convenience method for creating a new [`ArticleBuilder`]
    ///
    /// Same as calling [`ArticleBuilder::new`]
    pub fn builder<T: IntoUrl>(url: T) -> Result<ArticleBuilder> {
        ArticleBuilder::new(url)
    }
}

pub struct ArticleBuilder {
    url: Option<Url>,
    timeout: Option<Duration>,
    http_success_only: Option<bool>,
    language: Option<Language>,
    browser_user_agent: Option<String>,
}

impl ArticleBuilder {
    pub fn new<T: IntoUrl>(url: T) -> Result<Self> {
        let url = url.into_url()?;

        Ok(ArticleBuilder {
            url: Some(url),
            timeout: None,
            http_success_only: None,
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

    pub fn http_success_only(mut self, http_success_only: bool) -> Self {
        self.http_success_only = Some(http_success_only);
        self
    }

    pub async fn get(self) -> Result<Article> {
        self.get_with_extractor(&DefaultExtractor::default()).await
    }

    pub async fn get_with_extractor<TExtract: Extractor>(
        self,
        extractor: &TExtract,
    ) -> Result<Article> {
        let url = self
            .url
            .context("Url of the article must be initialized.")?;

        let timeout = self
            .timeout
            .unwrap_or_else(|| Duration::from_secs(Config::DEFAULT_REQ_TIMEOUT_SEC));

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

        let http_success_only = self.http_success_only.unwrap_or(true);
        if http_success_only && !resp.status().is_success() {
            let msg = format!("Unsuccessful request to {:?}", resp.url());
            return Err(ExtrablattError::NoHttpSuccessResponse { response: resp }).context(msg);
        }

        let url = resp.url().to_owned();
        let doc = Document::from_read(&*resp.bytes().await?)
            .context(format!("Failed to read {:?} html as document.", url))?;

        let content = extractor.article_content(&doc).into_owned();

        Ok(Article {
            url,
            doc,
            content,
            language: self.language.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ArticleContent<'a> {
    pub authors: Option<Vec<Cow<'a, str>>>,
    pub title: Option<Cow<'a, str>>,
    pub date: Option<ArticleDate>,
    pub keywords: Option<Vec<Cow<'a, str>>>,
    pub summary: Option<Cow<'a, str>>,
    pub text: Option<Cow<'a, str>>,
    pub language: Option<Language>,
    pub thumbnail: Option<Url>,
    pub top_image: Option<Url>,
    pub images: Option<Vec<Url>>,
    pub videos: Option<Vec<Url>>,
}

impl<'a> ArticleContent<'a> {
    pub fn into_owned(self) -> ArticleContent<'static> {
        ArticleContent {
            authors: self
                .authors
                .map(|x| x.into_iter().map(Cow::into_owned).map(Cow::Owned).collect()),
            title: self.title.map(Cow::into_owned).map(Cow::Owned),
            date: self.date,
            keywords: self
                .keywords
                .map(|x| x.into_iter().map(Cow::into_owned).map(Cow::Owned).collect()),
            summary: self.summary.map(Cow::into_owned).map(Cow::Owned),
            text: self.text.map(Cow::into_owned).map(Cow::Owned),
            language: self.language,
            thumbnail: self.thumbnail,
            top_image: self.top_image,
            images: self.images,
            videos: self.videos,
        }
    }
}

#[derive(Debug, Default)]
pub struct ArticleContentBuilder<'a> {
    pub authors: Option<Vec<Cow<'a, str>>>,
    pub title: Option<Cow<'a, str>>,
    pub date: Option<ArticleDate>,
    pub keywords: Option<Vec<Cow<'a, str>>>,
    pub summary: Option<Cow<'a, str>>,
    pub text: Option<Cow<'a, str>>,
    pub language: Option<Language>,
    pub thumbnail: Option<Url>,
    pub top_image: Option<Url>,
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

    pub fn date(mut self, date: ArticleDate) -> Self {
        self.date = Some(date);
        self
    }

    pub fn keywords(mut self, keywords: Vec<Cow<'a, str>>) -> Self {
        self.keywords = Some(keywords);
        self
    }

    pub fn summary(mut self, summary: Cow<'a, str>) -> Self {
        self.summary = Some(summary);
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
            authors: self.authors,
            title: self.title,
            date: self.date,
            keywords: self.keywords,
            summary: self.summary,
            text: self.text,
            language: self.language,
            thumbnail: self.thumbnail,
            top_image: self.top_image,
            images: self.images,
            videos: self.videos,
        }
    }
}
