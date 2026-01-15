use diesel::{
    ExpressionMethods, QueryDsl, RunQueryDsl,
    prelude::{Insertable, Queryable, Selectable, *},
};
use serde::{Deserialize, Serialize};

use crate::{
    DbHandle, M,
    schema::{claude_messages::dsl::claude_messages, claude_sessions::dsl::claude_sessions},
};

pub(crate) const M: &[M<'static>] = &[
    M::up(
        20250703203544,
        "CREATE TABLE `claude_code_sessions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`stack_id` TEXT NOT NULL
);",
    ),
    M::up(
        20250811130145,
        "CREATE TABLE `claude_sessions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`current_id` TEXT NOT NULL UNIQUE,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL
);

CREATE INDEX index_claude_sessions_on_current_id ON claude_sessions (current_id);

CREATE TABLE `claude_messages`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`session_id` TEXT NOT NULL REFERENCES claude_sessions(id),
	`created_at` TIMESTAMP NOT NULL,
	`content_type` TEXT NOT NULL,
	`content` TEXT NOT NULL
);

CREATE INDEX index_claude_messages_on_session_id ON claude_messages (session_id);
CREATE INDEX index_claude_messages_on_created_at ON claude_messages (created_at);",
    ),
    M::up(
        20250812093543,
        "DROP TABLE IF EXISTS `claude_code_sessions`;",
    ),
    M::up(
        20250817195624,
        "CREATE TABLE `claude_permission_requests`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL,
	`tool_name` TEXT NOT NULL,
	`input` TEXT NOT NULL,
	`approved` BOOL
);",
    ),
    M::up(
        20250821095340,
        "ALTER TABLE claude_sessions ADD COLUMN in_gui BOOLEAN NOT NULL DEFAULT FALSE;",
    ),
    M::up(
        20250821142109,
        "-- Add session_ids column to claude_sessions table to track all session IDs used
ALTER TABLE claude_sessions ADD COLUMN session_ids TEXT NOT NULL DEFAULT '[]';

-- Initialize existing sessions with their current_id in the session_ids array
UPDATE claude_sessions SET session_ids = json_array(current_id);",
    ),
    M::up(
        20251106102333,
        r#"-- Replace approved (bool) and scope (text) with a single decision (text) column
ALTER TABLE `claude_permission_requests` ADD COLUMN `decision` TEXT;

-- Migrate existing data: approved=true -> allowSession, approved=false -> denySession
UPDATE `claude_permission_requests`
SET `decision` = CASE
    WHEN `approved` = 1 THEN '"allowSession"'
    WHEN `approved` = 0 THEN '"denySession"'
    ELSE NULL
END;

-- Drop the old columns
ALTER TABLE `claude_permission_requests` DROP COLUMN `approved`;"#,
    ),
    M::up(
        20251107134016,
        "-- Add permissions columns to claude_sessions table to track approved and denied permissions
ALTER TABLE claude_sessions ADD COLUMN approved_permissions TEXT NOT NULL DEFAULT '[]';
ALTER TABLE claude_sessions ADD COLUMN denied_permissions TEXT NOT NULL DEFAULT '[]';",
    ),
    M::up(
        20251110103940,
        "-- Add use_wildcard column to claude_permission_requests table
ALTER TABLE `claude_permission_requests` ADD COLUMN `use_wildcard` BOOLEAN NOT NULL DEFAULT 0;",
    ),
];

#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable, Identifiable,
)]
#[diesel(table_name = crate::schema::claude_sessions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ClaudeSession {
    pub id: String,
    pub current_id: String,
    pub session_ids: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub in_gui: bool,
    pub approved_permissions: String,
    pub denied_permissions: String,
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
    pub decision: Option<String>,
    pub use_wildcard: bool,
}

impl DbHandle {
    pub fn claude_sessions(&mut self) -> ClaudeSessionsHandle<'_> {
        ClaudeSessionsHandle { db: self }
    }

    pub fn claude_messages(&mut self) -> ClaudeMessagesHandle<'_> {
        ClaudeMessagesHandle { db: self }
    }

    pub fn claude_permission_requests(&mut self) -> ClaudePermissionRequestsHandle<'_> {
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

    pub fn set_decision(
        &mut self,
        id: &str,
        decision: Option<String>,
    ) -> Result<(), diesel::result::Error> {
        diesel::update(
            crate::schema::claude_permission_requests::table
                .filter(crate::schema::claude_permission_requests::id.eq(id)),
        )
        .set((
            crate::schema::claude_permission_requests::decision.eq(decision),
            crate::schema::claude_permission_requests::updated_at
                .eq(chrono::Local::now().naive_local()),
        ))
        .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn set_decision_and_wildcard(
        &mut self,
        id: &str,
        decision: Option<String>,
        use_wildcard: bool,
    ) -> Result<(), diesel::result::Error> {
        diesel::update(
            crate::schema::claude_permission_requests::table
                .filter(crate::schema::claude_permission_requests::id.eq(id)),
        )
        .set((
            crate::schema::claude_permission_requests::decision.eq(decision),
            crate::schema::claude_permission_requests::use_wildcard.eq(use_wildcard),
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

    pub fn update_session_ids(
        &mut self,
        id: &str,
        session_ids: &str,
    ) -> Result<(), diesel::result::Error> {
        diesel::update(claude_sessions.filter(crate::schema::claude_sessions::id.eq(id)))
            .set((
                crate::schema::claude_sessions::session_ids.eq(session_ids),
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

    pub fn update_permissions(
        &mut self,
        id: &str,
        approved_permissions: &str,
        denied_permissions: &str,
    ) -> Result<(), diesel::result::Error> {
        diesel::update(claude_sessions.filter(crate::schema::claude_sessions::id.eq(id)))
            .set((
                crate::schema::claude_sessions::approved_permissions.eq(approved_permissions),
                crate::schema::claude_sessions::denied_permissions.eq(denied_permissions),
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

    /// Gets the most recent message matching a provided content type
    pub fn get_message_of_type(
        &mut self,
        content_type: String,
        offset: Option<i64>,
    ) -> Result<Option<ClaudeMessage>, diesel::result::Error> {
        let offset = offset.unwrap_or(0);
        let message = claude_messages
            .filter(crate::schema::claude_messages::content_type.eq(content_type))
            .order(crate::schema::claude_messages::created_at.desc())
            .offset(offset)
            .limit(1)
            .first::<ClaudeMessage>(&mut self.db.conn)
            .optional()?;
        Ok(message)
    }
}
