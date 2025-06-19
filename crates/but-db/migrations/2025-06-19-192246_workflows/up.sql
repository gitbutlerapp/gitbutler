-- Your SQL goes here


CREATE TABLE `workflows`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`kind` TEXT NOT NULL,
	`triggered_by` TEXT NOT NULL,
	`status` TEXT NOT NULL,
	`input_commits` TEXT NOT NULL,
	`output_commits` TEXT NOT NULL,
	`summary` TEXT
);

