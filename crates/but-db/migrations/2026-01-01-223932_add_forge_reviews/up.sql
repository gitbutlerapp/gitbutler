-- Your SQL goes here
CREATE TABLE `forge_reviews`(
	`html_url` TEXT NOT NULL,
	`number` BIGINT NOT NULL PRIMARY KEY,
	`title` TEXT NOT NULL,
	`body` TEXT,
	`author` TEXT,
	`labels` TEXT NOT NULL,
	`draft` BOOL NOT NULL,
	`source_branch` TEXT NOT NULL,
	`target_branch` TEXT NOT NULL,
	`sha` TEXT NOT NULL,
	`created_at` TIMESTAMP,
	`modified_at` TIMESTAMP,
	`merged_at` TIMESTAMP,
	`closed_at` TIMESTAMP,
	`repository_ssh_url` TEXT,
	`repository_https_url` TEXT,
	`repo_owner` TEXT,
	`reviewers` TEXT NOT NULL,
	`unit_symbol` TEXT NOT NULL,
	`last_sync_at` TIMESTAMP NOT NULL,
	`struct_version` INTEGER NOT NULL
);

