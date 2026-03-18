use std::fmt::Write as _;

use bstr::BString;
use but_core::ChangeId;
use gix::ObjectId;

use crate::{CacheHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
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
)];

/// A utility for reading metadata associated with commit hashes.
pub struct CommitMetadata<'conn> {
    conn: &'conn rusqlite::Connection,
}

/// A utility for mutating metadata associated with commit hashes.
pub struct CommitMetadataMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl CacheHandle {
    /// Return a handle for read-only commit metadata.
    pub fn commit_metadata(&self) -> CommitMetadata<'_> {
        CommitMetadata { conn: &self.conn }
    }

    /// Return a handle for mutating commit metadata.
    pub fn commit_metadata_mut(&mut self) -> rusqlite::Result<CommitMetadataMut<'_>> {
        Ok(CommitMetadataMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl Transaction<'_> {
    /// Return a handle for read-only commit metadata.
    pub fn commit_metadata(&self) -> CommitMetadata<'_> {
        CommitMetadata { conn: self.inner() }
    }

    /// Return a handle for mutating commit metadata.
    pub fn commit_metadata_mut(&mut self) -> rusqlite::Result<CommitMetadataMut<'_>> {
        Ok(CommitMetadataMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

impl CommitMetadata<'_> {
    /// Return the `ChangeId` associated with each `commit_hash`, preserving input order
    /// in the output `[(hash, Option<ChangeId>)]`.
    pub fn change_ids_for_commits(
        &self,
        commit_hashes: impl IntoIterator<Item = ObjectId>,
    ) -> rusqlite::Result<Vec<(ObjectId, Option<ChangeId>)>> {
        const SQLITE_VARIABLE_LIMIT_CHUNK_SIZE: usize = 400;

        let commit_hashes: Vec<ObjectId> = commit_hashes.into_iter().collect();
        let mut out = Vec::with_capacity(commit_hashes.len());

        for chunk in commit_hashes.chunks(SQLITE_VARIABLE_LIMIT_CHUNK_SIZE) {
            let mut values = String::new();
            for (idx, _) in chunk.iter().enumerate() {
                if idx > 0 {
                    values.push_str(", ");
                }
                write!(&mut values, "(?{}, {})", idx + 1, idx + 1)
                    .expect("writing SQL into string");
            }

            let sql = format!(
                "WITH requested(commit_hash, ord) AS (
                     VALUES {values}
                 )
                 SELECT requested.commit_hash, commit_change_ids.change_id
                 FROM requested
                 LEFT JOIN commit_change_ids ON commit_change_ids.commit_hash = requested.commit_hash
                 ORDER BY requested.ord"
            );

            let mut stmt = self.conn.prepare(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(chunk.iter().map(|commit_hash| commit_hash.as_slice())),
                |row| {
                    let commit_hash = {
                        let bytes = row.get::<_, Vec<u8>>(0)?;
                        ObjectId::try_from(bytes.as_slice()).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Blob,
                                Box::new(err),
                            )
                        })?
                    };
                    let change_id = row.get::<_, Option<Vec<u8>>>(1)?.map(decode_change_id);
                    Ok((commit_hash, change_id))
                },
            )?;
            out.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }

        Ok(out)
    }

    /// List all commit hashes (ordered) associated with `change_id`.
    pub fn commit_hashes_by_change_id(
        &self,
        change_id: &ChangeId,
    ) -> rusqlite::Result<Vec<ObjectId>> {
        let encoded = encode_change_id(change_id);
        let mut stmt = self.conn.prepare(
            "SELECT commit_hash
             FROM commit_change_ids
             WHERE change_id = ?1
             ORDER BY commit_hash",
        )?;

        stmt.query_map([encoded], |row| {
            let bytes = row.get::<_, Vec<u8>>(0)?;
            ObjectId::try_from(bytes.as_slice()).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Blob,
                    Box::new(err),
                )
            })
        })?
        .collect()
    }
}

impl CommitMetadataMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> CommitMetadata<'_> {
        CommitMetadata { conn: &self.sp }
    }

    /// Set one `ChangeId` per commit hash, processing `entries` in a single savepoint for efficiency.
    ///
    /// Note that this is *probably* best executed in an immediate transaction, whose acquisition is also serving
    /// as synchronisation point for multiple writers.
    pub fn set_change_ids(
        self,
        entries: impl IntoIterator<Item = (ObjectId, ChangeId)>,
    ) -> rusqlite::Result<()> {
        let sp = self.sp;

        let mut insert_commit = sp.prepare(
            "INSERT OR IGNORE INTO commit_metadata (commit_hash)
             VALUES (?1)",
        )?;
        let mut upsert_change_id = sp.prepare(
            "INSERT INTO commit_change_ids (commit_hash, change_id)
             VALUES (?1, ?2)
             ON CONFLICT(commit_hash) DO UPDATE SET change_id = excluded.change_id",
        )?;

        for (commit_hash, change_id) in entries {
            insert_commit.execute([commit_hash.as_slice()])?;
            upsert_change_id.execute(rusqlite::params![
                commit_hash.as_slice(),
                encode_change_id(&change_id)
            ])?;
        }
        drop(upsert_change_id);
        drop(insert_commit);

        sp.commit()?;
        Ok(())
    }

    /// Delete all metadata rows for the given commit hashes.
    pub fn delete_commits(
        self,
        commit_hashes: impl IntoIterator<Item = ObjectId>,
    ) -> rusqlite::Result<()> {
        let sp = self.sp;
        let mut stmt = sp.prepare("DELETE FROM commit_metadata WHERE commit_hash = ?1")?;

        for commit_hash in commit_hashes {
            stmt.execute([commit_hash.as_slice()])?;
        }
        drop(stmt);

        sp.commit()?;
        Ok(())
    }
}

const RAW_CHANGE_ID_ENCODING: u8 = 0;
const REVERSE_HEX_CHANGE_ID_ENCODING: u8 = 1;

fn encode_change_id(change_id: &ChangeId) -> Vec<u8> {
    match change_id.decode_reverse_hex_bytes() {
        Some(decoded) => {
            let mut out = Vec::with_capacity(1 + decoded.len());
            out.push(REVERSE_HEX_CHANGE_ID_ENCODING);
            out.extend(decoded);
            out
        }
        None => {
            let bytes = change_id.as_bytes();
            let mut out = Vec::with_capacity(1 + bytes.len());
            out.push(RAW_CHANGE_ID_ENCODING);
            out.extend_from_slice(bytes);
            out
        }
    }
}

fn decode_change_id(bytes: Vec<u8>) -> ChangeId {
    match bytes.split_first() {
        Some((&REVERSE_HEX_CHANGE_ID_ENCODING, rest)) => ChangeId::from_bytes(rest),
        Some((&RAW_CHANGE_ID_ENCODING, rest)) => ChangeId::from(BString::from(rest)),
        Some((_unknown, rest)) => ChangeId::from(BString::from(rest)),
        None => ChangeId::default(),
    }
}
