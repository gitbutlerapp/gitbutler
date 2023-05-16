CREATE TABLE `sessions` (
    `id` text NOT NULL PRIMARY KEY,
    `project_id` text NOT NULL,
    `hash` text,
    `branch` text,
    `commit` text,
    `start_timestamp_ms` text NOT NULL,
    `last_timestamp_ms` text NOT NULL
);

CREATE INDEX `sessions_project_id_index` ON `sessions` (`project_id`);
