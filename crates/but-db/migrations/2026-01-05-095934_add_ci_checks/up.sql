-- Your SQL goes here
CREATE TABLE `ci_checks`(
	`id` BIGINT NOT NULL PRIMARY KEY,
	`name` TEXT NOT NULL,
	`output_summary` TEXT NOT NULL,
	`output_text` TEXT NOT NULL,
	`output_title` TEXT NOT NULL,
	`started_at` TIMESTAMP,
	`status_type` TEXT NOT NULL,
	`status_conclusion` TEXT,
	`status_completed_at` TIMESTAMP,
	`head_sha` TEXT NOT NULL,
	`url` TEXT NOT NULL,
	`html_url` TEXT NOT NULL,
	`details_url` TEXT NOT NULL,
	`pull_requests` TEXT NOT NULL,
	`reference` TEXT NOT NULL,
	`last_sync_at` TIMESTAMP NOT NULL,
	`struct_version` INTEGER NOT NULL
);

CREATE INDEX `idx_ci_checks_reference` ON `ci_checks`(`reference`);
