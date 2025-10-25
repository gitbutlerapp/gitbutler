use std::sync::Arc;

use anyhow::Result;
use but_broadcaster::{Broadcaster, FrontendEvent};
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;
pub mod bridge;
pub use bridge::ClaudeCheckResult;
pub(crate) mod claude_config;
pub mod claude_mcp;
pub mod claude_settings;
pub mod claude_sub_agents;
pub mod compact;
pub use claude_sub_agents::SubAgent;
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

/// File attachment with full content.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttachedHunk {
    pub path: String,
    pub start: usize,
    pub end: usize,
}

/// File attachment with full content.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttachedFile {
    pub path: String,
}

/// File attachment with full content.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttachedCommit {
    pub commit_id: String,
}

/// Represents a file attachment with full content (used in API input).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum PromptAttachment {
    Hunk(AttachedHunk),
    File(AttachedFile),
    Commit(AttachedCommit),
}

/// Represents user input in a Claude session.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    /// The user message
    pub message: String,
    /// Optional attached file references
    pub attachments: Option<Vec<PromptAttachment>>,
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
    /// Compact operation has started.
    CompactStart,
    /// Compact operation has finished.
    CompactFinished {
        summary: String,
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
    Haiku,
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
            ModelType::Haiku => "haiku",
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

/// Represents user-provided parameters for Claude requests.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUserParams {
    pub message: String,
    pub thinking_level: ThinkingLevel,
    pub model: ModelType,
    pub permission_mode: PermissionMode,
    pub disabled_mcp_servers: Vec<String>,
    pub add_dirs: Vec<String>,
    pub attachments: Option<Vec<PromptAttachment>>,
}

pub async fn send_claude_message(
    ctx: &mut CommandContext,
    broadcaster: Arc<Mutex<Broadcaster>>,
    session_id: uuid::Uuid,
    stack_id: StackId,
    content: ClaudeMessageContent,
) -> Result<()> {
    let message = db::save_new_message(ctx, session_id, content.clone())?;
    let project_id = ctx.project().id;

    broadcaster.lock().await.send(FrontendEvent {
        name: format!("project://{project_id}/claude/{stack_id}/message_recieved"),
        payload: json!(message),
    });
    Ok(())
}
