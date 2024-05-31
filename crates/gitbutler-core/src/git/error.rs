use std::str::Utf8Error;

use crate::error::{Context, ErrorWithContext};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(git2::Error),
    #[error("authentication failed")]
    Auth(git2::Error),
    #[error("remote url error: {0}")]
    Url(super::url::ParseError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("network error: {0}")]
    Network(git2::Error),
    #[error("hook error: {0}")]
    Hooks(#[from] git2_hooks::HooksError),
    #[error("http error: {0}")]
    Http(git2::Error),
    #[error("blame error: {0}")]
    Blame(git2::Error),
    #[error("checkout error: {0}")]
    Checkout(git2::Error),
    #[error(transparent)]
    Other(git2::Error),
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
    #[error(transparent)]
    Remotes(git2::Error),
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Self {
        match err.class() {
            git2::ErrorClass::Ssh => match err.code() {
                git2::ErrorCode::GenericError | git2::ErrorCode::Auth => Error::Auth(err),
                _ => Error::Other(err),
            },
            git2::ErrorClass::Checkout => Error::Checkout(err),
            git2::ErrorClass::Http => Error::Http(err),
            git2::ErrorClass::Net => Error::Network(err),
            _ => match err.code() {
                git2::ErrorCode::NotFound => Error::NotFound(err),
                git2::ErrorCode::Auth => Error::Auth(err),
                _ => Error::Other(err),
            },
        }
    }
}

impl ErrorWithContext for Error {
    fn context(&self) -> Option<Context> {
        None
    }
}

impl From<super::url::ParseError> for Error {
    fn from(err: super::url::ParseError) -> Self {
        Error::Url(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
