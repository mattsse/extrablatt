use thiserror::Error;

/// all different error types this crate uses
#[derive(Error, Debug)]
pub enum BlizzError {
    /// a config error
    #[error("{msg}")]
    Config {
        /// the notification
        msg: String,
    },

    /// an error that occurred while operating with `reqwest`
    #[error("{reqwest}")]
    ReqWest {
        /// the notification
        reqwest: reqwest::Error,
    },
    /// if a error in serde occurred
    #[error("{error}")]
    Serde { error: serde_json::Error },
}

impl From<serde_json::Error> for BlizzError {
    fn from(error: serde_json::Error) -> BlizzError {
        BlizzError::Serde { error }.into()
    }
}

impl From<reqwest::Error> for BlizzError {
    fn from(reqwest: reqwest::Error) -> BlizzError {
        BlizzError::ReqWest { reqwest }.into()
    }
}
