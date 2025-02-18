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
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Standard data we want to know about any ref.
    pub ref_info: RefInfo,

    /// An array entry for each parent of the *workspace commit* the last time we saw it.
    /// The first parent, and always the first parent, could have a tip that is named `Self::target_ref`,
    /// and if so it's not meant to be visible when asking for stacks.
    pub stacks: Vec<WorkspaceStack>,

    /// The name of the reference to integrate with, if present.
    ///
    /// If there is no target name, this is a local workspace.
    pub target_ref: Option<gix::refs::FullName>,
}

/// Metadata about branches, associated with any Git branch.
#[derive(Debug, Clone)]
pub struct Branch {
    /// Standard data we want to know about any ref.
    pub ref_info: RefInfo,
    /// More details about the branch.
    pub description: Option<String>,
    /// Information about possibly ongoing reviews in various forges.
    pub review: Review,
    /// If `true`, this means we created the branch as part of creating a new stack.
    /// This means we may also remove it and its remote tracking branch if it's removed
    /// from the stack *and* integrated.
    pub is_managed: bool,
}

/// Metadata about the repository Target.
///
/// This is only needed while we have to be compatible with the TOML file,
/// and otherwise is associated with `GITBUTLER_TARGET`.
#[derive(Debug, Clone)]
pub struct Target {
    /// The configured target, as read from the TOML file.
    pub ref_name: Option<gix::refs::FullName>,
}

/// Basic information to know about an reference we store with the metadata system.
// TODO: is this really needed?
#[derive(Debug, Clone)]
pub struct RefInfo {
    /// The time of creation, if we created the reference.
    pub create_at: Option<gix::date::Time>,
    /// The time at which the reference was last modified, if we modified it.
    pub updated_at: Option<gix::date::Time>,
}

/// A stack that was applied to the workspace, i.e. a parent of the *workspace commit*.
#[derive(Debug, Clone)]
pub struct WorkspaceStack {
    /// All refs that were reachable from the tip of the stack that at the time it was merged into
    /// the *workspace commit*.
    /// `[0]` is the first reachable ref-name, usually the tip of the stack, and `[N]` is the last
    /// reachable branch before reaching the merge-base among all stacks or the `target_ref`.
    ///
    /// Thus, reference names are stored in traversal order, from the tip towards the base.
    pub ref_names: Vec<gix::refs::FullName>,
}

impl WorkspaceStack {
    /// The name of the stack itself, if it exists.
    pub fn ref_name(&self) -> Option<&gix::refs::FullName> {
        self.ref_names.first()
    }
}

/// Metadata about branches, associated with any Git branch.
#[derive(Debug, Clone)]
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

mod virtual_branches_toml {
    use crate::ref_metadata::{Branch, Target, ValueInfo, Workspace};
    use crate::RefMetadata;
    use gix::refs::{FullName, FullNameRef};
    use std::any::Any;
    use std::ops::{Deref, DerefMut};
    use std::path::PathBuf;

    /// An implementation to read and write metadata from the `virtual_branches.toml` file.
    pub struct VirtualBranchesTomlMetadata {
        _path: PathBuf,
    }

    impl RefMetadata for VirtualBranchesTomlMetadata {
        type Handle<T> = VBTomlMetadataHandle<T>;

        fn iter(&self) -> impl Iterator<Item = (FullName, Box<dyn Any>)> + '_ {
            vec![].into_iter()
        }

        fn target(&self) -> anyhow::Result<Self::Handle<Target>> {
            todo!()
        }

        fn workspace(&self, _ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Workspace>> {
            todo!()
        }

        fn branch(&self, _ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Branch>> {
            todo!()
        }

        fn set_workspace(
            &mut self,
            _ref_name: &FullNameRef,
            _value: &Self::Handle<Workspace>,
        ) -> anyhow::Result<()> {
            todo!()
        }

        fn set_branch(
            &mut self,
            _ref_name: &FullNameRef,
            _value: &Self::Handle<Branch>,
        ) -> anyhow::Result<()> {
            todo!()
        }

        fn remove(&mut self, _ref_name: &FullNameRef) -> anyhow::Result<Box<dyn Any>> {
            todo!()
        }
    }

    pub struct VBTomlMetadataHandle<T> {
        is_default: bool,
        value: T,
    }

    impl<T> Deref for VBTomlMetadataHandle<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.value
        }
    }

    impl<T> DerefMut for VBTomlMetadataHandle<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.value
        }
    }

    impl<T> ValueInfo for VBTomlMetadataHandle<T> {
        fn is_default(&self) -> bool {
            self.is_default
        }
    }
}
pub use virtual_branches_toml::VirtualBranchesTomlMetadata;
