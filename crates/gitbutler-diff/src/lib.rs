mod diff;
mod hunk;
pub use diff::{ChangeType, FileDiff, GitHunk, diff_files_into_hunks, trees};
pub use hunk::{Hunk, HunkHash, HunkHeader};
