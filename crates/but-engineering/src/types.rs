//! Core types for but-engineering.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum length for agent ID.
pub const MAX_AGENT_ID_LEN: usize = 256;

/// Maximum length for status message.
pub const MAX_STATUS_LEN: usize = 256;

/// Maximum length for message content.
pub const MAX_CONTENT_LEN: usize = 16384;

/// An agent record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    /// Unique identifier for the agent.
    pub id: String,
    /// Optional status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// When the agent was last active.
    pub last_active: DateTime<Utc>,
    /// When the agent last read messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_read: Option<DateTime<Utc>>,
    /// Current plan (what the agent intends to do before starting).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// When the plan was last set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_updated_at: Option<DateTime<Utc>>,
}

/// An agent for public listing (without last_read).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Unique identifier for the agent.
    pub id: String,
    /// Optional status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// When the agent was last active.
    pub last_active: DateTime<Utc>,
}

impl From<Agent> for AgentInfo {
    fn from(agent: Agent) -> Self {
        AgentInfo {
            id: agent.id,
            status: agent.status,
            last_active: agent.last_active,
        }
    }
}

/// The kind of a message in the channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageKind {
    /// A regular chat message (default).
    #[default]
    Message,
    /// A discovery — a finding or insight other agents should know.
    Discovery,
    /// A block notification — an agent is blocked waiting for another.
    Block,
}

impl MessageKind {
    /// Returns the string representation used for DB storage.
    pub fn as_str(self) -> &'static str {
        match self {
            MessageKind::Message => "message",
            MessageKind::Discovery => "discovery",
            MessageKind::Block => "block",
        }
    }

    /// Parse from a DB string, defaulting to `Message` for unknown values.
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "discovery" => MessageKind::Discovery,
            "block" => MessageKind::Block,
            _ => MessageKind::Message,
        }
    }

    fn is_default(&self) -> bool {
        matches!(self, MessageKind::Message)
    }
}

/// A message record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID (ULID).
    pub id: String,
    /// ID of the agent that posted this message.
    pub agent_id: String,
    /// Message content.
    pub content: String,
    /// When the message was posted.
    pub timestamp: DateTime<Utc>,
    /// Message kind.
    #[serde(default)]
    #[serde(skip_serializing_if = "MessageKind::is_default")]
    pub kind: MessageKind,
}

/// A file claim record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    /// The claimed file path.
    pub file_path: String,
    /// ID of the agent that owns the claim.
    pub agent_id: String,
    /// When the claim was created or refreshed.
    pub claimed_at: DateTime<Utc>,
}

/// Error response for JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error message.
    pub error: String,
}

impl ErrorResponse {
    /// Create a new error response.
    pub fn new(message: impl Into<String>) -> Self {
        ErrorResponse { error: message.into() }
    }
}

/// Validate agent ID.
pub fn validate_agent_id(id: &str) -> anyhow::Result<()> {
    if id.is_empty() {
        anyhow::bail!("agent_id cannot be empty");
    }
    if id.len() > MAX_AGENT_ID_LEN {
        anyhow::bail!("agent_id exceeds maximum length of {MAX_AGENT_ID_LEN}");
    }
    Ok(())
}

/// Validate status message.
pub fn validate_status(status: &str) -> anyhow::Result<()> {
    if status.len() > MAX_STATUS_LEN {
        anyhow::bail!("status exceeds maximum length of {MAX_STATUS_LEN}");
    }
    Ok(())
}

/// Validate message content.
pub fn validate_content(content: &str) -> anyhow::Result<()> {
    if content.is_empty() {
        anyhow::bail!("content cannot be empty");
    }
    if content.len() > MAX_CONTENT_LEN {
        anyhow::bail!("content exceeds maximum length of {MAX_CONTENT_LEN}");
    }
    Ok(())
}
