CREATE TABLE `deltas` (
    `session_id` text NOT NULL,
    `timestamp_ms` text NOT NULL,
    `operations` blob NOT NULL,
    `file_path` text NOT NULL,
    PRIMARY KEY (`session_id`, `timestamp_ms`, `file_path`)
);

CREATE INDEX `deltas_session_id_index` ON `deltas` (`session_id`);
