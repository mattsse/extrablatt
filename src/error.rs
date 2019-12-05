use thiserror::Error;

/// all different error types this crate uses
#[derive(Error, Debug)]
pub enum ExtrablattError {
    /// An error while operating with urls.
    #[error("{msg}")]
    UrlError {
        /// The notification.
        msg: String,
    },
    /// Received a non success Http response
    #[error("Expected a 2xx Success but got: {status}")]
    NoHttpSuccess {
        /// The response Statuscode
        status: reqwest::StatusCode,
    },
}
