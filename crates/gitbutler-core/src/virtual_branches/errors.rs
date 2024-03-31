use super::{branch::BranchOwnershipClaims, BranchId, GITBUTLER_INTEGRATION_REFERENCE};
use crate::error::{AnyhowContextExt, Code, Context, ErrorWithContext};
use crate::{
    error,
    error::Error,
    git,
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

impl From<VerifyError> for crate::error::Error {
    fn from(value: VerifyError) -> Self {
        match value {
            VerifyError::DetachedHead => crate::error::Error::UserError {
                code: crate::error::Code::ProjectHead,
                message: format!(
                    "Project in detached head state. Please checkout {0} to continue.",
                    GITBUTLER_INTEGRATION_REFERENCE.branch()
                ),
            },
            VerifyError::InvalidHead(head) => crate::error::Error::UserError {
                code: crate::error::Code::ProjectHead,
                message: format!(
                    "Project is on {}. Please checkout {} to continue.",
                    head,
                    GITBUTLER_INTEGRATION_REFERENCE.branch()
                ),
            },
            VerifyError::NoIntegrationCommit => crate::error::Error::UserError {
                code: crate::error::Code::ProjectHead,
                message: "GibButler's integration commit not found on head.".to_string(),
            },
            VerifyError::Other(error) => {
                tracing::error!(?error);
                crate::error::Error::Unknown
            }
        }
    }
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
pub enum DeleteBranchError {
    #[error(transparent)]
    UnapplyBranch(#[from] UnapplyBranchError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<DeleteBranchError> for Error {
    fn from(value: DeleteBranchError) -> Self {
        match value {
            DeleteBranchError::UnapplyBranch(error) => error.into(),
            DeleteBranchError::Other(error) => {
                tracing::error!(?error, "delete branch error");
                Error::Unknown
            }
        }
    }
}

impl ErrorWithContext for DeleteBranchError {
    fn context(&self) -> Option<Context> {
        match self {
            DeleteBranchError::UnapplyBranch(error) => error.context(),
            DeleteBranchError::Other(error) => error.custom_context(),
        }
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

impl From<ResetBranchError> for Error {
    fn from(value: ResetBranchError) -> Self {
        match value {
            ResetBranchError::BranchNotFound(error) => error.into(),
            ResetBranchError::DefaultTargetNotSet(error) => error.into(),
            ResetBranchError::CommitNotFoundInBranch(oid) => Error::UserError {
                code: crate::error::Code::Branches,
                message: format!("commit {} not found", oid),
            },
            ResetBranchError::Other(error) => {
                tracing::error!(?error, "reset branch error");
                Error::Unknown
            }
        }
    }
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

impl From<ApplyBranchError> for Error {
    fn from(value: ApplyBranchError) -> Self {
        match value {
            ApplyBranchError::DefaultTargetNotSet(error) => error.into(),
            ApplyBranchError::Conflict(error) => error.into(),
            ApplyBranchError::BranchNotFound(error) => error.into(),
            ApplyBranchError::BranchConflicts(id) => Error::UserError {
                message: format!("Branch {} is in a conflicing state", id),
                code: crate::error::Code::Branches,
            },
            ApplyBranchError::Other(error) => {
                tracing::error!(?error, "apply branch error");
                Error::Unknown
            }
        }
    }
}

impl ErrorWithContext for ApplyBranchError {
    fn context(&self) -> Option<Context> {
        Some(match self {
            ApplyBranchError::DefaultTargetNotSet(ctx) => ctx.to_context(),
            ApplyBranchError::Conflict(ctx) => ctx.to_context(),
            ApplyBranchError::BranchNotFound(ctx) => ctx.to_context(),
            ApplyBranchError::BranchConflicts(id) => error::Context::new(
                Code::Branches,
                format!("Branch {} is in a conflicing state", id),
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

impl From<UnapplyOwnershipError> for Error {
    fn from(value: UnapplyOwnershipError) -> Self {
        match value {
            UnapplyOwnershipError::DefaultTargetNotSet(error) => error.into(),
            UnapplyOwnershipError::Conflict(error) => error.into(),
            UnapplyOwnershipError::Other(error) => {
                tracing::error!(?error, "unapply ownership error");
                Error::Unknown
            }
        }
    }
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

impl From<UnapplyBranchError> for Error {
    fn from(value: UnapplyBranchError) -> Self {
        match value {
            UnapplyBranchError::DefaultTargetNotSet(error) => error.into(),
            UnapplyBranchError::BranchNotFound(error) => error.into(),
            UnapplyBranchError::Other(error) => {
                tracing::error!(?error, "unapply branch error");
                Error::Unknown
            }
        }
    }
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

impl From<ListVirtualBranchesError> for Error {
    fn from(value: ListVirtualBranchesError) -> Self {
        match value {
            ListVirtualBranchesError::DefaultTargetNotSet(error) => error.into(),
            ListVirtualBranchesError::Other(error) => {
                tracing::error!(?error, "list virtual branches error");
                Error::Unknown
            }
        }
    }
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

impl From<CreateVirtualBranchError> for Error {
    fn from(value: CreateVirtualBranchError) -> Self {
        match value {
            CreateVirtualBranchError::DefaultTargetNotSet(error) => error.into(),
            CreateVirtualBranchError::Other(error) => {
                tracing::error!(?error, "create virtual branch error");
                Error::Unknown
            }
        }
    }
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

impl From<MergeVirtualBranchUpstreamError> for Error {
    fn from(value: MergeVirtualBranchUpstreamError) -> Self {
        match value {
            MergeVirtualBranchUpstreamError::BranchNotFound(error) => error.into(),
            MergeVirtualBranchUpstreamError::Conflict(error) => error.into(),
            MergeVirtualBranchUpstreamError::Other(error) => {
                tracing::error!(?error, "merge virtual branch upstream error");
                Error::Unknown
            }
        }
    }
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

impl From<CommitError> for Error {
    fn from(value: CommitError) -> Self {
        match value {
            CommitError::BranchNotFound(error) => error.into(),
            CommitError::DefaultTargetNotSet(error) => error.into(),
            CommitError::Conflicted(error) => error.into(),
            CommitError::CommitHookRejected(error) => Error::UserError {
                code: crate::error::Code::PreCommitHook,
                message: error,
            },
            CommitError::CommitMsgHookRejected(error) => Error::UserError {
                code: crate::error::Code::CommitMsgHook,
                message: error,
            },
            CommitError::Other(error) => {
                tracing::error!(?error, "commit error");
                Error::Unknown
            }
        }
    }
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

impl From<PushError> for Error {
    fn from(value: PushError) -> Self {
        match value {
            PushError::Remote(error) => error.into(),
            PushError::BranchNotFound(error) => error.into(),
            PushError::DefaultTargetNotSet(error) => error.into(),
            PushError::Other(error) => {
                tracing::error!(?error, "push error");
                Error::Unknown
            }
        }
    }
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

impl From<IsRemoteBranchMergableError> for Error {
    fn from(value: IsRemoteBranchMergableError) -> Self {
        match value {
            IsRemoteBranchMergableError::BranchNotFound(name) => Error::UserError {
                message: format!("Remote branch {} not found", name),
                code: crate::error::Code::Branches,
            },
            IsRemoteBranchMergableError::DefaultTargetNotSet(error) => error.into(),
            IsRemoteBranchMergableError::Other(error) => {
                tracing::error!(?error, "is remote branch mergable error");
                Error::Unknown
            }
        }
    }
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

impl From<IsVirtualBranchMergeable> for Error {
    fn from(value: IsVirtualBranchMergeable) -> Self {
        match value {
            IsVirtualBranchMergeable::BranchNotFound(error) => error.into(),
            IsVirtualBranchMergeable::DefaultTargetNotSet(error) => error.into(),
            IsVirtualBranchMergeable::Other(error) => {
                tracing::error!(?error, "is remote branch mergable error");
                Error::Unknown
            }
        }
    }
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

impl From<ForcePushNotAllowed> for Error {
    fn from(_value: ForcePushNotAllowed) -> Self {
        Error::UserError {
            code: crate::error::Code::Branches,
            message: "Action will lead to force pushing, which is not allowed for this".to_string(),
        }
    }
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

impl From<AmendError> for Error {
    fn from(value: AmendError) -> Self {
        match value {
            AmendError::ForcePushNotAllowed(error) => error.into(),
            AmendError::Conflict(error) => error.into(),
            AmendError::BranchNotFound(error) => error.into(),
            AmendError::BranchHasNoCommits => Error::UserError {
                message: "Branch has no commits - there is nothing to amend to".to_string(),
                code: crate::error::Code::Branches,
            },
            AmendError::DefaultTargetNotSet(error) => error.into(),
            AmendError::TargetOwnerhshipNotFound(_) => Error::UserError {
                message: "target ownership not found".to_string(),
                code: crate::error::Code::Branches,
            },
            AmendError::Other(error) => {
                tracing::error!(?error, "amend error");
                Error::Unknown
            }
        }
    }
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

impl From<CherryPickError> for Error {
    fn from(value: CherryPickError) -> Self {
        match value {
            CherryPickError::NotApplied => Error::UserError {
                message: "can not cherry pick non applied branch".to_string(),
                code: crate::error::Code::Branches,
            },
            CherryPickError::Conflict(error) => error.into(),
            CherryPickError::CommitNotFound(oid) => Error::UserError {
                message: format!("commit {oid} not found"),
                code: crate::error::Code::Branches,
            },
            CherryPickError::Other(error) => {
                tracing::error!(?error, "cherry pick error");
                Error::Unknown
            }
        }
    }
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

impl From<SquashError> for Error {
    fn from(value: SquashError) -> Self {
        match value {
            SquashError::ForcePushNotAllowed(error) => error.into(),
            SquashError::DefaultTargetNotSet(error) => error.into(),
            SquashError::BranchNotFound(error) => error.into(),
            SquashError::Conflict(error) => error.into(),
            SquashError::CantSquashRootCommit => Error::UserError {
                message: "can not squash root branch commit".to_string(),
                code: crate::error::Code::Branches,
            },
            SquashError::CommitNotFound(oid) => Error::UserError {
                message: format!("commit {oid} not found"),
                code: crate::error::Code::Branches,
            },
            SquashError::Other(error) => {
                tracing::error!(?error, "squash error");
                Error::Unknown
            }
        }
    }
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

impl From<FetchFromTargetError> for Error {
    fn from(value: FetchFromTargetError) -> Self {
        match value {
            FetchFromTargetError::DefaultTargetNotSet(error) => error.into(),
            FetchFromTargetError::Remote(error) => error.into(),
            FetchFromTargetError::Other(error) => {
                tracing::error!(?error, "fetch from target error");
                Error::Unknown
            }
        }
    }
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

impl From<UpdateCommitMessageError> for Error {
    fn from(value: UpdateCommitMessageError) -> Self {
        match value {
            UpdateCommitMessageError::ForcePushNotAllowed(error) => error.into(),
            UpdateCommitMessageError::EmptyMessage => Error::UserError {
                message: "Commit message can not be empty".to_string(),
                code: crate::error::Code::Branches,
            },
            UpdateCommitMessageError::DefaultTargetNotSet(error) => error.into(),
            UpdateCommitMessageError::CommitNotFound(oid) => Error::UserError {
                message: format!("Commit {} not found", oid),
                code: crate::error::Code::Branches,
            },
            UpdateCommitMessageError::BranchNotFound(error) => error.into(),
            UpdateCommitMessageError::Conflict(error) => error.into(),
            UpdateCommitMessageError::Other(error) => {
                tracing::error!(?error, "update commit message error");
                Error::Unknown
            }
        }
    }
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

impl From<SetBaseBranchError> for Error {
    fn from(value: SetBaseBranchError) -> Self {
        match value {
            SetBaseBranchError::DirtyWorkingDirectory => Error::UserError {
                message: "Current HEAD is dirty.".to_string(),
                code: crate::error::Code::ProjectConflict,
            },
            SetBaseBranchError::BranchNotFound(name) => Error::UserError {
                message: format!("remote branch '{}' not found", name),
                code: crate::error::Code::Branches,
            },
            SetBaseBranchError::Other(error) => {
                tracing::error!(?error, "set base branch error");
                Error::Unknown
            }
        }
    }
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

impl From<UpdateBaseBranchError> for Error {
    fn from(value: UpdateBaseBranchError) -> Self {
        match value {
            UpdateBaseBranchError::Conflict(error) => error.into(),
            UpdateBaseBranchError::DefaultTargetNotSet(error) => error.into(),
            UpdateBaseBranchError::Other(error) => {
                tracing::error!(?error, "update base branch error");
                Error::Unknown
            }
        }
    }
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

impl From<MoveCommitError> for crate::error::Error {
    fn from(value: MoveCommitError) -> Self {
        match value {
            MoveCommitError::SourceLocked => Error::UserError {
                message: "Source branch contains hunks locked to the target commit".to_string(),
                code: crate::error::Code::Branches,
            },
            MoveCommitError::Conflicted(error) => error.into(),
            MoveCommitError::DefaultTargetNotSet(error) => error.into(),
            MoveCommitError::BranchNotFound(error) => error.into(),
            MoveCommitError::CommitNotFound(oid) => Error::UserError {
                message: format!("Commit {} not found", oid),
                code: crate::error::Code::Branches,
            },
            MoveCommitError::Other(error) => {
                tracing::error!(?error, "move commit to vbranch error");
                Error::Unknown
            }
        }
    }
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

impl From<CreateVirtualBranchFromBranchError> for Error {
    fn from(value: CreateVirtualBranchFromBranchError) -> Self {
        match value {
            CreateVirtualBranchFromBranchError::ApplyBranch(error) => error.into(),
            CreateVirtualBranchFromBranchError::CantMakeBranchFromDefaultTarget => {
                Error::UserError {
                    message: "Can not create a branch from default target".to_string(),
                    code: crate::error::Code::Branches,
                }
            }
            CreateVirtualBranchFromBranchError::DefaultTargetNotSet(error) => error.into(),
            CreateVirtualBranchFromBranchError::BranchNotFound(name) => Error::UserError {
                message: format!("Branch {} not found", name),
                code: crate::error::Code::Branches,
            },
            CreateVirtualBranchFromBranchError::Other(error) => {
                tracing::error!(?error, "create virtual branch from branch error");
                Error::Unknown
            }
        }
    }
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

impl From<ProjectConflict> for Error {
    fn from(value: ProjectConflict) -> Self {
        Error::UserError {
            code: crate::error::Code::ProjectConflict,
            message: format!("project {} is in a conflicted state", value.project_id),
        }
    }
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

impl From<DefaultTargetNotSet> for Error {
    fn from(value: DefaultTargetNotSet) -> Self {
        Error::UserError {
            code: crate::error::Code::ProjectConflict,
            message: format!(
                "project {} does not have a default target set",
                value.project_id
            ),
        }
    }
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

impl From<BranchNotFound> for Error {
    fn from(value: BranchNotFound) -> Self {
        Error::UserError {
            code: crate::error::Code::Branches,
            message: format!("branch {} not found", value.branch_id),
        }
    }
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

impl From<UpdateBranchError> for Error {
    fn from(value: UpdateBranchError) -> Self {
        match value {
            UpdateBranchError::DefaultTargetNotSet(error) => error.into(),
            UpdateBranchError::BranchNotFound(error) => error.into(),
            UpdateBranchError::Other(error) => {
                tracing::error!(?error, "update branch error");
                Error::Unknown
            }
        }
    }
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

impl From<ListRemoteCommitFilesError> for Error {
    fn from(value: ListRemoteCommitFilesError) -> Self {
        match value {
            ListRemoteCommitFilesError::CommitNotFound(oid) => Error::UserError {
                message: format!("Commit {} not found", oid),
                code: crate::error::Code::Branches,
            },
            ListRemoteCommitFilesError::Other(error) => {
                tracing::error!(?error, "list remote commit files error");
                Error::Unknown
            }
        }
    }
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

impl From<ListRemoteBranchesError> for Error {
    fn from(value: ListRemoteBranchesError) -> Self {
        match value {
            ListRemoteBranchesError::DefaultTargetNotSet(error) => error.into(),
            ListRemoteBranchesError::Other(error) => {
                tracing::error!(?error, "list remote branches error");
                Error::Unknown
            }
        }
    }
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

impl From<GetRemoteBranchDataError> for Error {
    fn from(value: GetRemoteBranchDataError) -> Self {
        match value {
            GetRemoteBranchDataError::DefaultTargetNotSet(error) => error.into(),
            GetRemoteBranchDataError::Other(error) => {
                tracing::error!(?error, "get remote branch data error");
                Error::Unknown
            }
        }
    }
}

impl ErrorWithContext for GetRemoteBranchDataError {
    fn context(&self) -> Option<Context> {
        match self {
            GetRemoteBranchDataError::DefaultTargetNotSet(ctx) => ctx.to_context().into(),
            GetRemoteBranchDataError::Other(error) => error.custom_context(),
        }
    }
}
