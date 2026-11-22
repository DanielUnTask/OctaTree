//! Error type shared by the library.

use std::path::PathBuf;

/// Everything that can go wrong while generating a tree.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A filesystem operation failed.
    #[error("I/O error on {}: {source}", path.display())]
    Io {
        /// Path the operation was acting on.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// The configuration file is not valid.
    #[error("invalid config {}: {source}", path.display())]
    Config {
        /// Config file path.
        path: PathBuf,
        /// Underlying parse error.
        source: serde_json::Error,
    },
    /// The `octatree.toml` settings are not valid.
    #[error("invalid settings {}: {message}", path.display())]
    Settings {
        /// Settings file path.
        path: PathBuf,
        /// What is wrong (includes line/column for syntax errors).
        message: String,
    },
    /// The tree template cannot host the configured mounts.
    #[error("template error: {0}")]
    Template(String),
    /// Two entries produced the same node name and `on_collision = "error"`.
    #[error("name collision: {0}")]
    Collision(String),
    /// A directory expected to exist disappeared during the scan.
    #[error("directory not found: {0}")]
    MissingDir(String),
    /// Output serialization failed.
    #[error("failed to serialize output: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Library result alias.
pub type Result<T> = std::result::Result<T, Error>;
