//! Functions that operate on commits.

/// Endpoints for amending commits.
pub mod amend;
/// Endpoints for creating commits.
pub mod create;
/// Endpoints for inserting blank commits.
pub mod insert_blank;
/// JSON transport types for commit APIs.
// Ideally, this would be private and only used for transport, but we don't have
// a good parameter mapping solution at the minute.
pub mod json;
/// Endpoints for moving changes.
pub mod move_changes;
/// Endpoints for moving commits.
pub mod move_commit;
/// Endpoints for rewording commits.
pub mod reword;
/// Shared types for commit APIs.
pub mod types;
/// Endpoints for uncommitting commits.
pub mod uncommit_changes;
