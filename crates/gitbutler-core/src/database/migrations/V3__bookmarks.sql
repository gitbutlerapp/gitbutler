CREATE TABLE `bookmarks` (
    `id` text NOT NULL PRIMARY KEY,
    `project_id` text NOT NULL,
    `timestamp_ms` text NOT NULL,
    `note` text NOT NULL
);

CREATE INDEX bookmarks_project_id_idx ON `bookmarks` (`project_id`);
