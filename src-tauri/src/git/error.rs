#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(Box<dyn std::error::Error + Send + Sync>),
    #[error("authentication failed")]
    Auth(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Self {
        match err.code() {
            git2::ErrorCode::NotFound => Error::NotFound(err.into()),
            git2::ErrorCode::Auth => Error::Auth(err.into()),
            _ => Error::Other(err.into()),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
