use diesel::RunQueryDsl;

use crate::DbHandle;

use diesel::prelude::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::butler_actions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ButlerAction {
    /// UUID identifier of the action.
    pub id: String,
    /// The time when the action was performed.
    pub created_at: chrono::NaiveDateTime,
    /// A description of the change that was made and why it was made - i.e. the information that can be obtained from the caller.
    pub external_prompt: String,
    /// The handler / implementation that performed the action.
    pub handler: String,
    /// An optional prompt that was used by the handler to perform the action, if applicable.
    pub handler_prompt: Option<String>,
    /// A GitBulter Oplog snapshot ID (git oid) before the action was performed.
    pub snapshot_before: String,
    /// A GitBulter Oplog snapshot ID (git oid) after the action was performed.
    pub snapshot_after: String,
    /// The outcome of the action, if it was successful.
    pub response: Option<String>,
    /// An error message if the action failed.
    pub error: Option<String>,
}

impl DbHandle {
    pub fn butler_actions(&mut self) -> ButlerActionsHandle {
        ButlerActionsHandle { db: self }
    }
}

pub struct ButlerActionsHandle<'a> {
    db: &'a mut DbHandle,
}

impl ButlerActionsHandle<'_> {
    pub fn insert(&mut self, action: ButlerAction) -> anyhow::Result<()> {
        use crate::schema::butler_actions::dsl::butler_actions;
        diesel::insert_into(butler_actions)
            .values(&action)
            .execute(&mut self.db.conn)?;
        Ok(())
    }
}
