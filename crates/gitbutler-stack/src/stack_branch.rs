use anyhow::Result;
use git2::{Commit, Oid};
use gitbutler_commit::commit_ext::{CommitExt, CommitVecExt};
use gitbutler_repo::{LogUntil, RepositoryExt as _};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use crate::{commit_by_oid_or_change_id, stack_context::StackContext, Stack};

/// A GitButler-specific reference type that points to a commit or a patch (change).
/// The principal difference between a `PatchReference` and a regular git reference is that a `PatchReference` can point to a change (patch) that is mutable.
///
/// Because this is **NOT** a regular git reference, it will not be found in the `.git/refs`. It is instead managed by GitButler.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StackBranch {
    /// The target of the reference - this can be a commit or a change that points to a commit.
    #[serde(alias = "target")]
    pub head: CommitOrChangeId,
    /// The name of the reference e.g. `master` or `feature/branch`. This should **NOT** include the `refs/heads/` prefix.
    /// The name must be unique within the repository.
    pub name: String,
    /// Optional description of the series. This could be markdown or anything our hearts desire.
    pub description: Option<String>,
    /// The pull request associated with the branch, or None if a pull request has not been created.
    #[serde(default)]
    pub pr_number: Option<usize>,
    /// Archived represents the state when series/branch has been integrated and is below the merge base of the branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    #[serde(default)]
    pub archived: bool,
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

pub trait RepositoryExt {
    fn lookup_change_id_or_oid(&self, oid: git2::Oid) -> Result<CommitOrChangeId>;
}

impl RepositoryExt for git2::Repository {
    fn lookup_change_id_or_oid(&self, oid: git2::Oid) -> Result<CommitOrChangeId> {
        let commit = self.find_commit(oid)?;

        Ok(commit.into())
    }
}

impl StackBranch {
    pub fn head_oid(&self, stack_context: &StackContext, stack: &Stack) -> Result<Oid> {
        let repository = stack_context.repository();
        let merge_base = stack.merge_base(stack_context)?;
        let head_commit =
            commit_by_oid_or_change_id(&self.head, repository, stack.head(), merge_base)?
                .head
                .id();
        Ok(head_commit)
    }
    /// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
    pub fn remote_reference(&self, remote: &str) -> String {
        format!("refs/remotes/{}/{}", remote, self.name)
    }

    /// Returns `true` if the reference is pushed to the provided remote
    pub fn pushed(&self, remote: &str, repository: &git2::Repository) -> bool {
        repository
            .find_reference(&self.remote_reference(remote))
            .is_ok()
    }

    /// Returns the commits that are part of the branch.
    pub fn commits<'a>(
        &self,
        stack_context: &'a StackContext,
        stack: &Stack,
    ) -> Result<BranchCommits<'a>> {
        let repository = stack_context.repository();
        let merge_base = stack.merge_base(stack_context)?;

        let head_commit =
            commit_by_oid_or_change_id(&self.head, repository, stack.head(), merge_base);
        if head_commit.is_err() {
            return Ok(BranchCommits {
                local_commits: vec![],
                remote_commits: vec![],
            });
        }
        let head_commit = head_commit?.head.id();

        // Find the previous head in the stack - if it is not archived, use it as base
        // Otherwise use the merge base
        let previous_head = stack
            .branch_predacessor(self)
            .filter(|predacessor| !predacessor.archived)
            .map_or(merge_base, |predacessor| {
                commit_by_oid_or_change_id(&predacessor.head, repository, stack.head(), merge_base)
                    .map(|commit| commit.head.id())
                    .unwrap_or(merge_base)
            });

        let local_patches = repository
            .log(head_commit, LogUntil::Commit(previous_head), false)?
            .into_iter()
            .rev()
            .collect_vec();

        let default_target = stack_context.target();
        let mut remote_patches: Vec<Commit<'_>> = vec![];

        // Use remote from upstream if available, otherwise default to push remote.
        let remote = stack
            .upstream
            .clone()
            .map(|ref_name| ref_name.remote().to_owned())
            .unwrap_or(default_target.push_remote_name());

        if self.pushed(&remote, repository) {
            let upstream_head = repository
                .find_reference(self.remote_reference(&remote).as_str())?
                .peel_to_commit()?;
            repository
                .log(upstream_head.id(), LogUntil::Commit(previous_head), false)?
                .into_iter()
                .rev()
                .for_each(|c| {
                    remote_patches.push(c);
                });
        }

        Ok(BranchCommits {
            local_commits: local_patches,
            remote_commits: remote_patches,
        })
    }
}

/// Represents the commits that belong to a `Branch` within a `Stack`.
#[derive(Debug, Clone)]
pub struct BranchCommits<'a> {
    /// The local commits that are part of this series.
    /// The commits in one "series" never overlap with the commits in another series.
    /// Topologically ordered, the first entry is the newest in the series.
    pub local_commits: Vec<Commit<'a>>,
    /// The remote commits that are part of this series.
    /// If the branch/series have never been pushed, this list will be empty.
    /// Topologically ordered, the first entry is the newest in the series.
    pub remote_commits: Vec<Commit<'a>>,
}

impl BranchCommits<'_> {
    /// Returns `true` if the provided commit is part of the remote commits in this series (i.e. has been pushed).
    pub fn remote(&self, commit: &Commit<'_>) -> bool {
        self.remote_commits.contains_by_commit_or_change_id(commit)
    }
}
