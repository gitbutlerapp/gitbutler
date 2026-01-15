use diesel::prelude::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

use crate::DbHandle;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::ci_checks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CiCheck {
    pub id: i64,
    pub name: String,
    pub output_summary: String,
    pub output_text: String,
    pub output_title: String,
    pub started_at: Option<chrono::NaiveDateTime>,
    pub status_type: String,
    pub status_conclusion: Option<String>,
    pub status_completed_at: Option<chrono::NaiveDateTime>,
    pub head_sha: String,
    pub url: String,
    pub html_url: String,
    pub details_url: String,
    pub pull_requests: String,
    pub reference: String,
    pub last_sync_at: chrono::NaiveDateTime,
    pub struct_version: i32,
}

impl DbHandle {
    pub fn ci_checks(&mut self) -> CiChecksHandle<'_> {
        CiChecksHandle { db: self }
    }
}
pub struct CiChecksHandle<'a> {
    db: &'a mut DbHandle,
}

impl CiChecksHandle<'_> {
    /// Lists CI checks for a specific reference.
    pub fn list_for_reference(&mut self, ref_name: &str) -> anyhow::Result<Vec<CiCheck>> {
        use crate::schema::ci_checks::dsl::{ci_checks as all_checks, reference};
        use diesel::prelude::*;
        let results = all_checks
            .filter(reference.eq(ref_name))
            .load::<CiCheck>(&mut self.db.conn)?;
        Ok(results)
    }

    /// Lists all unique references that have CI checks in the database.
    pub fn list_all_references(&mut self) -> anyhow::Result<Vec<String>> {
        use crate::schema::ci_checks::dsl::{ci_checks as all_checks, reference};
        use diesel::prelude::*;
        let results = all_checks
            .select(reference)
            .distinct()
            .load::<String>(&mut self.db.conn)?;
        Ok(results)
    }

    /// Sets the ci_checks table for a specific reference to the provided values.
    /// Any existing entries for this reference are deleted.
    pub fn set_for_reference(
        &mut self,
        ref_name: &str,
        checks: Vec<CiCheck>,
    ) -> anyhow::Result<()> {
        use crate::schema::ci_checks::dsl::{ci_checks as all_checks, reference};
        use diesel::prelude::*;

        self.db.conn.transaction(|conn| {
            diesel::delete(all_checks.filter(reference.eq(ref_name))).execute(conn)?;
            if !checks.is_empty() {
                diesel::insert_into(all_checks)
                    .values(&checks)
                    .execute(conn)?;
            }
            diesel::result::QueryResult::Ok(())
        })?;
        Ok(())
    }

    /// Deletes all CI check entries for a specific reference.
    pub fn delete_for_reference(&mut self, ref_name: &str) -> anyhow::Result<()> {
        use crate::schema::ci_checks::dsl::{ci_checks as all_checks, reference};
        use diesel::prelude::*;
        diesel::delete(all_checks.filter(reference.eq(ref_name))).execute(&mut self.db.conn)?;
        Ok(())
    }
}
