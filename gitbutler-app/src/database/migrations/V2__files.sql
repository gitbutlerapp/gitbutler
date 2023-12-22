CREATE TABLE `files` (
    `project_id` text NOT NULL,
    `session_id` text NOT NULL,
    `file_path` text NOT NULL,
    `sha1` blob NOT NULL,
    PRIMARY KEY (`project_id`, `session_id`, `file_path`)
);

CREATE INDEX `files_project_id_session_id_index` ON `files` (`project_id`, `session_id`);

CREATE TABLE `contents` (
    `sha1` blob NOT NULL PRIMARY KEY,
    `content` blob NOT NULL
);
