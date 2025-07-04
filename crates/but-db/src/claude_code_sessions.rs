use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::DbHandle;
use crate::schema::claude_code_sessions::dsl::claude_code_sessions;

use diesel::prelude::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::claude_code_sessions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ClaudeCodeSession {
    pub id: String,
    pub created_at: chrono::NaiveDateTime,
    pub stack_id: String,
}

impl DbHandle {
    pub fn claude_code_sessions(&mut self) -> ClaudeCodeSessionsHandle {
        ClaudeCodeSessionsHandle { db: self }
    }
}

pub struct ClaudeCodeSessionsHandle<'a> {
    db: &'a mut DbHandle,
}

impl ClaudeCodeSessionsHandle<'_> {
    pub fn insert(&mut self, session: ClaudeCodeSession) -> Result<(), diesel::result::Error> {
        diesel::insert_into(claude_code_sessions)
            .values(session)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn update_stack_id(
        &mut self,
        id: &str,
        stack_id: &str,
    ) -> Result<(), diesel::result::Error> {
        diesel::update(claude_code_sessions.filter(crate::schema::claude_code_sessions::id.eq(id)))
            .set(crate::schema::claude_code_sessions::stack_id.eq(stack_id))
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn list(&mut self) -> Result<Vec<ClaudeCodeSession>, diesel::result::Error> {
        let sessions = claude_code_sessions.load::<ClaudeCodeSession>(&mut self.db.conn)?;
        Ok(sessions)
    }
}
