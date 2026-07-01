//! Lightweight metadata implementations for debug-only commands.

use std::{
    any::Any,
    ops::{Deref, DerefMut},
};

use but_core::{
    RefMetadata,
    ref_metadata::{Branch, ValueInfo, Workspace},
};

/// A metadata provider that exposes no persisted workspace or branch metadata.
#[derive(Debug, Default)]
pub(crate) struct EmptyRefMetadata;

/// A handle returned by [`EmptyRefMetadata`] for defaulted metadata values.
pub(crate) struct EmptyRefMetadataHandle<T> {
    /// Whether the value was synthesized because no metadata existed.
    pub(crate) is_default: bool,
    /// The reference name the metadata is associated with.
    pub(crate) ref_name: gix::refs::FullName,
    /// The metadata value itself.
    pub(crate) value: T,
}

impl<T> AsRef<gix::refs::FullNameRef> for EmptyRefMetadataHandle<T> {
    fn as_ref(&self) -> &gix::refs::FullNameRef {
        self.ref_name.as_ref()
    }
}

impl<T> Deref for EmptyRefMetadataHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for EmptyRefMetadataHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> ValueInfo for EmptyRefMetadataHandle<T> {
    fn is_default(&self) -> bool {
        self.is_default
    }
}

impl RefMetadata for EmptyRefMetadata {
    type Handle<T> = EmptyRefMetadataHandle<T>;

    fn iter(&self) -> impl Iterator<Item = anyhow::Result<(gix::refs::FullName, Box<dyn Any>)>> {
        std::iter::empty()
    }

    fn workspace(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Self::Handle<Workspace>> {
        Ok(EmptyRefMetadataHandle {
            is_default: true,
            ref_name: ref_name.to_owned(),
            value: Workspace::default(),
        })
    }

    fn branch(&self, ref_name: &gix::refs::FullNameRef) -> anyhow::Result<Self::Handle<Branch>> {
        Ok(EmptyRefMetadataHandle {
            is_default: true,
            ref_name: ref_name.to_owned(),
            value: Branch::default(),
        })
    }

    fn set_workspace(&mut self, _value: &Self::Handle<Workspace>) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_branch(&mut self, _value: &Self::Handle<Branch>) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove(&mut self, _ref_name: &gix::refs::FullNameRef) -> anyhow::Result<bool> {
        Ok(false)
    }
}
