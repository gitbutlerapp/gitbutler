//! Post command implementation.

use chrono::Utc;
use ulid::Ulid;

use crate::db::DbHandle;
use crate::types::{Message, MessageKind, validate_agent_id, validate_content};

/// Post a message to the shared channel.
pub fn execute(db: &DbHandle, content: String, agent_id: String) -> anyhow::Result<Message> {
    execute_with_kind(db, content, agent_id, MessageKind::Message)
}

/// Post a message with a specific kind.
pub fn execute_with_kind(
    db: &DbHandle,
    content: String,
    agent_id: String,
    kind: MessageKind,
) -> anyhow::Result<Message> {
    validate_agent_id(&agent_id)?;
    validate_content(&content)?;

    let now = Utc::now();
    let message_id = Ulid::new().to_string();

    db.upsert_agent(&agent_id, now)?;

    let message = Message {
        id: message_id,
        agent_id,
        content,
        timestamp: now,
        kind,
    };

    db.insert_message(&message)?;

    Ok(message)
}
