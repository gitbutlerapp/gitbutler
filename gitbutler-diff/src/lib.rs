//! # GitButler diff algorithms and datastructures library
//!
//! This crate implements the GitButler algorithms and data structures
//! for working with diffs/changesets. Diffs are, at their core,
//! sets of hunks within one or more files. Canonically, Git
//! uses its own system for managing changesets called the staging
//! area, whereas GitButler uses multiple changesets within
//! a single Git staging area to add a level of granularity and
//! organization of multiple distinct groups of changes, called
//! "virtual branches". This library implements algorithms for
//! managing, moving, and otherwise working with diffs
//! across virtual branches.
//!
//! Note that all text is assumed UTF-8 due to how Git is designed.
//! For more information about how Git handles encoding, see
//! <https://git-scm.com/docs/gitattributes/2.19.2#_working_tree_encoding>.
//!
//! ---
//!
//! ## Hunk Theory
//!
//! GitButler's hunk theory is as follows:
//!
//! A hunk is a contiguous span of lines in a file that are being
//! changed. Boiling away the quintessential 'diff' representation
//! (with `-` indicating a line being removed and `+` indicating
//! a line being added, as one nuclear operation), the algorithms
//! herein treat a hunk as two discrete operations - a removal
//! of the source range and an insertion of the replacement text.
//!
//! Thus, conflicts are determined based on whether or not two hunks
//! remove all or part of the same source range. As a corollary,
//! two source ranges that intersect are thought of as conflicts.
//!
//! This allows us to trivially detect and help to reason about
//! conflicts, and to easily re-calculate and reflow content
//! within a file given a list of hunks (including sorting and
//! efficiently calculating new line number ranges).
//!
//! ---
//!
//! For more information, see the
//! [GitButler organization](https://github.com/gitbutlerapp)
//! or the [GitButler website](https://gitbutler.com).
#![deny(missing_docs)]
#![feature(impl_trait_in_assoc_type, iter_map_windows, slice_as_chunks)]

mod linefile;
mod signature;
mod span;

#[cfg(feature = "mmap")]
pub use self::linefile::mmap::MmapLineFile;
pub use self::{
    linefile::{memory::MemoryLineFile, CrlfBehavior, LineEndings, LineFile},
    signature::Signature,
    span::LineSpan,
};
