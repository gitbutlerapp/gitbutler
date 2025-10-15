use but_core::ref_metadata;
use but_core::ref_metadata::{Branch, Workspace};
use gix::refs::FullName;
use std::any::Any;
use std::ops::{Deref, DerefMut};

/// A trivial in-memory implementation of the ref-metadata trait, and one that ideally works correctly.
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
