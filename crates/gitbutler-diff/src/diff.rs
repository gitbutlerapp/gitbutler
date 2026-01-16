use but_serde::BStringForFrontend;
use git2::DiffHunk;
use serde::{Deserialize, Serialize};

/// The type of change
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// Entry does not exist in old version
    Added,
    /// Entry is untracked item in workdir
    Untracked,
    /// Entry does not exist in new version
    Deleted,
    /// Entry content changed between old and new
    Modified,
}
impl From<git2::Delta> for ChangeType {
    fn from(v: git2::Delta) -> Self {
        use ChangeType as C;
        use git2::Delta as D;
        match v {
            D::Added => C::Added,
            D::Untracked => C::Untracked,
            D::Modified
            | D::Unmodified
            | D::Renamed
            | D::Copied
            | D::Typechange
            | D::Conflicted => C::Modified,
            D::Ignored | D::Unreadable | D::Deleted => C::Deleted,
        }
    }
}

/// A description of a hunk, as identified by its line number and the amount of lines it spans
/// before and after the change.
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// The `+`, `-` or ` ` prefixed lines of the diff produced by `git2`, along with their line separator.
    #[serde(rename = "diff")]
    pub diff_lines: BStringForFrontend,
    pub binary: bool,
    pub change_type: ChangeType,
}

/// Comparison
impl GitHunk {
    /// workspace_intersects_unapplied is used to determine if a hunk from a diff between workspace
    /// and the trunk intersects with an unapplied hunk. We want to use the new start/end for the
    /// integration hunk and the old start/end for the unapplied hunk.
    pub fn workspace_intersects_unapplied(
        workspace_hunk: &GitHunk,
        unapplied_hunk: &GitHunk,
    ) -> bool {
        let unapplied_old_end = unapplied_hunk.old_start + unapplied_hunk.old_lines;
        let workspace_new_end = workspace_hunk.new_start + workspace_hunk.new_lines;

        unapplied_hunk.old_start <= workspace_new_end
            && workspace_hunk.new_start <= unapplied_old_end
    }
}

impl PartialEq<DiffHunk<'_>> for &GitHunk {
    fn eq(&self, other: &DiffHunk) -> bool {
        self.new_start == other.new_start()
            && self.new_lines == other.new_lines()
            && self.old_start == other.old_start()
            && self.old_lines == other.old_lines()
    }
}
