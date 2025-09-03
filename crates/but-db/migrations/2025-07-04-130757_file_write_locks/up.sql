-- Your SQL goes here
CREATE TABLE `file_write_locks`(
	`path` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`owner` TEXT NOT NULL
);
