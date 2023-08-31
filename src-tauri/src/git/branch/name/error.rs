use crate::git;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("branch name is invalid: {0}")]
    InvalidName(String),
    #[error("branch is not local: {0}")]
    NotLocal(String),
    #[error("branch is not remote: {0}")]
    NotRemote(String),
    #[error(transparent)]
    GitError(#[from] git::Error),
    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),
}
