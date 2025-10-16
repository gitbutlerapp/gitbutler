-- Your SQL goes here
CREATE TABLE `gerrit_metadata`(
	`change_id` TEXT NOT NULL PRIMARY KEY,
	`commit_id` TEXT NOT NULL,
	`review_url` TEXT NOT NULL,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL
);
