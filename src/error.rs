use bytes::Bytes;
use thiserror::Error;
use url::Url;

use crate::article::{Article, ArticleContent};

/// All different error types this crate uses.
#[derive(Error, Debug)]
pub enum ExtrablattError {
    /// Received a good non success Http response
    #[error("Expected a 2xx Success but got: {}", response.status())]
    NoHttpSuccessResponse {
        /// The good reqwest response.
        response: reqwest::Response,
    },
    /// Failed to get a response.
    #[error("Request failed: {error}")]
    HttpRequestFailure {
        /// The reqwest error.
        error: reqwest::Error,
    },
    /// Failed to read a document.
    #[error("Failed to read document")]
    ReadDocumentError {
        /// The content the resulted in the error.
        body: Bytes,
    },
    /// Identified an article, but it's content doesn't fulfill the configured
    /// requirements.
    #[error("Found incomplete Article for {}", url)]
    IncompleteArticle {
        /// The found article and its content.
        article: Box<ArticleContent<'static>>,
        /// The url of the article.
        url: Url,
    },
}
