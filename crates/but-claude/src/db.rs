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
        approved_permissions: vec![],
        denied_permissions: vec![],
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

/// Updates the permissions for a given session in the database.
pub fn update_session_permissions(
    ctx: &mut CommandContext,
    session_id: Uuid,
    approved_permissions: &[crate::Permission],
    denied_permissions: &[crate::Permission],
) -> anyhow::Result<()> {
    let approved_json = serde_json::to_string(approved_permissions)?;
    let denied_json = serde_json::to_string(denied_permissions)?;
    ctx.db()?.claude_sessions().update_permissions(
        &session_id.to_string(),
        &approved_json,
        &denied_json,
    )?;
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

/// Creates a new ClaudeMessage with the provided session_id and payload, and saves it to the database.
pub fn save_new_message(
    ctx: &mut CommandContext,
    session_id: Uuid,
    payload: crate::MessagePayload,
) -> anyhow::Result<crate::ClaudeMessage> {
    let message = crate::ClaudeMessage {
        id: Uuid::new_v4(),
        session_id,
        created_at: chrono::Utc::now().naive_utc(),
        payload,
    };
    ctx.db()?
        .claude_messages()
        .insert(message.clone().try_into()?)?;
    Ok(message)
}

/// Lists all messages associated with a given session ID from the database.
/// Messages that fail to deserialize are skipped and logged as warnings.
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
        .get_message_of_type(MessagePayloadDbType::UserInput.to_string(), offset)?;

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
        let approved_permissions: Vec<crate::Permission> =
            serde_json::from_str(&value.approved_permissions)?;
        let denied_permissions: Vec<crate::Permission> =
            serde_json::from_str(&value.denied_permissions)?;
        Ok(crate::ClaudeSession {
            id: Uuid::parse_str(&value.id)?,
            current_id: Uuid::parse_str(&value.current_id)?,
            session_ids,
            created_at: value.created_at,
            updated_at: value.updated_at,
            in_gui: value.in_gui,
            approved_permissions,
            denied_permissions,
        })
    }
}

impl TryFrom<crate::ClaudeSession> for but_db::ClaudeSession {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudeSession) -> Result<Self, Self::Error> {
        let session_ids = serde_json::to_string(&value.session_ids)?;
        let approved_permissions = serde_json::to_string(&value.approved_permissions)?;
        let denied_permissions = serde_json::to_string(&value.denied_permissions)?;
        Ok(but_db::ClaudeSession {
            id: value.id.to_string(),
            current_id: value.current_id.to_string(),
            session_ids,
            created_at: value.created_at,
            updated_at: value.updated_at,
            in_gui: value.in_gui,
            approved_permissions,
            denied_permissions,
        })
    }
}

#[derive(Debug, Clone, Copy, strum::EnumString, strum::Display)]
enum MessagePayloadDbType {
    Claude,
    User,
    System,
    GitButlerUpdate,
    // Legacy names for backward compatibility
    ClaudeOutput,
    UserInput,
    GitButlerMessage,
}

impl TryFrom<but_db::ClaudeMessage> for crate::ClaudeMessage {
    type Error = anyhow::Error;
    fn try_from(value: but_db::ClaudeMessage) -> Result<Self, Self::Error> {
        let payload_type: MessagePayloadDbType = value.content_type.parse()?;
        let payload = match payload_type {
            MessagePayloadDbType::Claude => {
                let data: serde_json::Value = serde_json::from_str(&value.content)?;
                crate::MessagePayload::Claude(crate::ClaudeOutput { data })
            }
            MessagePayloadDbType::ClaudeOutput => {
                crate::legacy::ClaudeMessageContent::ClaudeOutput(serde_json::from_str(
                    &value.content,
                )?)
                .into()
            }
            MessagePayloadDbType::User => {
                crate::MessagePayload::User(serde_json::from_str(&value.content)?)
            }
            MessagePayloadDbType::UserInput => crate::legacy::ClaudeMessageContent::UserInput(
                serde_json::from_str(&value.content)?,
            )
            .into(),
            MessagePayloadDbType::System => {
                crate::MessagePayload::System(serde_json::from_str(&value.content)?)
            }
            MessagePayloadDbType::GitButlerUpdate => {
                crate::MessagePayload::GitButler(serde_json::from_str(&value.content)?)
            }
            MessagePayloadDbType::GitButlerMessage => {
                crate::legacy::ClaudeMessageContent::GitButlerMessage(serde_json::from_str(
                    &value.content,
                )?)
                .into()
            }
        };
        Ok(crate::ClaudeMessage {
            id: Uuid::parse_str(&value.id)?,
            session_id: Uuid::parse_str(&value.session_id)?,
            created_at: value.created_at,
            payload,
        })
    }
}

impl TryFrom<crate::ClaudeMessage> for but_db::ClaudeMessage {
    type Error = anyhow::Error;
    fn try_from(value: crate::ClaudeMessage) -> Result<Self, Self::Error> {
        let (payload_type, content) = match value.payload {
            crate::MessagePayload::Claude(output) => {
                let content = serde_json::to_string(&output.data)?;
                (MessagePayloadDbType::Claude, content)
            }
            crate::MessagePayload::User(input) => {
                let content = serde_json::to_string(&input)?;
                (MessagePayloadDbType::User, content)
            }
            crate::MessagePayload::System(msg) => {
                let content = serde_json::to_string(&msg)?;
                (MessagePayloadDbType::System, content)
            }
            crate::MessagePayload::GitButler(msg) => {
                let content = serde_json::to_string(&msg)?;
                (MessagePayloadDbType::GitButlerUpdate, content)
            }
        };

        Ok(but_db::ClaudeMessage {
            id: value.id.to_string(),
            session_id: value.session_id.to_string(),
            created_at: value.created_at,
            content_type: payload_type.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gitbutler_message_claude_exit() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440000".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "GitButlerMessage".to_string(),
            content: r#"{
                "subject": {
                    "code": 0,
                    "message": ""
                },
                "type": "claudeExit"
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        assert_eq!(
            message.id,
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
        assert_eq!(
            message.session_id,
            Uuid::parse_str("650e8400-e29b-41d4-a716-446655440000").unwrap()
        );

        match message.payload {
            crate::MessagePayload::System(crate::SystemMessage::ClaudeExit { code, message }) => {
                assert_eq!(code, 0);
                assert_eq!(message, "");
            }
            _ => panic!("Expected SystemMessage::ClaudeExit"),
        }
    }

    #[test]
    fn test_gitbutler_message_user_abort() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440001".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440001".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "GitButlerMessage".to_string(),
            content: r#"{
                "type": "userAbort"
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        match message.payload {
            crate::MessagePayload::System(crate::SystemMessage::UserAbort) => {}
            _ => panic!("Expected SystemMessage::UserAbort"),
        }
    }

    #[test]
    fn test_user_input_without_attachments() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440002".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440002".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "UserInput".to_string(),
            content: r#"{
                "message": "Okay but i need a test where `attachments` is not set at all"
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        match message.payload {
            crate::MessagePayload::User(user_input) => {
                assert_eq!(
                    user_input.message,
                    "Okay but i need a test where `attachments` is not set at all"
                );
                assert!(user_input.attachments.is_none());
            }
            _ => panic!("Expected MessagePayload::User"),
        }
    }

    #[test]
    fn test_user_input_with_empty_attachments_array() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440003".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440003".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "UserInput".to_string(),
            content: r#"{
                "attachments": [],
                "message": "Message with empty attachments"
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        match message.payload {
            crate::MessagePayload::User(user_input) => {
                assert_eq!(user_input.message, "Message with empty attachments");
                assert!(user_input.attachments.is_some());
                assert_eq!(user_input.attachments.unwrap().len(), 0);
            }
            _ => panic!("Expected MessagePayload::User"),
        }
    }

    #[test]
    fn test_user_input_with_file_attachment() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440004".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440004".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "UserInput".to_string(),
            content: r#"{
                "attachments": [
                    {
                        "path": "ASSETS_LICENSE",
                        "type": "file"
                    }
                ],
                "message": "Check this file out"
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        match message.payload {
            crate::MessagePayload::User(user_input) => {
                assert_eq!(user_input.message, "Check this file out");
                assert!(user_input.attachments.is_some());
                let attachments = user_input.attachments.unwrap();
                assert_eq!(attachments.len(), 1);
                match &attachments[0] {
                    crate::PromptAttachment::File(file_att) => {
                        assert_eq!(file_att.path, "ASSETS_LICENSE");
                        assert!(file_att.commit_id.is_none());
                    }
                    _ => panic!("Expected File attachment"),
                }
            }
            _ => panic!("Expected MessagePayload::User"),
        }
    }

    #[test]
    fn test_claude_output() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440005".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440005".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "ClaudeOutput".to_string(),
            content: r#"{
                "message": {
                    "content": [
                        {
                            "text": "Perfect! Now let's run the tests to verify they all pass:",
                            "type": "text"
                        }
                    ],
                    "id": "msg_01Eu1HSLVLWD64FDD1j8KGgQ",
                    "model": "claude-sonnet-4-5-20250929",
                    "role": "assistant",
                    "stop_reason": null,
                    "stop_sequence": null,
                    "type": "message",
                    "usage": {
                        "cache_creation": {
                            "ephemeral_1h_input_tokens": 0,
                            "ephemeral_5m_input_tokens": 1327
                        },
                        "cache_creation_input_tokens": 1327,
                        "cache_read_input_tokens": 28563,
                        "input_tokens": 4,
                        "output_tokens": 1,
                        "service_tier": "standard"
                    }
                },
                "parent_tool_use_id": null,
                "session_id": "a9a3c83b-fdf4-4eee-964e-1043c7b8ac0b",
                "type": "assistant",
                "uuid": "aab0b9a3-be9f-4ca0-adac-3cdaeebf889d"
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        match message.payload {
            crate::MessagePayload::Claude(claude_output) => {
                // Verify it's valid JSON and contains expected fields
                assert!(claude_output.data.is_object());
                assert!(claude_output.data.get("message").is_some());
                assert!(claude_output.data.get("type").is_some());
                assert_eq!(
                    claude_output.data.get("type").unwrap().as_str().unwrap(),
                    "assistant"
                );
            }
            _ => panic!("Expected MessagePayload::Claude"),
        }
    }

    #[test]
    fn test_new_claude_type() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440006".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440006".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "Claude".to_string(),
            content: r#"{
                "content": [
                    {
                        "text": "Some claude response",
                        "type": "text"
                    }
                ],
                "id": "msg_123",
                "role": "assistant"
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        match message.payload {
            crate::MessagePayload::Claude(claude_output) => {
                assert!(claude_output.data.is_object());
                assert!(claude_output.data.get("content").is_some());
            }
            _ => panic!("Expected MessagePayload::Claude"),
        }
    }

    #[test]
    fn test_new_user_type() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440007".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440007".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "User".to_string(),
            content: r#"{
                "message": "Test user message",
                "attachments": null
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        match message.payload {
            crate::MessagePayload::User(user_input) => {
                assert_eq!(user_input.message, "Test user message");
                assert!(user_input.attachments.is_none());
            }
            _ => panic!("Expected MessagePayload::User"),
        }
    }

    #[test]
    fn test_new_system_type() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440008".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440008".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "System".to_string(),
            content: r#"{
                "type": "userAbort"
            }"#
            .to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_ok());

        let message = result.unwrap();
        match message.payload {
            crate::MessagePayload::System(crate::SystemMessage::UserAbort) => {}
            _ => panic!("Expected MessagePayload::System(UserAbort)"),
        }
    }

    #[test]
    fn test_invalid_content_type() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440000".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "InvalidType".to_string(),
            content: r#"{"message": "test"}"#.to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_json_content() {
        let db_message = but_db::ClaudeMessage {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            session_id: "650e8400-e29b-41d4-a716-446655440000".to_string(),
            created_at: chrono::DateTime::from_timestamp(1234567890, 0)
                .unwrap()
                .naive_utc(),
            content_type: "User".to_string(),
            content: "not valid json".to_string(),
        };

        let result: Result<crate::ClaudeMessage, _> = db_message.try_into();
        assert!(result.is_err());
    }
}
