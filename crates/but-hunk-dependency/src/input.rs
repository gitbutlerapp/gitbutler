use but_core::TreeStatusKind;
use but_workspace::StackId;
use gix::bstr::BString;

/// A whole stack for the purpose of generating hunk locking information from it, for use in [`WorkspaceRanges::try_from_stacks()`](crate::WorkspaceRanges::try_from_stacks()) .
#[derive(Debug, Clone)]
pub struct InputStack {
    /// The stack that contains [commits](InputStack::commits_from_base_to_tip).
    pub stack_id: StackId,
    /// The commits in the stack.
    ///
    /// **The commits are ordered from the base to the tip of the stack (application order)**.
    pub commits_from_base_to_tip: Vec<InputCommit>,
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
    pub path: BString,
    /// The hunks that changed in this file.
    pub hunks: Vec<InputDiffHunk>,
    /// The kind of change of the parent file.
    pub change_type: TreeStatusKind,
}

/// A
#[derive(Debug, Clone, Copy)]
pub struct InputDiffHunk {
    /// The 1-based line number at which the previous version of the file started.
    pub old_start: u32,
    /// The non-zero amount of lines included in the previous version of the file.
    pub old_lines: u32,
    /// The 1-based line number at which the new version of the file started.
    pub new_start: u32,
    /// The non-zero amount of lines included in the new version of the file.
    pub new_lines: u32,
}

impl InputDiffHunk {
    /// Compute the amount of lines that are left when substracting old-lines from new-lines.
    pub fn net_lines(&self) -> anyhow::Result<i32> {
        let old_lines = i32::try_from(self.old_lines)?;
        let new_lines = i32::try_from(self.new_lines)?;
        Ok(new_lines - old_lines)
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
    ) -> Self {
        InputDiffHunk {
            old_start: *old_start,
            old_lines: *old_lines,
            new_start: *new_start,
            new_lines: *new_lines,
        }
    }
}
