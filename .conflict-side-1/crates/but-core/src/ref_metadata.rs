use crate::Id;
use gix::refs::FullNameRef;

/// Metadata about workspaces, associated with references that are designated to a workspace,
/// i.e. `refs/heads/gitbutler/workspaces/<name>`.
/// Such a ref either points to a *Workspace Commit* which we rewrite at will, or a commit
/// owned by the user.
///
/// Note that associating data with the workspace, particularly with its parents, is very safe
/// as the commit is under our control and merges aren't usually changed. However, users could
/// point it to another commit merely by `git checkout` which means our stored data is completely
/// out of sync.
///
/// We would have to detect this case by validating parents, and the refs pointing to it, before
/// using the metadata, or at least have a way to communicate possible states when trying to use
/// this information.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// Standard data we want to know about any ref.
    pub ref_info: RefInfo,

    /// An array entry for each parent of the *workspace commit* the last time we saw it.
    /// The first parent, and always the first parent, could have a tip named `Self::target_ref`,
    /// and if so, it's not meant to be visible when asking for stacks.
    pub stacks: Vec<WorkspaceStack>,

    /// The name of the reference to integrate with, if present.
    /// Fetch its metadata for more information.
    ///
    /// If there is no target name, this is a local workspace (and if no global target is set).
    /// Note that even though this is per workspace, the implementation can fill in global information at will.
    pub target_ref: Option<gix::refs::FullName>,
    /// The symbolic name of the remote to push branches to.
    ///
    /// This is useful when there are no push permissions for the remote behind `target_ref`.
    pub push_remote: Option<String>,
}

impl Workspace {}

/// Mutations
impl Workspace {
    /// Remove the named segment `branch`, which removes the whole stack if it's empty after removing a segment
    /// of that name.
    /// Returns `true` if it was removed or `false` if it wasn't found.
    pub fn remove_segment(&mut self, branch: &FullNameRef) -> bool {
        let Some((stack_idx, segment_idx)) = self.find_owner_indexes_by_name(branch) else {
            return false;
        };

        let stack = &mut self.stacks[stack_idx];
        stack.branches.remove(segment_idx);

        if stack.branches.is_empty() {
            self.stacks.remove(stack_idx);
        }
        true
    }

    /// Insert `branch` as new stack if it's not yet contained in the workspace and if `order` is not `None` or push
    /// it to the end of the stack list.
    /// Note that `order` is only relevant at insertion time.
    /// Returns `true` if the ref was newly added, or `false` if it already existed.
    pub fn add_or_insert_new_stack_if_not_present(
        &mut self,
        branch: &FullNameRef,
        order: Option<usize>,
    ) -> bool {
        if self.contains_ref(branch) {
            return false;
        };

        let stack = WorkspaceStack {
            id: StackId::generate(),
            branches: vec![WorkspaceStackBranch {
                ref_name: branch.to_owned(),
                archived: false,
            }],
        };
        match order.map(|idx| idx.min(self.stacks.len())) {
            None => {
                self.stacks.push(stack);
            }
            Some(existing_index) => {
                self.stacks.insert(existing_index, stack);
            }
        }
        true
    }

    /// Insert `branch` as new segment if it's not yet contained in the workspace,
    /// and insert it above the given `anchor` segment name, which maybe the tip of a stack or any segment within one
    /// Returns `true` if the ref was newly added, or `false` if it already existed, or `None` if `anchor` didn't exist.
    pub fn insert_new_segment_above_anchor_if_not_present(
        &mut self,
        branch: &FullNameRef,
        anchor: &FullNameRef,
    ) -> Option<bool> {
        if self.contains_ref(branch) {
            return Some(false);
        };
        let (stack_idx, segment_idx) = self.find_owner_indexes_by_name(anchor)?;
        self.stacks[stack_idx].branches.insert(
            segment_idx,
            WorkspaceStackBranch {
                ref_name: branch.to_owned(),
                archived: false,
            },
        );
        Some(true)
    }
}

/// Access
impl Workspace {
    /// Return the names of the tips of all stacks in the workspace.
    pub fn stack_names(&self) -> impl Iterator<Item = &gix::refs::FullNameRef> {
        self.stacks
            .iter()
            .filter_map(|s| s.ref_name().map(|rn| rn.as_ref()))
    }

    /// Return `true` if the branch with `name` is the workspace target or the targets local tracking branch,
    /// using `repo` for the lookup of the local tracking branch.
    pub fn is_branch_the_target_or_its_local_tracking_branch(
        &self,
        name: &gix::refs::FullNameRef,
        repo: &gix::Repository,
    ) -> anyhow::Result<bool> {
        let Some(target_ref) = self.target_ref.as_ref() else {
            return Ok(false);
        };

        if target_ref.as_ref() == name {
            Ok(true)
        } else {
            let Some((local_tracking_branch, _remote_name)) =
                repo.upstream_branch_and_remote_for_tracking_branch(target_ref.as_ref())?
            else {
                return Ok(false);
            };
            Ok(local_tracking_branch.as_ref() == name)
        }
    }

    /// Return `true` if `name` is a reference mentioned in our [stacks](Workspace::stacks).
    pub fn contains_ref(&self, name: &gix::refs::FullNameRef) -> bool {
        self.stacks
            .iter()
            .any(|stack| stack.branches.iter().any(|b| b.ref_name.as_ref() == name))
    }

    /// Find a given `name` within our stack branches and return it for modification.
    pub fn find_branch_mut(
        &mut self,
        name: &gix::refs::FullNameRef,
    ) -> Option<&mut WorkspaceStackBranch> {
        self.stacks.iter_mut().find_map(|stack| {
            stack
                .branches
                .iter_mut()
                .find(|b| b.ref_name.as_ref() == name)
        })
    }

    /// Find a given `name` within our stack branches and return it.
    pub fn find_branch(&self, name: &gix::refs::FullNameRef) -> Option<&WorkspaceStackBranch> {
        self.stacks
            .iter()
            .find_map(|stack| stack.branches.iter().find(|b| b.ref_name.as_ref() == name))
    }

    /// Find the `(stack_idx, branch_idx)` of `name` within our stack branches and return it,
    /// for direct access like `ws.stacks[stack_idx].branches[branch_idx]`.
    pub fn find_owner_indexes_by_name(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> Option<(usize, usize)> {
        self.stacks
            .iter()
            .enumerate()
            .find_map(|(stack_idx, stack)| {
                stack.branches.iter().enumerate().find_map(|(seg_idx, b)| {
                    (b.ref_name.as_ref() == name).then_some((stack_idx, seg_idx))
                })
            })
    }
}

/// Metadata about branches, associated with any Git branch.
#[derive(serde::Serialize, Clone, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// Standard data we want to know about any ref.
    pub ref_info: RefInfo,
    /// More details about the branch.
    pub description: Option<String>,
    /// Information about possibly ongoing reviews in various forges.
    pub review: Review,
}

/// Mutations
impl Branch {
    /// Claim that we now updated the branch in some way, and possibly also set the created time
    /// if `is_new_ref` is `true`
    pub fn update_times(&mut self, is_new_ref: bool) {
        self.ref_info.set_updated_to_now();
        if is_new_ref {
            self.ref_info.set_created_to_now();
        }
    }
}

impl std::fmt::Debug for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const DEFAULT_IN_TESTSUITE: gix::date::Time = gix::date::Time {
            seconds: 0,
            offset: 0,
        };
        let mut d = f.debug_struct("Branch");
        if self
            .ref_info
            .created_at
            .is_some_and(|t| t != DEFAULT_IN_TESTSUITE)
            || self
                .ref_info
                .updated_at
                .is_some_and(|t| t != DEFAULT_IN_TESTSUITE)
            || self.description.is_some()
            || self.review.pull_request.is_some()
        {
            d.field("ref_info", &self.ref_info)
                .field("description", &MaybeDebug(&self.description))
                .field("review", &self.review);
        }
        d.finish()
    }
}

/// A utility to prevent `Option` from being too verbose in debug printings.
pub struct MaybeDebug<'a, T: std::fmt::Debug>(pub &'a Option<T>);

impl<T: std::fmt::Debug> std::fmt::Debug for MaybeDebug<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            None => f.write_str("None"),
            Some(dbg) => dbg.fmt(f),
        }
    }
}

/// Basic information to know about a reference we store with the metadata system.
///
/// It allows keeping track of when it changed, but also if we created it initially, a useful
/// bit of information.
#[derive(serde::Serialize, Default, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RefInfo {
    /// The time of creation, *if we created the reference*.
    pub created_at: Option<gix::date::Time>,
    /// The time at which the reference was last modified if we modified it.
    pub updated_at: Option<gix::date::Time>,
}

/// Mutations
impl RefInfo {
    /// Set the `updated_at` field to the current time.
    pub fn set_updated_to_now(&mut self) {
        self.updated_at = Some(gix::date::Time::now_local_or_utc());
    }
    /// Set the `created_at` field to the current time.
    pub fn set_created_to_now(&mut self) {
        self.created_at = Some(gix::date::Time::now_local_or_utc());
    }
}

impl std::fmt::Debug for RefInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format = gix::date::time::format::ISO8601;
        write!(
            f,
            "RefInfo {{ created_at: {:?}, updated_at: {:?} }}",
            MaybeDebug(&self.created_at.map(|date| date.format(format))),
            MaybeDebug(&self.updated_at.map(|date| date.format(format))),
        )
    }
}

/// Access
impl RefInfo {
    /// If `true`, this means we created the branch as part of creating a new stack.
    /// This means we may also remove it and its remote tracking branch if it's removed
    /// from the stack *and* integrated.
    pub fn is_managed(&self) -> bool {
        self.created_at.is_some()
    }
}

/// The ID of a stack for somewhat stable identification of ever-changing stacks.
pub type StackId = Id<'S'>;

/// A stack that was applied to the workspace, i.e. a parent of the *workspace commit*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceStack {
    /// A unique and stable identifier for the stack itself.
    pub id: StackId,
    /// All branches that were reachable from the tip of the stack that at the time it was merged into
    /// the *workspace commit*.
    /// `[0]` is the first reachable branch, usually the tip of the stack, and `[N]` is the last
    /// reachable branch before reaching the merge-base among all stacks or the `target_ref`.
    ///
    /// Thus, branches are stored in traversal order, from the tip towards the base.
    pub branches: Vec<WorkspaceStackBranch>,
}

/// A branch within a [`WorkspaceStack`], holding per-branch metadata that is
/// stored alongside a stack that is available in a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceStackBranch {
    /// The name of the branch.
    pub ref_name: gix::refs::FullName,
    /// If `true`, the branch is now underneath the lower-base of the workspace after a workspace update.
    /// This means it's not interesting anymore, by all means, but we'd still have to keep it available and list
    /// these segments as being part of the workspace when creating PRs. Their descriptions contain references
    /// to archived segments, which simply shouldn't disappear from PRs just yet.
    /// However, they must disappear once the whole stack has been integrated and the workspace has moved past it.
    /// Note that this flag must be stored with the workspace as it must survive the deletion of a reference.
    pub archived: bool,
}

impl WorkspaceStack {
    /// The name of the stack itself, if it exists.
    pub fn ref_name(&self) -> Option<&gix::refs::FullName> {
        self.branches.first().map(|b| &b.ref_name)
    }

    /// The same as [`ref_name()`](Self::ref_name()), but returns an actual `Ref`.
    pub fn name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_name().map(|rn| rn.as_ref())
    }
}

/// Metadata about branches, associated with any Git branch.
#[derive(serde::Serialize, Clone, Eq, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    /// The number for the PR that was associated with this branch.
    pub pull_request: Option<usize>,
    /// A handle to the review created with the GitButler review system.
    pub review_id: Option<String>,
}

impl std::fmt::Debug for Review {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Review {{ pull_request: {:?}, review_id: {:?} }}",
            MaybeDebug(&self.pull_request),
            MaybeDebug(&self.review_id)
        )
    }
}

/// Additional information about the RefMetadata value itself.
pub trait ValueInfo {
    /// Return `true` if the value didn't exist for a given `ref_name` and thus was defaulted.
    fn is_default(&self) -> bool;
}
