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
pub(crate) mod worktree_meta;

/// Move migrations that relate to tables that don't have their module anymore here.
///
/// Preserve historical SQL and schema versions here. Planned incompatible removals belong in
/// a new migration, with the corresponding forward-compatibility boundary documented there.
pub(crate) const M_FULLY_REMOVED: &[M<'static>] = &[
    M::up(
        20260219130000,
        SchemaVersion::Zero,
        "CREATE TABLE `vb_state`(
	`id` INTEGER PRIMARY KEY CHECK (`id` = 1),
	`initialized` INTEGER NOT NULL DEFAULT 0,
	`default_target_remote_name` TEXT,
	`default_target_branch_name` TEXT,
	`default_target_remote_url` TEXT,
	`default_target_sha` TEXT,
	`default_target_push_remote_name` TEXT,
	`last_pushed_base_sha` TEXT,
	`toml_last_seen_mtime_ns` INTEGER,
	`toml_last_seen_sha256` TEXT
);

CREATE TABLE `vb_stacks`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`source_refname` TEXT,
	`upstream_remote_name` TEXT,
	`upstream_branch_name` TEXT,
	`sort_order` INTEGER NOT NULL,
	`in_workspace` INTEGER NOT NULL,
	`legacy_name` TEXT NOT NULL DEFAULT '',
	`legacy_notes` TEXT NOT NULL DEFAULT '',
	`legacy_ownership` TEXT NOT NULL DEFAULT '',
	`legacy_allow_rebasing` INTEGER NOT NULL DEFAULT 1,
	`legacy_post_commits` INTEGER NOT NULL DEFAULT 0,
	`legacy_tree_sha` TEXT NOT NULL DEFAULT '0000000000000000000000000000000000000000',
	`legacy_head_sha` TEXT NOT NULL DEFAULT '0000000000000000000000000000000000000000',
	`legacy_created_timestamp_ms` TEXT NOT NULL DEFAULT '0',
	`legacy_updated_timestamp_ms` TEXT NOT NULL DEFAULT '0'
);

CREATE TABLE `vb_stack_heads`(
	`stack_id` TEXT NOT NULL,
	`position` INTEGER NOT NULL,
	`name` TEXT NOT NULL,
	`head_sha` TEXT NOT NULL,
	`pr_number` INTEGER,
	`archived` INTEGER NOT NULL DEFAULT 0,
	`review_id` TEXT,
	PRIMARY KEY(`stack_id`, `position`),
	FOREIGN KEY(`stack_id`) REFERENCES `vb_stacks`(`id`) ON DELETE CASCADE
);

CREATE TABLE `vb_branch_targets`(
	`stack_id` TEXT NOT NULL PRIMARY KEY,
	`remote_name` TEXT NOT NULL,
	`branch_name` TEXT NOT NULL,
	`remote_url` TEXT NOT NULL,
	`sha` TEXT NOT NULL,
	`push_remote_name` TEXT,
	FOREIGN KEY(`stack_id`) REFERENCES `vb_stacks`(`id`) ON DELETE CASCADE
);

CREATE INDEX `idx_vb_stacks_sort_order` ON `vb_stacks`(`sort_order`);
CREATE INDEX `idx_vb_stacks_in_workspace` ON `vb_stacks`(`in_workspace`);
CREATE INDEX `idx_vb_stack_heads_stack_id` ON `vb_stack_heads`(`stack_id`);
",
    ),
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
