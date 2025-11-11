use crate::TreeChange;
use bstr::BString;
use serde::{Deserialize, Serialize};

/// A change that should be used to create a new commit or alter an existing one, along with enough information to know where to find it.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSpec {
    /// The previous location of the entry, the source of a rename if there was one.
    #[serde(rename = "previousPathBytes")]
    pub previous_path: Option<BString>,
    /// The worktree-relative path to the worktree file with the content to commit.
    ///
    /// If `hunks` is empty, this means the current content of the file should be committed.
    #[serde(rename = "pathBytes")]
    pub path: BString,
    /// If one or more hunks are specified, match them with actual changes currently in the worktree.
    /// Failure to match them will lead to the change being dropped.
    /// If empty, the whole file is taken as is if this seems to be an addition.
    /// Otherwise, the whole file is being deleted.
    pub hunk_headers: Vec<HunkHeader>,
}

impl From<&TreeChange> for DiffSpec {
    fn from(change: &crate::TreeChange) -> Self {
        Self {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path.to_owned(),
            hunk_headers: vec![],
        }
    }
}

impl From<TreeChange> for DiffSpec {
    fn from(change: crate::TreeChange) -> Self {
        Self {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path.to_owned(),
            hunk_headers: vec![],
        }
    }
}

/// The header of a hunk that represents a change to a file.
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkHeader {
    /// The 1-based line number at which the previous version of the file started.
    pub old_start: u32,
    /// The non-zero number of lines included in the previous version of the file.
    pub old_lines: u32,
    /// The 1-based line number at which the new version of the file started.
    pub new_start: u32,
    /// The non-zero number of lines included in the new version of the file.
    pub new_lines: u32,
}

impl From<&crate::unified_diff::DiffHunk> for HunkHeader {
    fn from(
        crate::unified_diff::DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            diff: _,
        }: &crate::unified_diff::DiffHunk,
    ) -> Self {
        Self {
            old_start: *old_start,
            old_lines: *old_lines,
            new_start: *new_start,
            new_lines: *new_lines,
        }
    }
}

impl From<crate::unified_diff::DiffHunk> for HunkHeader {
    fn from(
        crate::unified_diff::DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            diff: _,
        }: crate::unified_diff::DiffHunk,
    ) -> Self {
        Self {
            old_start,
            old_lines,
            new_start,
            new_lines,
        }
    }
}

impl HunkHeader {
    /// Returns the hunk header with the old and new ranges swapped.
    ///
    /// This is useful for applying the hunk in reverse.
    pub fn reverse(&self) -> Self {
        Self {
            old_start: self.new_start,
            old_lines: self.new_lines,
            new_start: self.old_start,
            new_lines: self.old_lines,
        }
    }
}

impl std::fmt::Debug for HunkHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"HunkHeader("-{},{}", "+{},{}")"#,
            self.old_start, self.old_lines, self.new_start, self.new_lines
        )
    }
}

/// Computed using the file kinds/modes of two [`ChangeState`] instances to represent
/// the *dominant* change to display. Note that it can stack with a content change,
/// but *should not only in case of a `TypeChange*`*.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[expect(missing_docs)]
pub enum ModeFlags {
    ExecutableBitAdded,
    ExecutableBitRemoved,
    TypeChangeFileToLink,
    TypeChangeLinkToFile,
    TypeChange,
}
