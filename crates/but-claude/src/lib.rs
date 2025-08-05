#![feature(anonymous_pipe)]

use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub mod bridge;
pub mod db;
pub mod hooks;
pub mod mcp;

/// Represents a Claude Code session that GitButler is tracking.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    /// The unique and stable identifier for the session. This is the first session_id that was used.
    id: Uuid,
    /// The most recent session ID. If a session is stopped and resumed, Claude will copy over the past context into a new session. This value is unique.
    current_id: Uuid,
    /// The timestamp when the first session was created.
    created_at: chrono::NaiveDateTime,
    /// The timestamp when the session was last updated.
    updated_at: chrono::NaiveDateTime,
}

/// Represents a message in a Claude session, referencing the stable session ID.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMessage {
    /// Message identifier
    id: Uuid,
    /// The stable session ID that this message belongs to.
    session_id: Uuid,
    /// The timestamp when the message was created.
    created_at: chrono::NaiveDateTime,
    /// The content of the message, which can be either output from Claude or user input.
    content: ClaudeMessageContent,
}

/// Represents the kind of content in a Claude message.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum ClaudeMessageContent {
    /// Came from Claude standard out stream
    ClaudeOutput(serde_json::Value),
    /// Inserted via  GitButler (what the user typed)
    UserInput(UserInput),
}

/// Represents user input in a Claude session.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    /// The user message
    pub message: String,
}
