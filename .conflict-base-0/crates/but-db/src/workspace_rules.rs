use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};

use crate::DbHandle;
use crate::schema::workspace_rules::dsl::workspace_rules;

use diesel::prelude::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::workspace_rules)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct WorkspaceRule {
    pub id: String,
    pub created_at: chrono::NaiveDateTime,
    pub enabled: bool,
    pub trigger: String,
    pub filters: String,
    pub action: String,
}

impl DbHandle {
    pub fn workspace_rules(&mut self) -> WorkspaceRulesHandle<'_> {
        WorkspaceRulesHandle { db: self }
    }
}

pub struct WorkspaceRulesHandle<'a> {
    db: &'a mut DbHandle,
}

impl WorkspaceRulesHandle<'_> {
    pub fn insert(&mut self, rule: WorkspaceRule) -> Result<(), diesel::result::Error> {
        diesel::insert_into(workspace_rules)
            .values(rule)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn update(&mut self, id: &str, rule: WorkspaceRule) -> Result<(), diesel::result::Error> {
        diesel::update(workspace_rules.filter(crate::schema::workspace_rules::id.eq(id)))
            .set((
                crate::schema::workspace_rules::enabled.eq(rule.enabled),
                crate::schema::workspace_rules::trigger.eq(rule.trigger),
                crate::schema::workspace_rules::filters.eq(rule.filters),
                crate::schema::workspace_rules::action.eq(rule.action),
            ))
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> Result<(), diesel::result::Error> {
        diesel::delete(workspace_rules.filter(crate::schema::workspace_rules::id.eq(id)))
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn list(&mut self) -> Result<Vec<WorkspaceRule>, diesel::result::Error> {
        let rules = workspace_rules.load::<WorkspaceRule>(&mut self.db.conn)?;
        Ok(rules)
    }

    pub fn get(&mut self, id: &str) -> Result<Option<WorkspaceRule>, diesel::result::Error> {
        let rule = workspace_rules
            .filter(crate::schema::workspace_rules::id.eq(id))
            .first::<WorkspaceRule>(&mut self.db.conn)
            .optional()?;
        Ok(rule)
    }
}
