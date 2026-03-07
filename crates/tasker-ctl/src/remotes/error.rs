//! Error types for remote operations.

#[derive(Debug, thiserror::Error)]
pub(crate) enum RemoteError {
    #[error("git operation failed for '{url}': {source}")]
    GitError {
        url: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("cache directory error: {0}")]
    CacheError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
