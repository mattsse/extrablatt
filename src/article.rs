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

    pub fn extract<T: Extractor>(&self, extractor: &T) -> Result<ArticleInfo> {
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

#[derive(Debug)]
pub struct ArticleInfo {
    pub authors: Option<Vec<String>>,
    pub title: Option<String>,
    pub date: Option<ArticleDate>,
    pub keywords: Option<Vec<String>>,
    pub summary: Option<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub url: Url,
    pub language: Language,
    pub thumbnail: Option<Url>,
    pub top_image: Option<Url>,
    pub images: Option<Vec<Url>>,
    pub videos: Option<Vec<Url>>,
}
