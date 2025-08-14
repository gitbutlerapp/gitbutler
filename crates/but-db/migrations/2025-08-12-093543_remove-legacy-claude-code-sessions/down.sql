-- This file should undo anything in `up.sql`

CREATE TABLE `claude_code_sessions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`stack_id` TEXT NOT NULL
);
