use crate::{M, SchemaVersion};

pub(crate) mod branch_order;
pub(crate) mod ci_checks;
pub(crate) mod fetch_status;
pub(crate) mod forge_reviews;
pub(crate) mod gerrit_metadata;
pub(crate) mod hunk_assignments;
pub(crate) mod obs_markers;
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
    M::up(
        20250703203544,
        SchemaVersion::Zero,
        "CREATE TABLE `claude_code_sessions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`stack_id` TEXT NOT NULL
);",
    ),
    M::up(
        20250811130145,
        SchemaVersion::Zero,
        "CREATE TABLE `claude_sessions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`current_id` TEXT NOT NULL UNIQUE,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL
);

CREATE INDEX index_claude_sessions_on_current_id ON claude_sessions (current_id);

CREATE TABLE `claude_messages`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`session_id` TEXT NOT NULL REFERENCES claude_sessions(id),
	`created_at` TIMESTAMP NOT NULL,
	`content_type` TEXT NOT NULL,
	`content` TEXT NOT NULL
);

CREATE INDEX index_claude_messages_on_session_id ON claude_messages (session_id);
CREATE INDEX index_claude_messages_on_created_at ON claude_messages (created_at);",
    ),
    M::up(
        20250812093543,
        SchemaVersion::Zero,
        "DROP TABLE IF EXISTS `claude_code_sessions`;",
    ),
    M::up(
        20250817195624,
        SchemaVersion::Zero,
        "CREATE TABLE `claude_permission_requests`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL,
	`tool_name` TEXT NOT NULL,
	`input` TEXT NOT NULL,
	`approved` BOOL
);",
    ),
    M::up(
        20250821095340,
        SchemaVersion::Zero,
        "ALTER TABLE claude_sessions ADD COLUMN in_gui BOOLEAN NOT NULL DEFAULT FALSE;",
    ),
    M::up(
        20250821142109,
        SchemaVersion::Zero,
        "-- Add session_ids column to claude_sessions table to track all session IDs used
ALTER TABLE claude_sessions ADD COLUMN session_ids TEXT NOT NULL DEFAULT '[]';

-- Initialize existing sessions with their current_id in the session_ids array
UPDATE claude_sessions SET session_ids = json_array(current_id);",
    ),
    M::up(
        20251106102333,
        SchemaVersion::Zero,
        r#"-- Replace approved (bool) and scope (text) with a single decision (text) column
ALTER TABLE `claude_permission_requests` ADD COLUMN `decision` TEXT;

-- Migrate existing data: approved=true -> allowSession, approved=false -> denySession
UPDATE `claude_permission_requests`
SET `decision` = CASE
    WHEN `approved` = 1 THEN '"allowSession"'
    WHEN `approved` = 0 THEN '"denySession"'
    ELSE NULL
END;

-- Drop the old columns
ALTER TABLE `claude_permission_requests` DROP COLUMN `approved`;"#,
    ),
    M::up(
        20251107134016,
        SchemaVersion::Zero,
        "-- Add permissions columns to claude_sessions table to track approved and denied permissions
ALTER TABLE claude_sessions ADD COLUMN approved_permissions TEXT NOT NULL DEFAULT '[]';
ALTER TABLE claude_sessions ADD COLUMN denied_permissions TEXT NOT NULL DEFAULT '[]';",
    ),
    M::up(
        20251110103940,
        SchemaVersion::Zero,
        "-- Add use_wildcard column to claude_permission_requests table
ALTER TABLE `claude_permission_requests` ADD COLUMN `use_wildcard` BOOLEAN NOT NULL DEFAULT 0;",
    ),
    M::up(
        20250704130757,
        SchemaVersion::Zero,
        "CREATE TABLE `file_write_locks`(
	`path` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`owner` TEXT NOT NULL
);",
    ),
    M::up(
        20250529110746,
        SchemaVersion::Zero,
        "CREATE TABLE `butler_actions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`external_prompt` TEXT NOT NULL,
	`handler` TEXT NOT NULL,
	`handler_prompt` TEXT,
	`snapshot_before` TEXT NOT NULL,
	`snapshot_after` TEXT NOT NULL,
	`response` TEXT,
	`error` TEXT
);

CREATE INDEX `idx_butler_actions_created_at` ON `butler_actions`(`created_at`);
",
    ),
    M::up(
        20250530112246,
        SchemaVersion::Zero,
        "ALTER TABLE `butler_actions` DROP COLUMN `external_prompt`;
ALTER TABLE `butler_actions` ADD COLUMN `external_summary` TEXT NOT NULL;
ALTER TABLE `butler_actions` ADD COLUMN `external_prompt` TEXT;
",
    ),
    M::up(
        20250616090656,
        SchemaVersion::Zero,
        "ALTER TABLE `butler_actions` ADD COLUMN `source` TEXT;",
    ),
    M::up(
        20250619181700,
        SchemaVersion::Zero,
        "ALTER TABLE `butler_actions` DROP COLUMN `handler_prompt`;",
    ),
];
