-- Change worktrees table path and reference columns to BLOB
-- SQLite doesn't support ALTER COLUMN, so we need to recreate the table

-- Create new table with BLOB columns
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
