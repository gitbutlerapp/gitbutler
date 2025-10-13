-- Create worktrees table
CREATE TABLE `worktrees`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`base` TEXT NOT NULL,
	`path` TEXT NOT NULL,
	`source` TEXT NOT NULL
);

CREATE INDEX index_worktrees_on_path ON worktrees (path);
