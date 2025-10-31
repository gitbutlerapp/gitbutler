use diesel::{
    ExpressionMethods, QueryDsl, RunQueryDsl,
    associations::HasTable,
    dsl::count_star,
    prelude::{Insertable, Queryable, Selectable},
};
use serde::{Deserialize, Serialize};

use crate::{
    DbHandle,
    schema::{butler_actions as schema, butler_actions::dsl::butler_actions},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::butler_actions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ButlerAction {
    /// UUID identifier of the action.
    pub id: String,
    /// The time when the action was performed.
    pub created_at: chrono::NaiveDateTime,
    /// The prompt that was used to generate the changes that were made, if applicable
    pub external_prompt: Option<String>,
    /// A description of the change that was made and why it was made - i.e. the information that can be obtained from the caller.
    pub external_summary: String,
    /// The handler / implementation that performed the action.
    pub handler: String,
    /// A GitBulter Oplog snapshot ID (git oid) before the action was performed.
    pub snapshot_before: String,
    /// A GitBulter Oplog snapshot ID (git oid) after the action was performed.
    pub snapshot_after: String,
    /// The outcome of the action, if it was successful.
    pub response: Option<String>,
    /// An error message if the action failed.
    pub error: Option<String>,
    /// The source of the action (e.g. "ButCli", "GitButler", "Mcp", "Unknown")
    pub source: Option<String>,
}

impl DbHandle {
    pub fn butler_actions(&mut self) -> ButlerActionsHandle<'_> {
        ButlerActionsHandle { db: self }
    }
}

pub struct ButlerActionsHandle<'a> {
    db: &'a mut DbHandle,
}

impl ButlerActionsHandle<'_> {
    pub fn insert(&mut self, action: ButlerAction) -> anyhow::Result<()> {
        diesel::insert_into(butler_actions)
            .values(&action)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn list(&mut self, offset: i64, limit: i64) -> anyhow::Result<(i64, Vec<ButlerAction>)> {
        let actions = butler_actions::table()
            .order(schema::created_at.desc())
            .limit(limit)
            .offset(offset)
            .load::<ButlerAction>(&mut self.db.conn)?;
        let total = butler_actions::table()
            .select(count_star())
            .first::<i64>(&mut self.db.conn)?;
        Ok((total, actions))
    }
}
