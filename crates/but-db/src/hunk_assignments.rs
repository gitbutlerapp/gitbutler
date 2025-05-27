use diesel::RunQueryDsl;

use crate::{DbHandle, schema::hunk_assignments::dsl::*};

/// Implements methods for managing hunk assignments in the database.
impl DbHandle {
    /// Lists all hunk assignments in the database.
    pub fn list_all(&mut self) -> anyhow::Result<Vec<crate::models::HunkAssignment>> {
        let results = hunk_assignments.load::<crate::models::HunkAssignment>(&mut self.conn)?;
        Ok(results)
    }

    /// Sets the hunk assignments in the database to the provided values. Any existing entries
    /// that are not in the provided values are deleted.
    pub fn set_all(
        &mut self,
        assignments: Vec<crate::models::HunkAssignment>,
    ) -> anyhow::Result<()> {
        // Set the hunk_assignments table to the values in `assignments`.
        // Any existing entries that are not in `assignments` are deleted.
        use crate::schema::hunk_assignments::dsl::hunk_assignments as all_assignments;
        use diesel::prelude::*;
        self.conn.transaction(|conn| {
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
