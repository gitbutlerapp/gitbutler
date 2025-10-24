pub(crate) mod hunk;
pub mod input;
pub mod locks;
pub(crate) mod path;
pub(crate) mod stack;
pub mod utils;
pub(crate) mod workspace;

pub use hunk::HunkRange;
pub use input::{InputCommit, InputDiff, InputFile, InputStack, parse_diff_from_string};
pub use locks::{HunkDependencyOptions, HunkLock, calculate_hunk_dependencies};
pub use path::PathRanges;
pub use stack::StackRanges;
pub use workspace::{RangeCalculationError, WorkspaceRanges};
