-- This file should undo anything in `up.sql`
-- Recreate table with old schema

CREATE TABLE `worktrees_new`(
	`path` BLOB NOT NULL PRIMARY KEY,
	`reference` BLOB NOT NULL,
	`base` TEXT NOT NULL,
	`source` TEXT NOT NULL
);

DROP TABLE worktrees;

ALTER TABLE worktrees_new RENAME TO worktrees;
