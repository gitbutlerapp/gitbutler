CREATE TABLE `deltas` (
    `session_id` text NOT NULL,
    `project_id` text NOT NULL,
    `timestamp_ms` text NOT NULL,
    `operations` blob NOT NULL,
    `file_path` text NOT NULL,
    PRIMARY KEY (`project_id`, `session_id`, `timestamp_ms`, `file_path`)
);

CREATE INDEX `deltas_project_id_session_id_index` ON `deltas` (`project_id`, `session_id`);

CREATE INDEX `deltas_project_id_session_id_file_path_index` ON `deltas` (`project_id`, `session_id`, `file_path`);
