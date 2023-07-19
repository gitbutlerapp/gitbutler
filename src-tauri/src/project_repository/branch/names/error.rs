#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("branch name is invalid")]
    InvalidName(String),
    #[error("branch is not local")]
    NotLocal(String),
    #[error("branch is not remote")]
    NotRemote(String),
    #[error(transparent)]
    GitError(#[from] git2::Error),
    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),
}
