//! Error types for Term SDK

use thiserror::Error;

/// SDK Result type
pub type Result<T> = std::result::Result<T, Error>;

/// SDK Error type
#[derive(Error, Debug)]
pub enum Error {
    #[error("Cost limit exceeded: {0}")]
    CostLimitExceeded(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timeout")]
    Timeout,

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("{0}")]
    Other(String),
}
