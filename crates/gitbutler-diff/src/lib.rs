mod diff;
mod hunk;
pub use diff::{ChangeType, FileDiff, GitHunk};
pub use hunk::{Hunk, HunkHash, HunkHeader};
