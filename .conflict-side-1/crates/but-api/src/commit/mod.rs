/// Functions for amending existing commits.
pub mod amend;

/// Functions for creating new commits.
pub mod create;

/// Functions for discarding commits (changes are lost).
pub mod discard_commit;

/// JSON transport types for commit APIs.
pub mod json;

/// Functions for inserting blank commits.
pub mod insert_blank;

/// Functions for moving changes between commits.
pub mod move_changes;

/// Functions for moving commits within the graph.
pub mod move_commit;

/// Functions for rewording commits.
pub mod reword;

/// Functions for squashing commits.
pub mod squash;

/// Shared result and selector types for commit APIs.
pub mod types;

/// Functions for uncommitting — either entire commits (changes kept in workspace)
/// or specific changes from a commit.
pub mod uncommit;
