pub(crate) mod hunk;
pub mod input;
pub mod locks;
pub(crate) mod path;
pub(crate) mod stack;
pub mod utils;
pub(crate) mod workspace;

pub use {
    hunk::HunkRange,
    input::{InputCommit, InputDiff, InputFile, InputStack, parse_diff_from_string},
    locks::{HunkDependencyOptions, HunkLock, calculate_hunk_dependencies},
    path::PathRanges,
    stack::StackRanges,
    workspace::RangeCalculationError,
    workspace::WorkspaceRanges,
};
