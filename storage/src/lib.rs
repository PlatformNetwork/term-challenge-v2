pub mod pg;
pub mod postgres;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] tokio_postgres::Error),

    #[error("pool error: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid hotkey: {0}")]
    InvalidHotkey(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid challenge id: {0}")]
    InvalidChallengeId(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
