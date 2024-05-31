use super::{branch::BranchOwnershipClaims, BranchId, GITBUTLER_INTEGRATION_REFERENCE};
use crate::error::{AnyhowContextExt, Code, Context, ErrorWithContext};
use crate::{
    error, git,
    project_repository::{self, RemoteError},
    projects::ProjectId,
};

// Generic error enum for use in the virtual branches module.
#[derive(Debug, thiserror::Error)]
pub enum VirtualBranchError {
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("target ownership not found")]
    TargetOwnerhshipNotFound(BranchOwnershipClaims),
    #[error("git object {0} not found")]
    GitObjectNotFound(git::Oid),
    #[error("commit failed")]
    CommitFailed,
    #[error("rebase failed")]
    RebaseFailed,
    #[error("force push not allowed")]
    ForcePushNotAllowed(ForcePushNotAllowed),
    #[error("Branch has no commits - there is nothing to amend to")]
    BranchHasNoCommits,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("project in detached head state. Please checkout {} to continue", GITBUTLER_INTEGRATION_REFERENCE.branch())]
    DetachedHead,
    #[error("project is on {0}. Please checkout {} to continue", GITBUTLER_INTEGRATION_REFERENCE.branch())]
    InvalidHead(String),
    #[error("Repo HEAD is unavailable")]
    HeadNotFound,
    #[error("GibButler's integration commit not found on head.")]
    NoIntegrationCommit,
    #[error(transparent)]
    GitError(#[from] git::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ResetBranchError {
    #[error("commit {0} not in the branch")]
    CommitNotFoundInBranch(git::Oid),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error(transparent)]
    Git(#[from] git::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyBranchError {
    #[error("project")]
    BranchNotFound(BranchNotFound),
    #[error("branch {0} is in a conflicting state")]
    BranchConflicts(BranchId),
    #[error(transparent)]
    GitError(#[from] git::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ListVirtualBranchesError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for ListVirtualBranchesError {
    fn context(&self) -> Option<Context> {
        match self {
            ListVirtualBranchesError::Other(error) => error.custom_context_or_root_cause().into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateVirtualBranchError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for CreateVirtualBranchError {
    fn context(&self) -> Option<Context> {
        match self {
            CreateVirtualBranchError::Other(error) => error.custom_context_or_root_cause().into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Remote(#[from] project_repository::RemoteError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for PushError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            PushError::BranchNotFound(ctx) => ctx.to_context(),
            PushError::Remote(error) => return error.context(),
            PushError::Other(error) => return error.custom_context_or_root_cause().into(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IsRemoteBranchMergableError {
    #[error("Remote branch {0} not found")]
    BranchNotFound(git::RemoteRefname),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum IsVirtualBranchMergeable {
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for IsVirtualBranchMergeable {
    fn context(&self) -> Option<Context> {
        Some(match self {
            IsVirtualBranchMergeable::BranchNotFound(ctx) => ctx.to_context(),
            IsVirtualBranchMergeable::Other(error) => {
                return error.custom_context_or_root_cause().into()
            }
        })
    }
}

#[derive(Debug)]
pub struct ForcePushNotAllowed {
    pub project_id: ProjectId,
}

#[derive(Debug, thiserror::Error)]
pub enum CherryPickError {
    #[error("commit {0} not found ")]
    CommitNotFound(git::Oid),
    #[error("can not cherry pick not applied branch")]
    NotApplied,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SquashError {
    #[error("force push not allowed")]
    ForcePushNotAllowed(ForcePushNotAllowed),
    #[error("commit {0} not in the branch")]
    CommitNotFound(git::Oid),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("can not squash root commit")]
    CantSquashRootCommit,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
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

#[derive(Debug, thiserror::Error)]
pub enum UpdateCommitMessageError {
    #[error("force push not allowed")]
    ForcePushNotAllowed(ForcePushNotAllowed),
    #[error("Commit message can not be empty")]
    EmptyMessage,
    #[error("commit {0} not in the branch")]
    CommitNotFound(git::Oid),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateVirtualBranchFromBranchError {
    #[error("failed to apply")]
    ApplyBranch(ApplyBranchError),
    #[error("can not create a branch from default target")]
    CantMakeBranchFromDefaultTarget,
    #[error("branch {0} not found")]
    BranchNotFound(git::Refname),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug)]
pub struct BranchNotFound {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
}

impl BranchNotFound {
    fn to_context(&self) -> error::Context {
        error::Context::new(format!("branch {} not found", self.branch_id))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateBranchError {
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for UpdateBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            UpdateBranchError::BranchNotFound(ctx) => ctx.to_context(),
            UpdateBranchError::Other(error) => return error.custom_context_or_root_cause().into(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListRemoteCommitFilesError {
    #[error("commit {0} not found")]
    CommitNotFound(git::Oid),
    #[error("failed to find commit")]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ListRemoteBranchesError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for ListRemoteBranchesError {
    fn context(&self) -> Option<Context> {
        match self {
            ListRemoteBranchesError::Other(error) => error.custom_context_or_root_cause().into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetRemoteBranchDataError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for GetRemoteBranchDataError {
    fn context(&self) -> Option<Context> {
        match self {
            GetRemoteBranchDataError::Other(error) => error.custom_context_or_root_cause().into(),
        }
    }
}
