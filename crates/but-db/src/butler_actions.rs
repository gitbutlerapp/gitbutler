use diesel::SelectableHelper;
use diesel::dsl::count_star;
use diesel::{Connection, ExpressionMethods, QueryDsl, RunQueryDsl, associations::HasTable};

use crate::schema::butler_actions::dsl::butler_actions;
use crate::schema::{butler_mcp_actions, butler_revert_actions};
use crate::{DbHandle, schema::butler_actions as schema};

use diesel::prelude::{Associations, Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::butler_mcp_actions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ButlerMcpAction {
    /// UUID identifier of the action.
    pub id: String,
    /// The prompt that was used to generate the changes that were made, if applicable
    pub external_prompt: Option<String>,
    /// A description of the change that was made and why it was made - i.e. the information that can be obtained from the caller.
    pub external_summary: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::butler_revert_actions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ButlerRevertAction {
    /// UUID identifier of the action.
    pub id: String,
    /// The snapshot representing the revert
    pub snapshot: String,
    /// A description of what was undone
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable, Associations)]
#[diesel(table_name = crate::schema::butler_actions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(belongs_to(ButlerRevertAction, foreign_key = revert_action_id))]
#[diesel(belongs_to(ButlerMcpAction, foreign_key = mcp_action_id))]
pub struct ButlerAction {
    /// UUID identifier of the action.
    pub id: String,
    /// When it was created
    pub created_at: chrono::NaiveDateTime,
    /// Optional butler mcp action
    pub mcp_action_id: Option<String>,
    /// Optional butler revert action
    pub revert_action_id: Option<String>,
}

pub struct FilledButlerAction {
    pub id: String,
    pub created_at: chrono::NaiveDateTime,
    pub mcp_action: Option<ButlerMcpAction>,
    pub revert_action: Option<ButlerRevertAction>,
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
    pub fn insert(&mut self, filled_action: FilledButlerAction) -> anyhow::Result<()> {
        self.db.conn.transaction(|conn| {
            // Insert the MCP action if it exists
            let mcp_action_id = if let Some(mcp_action) = filled_action.mcp_action {
                diesel::insert_into(butler_mcp_actions::table)
                    .values(&mcp_action)
                    .execute(conn)?;
                Some(mcp_action.id)
            } else {
                None
            };

            // Insert the revert action if it exists
            let revert_action_id = if let Some(revert_action) = filled_action.revert_action {
                diesel::insert_into(butler_revert_actions::table)
                    .values(&revert_action)
                    .execute(conn)?;
                Some(revert_action.id)
            } else {
                None
            };

            // Create and insert the main butler_action with the appropriate foreign keys
            let butler_action = ButlerAction {
                id: filled_action.id,
                created_at: filled_action.created_at,
                mcp_action_id,
                revert_action_id,
            };

            diesel::insert_into(butler_actions)
                .values(&butler_action)
                .execute(conn)?;

            Ok(())
        })
    }

    pub fn list(
        &mut self,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(i64, Vec<FilledButlerAction>)> {
        let offset = (page - 1) * page_size;
        let actions = butler_actions::table()
            .left_join(butler_mcp_actions::table)
            .left_join(butler_revert_actions::table)
            .order(schema::created_at.desc())
            .limit(page_size)
            .offset(offset)
            .select((
                ButlerAction::as_select(),
                Option::<ButlerMcpAction>::as_select(),
                Option::<ButlerRevertAction>::as_select(),
            ))
            .load::<(
                ButlerAction,
                Option<ButlerMcpAction>,
                Option<ButlerRevertAction>,
            )>(&mut self.db.conn)?;

        let actions = actions
            .into_iter()
            .map(|(action, mcp, revert)| FilledButlerAction {
                id: action.id,
                created_at: action.created_at,
                mcp_action: mcp,
                revert_action: revert,
            })
            .collect();

        let total = butler_actions::table()
            .select(count_star())
            .first::<i64>(&mut self.db.conn)?;
        Ok((total, actions))
    }
}
