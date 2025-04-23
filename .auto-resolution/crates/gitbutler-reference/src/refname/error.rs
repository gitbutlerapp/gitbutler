#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("branch name is invalid: {0}")]
    InvalidName(String),
    #[error("reference is not a tag: {0}")]
    NotTag(String),
    #[error("branch is not local: {0}")]
    NotLocal(String),
    #[error("branch is not remote: {0}")]
    NotRemote(String),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}
