use crate::M;

pub(crate) mod butler_actions;
pub(crate) mod ci_checks;
pub(crate) mod claude;
pub(crate) mod file_write_locks;
pub(crate) mod forge_reviews;
pub(crate) mod gerrit_metadata;
pub(crate) mod hunk_assignments;
pub(crate) mod virtual_branches;
pub(crate) mod workflows;
pub(crate) mod workspace_rules;

/// Move migrations that relate to tables that don't have their module anymore here.
pub(crate) const M_FULLY_REMOVED: &[M<'static>] = &[
    M::up(
        20251013092749,
        "CREATE TABLE `worktrees`(
	`path` TEXT NOT NULL PRIMARY KEY,
	`reference` TEXT NOT NULL,
	`base` TEXT NOT NULL,
	`source` TEXT NOT NULL
);
",
    ),
    M::up(
        20251014144801,
        "-- Create new table with BLOB columns
CREATE TABLE `worktrees_new`(
	`path` BLOB NOT NULL PRIMARY KEY,
	`reference` BLOB NOT NULL,
	`base` TEXT NOT NULL,
	`source` TEXT NOT NULL
);

-- Drop old table
DROP TABLE worktrees;

-- Rename new table to original name
ALTER TABLE worktrees_new RENAME TO worktrees;
",
    ),
    M::up(
        20251015105125,
        "-- Create new table with updated schema
CREATE TABLE `worktrees_new`(
	`path` BLOB NOT NULL PRIMARY KEY,
	`base` TEXT NOT NULL,
	`created_from_ref` BLOB
);

-- Drop old table and all existing entries (as requested)
DROP TABLE worktrees;

-- Rename new table to original name
ALTER TABLE worktrees_new RENAME TO worktrees;",
    ),
    M::up(
        20251017092314,
        "-- Drop worktrees table as metadata is now stored in .git/worktrees/ as files
DROP TABLE IF EXISTS worktrees;",
    ),
];
