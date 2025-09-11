-- Your SQL goes here

CREATE TABLE `butler_actions`(
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
