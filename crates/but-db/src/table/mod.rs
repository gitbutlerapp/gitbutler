use crate::{M, SchemaVersion};

pub(crate) mod branch_order;
pub(crate) mod butler_actions;
pub(crate) mod ci_checks;
pub(crate) mod claude;
pub(crate) mod fetch_status;
pub(crate) mod file_write_locks;
pub(crate) mod forge_reviews;
pub(crate) mod gerrit_metadata;
pub(crate) mod hunk_assignments;
pub(crate) mod virtual_branches;
pub(crate) mod worktree_meta;

/// Move migrations that relate to tables that don't have their module anymore here.
///
/// Removing a feature does not justify a schema-version bump: keep these at
/// [`SchemaVersion::Zero`] and leave tables that older binaries still read in place.
pub(crate) const M_FULLY_REMOVED: &[M<'static>] = &[
    M::up(
        20251013092749,
        SchemaVersion::Zero,
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
        SchemaVersion::Zero,
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
        SchemaVersion::Zero,
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
        SchemaVersion::Zero,
        "-- Drop worktrees table as metadata is now stored in .git/worktrees/ as files
DROP TABLE IF EXISTS worktrees;",
    ),
    M::up(
        20250717150441,
        SchemaVersion::Zero,
        "CREATE TABLE `workspace_rules`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`enabled` BOOL NOT NULL,
	`trigger` TEXT NOT NULL,
	`filters` TEXT NOT NULL,
	`action` TEXT NOT NULL
);",
    ),
    M::up(
        20260626120000,
        SchemaVersion::Zero,
        "-- Drop workspace_rules table as the rules feature has been removed
DROP TABLE IF EXISTS workspace_rules;",
    ),
    // Keep this inert table so older SchemaVersion::Zero binaries can still open the database.
    M::up(
        20250619192246,
        SchemaVersion::Zero,
        "CREATE TABLE `workflows`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`kind` TEXT NOT NULL,
	`triggered_by` TEXT NOT NULL,
	`status` TEXT NOT NULL,
	`input_commits` TEXT NOT NULL,
	`output_commits` TEXT NOT NULL,
	`summary` TEXT
);",
    ),
];
