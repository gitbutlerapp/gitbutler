CREATE TABLE `files` (
    `session_id` text NOT NULL,
    `file_path` text NOT NULL,
    `content` blob NOT NULL,
    PRIMARY KEY (`session_id`, `file_path`)
);

CREATE INDEX `files_session_id_index` ON `files` (`session_id`);
