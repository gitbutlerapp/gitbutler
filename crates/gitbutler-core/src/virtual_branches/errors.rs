use super::BranchId;
use crate::error::{AnyhowContextExt, Context, ErrorWithContext};
use crate::{git, project_repository::RemoteError, projects::ProjectId};

/// A way to mark errors using `[anyhow::Context::context]` for later retrieval.
///
/// Note that the display implementation is visible to users in logs, so it's a bit 'special'
/// to signify its marker status.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Marker {
    /// Invalid state was detected, making the repository invalid for operation.
    VerificationFailure,
}

impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Marker::VerificationFailure => f.write_str("<verification-failed>"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyBranchError {
    // TODO(ST): use local Marker to detect this case, apply the same to ProjectConflict
    #[error("branch {0} is in a conflicting state")]
    BranchConflicts(BranchId),
    #[error(transparent)]
    GitError(#[from] git::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error(transparent)]
    Remote(#[from] RemoteError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for PushError {
    fn context(&self) -> Option<Context> {
        match self {
            PushError::Remote(error) => error.context(),
            PushError::Other(error) => error.custom_context_or_root_cause().into(),
        }
    }
}

#[derive(Debug)]
pub struct ForcePushNotAllowed {
    pub project_id: ProjectId,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchFromTargetError {
    #[error("failed to fetch")]
    Remote(RemoteError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for FetchFromTargetError {
    fn context(&self) -> Option<Context> {
        match self {
            FetchFromTargetError::Remote(error) => error.context(),
            FetchFromTargetError::Other(error) => error.custom_context_or_root_cause().into(),
        }
    }
}
