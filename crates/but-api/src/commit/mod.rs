pub mod amend;

pub mod cherry_pick;

pub mod create;

/// Functions for discarding commits (changes are lost).
pub mod discard_commit;

pub mod json;

pub mod insert_blank;

pub mod move_changes;

pub mod move_commit;

pub mod reword;

pub mod squash;

pub mod types;

/// Functions for uncommitting — either entire commits (changes kept in workspace)
/// or specific changes from a commit.
pub mod uncommit;
