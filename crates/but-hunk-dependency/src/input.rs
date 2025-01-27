use std::path::PathBuf;

use anyhow::anyhow;
use but_core::TreeStatusKind;
use but_workspace::StackId;

/// A whole stack for the purpose of generating hunk locking information from it, for use in [Dependencies::calculate()](crate::Dependencies::calculate()).
#[derive(Debug, Clone)]
pub struct InputStack {
    /// The stack that contains [`commits`](InputStack::commits).
    pub stack_id: StackId,
    /// The commits in the stack.
    ///
    /// The commits are ordered from the base to the top of the stack (application order).
    pub commits: Vec<InputCommit>,
}

/// A commit along with the files that it changes, used in [`InputStack`].
#[derive(Debug, Clone)]
pub struct InputCommit {
    /// The id of the commit this instance refers to.
    pub commit_id: gix::ObjectId,
    /// The files were changed by this commit.
    pub files: Vec<InputFile>,
}

/// A single file changed in an [`InputCommit`].
#[derive(Debug, Clone)]
pub struct InputFile {
    /// The worktree-relative path to the file.
    // TODO: make this BString.
    pub path: PathBuf,
    /// The hunks that changed in this file.
    pub hunks: Vec<InputDiffHunk>,
    // TODO: add `change_type` here.
}

/// A
#[derive(Debug, Clone, Copy)]
// TODO: revise this name, something with Hunk?
pub struct InputDiffHunk {
    /// The 1-based line number at which the previous version of the file started.
    pub old_start: u32,
    /// The non-zero amount of lines included in the previous version of the file.
    pub old_lines: u32,
    /// The 1-based line number at which the new version of the file started.
    pub new_start: u32,
    /// The non-zero amount of lines included in the new version of the file.
    pub new_lines: u32,
    /// The kind of change of the parent file.
    // TODO: remove from here in favor of changetype above.
    pub change_type: TreeStatusKind,
}

impl InputDiffHunk {
    /// Compute the amount of lines that are left when substracting old-lines from new-lines.
    pub fn net_lines(&self) -> anyhow::Result<i32> {
        self.new_lines
            .checked_signed_diff(self.old_lines)
            .ok_or(anyhow!("u32 -> i32 conversion overflow"))
    }
}

impl InputDiffHunk {
    /// Create a new instance from unified `diff`.
    pub fn from_unified_diff(
        but_core::unified_diff::DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            diff: _,
        }: &but_core::unified_diff::DiffHunk,
        change_type: TreeStatusKind,
    ) -> Self {
        InputDiffHunk {
            old_start: *old_start,
            old_lines: *old_lines,
            new_start: *new_start,
            new_lines: *new_lines,
            change_type,
        }
    }
}
