use serde::{Deserialize, Serialize};

use crate::json::HexHash;

use super::types::{
    CommitCreateResult, CommitInsertBlankResult, CommitMoveResult, CommitRewordResult,
    MoveChangesResult,
};

/// UI type for a move changes between commits result.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UIMoveChangesResult {
    /// Commits that have been mapped from one thing to another.
    /// Maps `oldId -> newId`.
    #[cfg_attr(
        feature = "export-schema",
        schemars(with = "std::collections::BTreeMap<String, String>")
    )]
    pub replaced_commits: std::collections::BTreeMap<HexHash, HexHash>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UIMoveChangesResult);

impl From<MoveChangesResult> for UIMoveChangesResult {
    fn from(value: MoveChangesResult) -> Self {
        let MoveChangesResult { replaced_commits } = value;

        Self {
            replaced_commits: replaced_commits
                .into_iter()
                .map(|(old, new)| (old.into(), new.into()))
                .collect(),
        }
    }
}

/// UI payload describing a rejected diff-spec path.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UIRejectedPath {
    /// The reason the diff spec was rejected.
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub reason: but_core::tree::create_tree::RejectionReason,
    /// The worktree-relative path, potentially degenerated if it can't be represented in Unicode.
    pub path: String,
    /// `path` without degeneration, as plain bytes.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_bytes")
    )]
    pub path_bytes: bstr::BString,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UIRejectedPath);

/// UI type for creating a commit in the rebase graph.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UICommitCreateResult {
    /// The new commit if one was created.
    #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
    pub new_commit: Option<HexHash>,
    /// Paths that contained at least one rejected hunk, matching legacy rejection reporting semantics.
    pub paths_to_rejected_changes: Vec<UIRejectedPath>,
    /// Commits that have been replaced as a side-effect of the create/amend.
    /// Maps `oldId -> newId`.
    #[cfg_attr(
        feature = "export-schema",
        schemars(with = "std::collections::BTreeMap<String, String>")
    )]
    pub replaced_commits: std::collections::BTreeMap<HexHash, HexHash>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UICommitCreateResult);

impl From<CommitCreateResult> for UICommitCreateResult {
    fn from(value: CommitCreateResult) -> Self {
        let CommitCreateResult {
            new_commit,
            rejected_specs,
            replaced_commits,
        } = value;

        Self {
            new_commit: new_commit.map(Into::into),
            paths_to_rejected_changes: rejected_specs
                .into_iter()
                .map(|(reason, diff)| UIRejectedPath {
                    reason,
                    path: diff.path.to_string(),
                    path_bytes: diff.path,
                })
                .collect(),
            replaced_commits: replaced_commits
                .into_iter()
                .map(|(old, new)| (old.into(), new.into()))
                .collect(),
        }
    }
}

/// UI type for rewording a commit.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UICommitRewordResult {
    /// The new commit ID after rewording.
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub new_commit: HexHash,
    /// Commits that have been replaced as a side-effect of the reword.
    /// Maps `oldId -> newId`.
    #[cfg_attr(
        feature = "export-schema",
        schemars(with = "std::collections::BTreeMap<String, String>")
    )]
    pub replaced_commits: std::collections::BTreeMap<HexHash, HexHash>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UICommitRewordResult);

impl From<CommitRewordResult> for UICommitRewordResult {
    fn from(value: CommitRewordResult) -> Self {
        let CommitRewordResult {
            new_commit,
            replaced_commits,
        } = value;

        Self {
            new_commit: new_commit.into(),
            replaced_commits: replaced_commits
                .into_iter()
                .map(|(old, new)| (old.into(), new.into()))
                .collect(),
        }
    }
}

/// UI type for inserting a blank commit.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UICommitInsertBlankResult {
    /// The new blank commit ID.
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub new_commit: HexHash,
    /// Commits that have been replaced as a side-effect of the insertion.
    /// Maps `oldId -> newId`.
    #[cfg_attr(
        feature = "export-schema",
        schemars(with = "std::collections::BTreeMap<String, String>")
    )]
    pub replaced_commits: std::collections::BTreeMap<HexHash, HexHash>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UICommitInsertBlankResult);

impl From<CommitInsertBlankResult> for UICommitInsertBlankResult {
    fn from(value: CommitInsertBlankResult) -> Self {
        let CommitInsertBlankResult {
            new_commit,
            replaced_commits,
        } = value;

        Self {
            new_commit: new_commit.into(),
            replaced_commits: replaced_commits
                .into_iter()
                .map(|(old, new)| (old.into(), new.into()))
                .collect(),
        }
    }
}

/// UI type for moving a commit.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UICommitMoveResult {
    /// Commits that have been replaced as a side-effect of the move.
    /// Maps `oldId -> newId`.
    #[cfg_attr(
        feature = "export-schema",
        schemars(with = "std::collections::BTreeMap<String, String>")
    )]
    pub replaced_commits: std::collections::BTreeMap<HexHash, HexHash>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UICommitMoveResult);

impl From<CommitMoveResult> for UICommitMoveResult {
    fn from(value: CommitMoveResult) -> Self {
        let CommitMoveResult { replaced_commits } = value;

        Self {
            replaced_commits: replaced_commits
                .into_iter()
                .map(|(old, new)| (old.into(), new.into()))
                .collect(),
        }
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
