use thiserror::Error;

/// Core error type. Messages must never contain secret material.
#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("database migration failed: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("secret store error: {0}")]
    Secrets(String),

    #[error("invalid value for {field}: {reason}")]
    Invalid { field: &'static str, reason: String },

    #[error("{0} not found")]
    NotFound(&'static str),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
