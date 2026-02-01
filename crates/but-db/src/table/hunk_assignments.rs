use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[
    M::up(
        20250526145725,
        "CREATE TABLE `hunk_assignments`(
	`hunk_header` TEXT,
	`path` TEXT NOT NULL,
	`path_bytes` BINARY NOT NULL,
	`stack_id` TEXT,
	`hunk_locks` TEXT NOT NULL,
	PRIMARY KEY(`path`, `hunk_header`)
);",
    ),
    M::up(
        20250603111503,
        "ALTER TABLE `hunk_assignments` ADD COLUMN `id` TEXT;",
    ),
    M::up(
        20250607113323,
        "ALTER TABLE `hunk_assignments` DROP COLUMN `hunk_locks`;",
    ),
];

/// Tests are in `but-db/tests/db/table/hunk_assignments.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HunkAssignment {
    pub id: Option<String>,
    pub hunk_header: Option<String>,
    pub path: String,
    pub path_bytes: Vec<u8>,
    pub stack_id: Option<String>,
}

impl DbHandle {
    pub fn hunk_assignments(&self) -> HunkAssignmentsHandle<'_> {
        HunkAssignmentsHandle { conn: &self.conn }
    }

    pub fn hunk_assignments_mut(&mut self) -> rusqlite::Result<HunkAssignmentsHandleMut<'_>> {
        Ok(HunkAssignmentsHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl<'conn> Transaction<'conn> {
    pub fn hunk_assignments(&self) -> HunkAssignmentsHandle<'_> {
        HunkAssignmentsHandle { conn: self.inner() }
    }

    pub fn hunk_assignments_mut(&mut self) -> rusqlite::Result<HunkAssignmentsHandleMut<'_>> {
        Ok(HunkAssignmentsHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

pub struct HunkAssignmentsHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct HunkAssignmentsHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl HunkAssignmentsHandle<'_> {
    /// Lists all hunk assignments in the database.
    pub fn list_all(&self) -> rusqlite::Result<Vec<HunkAssignment>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, hunk_header, path, path_bytes, stack_id FROM hunk_assignments")?;

        let results = stmt.query_map([], |row| {
            Ok(HunkAssignment {
                id: row.get(0)?,
                hunk_header: row.get(1)?,
                path: row.get(2)?,
                path_bytes: row.get(3)?,
                stack_id: row.get(4)?,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }
}

impl HunkAssignmentsHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> HunkAssignmentsHandle<'_> {
        HunkAssignmentsHandle { conn: &self.sp }
    }

    /// Sets the hunk assignments table to the provided values.
    /// Any existing entries that are not in the provided values are deleted.
    pub fn set_all(self, assignments: Vec<HunkAssignment>) -> rusqlite::Result<()> {
        self.sp.execute("DELETE FROM hunk_assignments", [])?;

        for assignment in assignments {
            self.sp.execute(
                "INSERT INTO hunk_assignments (id, hunk_header, path, path_bytes, stack_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    assignment.id,
                    assignment.hunk_header,
                    assignment.path,
                    assignment.path_bytes,
                    assignment.stack_id,
                ],
            )?;
        }

        self.sp.commit()?;
        Ok(())
    }
}
