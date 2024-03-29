ALTER TABLE `bookmarks`
    ADD `created_timestamp_ms` text NOT NULL DEFAULT 0;

UPDATE
    `bookmarks`
SET
    `created_timestamp_ms` = `timestamp_ms`;

ALTER TABLE `bookmarks`
    DROP COLUMN `timestamp_ms`;

ALTER TABLE `bookmarks`
    ADD `updated_timestamp_ms` text;

ALTER TABLE `bookmarks`
    ADD `deleted` boolean NOT NULL DEFAULT FALSE;
