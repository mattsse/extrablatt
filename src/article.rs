use crate::date::ArticleDate;
use crate::extract::{DefaultExtractor, Extractor};
use crate::language::Language;
use crate::Config;
use anyhow::{Context, Result};
use reqwest::{IntoUrl, RequestBuilder, Url};
use select::document::Document;
use std::borrow::Cow;
use std::ops::Deref;
use std::time::Duration;
use tokio::future::FutureExt;

#[derive(Debug)]
pub struct Article {
    pub url: Url,
    pub doc: Document,
}

impl Article {
    pub fn builder<T: IntoUrl>(url: T) -> Result<ArticleBuilder> {
        ArticleBuilder::new(url)
    }

    pub fn extract<T: Extractor>(&self, extractor: T) -> Result<ArticleInfo> {
        unimplemented!()
    }
}

pub struct ArticleBuilder {
    url: Option<Url>,
    timeout: Option<Duration>,
    http_success_only: Option<bool>,
}

impl ArticleBuilder {
    pub fn new<T: IntoUrl>(url: T) -> Result<Self> {
        let url = url.into_url()?;

        Ok(ArticleBuilder {
            url: Some(url),
            timeout: None,
            http_success_only: None,
        })
    }

    pub fn timeout(mut self, dur: Duration) -> Self {
        self.timeout = Some(dur);
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
            .unwrap_or_else(|| Duration::from_secs(Config::DEFAULT_TIMEOUT_SEC));

        let resp = reqwest::get(url).timeout(timeout).await??;

        let http_success_only = self.http_success_only.unwrap_or(true);
        // TODO check success

        let url = resp.url().to_owned();
        let doc = Document::from_read(resp.bytes().await?.deref())?;

        Ok(Article { url, doc })
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

pub struct ArticlePool {}
