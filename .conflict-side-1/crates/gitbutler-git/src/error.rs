/// A backend-agnostic operation error.
#[derive(Debug, thiserror::Error)]
pub enum Error<BE: std::error::Error + core::fmt::Debug + Send + Sync + 'static> {
    /// An otherwise backend-specific error that occurred and was not
    /// directly related to the inputs or repository state related to
    /// the operation, and instead occurred as a result of the backend
    /// executing the operation itself.
    #[error("backend error: {0}")]
    Backend(#[from] BE),
    /// The given refspec was not found.
    /// Usually returned by a push or fetch operation.
    #[error("a ref-spec was not found: {0}")]
    RefNotFound(String),
    /// An authorized operation was attempted, but the authorization
    /// credentials were rejected by the remote (or further credentials
    /// were required).
    ///
    /// The inner error is the backend-specific error that may provide
    /// more context.
    #[error("authorization failed: {0}")]
    AuthorizationFailed(BE),
    /// An operation interacting with a remote by name failed to find
    /// the remote.
    #[error("no such remote: {0}")]
    NoSuchRemote(String, #[source] BE),
    /// An operation that expected a remote not to exist found that
    /// the remote already existed.
    #[error("remote already exists: {0}")]
    RemoteExists(String, #[source] BE),
    /// A force push was rejected due to force push protection.
    #[error(
        "the force push was blocked because the remote branch contains commits that would be overwritten"
    )]
    ForcePushProtection(BE),
}
