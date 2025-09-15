use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub mod bridge;
pub use bridge::ClaudeCheckResult;
pub(crate) mod claude_config;
pub(crate) mod claude_settings;
pub(crate) mod claude_transcript;
pub use claude_transcript::Transcript;
pub mod db;
pub mod hooks;
pub mod mcp;
pub mod notifications;
pub mod prompt_templates;
mod rules;

/// Represents a Claude Code session that GitButler is tracking.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    /// The unique and stable identifier for the session. This is the first session_id that was used.
    id: Uuid,
    /// The most recent session ID. If a session is stopped and resumed, Claude will copy over the past context into a new session. This value is unique.
    current_id: Uuid,
    /// All session IDs that have been used for this session, including the current one.
    session_ids: Vec<Uuid>,
    /// The timestamp when the first session was created.
    created_at: chrono::NaiveDateTime,
    /// The timestamp when the session was last updated.
    updated_at: chrono::NaiveDateTime,
    /// Whether this session is used by the GUI.
    pub in_gui: bool,
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
    /// Metadata provided by GitButler around the Claude Code statuts
    GitButlerMessage(GitButlerMessage),
}

/// Represents user input in a Claude session.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    /// The user message
    pub message: String,
}

/// Metadata provided by GitButler.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum GitButlerMessage {
    /// Claude code has exited naturally.
    ClaudeExit {
        code: i32,
        message: String,
    },
    /// Claude code has exited due to a user abortion.
    UserAbort,
    UnhandledException {
        message: String,
    },
}

/// Details about a Claude session, extracted from the Claude transcript.
/// This data is derived just in time, i.e. not persisted by GitButler.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionDetails {
    pub summary: Option<String>,
    pub last_prompt: Option<String>,
    pub in_gui: bool,
}

/// Represents a request for permission to use a tool in the Claude MCP.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudePermissionRequest {
    /// Maps to the tool_use_id from the MCP request
    pub id: String,
    /// When the requst was made.
    pub created_at: chrono::NaiveDateTime,
    /// When the request was updated.
    pub updated_at: chrono::NaiveDateTime,
    /// The tool for which permission is requested
    pub tool_name: String,
    /// The input for the tool
    pub input: serde_json::Value,
    /// The status of the request or None if not yet handled
    pub approved: Option<bool>,
}

/// Represents the thinking level for Claude Code.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ThinkingLevel {
    Normal,
    Think,
    MegaThink,
    UltraThink,
}

/// Represents the model type for Claude Code.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ModelType {
    Sonnet,
    #[serde(rename = "sonnet[1m]")]
    Sonnet1m,
    Opus,
    #[serde(rename = "opusplan")]
    OpusPlan,
}

impl ModelType {
    /// Convert the model type to the CLI argument string format.
    pub fn to_cli_string(&self) -> &str {
        match self {
            ModelType::Sonnet => "sonnet",
            ModelType::Sonnet1m => "sonnet[1m]",
            ModelType::Opus => "opus",
            ModelType::OpusPlan => "opusplan",
        }
    }
}

/// Represents the permission mode for Claude Code.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,
    Plan,
    AcceptEdits,
}
