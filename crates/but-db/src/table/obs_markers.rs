use rusqlite::params;

use crate::{DbHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20261102120000,
    SchemaVersion::Zero,
    "CREATE TABLE `obs_markers`(
    `id` BLOB NOT NULL PRIMARY KEY,
    `prec` BLOB NOT NULL,
    `date_seconds` INTEGER NOT NULL,
    `date_offset` INTEGER NOT NULL,
    `parents_recorded` INTEGER NOT NULL
);
CREATE INDEX `idx_obs_markers_prec` ON `obs_markers`(`prec`);

CREATE TABLE `obs_marker_succs`(
    `marker_id` BLOB NOT NULL,
    `position` INTEGER NOT NULL,
    `succ` BLOB NOT NULL,
    PRIMARY KEY (`marker_id`, `position`),
    FOREIGN KEY (`marker_id`) REFERENCES `obs_markers`(`id`)
);
CREATE INDEX `idx_obs_marker_succs_succ` ON `obs_marker_succs`(`succ`);

CREATE TABLE `obs_marker_parents`(
    `marker_id` BLOB NOT NULL,
    `position` INTEGER NOT NULL,
    `parent` BLOB NOT NULL,
    PRIMARY KEY (`marker_id`, `position`),
    FOREIGN KEY (`marker_id`) REFERENCES `obs_markers`(`id`)
);

CREATE TABLE `obs_marker_meta`(
    `marker_id` BLOB NOT NULL,
    `key` BLOB NOT NULL,
    `value` BLOB NOT NULL,
    PRIMARY KEY (`marker_id`, `key`),
    FOREIGN KEY (`marker_id`) REFERENCES `obs_markers`(`id`)
);",
)];

/// An obsolescence marker as it is stored, see the `but-evo` crate for its meaning.
///
/// Tests are in `but-db/tests/db/table/obs_markers.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsMarker {
    /// A content hash of all other fields, which makes storing a marker idempotent.
    pub id: Vec<u8>,
    pub prec: Vec<u8>,
    pub succs: Vec<Vec<u8>>,
    /// Pairs in key order, as that's how they are read back.
    pub meta: Vec<(Vec<u8>, Vec<u8>)>,
    pub date_seconds: i64,
    pub date_offset: i32,
    /// `None` means not recorded, and `Some(vec![])` means `prec` is a root commit.
    pub parents: Option<Vec<Vec<u8>>>,
}

impl DbHandle {
    pub fn obs_markers(&self) -> ObsMarkersHandle<'_> {
        ObsMarkersHandle { conn: &self.conn }
    }
}

impl Transaction<'_> {
    pub fn obs_markers_mut(&mut self) -> ObsMarkersHandleMut<'_> {
        ObsMarkersHandleMut { conn: self.inner() }
    }
}

pub struct ObsMarkersHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

/// Only available from a [`Transaction`], which is what makes a multi-table insert atomic.
pub struct ObsMarkersHandleMut<'conn> {
    conn: &'conn rusqlite::Connection,
}

impl ObsMarkersHandle<'_> {
    pub fn find_by_prec(&self, prec: &[u8]) -> rusqlite::Result<Vec<ObsMarker>> {
        self.find("SELECT id FROM obs_markers WHERE prec = ?1", prec)
    }

    pub fn find_by_succ(&self, succ: &[u8]) -> rusqlite::Result<Vec<ObsMarker>> {
        self.find(
            "SELECT DISTINCT marker_id FROM obs_marker_succs WHERE succ = ?1",
            succ,
        )
    }

    fn find(&self, ids_query: &str, commit_id: &[u8]) -> rusqlite::Result<Vec<ObsMarker>> {
        let ids = self
            .conn
            .prepare_cached(ids_query)?
            .query_map([commit_id], |row| row.get::<_, Vec<u8>>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.into_iter().map(|id| self.load(id)).collect()
    }

    fn load(&self, id: Vec<u8>) -> rusqlite::Result<ObsMarker> {
        let (prec, date_seconds, date_offset, parents_recorded) = self
            .conn
            .prepare_cached(
                "SELECT prec, date_seconds, date_offset, parents_recorded
                 FROM obs_markers WHERE id = ?1",
            )?
            .query_row([&id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get::<_, bool>(3)?,
                ))
            })?;
        let succs = self.commit_ids(
            "SELECT succ FROM obs_marker_succs WHERE marker_id = ?1 ORDER BY position",
            &id,
        )?;
        let parents = parents_recorded
            .then(|| {
                self.commit_ids(
                    "SELECT parent FROM obs_marker_parents WHERE marker_id = ?1 ORDER BY position",
                    &id,
                )
            })
            .transpose()?;
        let meta = self
            .conn
            .prepare_cached(
                "SELECT key, value FROM obs_marker_meta WHERE marker_id = ?1 ORDER BY key",
            )?
            .query_map([&id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ObsMarker {
            id,
            prec,
            succs,
            meta,
            date_seconds,
            date_offset,
            parents,
        })
    }

    fn commit_ids(&self, query: &str, marker_id: &[u8]) -> rusqlite::Result<Vec<Vec<u8>>> {
        self.conn
            .prepare_cached(query)?
            .query_map([marker_id], |row| row.get(0))?
            .collect()
    }
}

impl ObsMarkersHandleMut<'_> {
    /// Store `markers`, skipping any whose `id` is already present.
    pub fn insert(&self, markers: &[ObsMarker]) -> rusqlite::Result<()> {
        let mut insert_marker = self.conn.prepare_cached(
            "INSERT OR IGNORE INTO obs_markers (id, prec, date_seconds, date_offset, parents_recorded)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut insert_succ = self.conn.prepare_cached(
            "INSERT INTO obs_marker_succs (marker_id, position, succ) VALUES (?1, ?2, ?3)",
        )?;
        let mut insert_parent = self.conn.prepare_cached(
            "INSERT INTO obs_marker_parents (marker_id, position, parent) VALUES (?1, ?2, ?3)",
        )?;
        let mut insert_meta = self.conn.prepare_cached(
            "INSERT INTO obs_marker_meta (marker_id, key, value) VALUES (?1, ?2, ?3)",
        )?;
        for marker in markers {
            let is_new = insert_marker.execute(params![
                marker.id,
                marker.prec,
                marker.date_seconds,
                marker.date_offset,
                marker.parents.is_some(),
            ])? == 1;
            if !is_new {
                continue;
            }
            for (position, succ) in (0_i64..).zip(&marker.succs) {
                insert_succ.execute(params![marker.id, position, succ])?;
            }
            for (position, parent) in (0_i64..).zip(marker.parents.iter().flatten()) {
                insert_parent.execute(params![marker.id, position, parent])?;
            }
            for (key, value) in &marker.meta {
                insert_meta.execute(params![marker.id, key, value])?;
            }
        }
        Ok(())
    }
}
