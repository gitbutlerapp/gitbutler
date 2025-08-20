use anyhow::Result;
use gitbutler_command_context::CommandContext;
use uuid::Uuid;

use crate::{ClaudePermissionRequest, ClaudeSession};

/// Creates a new ClaudeSession with the session_id provided and saves it to the database.
pub fn save_new_session(ctx: &mut CommandContext, id: Uuid) -> anyhow::Result<ClaudeSession> {
    let now = chrono::Utc::now().naive_utc();
    let session = ClaudeSession {
        id,
        current_id: id,
        created_at: now,
        updated_at: now,
    };
    ctx.db()?
        .claude_sessions()
        .insert(session.clone().try_into()?)?;
    Ok(session)
}

/// Updates the current session ID for a given session in the database.
pub fn set_session_current_id(
    ctx: &mut CommandContext,
    session_id: Uuid,
    current_id: Uuid,
) -> anyhow::Result<()> {
    ctx.db()?
        .claude_sessions()
        .update(&session_id.to_string(), &current_id.to_string())?;
    Ok(())
}

/// Lists all known Claude sessions
pub fn list_all_sessions(ctx: &mut CommandContext) -> anyhow::Result<Vec<ClaudeSession>> {
    let sessions = ctx.db()?.claude_sessions().list()?;
    sessions
        .into_iter()
        .map(|s| s.try_into())
        .collect::<Result<_, _>>()
}

/// Retrieves a Claude session by its ID from the database.
pub fn get_session_by_id(
    ctx: &mut CommandContext,
    session_id: Uuid,
) -> anyhow::Result<Option<ClaudeSession>> {
    let session = ctx.db()?.claude_sessions().get(&session_id.to_string())?;
    match session {
        Some(s) => Ok(Some(s.try_into()?)),
        None => Ok(None),
    }
}

pub fn get_session_by_current_id(
    ctx: &mut CommandContext,
    current_id: Uuid,
) -> anyhow::Result<Option<ClaudeSession>> {
    let session = ctx
        .db()?
        .claude_sessions()
        .get_by_current_id(&current_id.to_string())?;
    match session {
        Some(s) => Ok(Some(s.try_into()?)),
        None => Ok(None),
    }
}

/// Deletes a Claude session and all associated messages from the database. This is what we want to use when we want to delete a session completely.
pub fn delete_session_and_messages_by_id(
    ctx: &mut CommandContext,
    session_id: Uuid,
) -> anyhow::Result<()> {
    ctx.db()?
        .delete_session_and_messages(&session_id.to_string())?;
    Ok(())
}

/// Creates a new ClaudeMessage with the provided session_id and content, and saves it to the database.
pub fn save_new_message(
    ctx: &mut CommandContext,
    session_id: Uuid,
    content: crate::ClaudeMessageContent,
) -> anyhow::Result<crate::ClaudeMessage> {
    let message = crate::ClaudeMessage {
        id: Uuid::new_v4(),
        session_id,
        created_at: chrono::Utc::now().naive_utc(),
        content,
    };
    ctx.db()?
        .claude_messages()
        .insert(message.clone().try_into()?)?;
    Ok(message)
}

/// Lists all messages associated with a given session ID from the database.
pub fn list_messages_by_session(
    ctx: &mut CommandContext,
    session_id: Uuid,
) -> anyhow::Result<Vec<crate::ClaudeMessage>> {
    let messages = ctx
        .db()?
        .claude_messages()
        .list_by_session(&session_id.to_string())?;
    messages
        .into_iter()
        .map(|m| m.try_into())
        .collect::<Result<_, _>>()
}

/// Lists all Permission Requests
pub fn list_all_permission_requests(
    ctx: &mut CommandContext,
) -> anyhow::Result<Vec<ClaudePermissionRequest>> {
    let requests = ctx.db()?.claude_permission_requests().list()?;
    requests
        .into_iter()
        .map(|s| s.try_into())
        .collect::<Result<_, _>>()
}

/// Update permission request approval state to either true or false
pub fn update_permission_request(
    ctx: &mut CommandContext,
    id: &str,
    approval: bool,
) -> anyhow::Result<()> {
    ctx.db()?
        .claude_permission_requests()
        .set_approval(id, approval)?;
    Ok(())
}

impl TryFrom<but_db::ClaudeSession> for crate::ClaudeSession {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ClaudeSession) -> Result<Self, Self::Error> {
        Ok(crate::ClaudeSession {
            id: Uuid::parse_str(&value.id)?,
            current_id: Uuid::parse_str(&value.current_id)?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl TryFrom<crate::ClaudeSession> for but_db::ClaudeSession {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudeSession) -> Result<Self, Self::Error> {
        Ok(but_db::ClaudeSession {
            id: value.id.to_string(),
            current_id: value.current_id.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Debug, Clone, Copy, strum::EnumString, strum::Display)]
enum ClaudeMessageDbContentType {
    ClaudeOutput,
    UserInput,
    GitButlerMessage,
}

impl TryFrom<but_db::ClaudeMessage> for crate::ClaudeMessage {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ClaudeMessage) -> Result<Self, Self::Error> {
        let content_type: ClaudeMessageDbContentType = value.content_type.parse()?;
        let content = match content_type {
            ClaudeMessageDbContentType::ClaudeOutput => {
                crate::ClaudeMessageContent::ClaudeOutput(serde_json::from_str(&value.content)?)
            }
            ClaudeMessageDbContentType::UserInput => {
                crate::ClaudeMessageContent::UserInput(serde_json::from_str(&value.content)?)
            }
            ClaudeMessageDbContentType::GitButlerMessage => {
                crate::ClaudeMessageContent::GitButlerMessage(serde_json::from_str(&value.content)?)
            }
        };
        Ok(crate::ClaudeMessage {
            id: Uuid::parse_str(&value.id)?,
            session_id: Uuid::parse_str(&value.session_id)?,
            created_at: value.created_at,
            content,
        })
    }
}

impl TryFrom<crate::ClaudeMessage> for but_db::ClaudeMessage {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudeMessage) -> Result<Self, Self::Error> {
        let (content_type, content) = match value.content {
            crate::ClaudeMessageContent::ClaudeOutput(value) => {
                let value = serde_json::to_string(&value)?;
                (ClaudeMessageDbContentType::ClaudeOutput, value)
            }
            crate::ClaudeMessageContent::UserInput(value) => {
                let value = serde_json::to_string(&value)?;
                (ClaudeMessageDbContentType::UserInput, value)
            }
            crate::ClaudeMessageContent::GitButlerMessage(value) => {
                let value = serde_json::to_string(&value)?;
                (ClaudeMessageDbContentType::GitButlerMessage, value)
            }
        };

        Ok(but_db::ClaudeMessage {
            id: value.id.to_string(),
            session_id: value.session_id.to_string(),
            created_at: value.created_at,
            content_type: content_type.to_string(),
            content,
        })
    }
}

impl TryFrom<but_db::ClaudePermissionRequest> for crate::ClaudePermissionRequest {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ClaudePermissionRequest) -> Result<Self, Self::Error> {
        Ok(crate::ClaudePermissionRequest {
            id: value.id.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            tool_name: value.tool_name,
            input: serde_json::from_str(&value.input)?,
            approved: value.approved,
        })
    }
}

impl TryFrom<crate::ClaudePermissionRequest> for but_db::ClaudePermissionRequest {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudePermissionRequest) -> Result<Self, Self::Error> {
        Ok(but_db::ClaudePermissionRequest {
            id: value.id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            tool_name: value.tool_name,
            input: serde_json::to_string(&value.input)?,
            approved: value.approved,
        })
    }
}
