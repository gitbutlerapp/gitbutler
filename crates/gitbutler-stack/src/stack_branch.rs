use anyhow::{Ok, Result};
use bstr::BString;
use git2::{Commit, Oid};
use gitbutler_commit::commit_ext::CommitVecExt;
use gitbutler_repo::logging::{LogUntil, RepositoryExt as _};
use gix::refs::{
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    Target,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

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

    #[serde(default)]
    pub review_id: Option<String>,
}

/// A patch identifier which is either `CommitId` or a `ChangeId`.
/// ChangeId should always be used if available.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommitOrChangeId {
    /// A reference that points directly to a commit.
    CommitId(String),
    /// A reference that points to a change (patch) through which a valid commit can be derived.
    #[deprecated(note = "Use CommitId instead")]
    ChangeId(String),
}

impl Display for CommitOrChangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitOrChangeId::CommitId(id) => write!(f, "CommitId: {}", id),
            #[allow(deprecated)]
            CommitOrChangeId::ChangeId(id) => write!(f, "ChangeId: {}", id),
        }
    }
}

impl From<git2::Commit<'_>> for CommitOrChangeId {
    fn from(commit: git2::Commit) -> Self {
        CommitOrChangeId::CommitId(commit.id().to_string())
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
    pub fn new(
        head: CommitOrChangeId,
        name: String,
        description: Option<String>,
        repo: &gix::Repository,
    ) -> Result<Self> {
        let branch = StackBranch {
            head,
            name,
            description,
            pr_number: None,
            archived: false,
            review_id: None,
        };
        branch.set_real_reference(repo, &branch.head)?;
        Ok(branch)
    }

    pub fn head(&self) -> &CommitOrChangeId {
        &self.head
    }

    pub fn full_name(&self) -> Result<gix::refs::FullName> {
        qualified_reference_name(&self.name)
            .try_into()
            .map_err(Into::into)
    }

    /// This will update the commit that this points to (the virtual reference in virtual_branches.toml) as well as update of create a real git reference.
    /// If this points to a change id, it's a noop operation. In practice, moving forward, new CommitOrChangeId entries will always be CommitId and ChangeId may only appear in deserialized data.
    pub fn set_head(
        &mut self,
        head: CommitOrChangeId,
        repo: &gix::Repository,
    ) -> Result<Option<BString>> {
        let refname = self.set_real_reference(repo, &head)?;
        self.head = head;
        Ok(refname)
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn set_name(&mut self, name: String, repo: &gix::Repository) -> Result<()> {
        self.rename_real_reference(&name, repo)?;
        self.name = name;
        Ok(())
    }

    pub fn delete_reference(&self, repo: &gix::Repository) -> Result<()> {
        let oid = match self.head.clone() {
            CommitOrChangeId::CommitId(id) => gix::ObjectId::from_str(&id)?,
            CommitOrChangeId::ChangeId(_) => return Ok(()), // noop
        };
        let current_name: BString = qualified_reference_name(self.name()).into();
        if let Some(reference) = repo.try_find_reference(&current_name)? {
            let delete = RefEdit {
                change: Change::Delete {
                    expected: PreviousValue::MustExistAndMatch(oid.into()),
                    log: RefLog::AndReference,
                },
                name: reference.name().into(),
                deref: false,
            };
            repo.edit_reference(delete)?;
        }
        Ok(())
    }

    fn rename_real_reference(&self, name: &str, repo: &gix::Repository) -> Result<()> {
        if self.name == name {
            return Ok(()); // noop
        }
        let current_name: BString = qualified_reference_name(self.name()).into();

        let oid = match self.head.clone() {
            CommitOrChangeId::CommitId(id) => gix::ObjectId::from_str(&id)?,
            CommitOrChangeId::ChangeId(_) => return Ok(()), // noop
        };

        if let Some(reference) = repo.try_find_reference(&current_name)? {
            let delete = RefEdit {
                change: Change::Delete {
                    expected: PreviousValue::MustExistAndMatch(oid.into()),
                    log: RefLog::AndReference,
                },
                name: reference.name().into(),
                deref: false,
            };
            let create = RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: "GitButler reference".into(),
                    },
                    expected: PreviousValue::ExistingMustMatch(oid.into()),
                    new: Target::Object(oid),
                },
                name: qualified_reference_name(name).try_into()?,
                deref: false,
            };
            repo.edit_references([delete, create])?;
        } else {
            repo.reference(
                qualified_reference_name(name),
                oid,
                PreviousValue::MustNotExist,
                "GitButler reference",
            )?;
        };
        Ok(())
    }

    /// Creates or updates a real git reference using the head information (target commit, name)
    /// NB: If the operation is an update of an existing reference, the operation will only succeed if the old reference matches the expected value.
    ///     Therefore this should be invoked before `self.head` has been updated.
    /// If the head is expressed as a change id, this is a noop
    fn set_real_reference(
        &self,
        repo: &gix::Repository,
        new_head: &CommitOrChangeId,
    ) -> Result<Option<BString>> {
        let new_oid = match new_head {
            CommitOrChangeId::CommitId(id) => gix::ObjectId::from_str(id)?,
            CommitOrChangeId::ChangeId(_) => return Ok(None), // noop
        };
        let reference = repo.reference(
            qualified_reference_name(self.name()),
            new_oid,
            PreviousValue::Any,
            "GitButler reference",
        )?;
        Ok(Some(reference.name().as_bstr().to_owned()))
    }

    pub fn head_oid(&self, stack_context: &StackContext, stack: &Stack) -> Result<Oid> {
        match self.head.clone() {
            CommitOrChangeId::CommitId(id) => id.parse().map_err(Into::into),
            #[allow(deprecated)]
            CommitOrChangeId::ChangeId(_) => {
                let repository = stack_context.repository();
                let merge_base = stack.merge_base(stack_context)?;
                let head_commit =
                    commit_by_oid_or_change_id(&self.head, repository, stack.head(), merge_base)?
                        .id();
                Ok(head_commit)
            }
        }
    }

    /// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
    pub fn remote_reference(&self, remote: &str) -> String {
        remote_reference(self.name(), remote)
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
                upstream_only: vec![],
            });
        }
        let head_commit = head_commit?.id();

        // Find the previous head in the stack - if it is not archived, use it as base
        // Otherwise use the merge base
        let previous_head = stack
            .branch_predacessor(self)
            .filter(|predacessor| !predacessor.archived)
            .map_or(merge_base, |predacessor| {
                commit_by_oid_or_change_id(&predacessor.head, repository, stack.head(), merge_base)
                    .map(|commit| commit.id())
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

        let upstream_only = if let core::result::Result::Ok(reference) =
            repository.find_reference(self.remote_reference(&remote).as_str())
        {
            let upstream_head = reference.peel_to_commit()?;
            let mut revwalk = repository.revwalk()?;
            revwalk.push(upstream_head.id())?;
            if let Some(pred) = stack.branch_predacessor(self) {
                if let core::result::Result::Ok(head_ref) =
                    repository.find_reference(pred.remote_reference(&remote).as_str())
                {
                    revwalk.hide(head_ref.peel_to_commit()?.id())?;
                }
            }
            revwalk.hide(previous_head)?;
            let mut upstream_only = revwalk
                .filter_map(|c| {
                    let commit = repository.find_commit(c.ok()?).ok()?;
                    Some(commit)
                })
                .collect::<Vec<_>>();
            upstream_only.reverse();
            upstream_only
        } else {
            vec![]
        };

        Ok(BranchCommits {
            local_commits: local_patches,
            remote_commits: remote_patches,
            upstream_only,
        })
    }
}

/// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
pub fn remote_reference(name: &String, remote: &str) -> String {
    format!("refs/remotes/{}/{}", remote, name)
}

/// Returns a fully qualified reference name e.g. `refs/heads/my-branch`
fn qualified_reference_name(name: &str) -> String {
    format!("refs/heads/{}", name.trim_matches('/'))
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
    /// List of commits that exist **only** on the upstream branch. Ordered from newest to oldest.
    /// Created from the tip of the local tracking branch eg. refs/remotes/origin/my-branch -> refs/heads/my-branch
    /// This does **not** include the commits that are in the commits list (local)
    /// This is effectively the list of commits that are on the remote branch but are not in the working copy.
    pub upstream_only: Vec<Commit<'a>>,
}

impl BranchCommits<'_> {
    /// Returns `true` if the provided commit is part of the remote commits in this series (i.e. has been pushed).
    pub fn remote(&self, commit: &Commit<'_>) -> bool {
        self.remote_commits.contains_by_commit_or_change_id(commit)
    }
}
