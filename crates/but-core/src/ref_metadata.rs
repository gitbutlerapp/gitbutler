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
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Workspace {
    /// Standard data we want to know about any ref.
    pub ref_info: RefInfo,

    /// An array entry for each parent of the *workspace commit* the last time we saw it.
    /// The first parent, and always the first parent, could have a tip that is named `Self::target_ref`,
    /// and if so it's not meant to be visible when asking for stacks.
    pub stacks: Vec<WorkspaceStack>,

    /// The name of the reference to integrate with, if present.
    /// Fetch its metadata for more inforamtion.
    ///
    /// If there is no target name, this is a local workspace (and if no global target is set).
    /// Note that even though this is per workspace, the implementation can fill in global information at will.
    pub target_ref: Option<gix::refs::FullName>,
}

impl Workspace {
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
}

/// Metadata about branches, associated with any Git branch.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Branch {
    /// Standard data we want to know about any ref.
    pub ref_info: RefInfo,
    /// More details about the branch.
    pub description: Option<String>,
    /// Information about possibly ongoing reviews in various forges.
    pub review: Review,
}

/// Basic information to know about a reference we store with the metadata system.
///
/// It allows to keep track of when it changed, but also if we created it initially, a useful
/// bit of information.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct RefInfo {
    /// The time of creation, *if we created the reference*.
    pub created_at: Option<gix::date::Time>,
    /// The time at which the reference was last modified, if we modified it.
    pub updated_at: Option<gix::date::Time>,
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

/// A stack that was applied to the workspace, i.e. a parent of the *workspace commit*.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceStack {
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
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceStackBranch {
    /// The name of the branch.
    pub ref_name: gix::refs::FullName,
    /// Archived represents the state when series/branch has been integrated and is below the merge base with the current target branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    ///
    /// Note that this is a cache to help speed up certain operations.
    /// NOTE: This is more like a proof of concept and for backwards compatibility - maybe we will make it go away.
    // TODO: given that most operations require a graph walk, will this really be necessary if a graph cache is used consistently?
    //       Staleness can be a problem if targets can be changed after the fact. At least we'd need to recompute it.
    pub archived: bool,
}

impl WorkspaceStack {
    /// The name of the stack itself, if it exists.
    pub fn ref_name(&self) -> Option<&gix::refs::FullName> {
        self.branches.first().map(|b| &b.ref_name)
    }
}

/// Metadata about branches, associated with any Git branch.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Review {
    /// The number for the PR that was associated with this branch.
    pub pull_request: Option<usize>,
    /// A handle to the review created with the GitButler review system.
    pub review_id: Option<String>,
}

/// Additional information about the RefMetadata value itself.
pub trait ValueInfo {
    /// Return `true` if the value didn't exist for a given `ref_name` and thus was defaulted.
    fn is_default(&self) -> bool;
}
