use diesel::{
    RunQueryDsl,
    prelude::{Insertable, Queryable, Selectable},
};
use serde::{Deserialize, Serialize};

use crate::M;
use crate::{DbHandle, schema::hunk_assignments::dsl::*};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::hunk_assignments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct HunkAssignment {
    pub id: Option<String>,
    pub hunk_header: Option<String>,
    pub path: String,
    pub path_bytes: Vec<u8>,
    pub stack_id: Option<String>,
}

impl DbHandle {
    pub fn hunk_assignments(&mut self) -> HunkAssignmentsHandle<'_> {
        HunkAssignmentsHandle { db: self }
    }
}

pub struct HunkAssignmentsHandle<'a> {
    db: &'a mut DbHandle,
}

impl HunkAssignmentsHandle<'_> {
    /// Lists all hunk assignments in the database.
    pub fn list_all(&mut self) -> anyhow::Result<Vec<HunkAssignment>> {
        let results = hunk_assignments.load::<HunkAssignment>(&mut self.db.conn)?;
        Ok(results)
    }

    /// Sets the hunk assignments in the database to the provided values. Any existing entries
    /// that are not in the provided values are deleted.
    pub fn set_all(&mut self, assignments: Vec<HunkAssignment>) -> anyhow::Result<()> {
        // Set the hunk_assignments table to the values in `assignments`.
        // Any existing entries that are not in `assignments` are deleted.
        use diesel::prelude::*;

        use crate::schema::hunk_assignments::dsl::hunk_assignments as all_assignments;
        self.db.conn.transaction(|conn| {
            // Delete all existing assignments
            diesel::delete(all_assignments).execute(conn)?;
            // Insert the new assignments
            for assignment in assignments {
                diesel::insert_into(hunk_assignments)
                    .values(&assignment)
                    .execute(conn)?;
            }
            diesel::result::QueryResult::Ok(())
        })?;
        Ok(())
    }
}
