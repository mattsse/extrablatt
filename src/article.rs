use std::borrow::{Borrow, Cow};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Client, IntoUrl, RequestBuilder, Url};
use select::document::Document;
use tokio::future::FutureExt;

use crate::date::ArticleDate;
use crate::error::ExtrablattError;
use crate::extract::{DefaultExtractor, Extractor};
use crate::language::Language;
use crate::newspaper::Config;

pub const ALLOWED_FILE_EXT: [&'static str; 12] = [
    "html", "htm", "md", "rst", "aspx", "jsp", "rhtml", "cgi", "xhtml", "jhtml", "asp", "shtml",
];

pub const GOOD_SEGMENTS: [&'static str; 15] = [
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

pub const BAD_SEGMENTS: [&'static str; 17] = [
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

pub const BAD_DOMAINS: [&'static str; 4] = ["amazon", "doubleclick", "twitter", "outbrain"];

#[derive(Debug, Clone)]
pub struct ArticleUrl<'a> {
    /// The url of the article.
    pub url: Url,
    /// The title of the article.
    pub title: Option<Cow<'a, str>>,
}

impl<'a> ArticleUrl<'a> {
    pub fn new(url: Url, title: Option<Cow<'a, str>>) -> Self {
        Self { url, title }
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

#[derive(Debug)]
pub struct Article {
    /// The url of the article.
    pub url: Url,
    /// The parsed response html `Document`.
    pub doc: Document,
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

    pub fn extract<T: Extractor>(&self, extractor: &T) -> Result<ArticleContent> {
        unimplemented!()
    }
}

pub struct ArticleBuilder {
    url: Option<Url>,
    timeout: Option<Duration>,
    http_success_only: Option<bool>,
    language: Option<Language>,
}

impl ArticleBuilder {
    pub fn new<T: IntoUrl>(url: T) -> Result<Self> {
        let url = url.into_url()?;

        Ok(ArticleBuilder {
            url: Some(url),
            timeout: None,
            http_success_only: None,
            language: None,
        })
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
        let url = self
            .url
            .context("Url of the article must be initialized.")?;

        let timeout = self
            .timeout
            .unwrap_or_else(|| Duration::from_secs(Config::DEFAULT_REQ_TIMEOUT_SEC));

        let resp = Client::builder()
            .timeout(timeout)
            .build()?
            .get(url)
            .send()
            .await?;

        let http_success_only = self.http_success_only.unwrap_or(true);
        if http_success_only {
            if !resp.status().is_success() {
                return Err(ExtrablattError::NoHttpSuccess {
                    status: resp.status(),
                })
                .context(format!("Unsuccessful request to {:?}", resp.url()));
            }
        }

        let url = resp.url().to_owned();
        let doc = Document::from_read(&*resp.bytes().await?)
            .context(format!("Failed to read {:?} html as document.", url))?;

        Ok(Article {
            url,
            doc,
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
