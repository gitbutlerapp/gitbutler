use crate::{Stack, VirtualBranchesHandle};
use anyhow::{Ok, Result};
use bstr::BString;
use but_graph::virtual_branches_legacy_types;
use git2::Commit;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::{CommitExt, CommitVecExt};
use gitbutler_oxidize::{ObjectIdExt, RepoExt};
use gitbutler_repo::logging::{LogUntil, RepositoryExt as _};
use gix::refs::{
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    Target,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

/// A GitButler-specific reference type that points to a commit or a patch (change).
/// The principal difference between a `PatchReference` and a regular git reference is that a `PatchReference` can point to a change (patch) that is mutable.
///
/// Because this is **NOT** a regular git reference, it will not be found in the `.git/refs`. It is instead managed by GitButler.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StackBranch {
    /// The target of the reference - this can be a commit or a change that points to a commit.
    #[deprecated(note = "Use the git reference instead")]
    head: CommitOrChangeId, // needs to stay private
    /// The name of the reference e.g. `master` or `feature/branch`. This should **NOT** include the `refs/heads/` prefix.
    /// The name must be unique within the repository.
    pub name: String,
    /// Optional description of the series. This could be markdown or anything our hearts desire.
    pub description: Option<String>,
    /// The pull request associated with the branch, or None if a pull request has not been created.
    pub pr_number: Option<usize>,
    /// Archived represents the state when series/branch has been integrated and is below the merge base of the branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    pub archived: bool,

    pub review_id: Option<String>,
}

impl From<virtual_branches_legacy_types::StackBranch> for StackBranch {
    fn from(
        virtual_branches_legacy_types::StackBranch {
            head,
            name,
            description,
            pr_number,
            archived,
            review_id,
        }: virtual_branches_legacy_types::StackBranch,
    ) -> Self {
        StackBranch {
            head: head.into(),
            name,
            description,
            pr_number,
            archived,
            review_id,
        }
    }
}

impl From<StackBranch> for virtual_branches_legacy_types::StackBranch {
    fn from(
        StackBranch {
            head,
            name,
            description,
            pr_number,
            archived,
            review_id,
        }: StackBranch,
    ) -> Self {
        virtual_branches_legacy_types::StackBranch {
            head: head.into(),
            name,
            description,
            pr_number,
            archived,
            review_id,
        }
    }
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

impl From<virtual_branches_legacy_types::CommitOrChangeId> for CommitOrChangeId {
    fn from(value: virtual_branches_legacy_types::CommitOrChangeId) -> Self {
        match value {
            virtual_branches_legacy_types::CommitOrChangeId::CommitId(v) => {
                CommitOrChangeId::CommitId(v)
            }
            virtual_branches_legacy_types::CommitOrChangeId::ChangeId(v) => {
                CommitOrChangeId::ChangeId(v)
            }
        }
    }
}

impl From<CommitOrChangeId> for virtual_branches_legacy_types::CommitOrChangeId {
    fn from(value: CommitOrChangeId) -> Self {
        match value {
            CommitOrChangeId::CommitId(v) => {
                virtual_branches_legacy_types::CommitOrChangeId::CommitId(v)
            }
            CommitOrChangeId::ChangeId(v) => {
                virtual_branches_legacy_types::CommitOrChangeId::ChangeId(v)
            }
        }
    }
}

impl Display for CommitOrChangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitOrChangeId::CommitId(id) => write!(f, "CommitId: {}", id),
            #[expect(deprecated)]
            CommitOrChangeId::ChangeId(id) => write!(f, "ChangeId: {}", id),
        }
    }
}

impl From<git2::Commit<'_>> for CommitOrChangeId {
    fn from(commit: git2::Commit) -> Self {
        CommitOrChangeId::CommitId(commit.id().to_string())
    }
}

impl From<git2::Oid> for CommitOrChangeId {
    fn from(oid: git2::Oid) -> Self {
        CommitOrChangeId::CommitId(oid.to_string())
    }
}

impl From<gix::ObjectId> for CommitOrChangeId {
    fn from(oid: gix::ObjectId) -> Self {
        CommitOrChangeId::CommitId(oid.to_string())
    }
}

impl StackBranch {
    pub fn new<T: Into<CommitOrChangeId>>(
        head: T,
        name: String,
        description: Option<String>,
        repo: &gix::Repository,
    ) -> Result<Self> {
        let branch = StackBranch {
            head: head.into(),
            name,
            description,
            pr_number: None,
            archived: false,
            review_id: None,
        };
        branch.set_real_reference(repo, &branch.head)?;
        Ok(branch)
    }

    pub fn new_with_zero_head(
        name: String,
        description: Option<String>,
        pr_number: Option<usize>,
        review_id: Option<String>,
        archived: bool,
    ) -> Self {
        StackBranch {
            name,
            description,
            pr_number,
            archived,
            review_id,
            head: CommitOrChangeId::CommitId(git2::Oid::zero().to_string()),
        }
    }

    pub fn full_name(&self) -> Result<gix::refs::FullName> {
        qualified_reference_name(&self.name)
            .try_into()
            .map_err(Into::into)
    }

    pub fn uses_change_id(&self) -> bool {
        matches!(self.head, CommitOrChangeId::ChangeId(_))
    }

    pub fn migrate_change_id(
        &mut self,
        repo: &git2::Repository,
        stack_head: git2::Oid,
        merge_base: git2::Oid,
    ) {
        #[expect(deprecated)]
        if let CommitOrChangeId::ChangeId(_) = &self.head {
            if let core::result::Result::Ok(commit) =
                commit_by_oid_or_change_id(&self.head.clone(), repo, stack_head, merge_base)
            {
                self.head = CommitOrChangeId::CommitId(commit.id().to_string());
            };
        }
    }

    /// This will update the commit that real git reference points to, so it points to `target`,
    /// as well as the cached data in this instance.
    /// Returns the full reference name like `refs/heads/name`.
    /// If this points to a change id, it's a noop operation. In practice, moving forward, new
    /// `CommitOrChangeId` entries will always be CommitId and ChangeId may only appear in deserialized data.
    pub fn set_head<T>(&mut self, target: T, repo: &gix::Repository) -> Result<Option<BString>>
    where
        T: Into<CommitOrChangeId> + Clone,
    {
        let refname = self.set_real_reference(repo, &target)?;
        self.head = target.into();
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
        let oid = self.head_oid(repo)?;
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

        let oid = self.head_oid(repo)?;

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
    fn set_real_reference<T>(&self, repo: &gix::Repository, new_head: &T) -> Result<Option<BString>>
    where
        T: Into<CommitOrChangeId> + Clone,
    {
        // let new_head = *new_head.to_owned();
        let new_oid = match new_head.clone().into() {
            CommitOrChangeId::CommitId(id) => gix::ObjectId::from_str(&id)?,
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

    pub fn head_oid(&self, repo: &gix::Repository) -> Result<gix::ObjectId> {
        if let Some(mut reference) = repo.try_find_reference(&self.name)? {
            let commit = reference.peel_to_commit()?;
            Ok(commit.id)
        } else if let CommitOrChangeId::CommitId(id) = &self.head {
            self.set_real_reference(repo, &self.head)?;
            Ok(gix::ObjectId::from_str(id)?)
        } else {
            Err(anyhow::anyhow!(
                "No reference found for branch {}. CommitOrChangeId is {}",
                &self.name,
                &self.head
            ))
        }
    }

    /// Updates the git reference to reflect what the current head property is (the head value from the persisted struct)
    ///
    /// This is basically the opposite of `sync_with_reference` and is something to do only after restoring from a snapshot.
    /// Only works if the head is a commit id (as opposed to legacy change id value)
    pub fn set_reference_to_head_value(&self, repo: &gix::Repository) -> Result<()> {
        self.set_real_reference(repo, &self.head)?;
        Ok(())
    }

    /// Updates the value on the struct to reflect the current value of the reference.
    /// Returns a boolean indicating whether the reference was updated.
    /// This should not really be needed since the head is always updated, but this function exists as a stopgap measure to be peformed before creating an oplog snapshot.
    /// Snapshot restoring is the only place where we read the value from the persisted struct to update the reference so we want to be sure that the refernce is in sync on snapshot creation.
    pub fn sync_with_reference(&mut self, repo: &gix::Repository) -> Result<bool> {
        let oid_from_ref = self.head_oid(repo)?;
        match self.head {
            CommitOrChangeId::ChangeId(_) => {
                self.head = CommitOrChangeId::CommitId(oid_from_ref.to_string());
                Ok(true)
            }
            CommitOrChangeId::CommitId(_) => {
                if oid_from_ref.to_string() != self.head.to_string() {
                    self.head = CommitOrChangeId::CommitId(oid_from_ref.to_string());
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
    pub fn remote_reference(&self, remote: &str) -> String {
        remote_reference(self.name(), remote)
    }

    /// Returns `true` if the reference is pushed to the provided remote
    pub fn pushed(&self, remote: &str, repo: &git2::Repository) -> bool {
        repo.find_reference(&self.remote_reference(remote)).is_ok()
    }

    /// Returns the commits that are part of the branch.
    pub fn commits<'a>(&self, ctx: &'a CommandContext, stack: &Stack) -> Result<BranchCommits<'a>> {
        let repo = ctx.repo();
        let merge_base = stack.merge_base(ctx)?.to_git2();

        let gix_repo = repo.to_gix()?;
        let head_commit = gix_repo.find_commit(self.head_oid(&gix_repo)?);
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
                predacessor
                    .head_oid(&gix_repo)
                    .map(|x| x.to_git2())
                    .unwrap_or(merge_base)
            });

        let local_patches = repo
            .log(
                head_commit.to_git2(),
                LogUntil::Commit(previous_head),
                false,
            )?
            .into_iter()
            .rev()
            .collect_vec();

        let virtual_branch_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
        let default_target = virtual_branch_state.get_default_target()?;
        let mut remote_patches: Vec<Commit<'_>> = vec![];

        // Use remote from upstream if available, otherwise default to push remote.
        let remote = stack
            .upstream
            .clone()
            .map(|ref_name| ref_name.remote().to_owned())
            .unwrap_or(default_target.push_remote_name());
        if self.pushed(&remote, repo) {
            let upstream_head = repo
                .find_reference(self.remote_reference(&remote).as_str())?
                .peel_to_commit()?;
            repo.log(upstream_head.id(), LogUntil::Commit(previous_head), false)?
                .into_iter()
                .rev()
                .for_each(|c| {
                    remote_patches.push(c);
                });
        }

        let upstream_only = if let core::result::Result::Ok(reference) =
            repo.find_reference(self.remote_reference(&remote).as_str())
        {
            let upstream_head = reference.peel_to_commit()?;
            let mut revwalk = repo.revwalk()?;
            revwalk.push(upstream_head.id())?;
            if let Some(pred) = stack.branch_predacessor(self) {
                if let core::result::Result::Ok(head_ref) =
                    repo.find_reference(pred.remote_reference(&remote).as_str())
                {
                    revwalk.hide(head_ref.peel_to_commit()?.id())?;
                }
            }
            revwalk.hide(previous_head)?;
            let mut upstream_only = revwalk
                .filter_map(|c| {
                    let commit = repo.find_commit(c.ok()?).ok()?;
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

// NB: There can be multiple commits with the same change id on the same branch id.
// This is an error condition but we must handle it.
// If there are multiple commits, they are ordered newest to oldest.
fn commit_by_oid_or_change_id<'a>(
    reference_target: &'a CommitOrChangeId,
    repo: &'a git2::Repository,
    stack_head: git2::Oid,
    merge_base: git2::Oid,
) -> Result<Commit<'a>> {
    Ok(match reference_target {
        CommitOrChangeId::CommitId(commit_id) => repo.find_commit(commit_id.parse()?)?,
        #[expect(deprecated)]
        CommitOrChangeId::ChangeId(change_id) => {
            commit_by_branch_id_and_change_id(repo, stack_head, merge_base, change_id)?
        }
    })
}

/// Given a branch id and a change id, returns the commit associated with the change id.
// TODO: We need a more efficient way of getting a commit by change id.
// NB: There can be multiple commits with the same change id on the same branch id.
// This is an error condition but we must handle it.
// If there are multiple commits, they are ordered newest to oldest.
fn commit_by_branch_id_and_change_id<'a>(
    repo: &'a git2::Repository,
    stack_head: git2::Oid, // branch.head
    merge_base: git2::Oid,
    change_id: &str,
) -> Result<Commit<'a>> {
    let commits = if stack_head == merge_base {
        vec![repo.find_commit(stack_head)?]
    } else {
        // Include the merge base, in case the change ID being searched for is the merge base itself.
        // TODO: Use the Stack `commits_with_merge_base` method instead.
        let mut commits = repo.log(stack_head, LogUntil::Commit(merge_base), false)?;
        commits.push(repo.find_commit(merge_base)?);
        commits
    };
    let commits = commits
        .into_iter()
        .filter(|c| c.change_id().as_deref() == Some(change_id))
        .collect_vec();
    if let Some(head) = commits.first() {
        Ok(head.clone())
    } else {
        Err(anyhow::anyhow!(
            "No commit with change id {} found",
            change_id
        ))
    }
}
