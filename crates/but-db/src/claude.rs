use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::DbHandle;
use crate::schema::claude_messages::dsl::claude_messages;
use crate::schema::claude_sessions::dsl::claude_sessions;
use diesel::prelude::*;

use diesel::prelude::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable, Identifiable,
)]
#[diesel(table_name = crate::schema::claude_sessions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ClaudeSession {
    pub id: String,
    pub current_id: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub in_gui: bool,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Associations,
)]
#[diesel(belongs_to(ClaudeSession, foreign_key = session_id))]
#[diesel(table_name = crate::schema::claude_messages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ClaudeMessage {
    pub id: String,
    pub session_id: String,
    pub created_at: chrono::NaiveDateTime,
    pub content_type: String,
    pub content: String,
}

#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable, Identifiable,
)]
#[diesel(table_name = crate::schema::claude_permission_requests)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ClaudePermissionRequest {
    pub id: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub tool_name: String,
    pub input: String,
    pub approved: Option<bool>,
}

impl DbHandle {
    pub fn claude_sessions(&mut self) -> ClaudeSessionsHandle {
        ClaudeSessionsHandle { db: self }
    }

    pub fn claude_messages(&mut self) -> ClaudeMessagesHandle {
        ClaudeMessagesHandle { db: self }
    }

    pub fn claude_permission_requests(&mut self) -> ClaudePermissionRequestsHandle {
        ClaudePermissionRequestsHandle { db: self }
    }

    pub fn delete_session_and_messages(
        &mut self,
        session_id: &str,
    ) -> Result<(), diesel::result::Error> {
        self.conn
            .transaction::<(), diesel::result::Error, _>(|conn| {
                diesel::delete(
                    claude_messages
                        .filter(crate::schema::claude_messages::session_id.eq(session_id)),
                )
                .execute(conn)?;
                diesel::delete(
                    claude_sessions.filter(crate::schema::claude_sessions::id.eq(session_id)),
                )
                .execute(conn)?;
                Ok(())
            })
    }
}

pub struct ClaudeSessionsHandle<'a> {
    db: &'a mut DbHandle,
}

pub struct ClaudeMessagesHandle<'a> {
    db: &'a mut DbHandle,
}

pub struct ClaudePermissionRequestsHandle<'a> {
    db: &'a mut DbHandle,
}

impl ClaudePermissionRequestsHandle<'_> {
    pub fn insert(
        &mut self,
        request: ClaudePermissionRequest,
    ) -> Result<(), diesel::result::Error> {
        diesel::insert_into(crate::schema::claude_permission_requests::table)
            .values(request)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn set_approval(&mut self, id: &str, approved: bool) -> Result<(), diesel::result::Error> {
        diesel::update(
            crate::schema::claude_permission_requests::table
                .filter(crate::schema::claude_permission_requests::id.eq(id)),
        )
        .set((
            crate::schema::claude_permission_requests::approved.eq(approved),
            crate::schema::claude_permission_requests::updated_at
                .eq(chrono::Local::now().naive_local()),
        ))
        .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn get(
        &mut self,
        id: &str,
    ) -> Result<Option<ClaudePermissionRequest>, diesel::result::Error> {
        let request = crate::schema::claude_permission_requests::table
            .filter(crate::schema::claude_permission_requests::id.eq(id))
            .first::<ClaudePermissionRequest>(&mut self.db.conn)
            .optional()?;
        Ok(request)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), diesel::result::Error> {
        diesel::delete(
            crate::schema::claude_permission_requests::table
                .filter(crate::schema::claude_permission_requests::id.eq(id)),
        )
        .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn list(&mut self) -> Result<Vec<ClaudePermissionRequest>, diesel::result::Error> {
        let requests = crate::schema::claude_permission_requests::table
            .load::<ClaudePermissionRequest>(&mut self.db.conn)?;
        Ok(requests)
    }
}

impl ClaudeSessionsHandle<'_> {
    pub fn insert(&mut self, session: ClaudeSession) -> Result<(), diesel::result::Error> {
        diesel::insert_into(claude_sessions)
            .values(session)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn update_current_id(
        &mut self,
        id: &str,
        current_id: &str,
    ) -> Result<(), diesel::result::Error> {
        diesel::update(claude_sessions.filter(crate::schema::claude_sessions::id.eq(id)))
            .set((
                crate::schema::claude_sessions::current_id.eq(current_id),
                crate::schema::claude_sessions::updated_at.eq(chrono::Local::now().naive_local()),
            ))
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn update_in_gui(&mut self, id: &str, in_gui: bool) -> Result<(), diesel::result::Error> {
        diesel::update(claude_sessions.filter(crate::schema::claude_sessions::id.eq(id)))
            .set((
                crate::schema::claude_sessions::in_gui.eq(in_gui),
                crate::schema::claude_sessions::updated_at.eq(chrono::Local::now().naive_local()),
            ))
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    /// If you intend delete the messages AND the session, you should use `delete_session_and_messages` instead, which does it all in a single transaction.
    pub fn delete(&mut self, id: &str) -> Result<(), diesel::result::Error> {
        diesel::delete(claude_sessions.filter(crate::schema::claude_sessions::id.eq(id)))
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn get(&mut self, id: &str) -> Result<Option<ClaudeSession>, diesel::result::Error> {
        let session = claude_sessions
            .filter(crate::schema::claude_sessions::id.eq(id))
            .first::<ClaudeSession>(&mut self.db.conn)
            .optional()?;
        Ok(session)
    }

    pub fn get_by_current_id(
        &mut self,
        current_id: &str,
    ) -> Result<Option<ClaudeSession>, diesel::result::Error> {
        let session = claude_sessions
            .filter(crate::schema::claude_sessions::current_id.eq(current_id))
            .first::<ClaudeSession>(&mut self.db.conn)
            .optional()?;
        Ok(session)
    }

    pub fn list(&mut self) -> Result<Vec<ClaudeSession>, diesel::result::Error> {
        let sessions = claude_sessions.load::<ClaudeSession>(&mut self.db.conn)?;
        Ok(sessions)
    }
}

impl ClaudeMessagesHandle<'_> {
    pub fn insert(&mut self, message: ClaudeMessage) -> Result<(), diesel::result::Error> {
        diesel::insert_into(claude_messages)
            .values(message)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn list_by_session(
        &mut self,
        session_id: &str,
    ) -> Result<Vec<ClaudeMessage>, diesel::result::Error> {
        let messages = claude_messages
            .filter(crate::schema::claude_messages::session_id.eq(session_id))
            .order(crate::schema::claude_messages::created_at.asc())
            .load::<ClaudeMessage>(&mut self.db.conn)?;
        Ok(messages)
    }

    /// If you intend delete the messages AND the session, you should use `delete_session_and_messages` instead, which does it all in a single transaction.
    pub fn delete_by_session(&mut self, session_id: &str) -> Result<(), diesel::result::Error> {
        diesel::delete(
            claude_messages.filter(crate::schema::claude_messages::session_id.eq(session_id)),
        )
        .execute(&mut self.db.conn)?;
        Ok(())
    }
}
