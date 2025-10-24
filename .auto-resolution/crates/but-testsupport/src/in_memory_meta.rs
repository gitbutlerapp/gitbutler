use std::{
    any::Any,
    ops::{Deref, DerefMut},
};

use but_core::ref_metadata::WorkspaceCommitRelation;
use but_core::{
    ref_metadata,
    ref_metadata::{Branch, StackId, Workspace, WorkspaceStackBranch},
};
use gix::refs::{Category, FullName, PartialName};

/// A trivial in-memory implementation of the ref-metadata trait, and one that ideally works correctly.
#[derive(Debug, Default)]
pub struct InMemoryRefMetadata {
    /// All the workspaces that should be available. Manipulate directly.
    pub workspaces: Vec<(gix::refs::FullName, ref_metadata::Workspace)>,
    /// All the branches that should be available. Manipulate directly.
    pub branches: Vec<(gix::refs::FullName, ref_metadata::Branch)>,
}

/// The handle used in the [InMemoryRefMetadata] implementation.
pub struct InMemoryRefMetadataHandle<T> {
    is_default: bool,
    ref_name: gix::refs::FullName,
    value: T,
}

impl<T> AsRef<gix::refs::FullNameRef> for InMemoryRefMetadataHandle<T> {
    fn as_ref(&self) -> &gix::refs::FullNameRef {
        self.ref_name.as_ref()
    }
}

impl<T> ref_metadata::ValueInfo for InMemoryRefMetadataHandle<T> {
    fn is_default(&self) -> bool {
        self.is_default
    }
}

impl<T> Deref for InMemoryRefMetadataHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for InMemoryRefMetadataHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl but_core::RefMetadata for InMemoryRefMetadata {
    type Handle<T> = InMemoryRefMetadataHandle<T>;

    fn iter(&self) -> impl Iterator<Item = anyhow::Result<(FullName, Box<dyn Any>)>> + '_ {
        self.workspaces
            .iter()
            .cloned()
            .map(|(name, v)| Ok((name, Box::new(v) as Box<dyn Any>)))
            .chain(
                self.branches
                    .iter()
                    .cloned()
                    .map(|(name, v)| Ok((name, Box::new(v) as Box<dyn Any>))),
            )
    }

    fn workspace(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Self::Handle<Workspace>> {
        Ok(self
            .workspaces
            .iter()
            .find_map(|(rn, v)| {
                (rn.as_ref() == ref_name).then(|| InMemoryRefMetadataHandle {
                    is_default: false,
                    ref_name: ref_name.to_owned(),
                    value: v.clone(),
                })
            })
            .unwrap_or_else(|| InMemoryRefMetadataHandle {
                is_default: true,
                ref_name: ref_name.to_owned(),
                value: Workspace::default(),
            }))
    }

    fn branch(&self, ref_name: &gix::refs::FullNameRef) -> anyhow::Result<Self::Handle<Branch>> {
        Ok(self
            .branches
            .iter()
            .find_map(|(rn, v)| {
                (rn.as_ref() == ref_name).then(|| InMemoryRefMetadataHandle {
                    is_default: false,
                    ref_name: ref_name.to_owned(),
                    value: v.clone(),
                })
            })
            .unwrap_or_else(|| InMemoryRefMetadataHandle {
                is_default: true,
                ref_name: ref_name.to_owned(),
                value: Branch::default(),
            }))
    }

    fn set_workspace(&mut self, value: &Self::Handle<Workspace>) -> anyhow::Result<()> {
        let ref_name = &value.ref_name;
        match self.workspaces.iter_mut().find(|w| w.0 == *ref_name) {
            None => {
                self.workspaces
                    .push((ref_name.clone(), value.value.clone()));
            }
            Some(existing) => existing.1 = value.value.clone(),
        };
        Ok(())
    }

    fn set_branch(&mut self, value: &Self::Handle<Branch>) -> anyhow::Result<()> {
        let ref_name = &value.ref_name;
        match self.branches.iter_mut().find(|w| w.0 == *ref_name) {
            None => {
                self.branches.push((ref_name.clone(), value.value.clone()));
            }
            Some(existing) => existing.1 = value.value.clone(),
        };
        Ok(())
    }

    fn remove(&mut self, ref_name: &gix::refs::FullNameRef) -> anyhow::Result<bool> {
        let branch_pos = self
            .branches
            .iter()
            .position(|(rn, _v)| rn.as_ref() == ref_name);
        let ws_pos = self
            .workspaces
            .iter()
            .position(|(rn, _v)| rn.as_ref() == ref_name);
        let res = match (branch_pos, ws_pos) {
            (Some(b), Some(w)) => {
                self.branches.remove(b);
                self.workspaces.remove(w);
                true
            }
            (None, Some(w)) => {
                self.workspaces.remove(w);
                true
            }
            (Some(b), None) => {
                self.branches.remove(b);
                true
            }
            (None, None) => false,
        };
        Ok(res)
    }
}

/// A more descriptive way of showing if the stack is included in the workspace or not.
pub enum StackState {
    /// The stack is suppsoed to be in the workspace, and applied.
    InWorkspace,
    /// The stack is supposed to be outside the workspace, and unapplied.
    Inactive,
}

/// Utilities
impl InMemoryRefMetadata {
    /// For now, we work like `vb.toml` which creates branches for each stack segment,
    /// along with a workspace that represents the branches.
    pub fn add_stack_with_segments(
        &mut self,
        stack_id: usize,
        stack_name: impl TryInto<PartialName>,
        _state: StackState,
        _segments: &[impl TryInto<PartialName>],
    ) -> StackId {
        let _stack = ref_metadata::WorkspaceStack {
            id: StackId::from_number_for_testing(stack_id as u128),
            branches: vec![stack_segment_from_partial_name(stack_name)],
            workspacecommit_relation: WorkspaceCommitRelation::Outside,
        };
        // Leave this for later, for now it's OK to use the vb.toml version.
        // Eventually we probably want to avoid using it.
        todo!()
        // let mut stack = Stack::new_with_just_heads(
        //     segments
        //         .iter()
        //         .rev()
        //         .map(|stack_name| {
        //             StackBranch::new_with_zero_head((*stack_name).into(), None, None, None, false)
        //         })
        //         .chain(std::iter::once(StackBranch::new_with_zero_head(
        //             stack_name.into(),
        //             None,
        //             None,
        //             None,
        //             false,
        //         )))
        //         .collect(),
        //     0,
        //     meta.data().branches.len(),
        //     match state {
        //         StackState::InWorkspace => true,
        //         StackState::Inactive => false,
        //     },
        // );
        // stack.order = stack_id;
        // let stack_id = StackId::from_number_for_testing(stack_id as u128);
        // stack.id = stack_id;
        // meta.data_mut().branches.insert(stack_id, stack);
        // // Assure we have a target set.
        // meta.data_mut().default_target = Some(Target {
        //     branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        //     remote_url: "does not matter".to_string(),
        //     sha: gix::hash::Kind::Sha1.null(),
        //     push_remote_name: None,
        // });
        // stack_id
    }
}

fn stack_segment_from_partial_name(name: impl TryInto<PartialName>) -> WorkspaceStackBranch {
    let Ok(name) = name.try_into() else {
        unreachable!("valid partial or full ref name");
    };

    WorkspaceStackBranch {
        ref_name: if name.as_ref().as_bstr().starts_with(b"refs/") {
            name.as_ref().as_bstr().to_owned().try_into().unwrap()
        } else {
            Category::LocalBranch
                .to_full_name(name.as_ref().as_bstr())
                .unwrap()
        },
        archived: false,
    }
}
