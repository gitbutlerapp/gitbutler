use crate::keys;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(Box<dyn std::error::Error + Send + Sync>),
    #[error("authentication failed")]
    AuthenticationFailed(Box<dyn std::error::Error + Send + Sync>),
    #[error("sign error: {0}")]
    SignError(Box<dyn std::error::Error + Send + Sync>),
    #[error("remote url error: {0}")]
    RemoteUrlError(Box<dyn std::error::Error + Send + Sync>),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Self {
        if err.class() == git2::ErrorClass::Ssh && err.code() == git2::ErrorCode::GenericError {
            Error::AuthenticationFailed(err.into())
        } else {
            match err.code() {
                git2::ErrorCode::NotFound => Error::NotFound(err.into()),
                git2::ErrorCode::Auth => Error::AuthenticationFailed(err.into()),
                _ => Error::Other(err.into()),
            }
        }
    }
}

impl From<keys::SignError> for Error {
    fn from(err: keys::SignError) -> Self {
        Error::SignError(err.into())
    }
}

impl From<super::url::ParseError> for Error {
    fn from(err: super::url::ParseError) -> Self {
        Error::RemoteUrlError(err.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
