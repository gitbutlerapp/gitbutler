-- Your SQL goes here
CREATE TABLE `claude_permission_requests`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL,
	`tool_name` TEXT NOT NULL,
	`input` TEXT NOT NULL,
	`approved` BOOL
);
