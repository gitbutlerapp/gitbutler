use anyhow::Result;
use bstr::{BString, ByteSlice};
use but_ctx::Context;
use but_meta::virtual_branches_legacy_types;
use gitbutler_repo::first_parent_commit_ids_until;
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
};

use crate::{Stack, VirtualBranchesHandle};

/// A GitButler-specific reference type that points to a commit or a patch (change).
/// The principal difference between a `PatchReference` and a regular git reference is that a `PatchReference` can point to a change (patch) that is mutable.
///
/// Because this is **NOT** a regular git reference, it will not be found in the `.git/refs`. It is instead managed by GitButler.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StackBranch {
    /// The target of the reference - the commit ID that this branch points to.
    /// This value is serialized and used when restoring from snapshots (via set_reference_to_head_value)
    #[deprecated(note = "Use the git reference instead")]
    head: gix::ObjectId, // needs to stay private
    /// The name of the reference e.g. `master` or `feature/branch`. This should **NOT** include the `refs/heads/` prefix.
    /// The name must be unique within the repository.
    pub name: String,
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
            pr_number,
            archived,
            review_id,
        }: virtual_branches_legacy_types::StackBranch,
    ) -> Self {
        StackBranch {
            head,
            name,
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
            pr_number,
            archived,
            review_id,
        }: StackBranch,
    ) -> Self {
        virtual_branches_legacy_types::StackBranch {
            head,
            name,
            pr_number,
            archived,
            review_id,
        }
    }
}

impl StackBranch {
    pub fn new(head: gix::ObjectId, name: String, repo: &gix::Repository) -> Result<Self> {
        let branch = StackBranch {
            head,
            name,
            pr_number: None,
            archived: false,
            review_id: None,
        };
        branch.set_real_reference(repo, branch.head)?;
        Ok(branch)
    }

    pub fn new_with_zero_head(
        name: String,
        pr_number: Option<usize>,
        review_id: Option<String>,
        archived: bool,
    ) -> Self {
        StackBranch {
            name,
            pr_number,
            archived,
            review_id,
            head: gix::hash::Kind::Sha1.null(),
        }
    }

    pub fn full_name(&self) -> Result<gix::refs::FullName> {
        qualified_reference_name(&self.name)
            .try_into()
            .map_err(Into::into)
    }

    /// This will update the commit that real git reference points to, so it points to `target`,
    /// as well as the cached data in this instance.
    /// Returns the full reference name like `refs/heads/name`.
    pub fn set_head(
        &mut self,
        target: gix::ObjectId,
        repo: &gix::Repository,
    ) -> Result<Option<BString>> {
        let refname = self.set_real_reference(repo, target)?;
        self.head = target;
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
        let current_name: BString = qualified_reference_name(self.name()).into();
        if let Some(reference) = repo.try_find_reference(&current_name)? {
            but_core::branch::SafeDelete::new(repo)?.delete_reference(&reference)?;
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
            let new_name: gix::refs::FullName = qualified_reference_name(name).try_into()?;
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
                name: new_name.clone(),
                deref: false,
            };

            let one_is_contained_in_the_other = [
                (new_name.as_bstr(), reference.name().as_bstr()),
                (reference.name().as_bstr(), new_name.as_bstr()),
            ]
            .iter()
            .any(|(a, b)| a.contains_str(b) && a.get(b.len()) == Some(&b'/'));
            if one_is_contained_in_the_other {
                // Workaround `gix` issue which can't deal with directories in one transactions.
                // TODO(gix): should be able to handle this.
                repo.edit_references([delete])?;
                repo.edit_references([create])?;
            } else {
                repo.edit_references([delete, create])?;
            }
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
    fn set_real_reference(
        &self,
        repo: &gix::Repository,
        new_head: gix::ObjectId,
    ) -> Result<Option<BString>> {
        let reference = repo.reference(
            qualified_reference_name(self.name()),
            new_head,
            PreviousValue::Any,
            "GitButler reference",
        )?;
        Ok(Some(reference.name().as_bstr().to_owned()))
    }

    pub fn head_oid(&self, repo: &gix::Repository) -> Result<gix::ObjectId> {
        if let Some(mut reference) = repo.try_find_reference(&self.name)? {
            let commit = reference.peel_to_commit()?;
            Ok(commit.id)
        } else {
            self.set_real_reference(repo, self.head)?;
            Ok(self.head)
        }
    }

    /// Updates the git reference to reflect what the current head property is (the head value from the persisted struct)
    ///
    /// This is basically the opposite of `sync_with_reference` and is something to do only after restoring from a snapshot.
    /// Only works if the head is a commit id (as opposed to legacy change id value)
    pub fn set_reference_to_head_value(&self, repo: &gix::Repository) -> Result<()> {
        self.set_real_reference(repo, self.head)?;
        Ok(())
    }

    /// Updates the value on the struct to reflect the current value of the reference.
    /// Returns a boolean indicating whether the reference was updated.
    /// This should not really be needed since the head is always updated, but this function exists as a stopgap measure to be performed before creating an oplog snapshot.
    /// Snapshot restoring is the only place where we read the value from the persisted struct to update the reference so we want to be sure that the reference is in sync on snapshot creation.
    pub fn sync_with_reference(&mut self, repo: &gix::Repository) -> Result<bool> {
        let oid_from_ref = self.head_oid(repo)?;
        if oid_from_ref != self.head {
            self.head = oid_from_ref;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
    pub fn remote_reference(&self, remote: &str) -> String {
        remote_reference(self.name(), remote)
    }

    /// Returns `true` if the reference is pushed to the provided remote
    pub fn pushed(&self, remote: &str, repo: &gix::Repository) -> bool {
        repo.find_reference(&self.remote_reference(remote)).is_ok()
    }

    /// Returns the commit IDs that are part of the branch.
    pub fn commit_ids(
        &self,
        repo: &gix::Repository,
        ctx: &Context,
        stack: &Stack,
    ) -> Result<BranchCommitIds> {
        use gix::prelude::ObjectIdExt as _;

        let merge_base = stack.merge_base_plumbing(ctx)?;
        let head_commit = match repo.find_commit(self.head_oid(repo)?) {
            Ok(commit) => commit.id,
            Err(_) => {
                return Ok(BranchCommitIds {
                    local_commits: vec![],
                    remote_commits: vec![],
                    upstream_only: vec![],
                });
            }
        };

        // Find the previous head in the stack - if it is not archived, use it as base.
        // Otherwise use the merge base.
        let previous_head = stack
            .branch_predacessor(self)
            .filter(|predacessor| !predacessor.archived)
            .map_or(merge_base, |predacessor| {
                predacessor.head_oid(repo).unwrap_or(merge_base)
            });

        let mut local_patches = first_parent_commit_ids_until(repo, head_commit, previous_head)?;
        local_patches.reverse();

        let virtual_branch_state = VirtualBranchesHandle::new(ctx.project_data_dir());
        let default_target = virtual_branch_state.get_default_target()?;

        // Use remote from upstream if available, otherwise default to push remote.
        let remote = stack
            .upstream
            .clone()
            .map(|ref_name| ref_name.remote().to_owned())
            .unwrap_or(default_target.push_remote_name());

        let mut remote_patches = vec![];
        let mut upstream_only = vec![];
        if self.pushed(&remote, repo) {
            let upstream_head = repo
                .find_reference(self.remote_reference(&remote).as_str())?
                .peel_to_commit()?
                .id;

            remote_patches = first_parent_commit_ids_until(repo, upstream_head, previous_head)?;
            remote_patches.reverse();

            let mut hidden = vec![previous_head];
            if let Some(pred) = stack.branch_predacessor(self)
                && let Ok(mut head_ref) =
                    repo.find_reference(pred.remote_reference(&remote).as_str())
            {
                hidden.push(head_ref.peel_to_commit()?.id);
            }
            upstream_only = upstream_head
                .attach(repo)
                .ancestors()
                .with_hidden(hidden)
                .all()?
                .map(|info| Ok(info?.id))
                .collect::<Result<Vec<_>>>()?;
            upstream_only.reverse();
        }

        Ok(BranchCommitIds {
            local_commits: local_patches,
            remote_commits: remote_patches,
            upstream_only,
        })
    }
}

/// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
pub fn remote_reference(name: &String, remote: &str) -> String {
    format!("refs/remotes/{remote}/{name}")
}

/// Returns a fully qualified reference name e.g. `refs/heads/my-branch`
fn qualified_reference_name(name: &str) -> String {
    format!("refs/heads/{}", name.trim_matches('/'))
}

/// Represents the commits that belong to a `Branch` within a `Stack`.
#[derive(Debug, Clone)]
pub struct BranchCommitIds {
    /// The local commits that are part of this series.
    pub local_commits: Vec<gix::ObjectId>,
    /// The remote commits that are part of this series.
    pub remote_commits: Vec<gix::ObjectId>,
    /// List of commits that exist only on the upstream branch.
    pub upstream_only: Vec<gix::ObjectId>,
}
