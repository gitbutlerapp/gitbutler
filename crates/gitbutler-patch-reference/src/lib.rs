use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// A GitButler-specific reference type that points to a commit or a patch (change).
/// The principal difference between a `PatchReference` and a regular git reference is that a `PatchReference` can point to a change (patch) that is mutable.
///
/// Because this is **NOT** a regular git reference, it will not be found in the `.git/refs`. It is instead managed by GitButler.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PatchReference {
    /// The target of the reference - this can be a commit or a change that points to a commit.
    pub target: CommitOrChangeId,
    /// The name of the reference e.g. `master` or `feature/branch`. This should **NOT** include the `refs/heads/` prefix.
    /// The name must be unique within the repository.
    pub name: String,
    /// Optional description of the series. This could be markdown or anything our hearts desire.
    pub description: Option<String>,
    /// A list of identifiers for the review unit at possible forges (eg. Pull Request).
    /// The list is empty if there is no review units, eg. no Pull Request has been created.
    #[serde(default)]
    pub forge_ids: Vec<ForgeIdentifier>,
}

/// Represents identifiers for the series at possible forges, eg. GitHub PR numbers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "subject")]
pub enum ForgeIdentifier {
    GitHub(GitHubIdentifier),
}

/// Represents a GitHub Pull Request identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubIdentifier {
    /// Pull Request number.
    pub pr_number: usize,
}

/// A patch identifier which is either `CommitId` or a `ChangeId`.
/// ChangeId should always be used if available.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommitOrChangeId {
    /// A reference that points directly to a commit.
    CommitId(String),
    /// A reference that points to a change (patch) through which a valid commit can be derived.
    ChangeId(String),
}

impl Display for CommitOrChangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitOrChangeId::CommitId(id) => write!(f, "CommitId: {}", id),
            CommitOrChangeId::ChangeId(id) => write!(f, "ChangeId: {}", id),
        }
    }
}

impl From<git2::Commit<'_>> for CommitOrChangeId {
    fn from(commit: git2::Commit) -> Self {
        if let Some(change_id) = commit.change_id() {
            CommitOrChangeId::ChangeId(change_id.to_string())
        } else {
            CommitOrChangeId::CommitId(commit.id().to_string())
        }
    }
}

impl PatchReference {
    /// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
    pub fn remote_reference(&self, remote: &str) -> Result<String> {
        Ok(format!("refs/remotes/{}/{}", remote, self.name))
    }

    /// Returns `true` if the reference is pushed to the provided remote
    pub fn pushed(&self, remote: &str, ctx: &CommandContext) -> Result<bool> {
        let remote_ref = self.remote_reference(remote)?; // todo: this should probably just return false
        Ok(ctx.repository().find_reference(&remote_ref).is_ok())
    }
}
