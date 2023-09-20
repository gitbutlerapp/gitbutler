use crate::keys;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(Box<dyn std::error::Error + Send + Sync>),
    #[error("authentication failed")]
    AuthenticationFailed(Box<dyn std::error::Error + Send + Sync>),
    #[error("ssh key error: {0}")]
    SshKeyError(Box<dyn std::error::Error + Send + Sync>),
    #[error("sign error: {0}")]
    SignError(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Self {
        if err.class() == git2::ErrorClass::Ssh && err.code() == git2::ErrorCode::GenericError {
            return Error::SshKeyError(err.into());
        }
        match err.code() {
            git2::ErrorCode::NotFound => Error::NotFound(err.into()),
            git2::ErrorCode::Auth => Error::AuthenticationFailed(err.into()),
            _ => Error::Other(err.into()),
        }
    }
}

impl From<keys::SignError> for Error {
    fn from(err: keys::SignError) -> Self {
        Error::SshKeyError(err.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
