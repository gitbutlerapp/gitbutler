use crate::{M, cache::SchemaVersion};

/// Historical project-cache migrations for ChangeID storage, which has been fully removed.
pub(crate) const M: &[M<'static>] = &[
    M::up_project_cache(
        2026_03_12__13_00_00,
        SchemaVersion::Zero,
        "CREATE TABLE `commit_metadata`(
    `commit_hash` BLOB NOT NULL PRIMARY KEY
);

CREATE TABLE `commit_change_ids`(
    `commit_hash` BLOB NOT NULL PRIMARY KEY,
    `change_id` BLOB NOT NULL,
    FOREIGN KEY (`commit_hash`) REFERENCES `commit_metadata`(`commit_hash`) ON DELETE CASCADE
);

CREATE INDEX `idx_commit_change_ids_change_id` ON `commit_change_ids`(`change_id`);",
    ),
    M::up_project_cache(
        2026_04_09__12_00_00,
        SchemaVersion::One,
        "-- ChangeIDs are no longer cached in project-local cache handles.
DROP TABLE IF EXISTS `commit_change_ids`;
DROP TABLE IF EXISTS `commit_metadata`;",
    ),
];
