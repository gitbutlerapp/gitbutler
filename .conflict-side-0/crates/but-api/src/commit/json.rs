use bstr::ByteSlice;
use serde::{Deserialize, Serialize};

use crate::{
    commit::types::CommitDiscardResult as EngineCommitDiscardResult,
    commit::types::UncommitResult as EngineUncommitResult, json::HexHash,
};

use super::types::{
    CommitCreateResult as EngineCommitCreateResult,
    CommitInsertBlankResult as EngineCommitInsertBlankResult,
    CommitMoveResult as EngineCommitMoveResult, CommitRewordResult as EngineCommitRewordResult,
    CommitSquashResult as EngineCommitSquashResult, MoveChangesResult as EngineMoveChangesResult,
};

/// JSON transport type for moving changes between commits.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MoveChangesResult {
    /// Workspace state after moving changes.
    pub workspace: crate::json::WorkspaceState,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(MoveChangesResult);

impl TryFrom<EngineMoveChangesResult> for MoveChangesResult {
    type Error = anyhow::Error;

    fn try_from(value: EngineMoveChangesResult) -> Result<Self, Self::Error> {
        let EngineMoveChangesResult { workspace } = value;

        Ok(Self {
            workspace: workspace.try_into()?,
        })
    }
}

/// A change that was rejected during commit creation, with the reason for rejection.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RejectedChange {
    /// The reason the change was rejected.
    pub reason: but_core::tree::create_tree::RejectionReason,
    /// The file path of the rejected change, potentially degenerated if it can't be represented in Unicode.
    pub path: String,
    /// `path` without degeneration, as plain bytes.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_bytes")
    )]
    pub path_bytes: bstr::BString,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RejectedChange);

/// JSON transport type for creating a commit in the rebase graph.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitCreateResult {
    /// The new commit if one was created.
    #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
    pub new_commit: Option<HexHash>,
    /// Changes that were rejected during commit creation.
    pub rejected_changes: Vec<RejectedChange>,
    /// Workspace state after the create or amend.
    pub workspace: crate::json::WorkspaceState,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommitCreateResult);

impl TryFrom<EngineCommitCreateResult> for CommitCreateResult {
    type Error = anyhow::Error;

    fn try_from(value: EngineCommitCreateResult) -> Result<Self, Self::Error> {
        let EngineCommitCreateResult {
            new_commit,
            rejected_specs,
            workspace,
        } = value;

        Ok(Self {
            new_commit: new_commit.map(Into::into),
            rejected_changes: rejected_specs
                .into_iter()
                .map(|(reason, diff)| RejectedChange {
                    reason,
                    path: diff.path.to_str_lossy().into(),
                    path_bytes: diff.path,
                })
                .collect(),
            workspace: workspace.try_into()?,
        })
    }
}

/// JSON transport type for rewording a commit.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitRewordResult {
    /// The new commit ID after rewording.
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub new_commit: HexHash,
    /// Workspace state after the reword.
    pub workspace: crate::json::WorkspaceState,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommitRewordResult);

impl TryFrom<EngineCommitRewordResult> for CommitRewordResult {
    type Error = anyhow::Error;

    fn try_from(value: EngineCommitRewordResult) -> Result<Self, Self::Error> {
        let EngineCommitRewordResult {
            new_commit,
            workspace,
        } = value;

        Ok(Self {
            new_commit: new_commit.into(),
            workspace: workspace.try_into()?,
        })
    }
}

/// JSON transport type for squashing commits.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitSquashResult {
    /// The new commit ID after squashing.
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub new_commit: HexHash,
    /// Workspace state after the squash.
    pub workspace: crate::json::WorkspaceState,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommitSquashResult);

impl TryFrom<EngineCommitSquashResult> for CommitSquashResult {
    type Error = anyhow::Error;

    fn try_from(value: EngineCommitSquashResult) -> Result<Self, Self::Error> {
        let EngineCommitSquashResult {
            new_commit,
            workspace,
        } = value;

        Ok(Self {
            new_commit: new_commit.into(),
            workspace: workspace.try_into()?,
        })
    }
}

/// JSON transport type for inserting a blank commit.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitInsertBlankResult {
    /// The new blank commit ID.
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub new_commit: HexHash,
    /// Workspace state after inserting the blank commit.
    pub workspace: crate::json::WorkspaceState,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommitInsertBlankResult);

impl TryFrom<EngineCommitInsertBlankResult> for CommitInsertBlankResult {
    type Error = anyhow::Error;

    fn try_from(value: EngineCommitInsertBlankResult) -> Result<Self, Self::Error> {
        let EngineCommitInsertBlankResult {
            new_commit,
            workspace,
        } = value;

        Ok(Self {
            new_commit: new_commit.into(),
            workspace: workspace.try_into()?,
        })
    }
}

/// JSON transport type for moving a commit.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitMoveResult {
    /// Workspace state after moving the commit.
    pub workspace: crate::json::WorkspaceState,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommitMoveResult);

impl TryFrom<EngineCommitMoveResult> for CommitMoveResult {
    type Error = anyhow::Error;

    fn try_from(value: EngineCommitMoveResult) -> Result<Self, Self::Error> {
        let EngineCommitMoveResult { workspace } = value;

        Ok(Self {
            workspace: workspace.try_into()?,
        })
    }
}
/// JSON transport type for discarding a commit.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitDiscardResult {
    /// The commit that was discarded as a result of this operation.
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub discarded_commit: HexHash,
    /// Workspace state after discarding the commit.
    pub workspace: crate::json::WorkspaceState,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommitDiscardResult);

impl TryFrom<EngineCommitDiscardResult> for CommitDiscardResult {
    type Error = anyhow::Error;

    fn try_from(value: EngineCommitDiscardResult) -> Result<Self, Self::Error> {
        let EngineCommitDiscardResult {
            discarded_commit,
            workspace,
        } = value;

        Ok(Self {
            discarded_commit: discarded_commit.into(),
            workspace: workspace.try_into()?,
        })
    }
}

/// JSON transport type for uncommitting one or more commits.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UncommitResult {
    /// The IDs of the commits that were uncommitted.
    #[cfg_attr(feature = "export-schema", schemars(with = "Vec<String>"))]
    pub uncommitted_ids: Vec<HexHash>,
    /// Workspace state after uncommitting.
    pub workspace: crate::json::WorkspaceState,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UncommitResult);

impl TryFrom<EngineUncommitResult> for UncommitResult {
    type Error = anyhow::Error;

    fn try_from(value: EngineUncommitResult) -> Result<Self, Self::Error> {
        let EngineUncommitResult {
            uncommitted_ids,
            workspace,
        } = value;

        Ok(Self {
            uncommitted_ids: uncommitted_ids.into_iter().map(Into::into).collect(),
            workspace: workspace.try_into()?,
        })
    }
}

/// Specifies a location, usually used to either have something inserted
/// relative to it, or for the selected object to actually be replaced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum RelativeTo {
    /// Relative to a commit.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    Commit(gix::ObjectId),
    /// Relative to a reference.
    #[serde(with = "but_serde::fullname_lossy")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    Reference(gix::refs::FullName),
    /// Relative to a reference, this time with teeth.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::fullname_bytes")
    )]
    ReferenceBytes(gix::refs::FullName),
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RelativeTo);

impl From<RelativeTo> for but_rebase::graph_rebase::mutate::RelativeTo {
    fn from(value: RelativeTo) -> Self {
        match value {
            RelativeTo::Commit(commit) => Self::Commit(commit),
            RelativeTo::Reference(reference) | RelativeTo::ReferenceBytes(reference) => {
                Self::Reference(reference)
            }
        }
    }
}
