-- Your SQL goes here
CREATE TABLE `hunk_assignments`(
	`hunk_header` TEXT,
	`path` TEXT NOT NULL,
	`path_bytes` BINARY NOT NULL,
	`stack_id` TEXT,
	`hunk_locks` TEXT NOT NULL,
	PRIMARY KEY(`path`, `hunk_header`)
);

