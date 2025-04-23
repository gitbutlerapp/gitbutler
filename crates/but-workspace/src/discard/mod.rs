//! Utility types related to discarding changes in the worktree.

use crate::commit_engine::DiffSpec;
use gix::object::tree::EntryKind;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

/// A specification of what should be discarded, either changes to the whole file, or a portion of it.
/// Note that these must match an actual worktree change, but also may only partially match them if individual ranges are chosen
/// for removal.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct DiscardSpec(DiffSpec);
impl From<DiffSpec> for DiscardSpec {
    fn from(value: DiffSpec) -> Self {
        DiscardSpec(value)
    }
}

impl Deref for DiscardSpec {
    type Target = DiffSpec;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DiscardSpec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<DiscardSpec> for DiffSpec {
    fn from(value: DiscardSpec) -> Self {
        value.0
    }
}

pub(super) mod function;
#[allow(missing_docs)]
pub mod ui {
    /// A specification of which worktree-change to discard.
    pub type DiscardSpec = crate::commit_engine::ui::DiffSpec;
}

mod file;
pub(crate) mod hunk;

#[cfg(unix)]
fn locked_resource_at(
    root: PathBuf,
    path: &Path,
    kind: EntryKind,
) -> anyhow::Result<gix::lock::File> {
    use std::os::unix::fs::PermissionsExt;
    Ok(
        gix::lock::File::acquire_to_update_resource_with_permissions(
            path,
            gix::lock::acquire::Fail::Immediately,
            Some(root),
            || std::fs::Permissions::from_mode(kind as u32),
        )?,
    )
}

#[cfg(windows)]
fn locked_resource_at(
    root: PathBuf,
    path: &Path,
    _kind: EntryKind,
) -> anyhow::Result<gix::lock::File> {
    Ok(gix::lock::File::acquire_to_update_resource(
        path,
        gix::lock::acquire::Fail::Immediately,
        Some(root),
    )?)
}
