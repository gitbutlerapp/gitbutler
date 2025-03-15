//! Utility types related to discarding changes in the worktree.

use crate::commit_engine::DiffSpec;
use std::ops::Deref;

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

pub(super) mod function;
