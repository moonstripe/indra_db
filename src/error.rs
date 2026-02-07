//! Error types for indra_db

use thiserror::Error;

/// Result type alias for indra_db operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in indra_db operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Object not found: {0}")]
    NotFound(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("Corruption detected: {0}")]
    Corruption(String),

    #[error("Invalid database file: {0}")]
    InvalidFile(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Ref not found: {0}")]
    RefNotFound(String),

    #[error("Merge conflict: {0}")]
    MergeConflict(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Database is locked")]
    Locked,

    #[error("Version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },

    #[error("Remote error: {0}")]
    Remote(String),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Config error: {0}")]
    Config(String),
}
