-- Update worktrees table to new schema
-- SQLite doesn't support ALTER COLUMN, so we need to recreate the table

-- Create new table with updated schema
CREATE TABLE `worktrees_new`(
	`path` BLOB NOT NULL PRIMARY KEY,
	`base` TEXT NOT NULL,
	`created_from_ref` BLOB
);

-- Drop old table and all existing entries (as requested)
DROP TABLE worktrees;

-- Rename new table to original name
ALTER TABLE worktrees_new RENAME TO worktrees;
