use super::migration::Migration;

const CREATE_BASE_SCHEMA: &str = "
CREATE TABLE changes (
    id BLOB PRIMARY KEY NOT NULL, -- A UUID v4
    is_unapplied_wip INTEGER NOT NULL,
    unapplied_vbranch_name TEXT,
    created_at INTEGER -- A unix timestamp in seconds when the record was created
);
CREATE TABLE commits (
    sha TEXT PRIMARY KEY NOT NULL, -- A commit SHA as a base16 string
    created_at INTEGER, -- A unix timestamp in seconds when the record was created
    change_id BLOB NOT NULL,
    FOREIGN KEY(change_id) REFERENCES changes(change_id)
);
";

pub(crate) fn gitbutler_migrations() -> Vec<Migration> {
    let base_migration = Migration {
        name: "base".to_string(),
        up: |connection| {
            connection.execute_batch(CREATE_BASE_SCHEMA)?;

            Ok(())
        },
    };

    vec![base_migration]
}
