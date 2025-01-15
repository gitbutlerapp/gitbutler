#![deny(missing_docs, rust_2018_idioms)]
#![deny(clippy::indexing_slicing)]

//! ### Terminology
//!
//! * **Workspace**
//!     - A GitButler concept of the combination of one or more branches into one worktree. This allows
//!       multiple branches to be perceived in one worktree, by merging multiple branches together.
//!     - Currently, there is only one workspace per repository, but this is something we intend to change
//!       in the future to facilitate new use cases.

use anyhow::Result;
use bstr::BString;
use gitbutler_id::id::Id;
use gitbutler_stack::VirtualBranchesHandle;
use std::path::Path;

/// Represents a lightweight version of a [`gitbutler_stack::Stack`] for listing.
#[derive(Debug, Clone)]
pub struct StackEntry {
    /// The ID of the stack.
    pub id: Id<gitbutler_stack::Stack>,
    /// The list of the branch names that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub branch_names: Vec<BString>,
}

/// Returns the list of stacks that are currently part of the workspace.
/// If there are no applied stacks, the returned Vec is empty.
/// If the GitButler state file in the provided path is missing or invalid, an error is returned.
///
/// - `gb_dir`: The path to the GitButler state for the project. Normally this is `.git/gitbutler` in the project's repository.
pub fn stacks(gb_dir: &Path) -> Result<Vec<StackEntry>> {
    let state = state_handle(gb_dir);
    Ok(state
        .list_stacks_in_workspace()?
        .into_iter()
        .map(|stack| StackEntry {
            id: stack.id,
            branch_names: stack.heads().into_iter().map(Into::into).collect(),
        })
        .collect())
}

fn state_handle(gb_state_path: &Path) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(gb_state_path)
}
