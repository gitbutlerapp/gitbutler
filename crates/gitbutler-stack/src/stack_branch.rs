use anyhow::Result;
use but_meta::virtual_branches_legacy_types;
use gix::refs::transaction::PreviousValue;

/// Legacy metadata for a branch within a stack, paired with a local Git reference.
/// The persisted `head` value remains as a fallback for restoring that reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StackBranch {
    /// The target of the reference - the commit ID that this branch points to.
    /// This value is serialized and used when restoring from snapshots.
    #[deprecated(note = "Use the git reference instead")]
    head: gix::ObjectId, // needs to stay private
    /// The name of the reference e.g. `master` or `feature/branch`. This should **NOT** include the `refs/heads/` prefix.
    /// The name must be unique within the repository.
    pub name: String,
    /// Legacy persisted pull-request association.
    ///
    /// PR associations are now derived from the forge review cache when projecting branch data.
    /// This field remains only for backwards-compatible snapshot and storage handling.
    #[deprecated(note = "derive PR associations from the forge review cache instead")]
    pub pr_number: Option<usize>,
    /// Archived represents the state when series/branch has been integrated and is below the merge base of the branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    pub archived: bool,

    /// Legacy persisted GitButler review identifier.
    ///
    /// Review identifiers are no longer populated. This field remains only for
    /// backwards-compatible snapshot and storage handling.
    #[deprecated(note = "review identifiers are no longer persisted or populated")]
    pub review_id: Option<String>,
}

#[expect(
    deprecated,
    reason = "the legacy head value is still serialized and restored from snapshots"
)]
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

#[expect(
    deprecated,
    reason = "the legacy head value is still serialized and restored from snapshots"
)]
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

#[expect(
    deprecated,
    reason = "the legacy head value is still needed to restore and synchronize git references"
)]
impl StackBranch {
    pub(crate) fn new(head: gix::ObjectId, name: String, repo: &gix::Repository) -> Result<Self> {
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

    pub fn name(&self) -> &String {
        &self.name
    }

    /// Creates or updates a real git reference using the head information (target commit, name)
    /// NB: If the operation is an update of an existing reference, the operation will only succeed if the old reference matches the expected value.
    ///     Therefore this should be invoked before `self.head` has been updated.
    fn set_real_reference(&self, repo: &gix::Repository, new_head: gix::ObjectId) -> Result<()> {
        repo.reference(
            qualified_reference_name(self.name()),
            new_head,
            PreviousValue::Any,
            "GitButler reference",
        )?;
        Ok(())
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

    /// Returns a fully qualified reference with the supplied remote e.g. `refs/remotes/origin/base-branch-improvements`
    pub fn remote_reference(&self, remote: &str) -> String {
        remote_reference(self.name(), remote)
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
