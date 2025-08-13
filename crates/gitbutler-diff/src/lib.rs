mod diff;
mod hunk;
pub mod write;
pub use diff::{
    diff_files_into_hunks, hunks_by_filepath, reverse_hunk, reverse_hunk_lines, trees, workdir,
    ChangeType, DiffByPathMap, FileDiff, GitHunk,
};
pub use hunk::{Hunk, HunkHash, HunkHeader};
