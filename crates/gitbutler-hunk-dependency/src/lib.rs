#![feature(unsigned_signed_diff)]
pub(crate) mod hunk;
pub mod input;
pub mod locks;
pub(crate) mod path;
pub(crate) mod stack;
pub(crate) mod workspace;

pub use {
    hunk::HunkRange,
    input::{InputCommit, InputDiff, InputFile, InputStack},
    locks::{calculate_hunk_dependencies, HunkDependencyOptions, HunkLock},
    path::PathRanges,
    stack::StackRanges,
    workspace::WorkspaceRanges,
};
