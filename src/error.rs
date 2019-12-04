use thiserror::Error;

/// all different error types this crate uses
#[derive(Error, Debug)]
pub enum ExtrablattError {
    /// A config error
    #[error("{msg}")]
    Config {
        /// the notification
        msg: String,
    },
    /// Received a non success Http response
    #[error("Expected a 2xx Success but got: {status}")]
    NoHttpSuccess {
        /// The response Statuscode
        status: reqwest::StatusCode,
    },
    /// if a error in serde occurred
    #[error("{error}")]
    Serde { error: serde_json::Error },
}

impl From<serde_json::Error> for ExtrablattError {
    fn from(error: serde_json::Error) -> ExtrablattError {
        ExtrablattError::Serde { error }.into()
    }
}
