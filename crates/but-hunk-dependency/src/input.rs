use anyhow::anyhow;
use but_core::TreeStatusKind;
use gix::bstr::BString;

use crate::ui::HunkLockTarget;

#[derive(Debug, Clone)]
pub struct InputStack {
    pub target: HunkLockTarget,
    /// TODO: Why base to tip? This is inconsistent with the rest of the codebase.
    pub commits_from_base_to_tip: Vec<InputCommit>,
}

#[derive(Debug, Clone)]
pub struct InputCommit {
    pub commit_id: gix::ObjectId,
    pub files: Vec<InputFile>,
}

#[derive(Debug, Clone)]
pub struct InputFile {
    /// The worktree-relative path to the file.
    pub path: BString,
    pub hunks: Vec<InputDiffHunk>,
    pub change_type: TreeStatusKind,
}

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
    pub fn net_lines(&self) -> anyhow::Result<i32> {
        // TODO: use `checked_signed_diff` instead when stable.
        (self.new_lines as i64)
            .checked_sub(self.old_lines as i64)
            .and_then(|n| i32::try_from(n).ok())
            .ok_or(anyhow!("u32 -> i32 conversion overflow"))
    }
}

impl InputDiffHunk {
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
