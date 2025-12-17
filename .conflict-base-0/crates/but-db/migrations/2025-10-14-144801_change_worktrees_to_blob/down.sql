-- Revert worktrees table path and reference columns back to TEXT

-- Create old table structure with TEXT columns
CREATE TABLE `worktrees_old`(
	`path` TEXT NOT NULL PRIMARY KEY,
	`reference` TEXT NOT NULL,
	`base` TEXT NOT NULL,
	`source` TEXT NOT NULL
);

-- Drop current table
DROP TABLE worktrees;

-- Rename old table to original name
ALTER TABLE worktrees_old RENAME TO worktrees;
