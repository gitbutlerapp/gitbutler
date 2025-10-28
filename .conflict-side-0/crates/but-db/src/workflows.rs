use diesel::{
    ExpressionMethods, QueryDsl, RunQueryDsl,
    associations::HasTable,
    dsl::count_star,
    prelude::{Insertable, Queryable, Selectable},
};
use serde::{Deserialize, Serialize};

use crate::{
    DbHandle,
    schema::{workflows as schema, workflows::dsl::workflows},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::workflows)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Workflow {
    /// Unique identifier for the workflow.
    pub id: String,
    /// The time when the workflow was captured.
    pub created_at: chrono::NaiveDateTime,
    /// The type of the workflow performed.
    pub kind: String,
    /// The trigger that initiated the workflow.
    pub triggered_by: String,
    /// The status of the workflow.
    pub status: String,
    /// Input commits
    pub input_commits: String,
    /// Output commits
    pub output_commits: String,
    /// Optional summary of the workflow
    pub summary: Option<String>,
}

impl DbHandle {
    pub fn workflows(&mut self) -> WorkflowsHandle<'_> {
        WorkflowsHandle { db: self }
    }
}

pub struct WorkflowsHandle<'a> {
    db: &'a mut DbHandle,
}

impl WorkflowsHandle<'_> {
    pub fn insert(&mut self, workflow: Workflow) -> anyhow::Result<()> {
        diesel::insert_into(workflows)
            .values(&workflow)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn list(&mut self, offset: i64, limit: i64) -> anyhow::Result<(i64, Vec<Workflow>)> {
        let out = workflows::table()
            .order(schema::created_at.desc())
            .limit(limit)
            .offset(offset)
            .load::<Workflow>(&mut self.db.conn)?;
        let total = workflows::table()
            .select(count_star())
            .first::<i64>(&mut self.db.conn)?;
        Ok((total, out))
    }
}
