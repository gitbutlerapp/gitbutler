use anyhow::Result;
use gitbutler_command_context::CommandContext;
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
}

/// A patch identifier which is either `CommitId` or a `ChangeId`.
/// ChangeId should always be used if available.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommitOrChangeId {
    /// A reference that points directly to a commit.
    CommitId(String),
    /// A referrence that points to a change (patch) through which a valid commit can be derived.
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

impl PatchReference {
    /// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
    pub fn remote_reference(&self, remote: String) -> Result<String> {
        Ok(format!("refs/remotes/{}/{}", remote, self.name))
    }

    /// Returns `true` if the reference is pushed to the provided remote
    pub fn pushed(&self, remote: String, ctx: CommandContext) -> Result<bool> {
        let remote_ref = self.remote_reference(remote)?;
        Ok(ctx.repository().find_reference(&remote_ref).is_ok())
    }
}
