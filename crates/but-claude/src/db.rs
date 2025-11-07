use anyhow::Result;
use gitbutler_command_context::CommandContext;
use uuid::Uuid;

use crate::{ClaudePermissionRequest, ClaudeSession};

/// Creates a new ClaudeSession with the session_id provided and saves it to the database.
pub fn save_new_session(ctx: &mut CommandContext, id: Uuid) -> anyhow::Result<ClaudeSession> {
    save_new_session_with_gui_flag(ctx, id, false)
}

/// Creates a new ClaudeSession with the session_id provided and saves it to the database.
pub fn save_new_session_with_gui_flag(
    ctx: &mut CommandContext,
    id: Uuid,
    in_gui: bool,
) -> anyhow::Result<ClaudeSession> {
    let now = chrono::Utc::now().naive_utc();
    let session = ClaudeSession {
        id,
        current_id: id,
        session_ids: vec![id],
        created_at: now,
        updated_at: now,
        in_gui,
    };
    ctx.db()?
        .claude_sessions()
        .insert(session.clone().try_into()?)?;
    Ok(session)
}

/// Adds a session ID to the list of session IDs for a given session.
pub fn add_session_id(
    ctx: &mut CommandContext,
    session_id: Uuid,
    new_session_id: Uuid,
) -> anyhow::Result<()> {
    if let Some(mut session) = get_session_by_id(ctx, session_id)?
        && !session.session_ids.contains(&new_session_id)
    {
        session.session_ids.push(new_session_id);
        session.current_id = new_session_id;

        let json = serde_json::to_string(&session.session_ids)?;

        ctx.db()?
            .claude_sessions()
            .update_session_ids(&session_id.to_string(), &json)?;
        ctx.db()?
            .claude_sessions()
            .update_current_id(&session_id.to_string(), &new_session_id.to_string())?;
    }
    Ok(())
}

/// Updates the current session ID for a given session in the database.
pub fn set_session_in_gui(
    ctx: &mut CommandContext,
    session_id: Uuid,
    in_gui: bool,
) -> anyhow::Result<()> {
    ctx.db()?
        .claude_sessions()
        .update_in_gui(&session_id.to_string(), in_gui)?;
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

/// Gets the most recent user input message
/// Optionally an offset may be provided. The offset must be a positive integer
pub fn get_user_message(
    ctx: &mut CommandContext,
    offset: Option<i64>,
) -> anyhow::Result<Option<crate::ClaudeMessage>> {
    let message = ctx
        .db()?
        .claude_messages()
        .get_message_of_type(ClaudeMessageDbContentType::UserInput.to_string(), offset)?;

    match message {
        Some(m) => Ok(Some(m.try_into()?)),
        None => Ok(None),
    }
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

/// Update permission request decision
pub fn update_permission_request(
    ctx: &mut CommandContext,
    id: &str,
    decision: crate::PermissionDecision,
) -> anyhow::Result<()> {
    let decision_str = serde_json::to_string(&decision)?;
    ctx.db()?
        .claude_permission_requests()
        .set_decision(id, Some(decision_str))?;
    Ok(())
}

impl TryFrom<but_db::ClaudeSession> for crate::ClaudeSession {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ClaudeSession) -> Result<Self, Self::Error> {
        let session_ids: Vec<Uuid> = serde_json::from_str(&value.session_ids)?;
        Ok(crate::ClaudeSession {
            id: Uuid::parse_str(&value.id)?,
            current_id: Uuid::parse_str(&value.current_id)?,
            session_ids,
            created_at: value.created_at,
            updated_at: value.updated_at,
            in_gui: value.in_gui,
        })
    }
}

impl TryFrom<crate::ClaudeSession> for but_db::ClaudeSession {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudeSession) -> Result<Self, Self::Error> {
        let session_ids = serde_json::to_string(&value.session_ids)?;
        Ok(but_db::ClaudeSession {
            id: value.id.to_string(),
            current_id: value.current_id.to_string(),
            session_ids,
            created_at: value.created_at,
            updated_at: value.updated_at,
            in_gui: value.in_gui,
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
        let decision = value
            .decision
            .as_ref()
            .map(|s| serde_json::from_str(s))
            .transpose()?;
        Ok(crate::ClaudePermissionRequest {
            id: value.id.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            tool_name: value.tool_name,
            input: serde_json::from_str(&value.input)?,
            decision,
        })
    }
}

impl TryFrom<crate::ClaudePermissionRequest> for but_db::ClaudePermissionRequest {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudePermissionRequest) -> Result<Self, Self::Error> {
        let decision = value
            .decision
            .map(|s| serde_json::to_string(&s))
            .transpose()?;
        Ok(but_db::ClaudePermissionRequest {
            id: value.id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            tool_name: value.tool_name,
            input: serde_json::to_string(&value.input)?,
            decision,
        })
    }
}
