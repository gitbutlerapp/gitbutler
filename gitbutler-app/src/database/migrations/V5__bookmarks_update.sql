ALTER TABLE bookmarks RENAME TO bookmarks_old;

DROP INDEX `bookmarks_project_id_idx`;

CREATE TABLE bookmarks (
    `project_id` text NOT NULL,
    `timestamp_ms` text NOT NULL,
    `note` text NOT NULL,
    `deleted` boolean NOT NULL,
    `created_timestamp_ms` text NOT NULL,
    `updated_timestamp_ms` text NOT NULL,
    PRIMARY KEY (`project_id`, `timestamp_ms`)
);

CREATE INDEX `bookmarks_project_id_idx` ON `bookmarks` (`project_id`);

INSERT INTO bookmarks (`project_id`, `timestamp_ms`, `note`, `deleted`, `created_timestamp_ms`, `updated_timestamp_ms`)
SELECT
    `project_id`,
    `created_timestamp_ms`,
    `note`,
    `deleted`,
    `created_timestamp_ms`,
    `updated_timestamp_ms`
FROM
    bookmarks_old;

DROP TABLE bookmarks_old;
