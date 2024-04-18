//! The diffing engine here provides a low-level mechanism for
//! expressing sets of changes, files, and hunks and calculating,
//! hopefully efficiently, the relationships between those hunks.

mod range_intersection;

pub(crate) mod conflict;
pub(crate) mod file;
pub(crate) mod hunk;
