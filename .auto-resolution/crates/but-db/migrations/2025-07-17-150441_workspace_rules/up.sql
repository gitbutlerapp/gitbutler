-- Your SQL goes here
CREATE TABLE `workspace_rules`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`enabled` BOOL NOT NULL,
	`trigger` TEXT NOT NULL,
	`filters` TEXT NOT NULL,
	`action` TEXT NOT NULL
);
