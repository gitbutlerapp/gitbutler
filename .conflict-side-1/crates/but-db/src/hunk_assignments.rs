use diesel::RunQueryDsl;

use crate::{DbHandle, schema::hunk_assignments::dsl::*};

use diesel::prelude::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

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
        use crate::schema::hunk_assignments::dsl::hunk_assignments as all_assignments;
        use diesel::prelude::*;
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
