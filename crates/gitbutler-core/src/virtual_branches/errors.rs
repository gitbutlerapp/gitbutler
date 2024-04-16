use super::{branch::BranchOwnershipClaims, BranchId, GITBUTLER_INTEGRATION_REFERENCE};
use crate::error::{AnyhowContextExt, Code, Context, ErrorWithContext};
use crate::{
    error, git,
    project_repository::{self, RemoteError},
    projects::ProjectId,
};

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("head is detached")]
    DetachedHead,
    #[error("head is {0}")]
    InvalidHead(String),
    #[error("integration commit not found")]
    NoIntegrationCommit,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for VerifyError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            VerifyError::DetachedHead => error::Context::new(
                Code::ProjectHead,
                format!(
                    "Project in detached head state. Please checkout {0} to continue.",
                    GITBUTLER_INTEGRATION_REFERENCE.branch()
                ),
            ),
            VerifyError::InvalidHead(head) => error::Context::new(
                Code::ProjectHead,
                format!(
                    "Project is on {}. Please checkout {} to continue.",
                    head,
                    GITBUTLER_INTEGRATION_REFERENCE.branch()
                ),
            ),
            VerifyError::NoIntegrationCommit => error::Context::new_static(
                Code::ProjectHead,
                "GibButler's integration commit not found on head.",
            ),
            VerifyError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResetBranchError {
    #[error("commit {0} not in the branch")]
    CommitNotFoundInBranch(git::Oid),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for ResetBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            ResetBranchError::BranchNotFound(ctx) => ctx.to_context(),
            ResetBranchError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            ResetBranchError::CommitNotFoundInBranch(oid) => {
                error::Context::new(Code::Branches, format!("commit {} not found", oid))
            }
            ResetBranchError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyBranchError {
    #[error("project")]
    Conflict(ProjectConflict),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("branch conflicts with other branches - sorry bro.")]
    BranchConflicts(BranchId),
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for ApplyBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            ApplyBranchError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            ApplyBranchError::Conflict(ctx) => ctx.to_context(),
            ApplyBranchError::BranchNotFound(ctx) => ctx.to_context(),
            ApplyBranchError::BranchConflicts(id) => error::Context::new(
                Code::Branches,
                format!("Branch {} is in a conflicting state", id),
            ),
            ApplyBranchError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UnapplyOwnershipError {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("project is in conflict state")]
    Conflict(ProjectConflict),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for UnapplyOwnershipError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            UnapplyOwnershipError::DefaultTargetNotSet(error) => error.to_context(),
            UnapplyOwnershipError::Conflict(error) => error.to_context(),
            UnapplyOwnershipError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UnapplyBranchError {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for UnapplyBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            UnapplyBranchError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            UnapplyBranchError::BranchNotFound(ctx) => ctx.to_context(),
            UnapplyBranchError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListVirtualBranchesError {
    #[error("project")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for ListVirtualBranchesError {
    fn context(&self) -> Option<Context> {
        match self {
            ListVirtualBranchesError::DefaultTargetNotSet(ctx) => ctx.to_context().into(),
            ListVirtualBranchesError::Other(error) => error.custom_context(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateVirtualBranchError {
    #[error("project")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for CreateVirtualBranchError {
    fn context(&self) -> Option<Context> {
        match self {
            CreateVirtualBranchError::DefaultTargetNotSet(ctx) => ctx.to_context().into(),
            CreateVirtualBranchError::Other(error) => error.custom_context(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MergeVirtualBranchUpstreamError {
    #[error("project")]
    Conflict(ProjectConflict),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for MergeVirtualBranchUpstreamError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            MergeVirtualBranchUpstreamError::BranchNotFound(ctx) => ctx.to_context(),
            MergeVirtualBranchUpstreamError::Conflict(ctx) => ctx.to_context(),
            MergeVirtualBranchUpstreamError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("will not commit conflicted files")]
    Conflicted(ProjectConflict),
    #[error("commit hook rejected")]
    CommitHookRejected(String),
    #[error("commit msg hook rejected")]
    CommitMsgHookRejected(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for CommitError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            CommitError::BranchNotFound(ctx) => ctx.to_context(),
            CommitError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            CommitError::Conflicted(ctx) => ctx.to_context(),
            CommitError::CommitHookRejected(error) => {
                error::Context::new(Code::PreCommitHook, error)
            }
            CommitError::CommitMsgHookRejected(error) => {
                error::Context::new(Code::CommitMsgHook, error)
            }
            CommitError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
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
            PushError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            PushError::BranchNotFound(ctx) => ctx.to_context(),
            PushError::Remote(error) => return error.context(),
            PushError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IsRemoteBranchMergableError {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("branch not found")]
    BranchNotFound(git::RemoteRefname),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for IsRemoteBranchMergableError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            IsRemoteBranchMergableError::BranchNotFound(name) => {
                error::Context::new(Code::Branches, format!("Remote branch {} not found", name))
            }
            IsRemoteBranchMergableError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            IsRemoteBranchMergableError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IsVirtualBranchMergeable {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for IsVirtualBranchMergeable {
    fn context(&self) -> Option<Context> {
        Some(match self {
            IsVirtualBranchMergeable::BranchNotFound(ctx) => ctx.to_context(),
            IsVirtualBranchMergeable::DefaultTargetNotSet(ctx) => ctx.to_context(),
            IsVirtualBranchMergeable::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug)]
pub struct ForcePushNotAllowed {
    pub project_id: ProjectId,
}

impl ForcePushNotAllowed {
    fn to_context(&self) -> error::Context {
        error::Context::new_static(
            Code::Branches,
            "Action will lead to force pushing, which is not allowed for this",
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AmendError {
    #[error("force push not allowed")]
    ForcePushNotAllowed(ForcePushNotAllowed),
    #[error("target ownership not found")]
    TargetOwnerhshipNotFound(BranchOwnershipClaims),
    #[error("branch has no commits")]
    BranchHasNoCommits,
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("project is in conflict state")]
    Conflict(ProjectConflict),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for AmendError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            AmendError::ForcePushNotAllowed(ctx) => ctx.to_context(),
            AmendError::Conflict(ctx) => ctx.to_context(),
            AmendError::BranchNotFound(ctx) => ctx.to_context(),
            AmendError::BranchHasNoCommits => error::Context::new_static(
                Code::Branches,
                "Branch has no commits - there is nothing to amend to",
            ),
            AmendError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            AmendError::TargetOwnerhshipNotFound(_) => {
                error::Context::new_static(Code::Branches, "target ownership not found")
            }
            AmendError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CherryPickError {
    #[error("target commit {0} not found ")]
    CommitNotFound(git::Oid),
    #[error("can not cherry pick not applied branch")]
    NotApplied,
    #[error("project is in conflict state")]
    Conflict(ProjectConflict),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for CherryPickError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            CherryPickError::NotApplied => {
                error::Context::new_static(Code::Branches, "can not cherry pick non applied branch")
            }
            CherryPickError::Conflict(ctx) => ctx.to_context(),
            CherryPickError::CommitNotFound(oid) => {
                error::Context::new(Code::Branches, format!("commit {oid} not found"))
            }
            CherryPickError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SquashError {
    #[error("force push not allowed")]
    ForcePushNotAllowed(ForcePushNotAllowed),
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("commit {0} not in the branch")]
    CommitNotFound(git::Oid),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("project is in conflict state")]
    Conflict(ProjectConflict),
    #[error("can not squash root commit")]
    CantSquashRootCommit,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for SquashError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            SquashError::ForcePushNotAllowed(ctx) => ctx.to_context(),
            SquashError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            SquashError::BranchNotFound(ctx) => ctx.to_context(),
            SquashError::Conflict(ctx) => ctx.to_context(),
            SquashError::CantSquashRootCommit => {
                error::Context::new_static(Code::Branches, "can not squash root branch commit")
            }
            SquashError::CommitNotFound(oid) => error::Context::new(
                crate::error::Code::Branches,
                format!("commit {oid} not found"),
            ),
            SquashError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchFromTargetError {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("failed to fetch")]
    Remote(RemoteError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for FetchFromTargetError {
    fn context(&self) -> Option<Context> {
        match self {
            FetchFromTargetError::DefaultTargetNotSet(ctx) => ctx.to_context().into(),
            FetchFromTargetError::Remote(error) => error.context(),
            FetchFromTargetError::Other(error) => error.custom_context(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateCommitMessageError {
    #[error("force push not allowed")]
    ForcePushNotAllowed(ForcePushNotAllowed),
    #[error("empty message")]
    EmptyMessage,
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("commit {0} not in the branch")]
    CommitNotFound(git::Oid),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("project is in conflict state")]
    Conflict(ProjectConflict),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for UpdateCommitMessageError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            UpdateCommitMessageError::ForcePushNotAllowed(ctx) => ctx.to_context(),
            UpdateCommitMessageError::EmptyMessage => {
                error::Context::new_static(Code::Branches, "Commit message can not be empty")
            }
            UpdateCommitMessageError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            UpdateCommitMessageError::CommitNotFound(oid) => {
                error::Context::new(Code::Branches, format!("Commit {} not found", oid))
            }
            UpdateCommitMessageError::BranchNotFound(ctx) => ctx.to_context(),
            UpdateCommitMessageError::Conflict(ctx) => ctx.to_context(),
            UpdateCommitMessageError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SetBaseBranchError {
    #[error("wd is dirty")]
    DirtyWorkingDirectory,
    #[error("branch {0} not found")]
    BranchNotFound(git::RemoteRefname),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for SetBaseBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            SetBaseBranchError::DirtyWorkingDirectory => {
                error::Context::new(Code::ProjectConflict, "Current HEAD is dirty.")
            }
            SetBaseBranchError::BranchNotFound(name) => error::Context::new(
                Code::Branches,
                format!("remote branch '{}' not found", name),
            ),
            SetBaseBranchError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateBaseBranchError {
    #[error("project is in conflicting state")]
    Conflict(ProjectConflict),
    #[error("no default target set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for UpdateBaseBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            UpdateBaseBranchError::Conflict(ctx) => ctx.to_context(),
            UpdateBaseBranchError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            UpdateBaseBranchError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MoveCommitError {
    #[error("source branch contains hunks locked to the target commit")]
    SourceLocked,
    #[error("project is in conflicted state")]
    Conflicted(ProjectConflict),
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error("commit not found")]
    CommitNotFound(git::Oid),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for MoveCommitError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            MoveCommitError::SourceLocked => error::Context::new_static(
                Code::Branches,
                "Source branch contains hunks locked to the target commit",
            ),
            MoveCommitError::Conflicted(ctx) => ctx.to_context(),
            MoveCommitError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            MoveCommitError::BranchNotFound(ctx) => ctx.to_context(),
            MoveCommitError::CommitNotFound(oid) => {
                error::Context::new(Code::Branches, format!("Commit {} not found", oid))
            }
            MoveCommitError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateVirtualBranchFromBranchError {
    #[error("failed to apply")]
    ApplyBranch(ApplyBranchError),
    #[error("can't make branch from default target")]
    CantMakeBranchFromDefaultTarget,
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("{0} not found")]
    BranchNotFound(git::Refname),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for CreateVirtualBranchFromBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            CreateVirtualBranchFromBranchError::ApplyBranch(err) => return err.context(),
            CreateVirtualBranchFromBranchError::CantMakeBranchFromDefaultTarget => {
                error::Context::new_static(
                    Code::Branches,
                    "Can not create a branch from default target",
                )
            }
            CreateVirtualBranchFromBranchError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            CreateVirtualBranchFromBranchError::BranchNotFound(name) => {
                error::Context::new(Code::Branches, format!("Branch {} not found", name))
            }
            CreateVirtualBranchFromBranchError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug)]
pub struct ProjectConflict {
    pub project_id: ProjectId,
}

impl ProjectConflict {
    fn to_context(&self) -> error::Context {
        error::Context::new(
            Code::ProjectConflict,
            format!("project {} is in a conflicted state", self.project_id),
        )
    }
}

#[derive(Debug)]
pub struct DefaultTargetNotSet {
    pub project_id: ProjectId,
}

impl DefaultTargetNotSet {
    fn to_context(&self) -> error::Context {
        error::Context::new(
            Code::ProjectConflict,
            format!(
                "project {} does not have a default target set",
                self.project_id
            ),
        )
    }
}

#[derive(Debug)]
pub struct BranchNotFound {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
}

impl BranchNotFound {
    fn to_context(&self) -> error::Context {
        error::Context::new(
            Code::Branches,
            format!("branch {} not found", self.branch_id),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateBranchError {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error("branch not found")]
    BranchNotFound(BranchNotFound),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for UpdateBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            UpdateBranchError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            UpdateBranchError::BranchNotFound(ctx) => ctx.to_context(),
            UpdateBranchError::Other(error) => return error.custom_context(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListRemoteCommitFilesError {
    #[error("failed to find commit {0}")]
    CommitNotFound(git::Oid),
    #[error("failed to find commit")]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for ListRemoteCommitFilesError {
    fn context(&self) -> Option<Context> {
        match self {
            ListRemoteCommitFilesError::CommitNotFound(oid) => {
                error::Context::new(Code::Branches, format!("Commit {} not found", oid)).into()
            }
            ListRemoteCommitFilesError::Other(error) => error.custom_context(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListRemoteBranchesError {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for ListRemoteBranchesError {
    fn context(&self) -> Option<Context> {
        match self {
            ListRemoteBranchesError::DefaultTargetNotSet(ctx) => ctx.to_context().into(),
            ListRemoteBranchesError::Other(error) => error.custom_context(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetRemoteBranchDataError {
    #[error("default target not set")]
    DefaultTargetNotSet(DefaultTargetNotSet),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorWithContext for GetRemoteBranchDataError {
    fn context(&self) -> Option<Context> {
        match self {
            GetRemoteBranchDataError::DefaultTargetNotSet(ctx) => ctx.to_context().into(),
            GetRemoteBranchDataError::Other(error) => error.custom_context(),
        }
    }
}
