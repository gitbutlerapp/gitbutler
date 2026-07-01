//! Error types for IRC client operations.

/// Result type alias for IRC operations.
pub type Result<T> = std::result::Result<T, IrcError>;

/// Errors that can occur during IRC operations.
#[derive(Debug, thiserror::Error)]
pub enum IrcError {
    /// IRC protocol error
    #[error("IRC protocol error: {0}")]
    Protocol(String),

    /// Connection not ready
    #[error("Connection not ready: {0}")]
    NotReady(String),

    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl IrcError {
    /// Create a protocol error with the given message.
    pub fn protocol(msg: impl Into<String>) -> Self {
        IrcError::Protocol(msg.into())
    }

    /// Create a not ready error with the given message.
    pub fn not_ready(msg: impl Into<String>) -> Self {
        IrcError::NotReady(msg.into())
    }

    /// Create a connection error with the given message.
    pub fn connection(msg: impl Into<String>) -> Self {
        IrcError::Connection(msg.into())
    }

    /// Create a generic error with the given message.
    pub fn other(msg: impl Into<String>) -> Self {
        IrcError::Other(msg.into())
    }
}
