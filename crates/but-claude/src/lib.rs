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
pub(crate) mod legacy;
pub mod mcp;
pub mod notifications;
pub mod permissions;
pub mod prompt_templates;
mod rules;

pub use permissions::Permission;

/// Represents a Claude Code session that GitButler is tracking.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    /// The unique and stable identifier for the session. This is the first session_id that was used.
    pub id: Uuid,
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
    /// Permissions that have been approved for this session.
    approved_permissions: Vec<Permission>,
    /// Permissions that have been denied for this session.
    denied_permissions: Vec<Permission>,
}

impl ClaudeSession {
    pub fn approved_permissions(&self) -> &[Permission] {
        &self.approved_permissions
    }

    pub fn denied_permissions(&self) -> &[Permission] {
        &self.denied_permissions
    }
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
    /// The payload of the message from different sources.
    payload: MessagePayload,
}

impl ClaudeMessage {
    pub fn created_at(&self) -> chrono::NaiveDateTime {
        self.created_at
    }

    pub fn content(&self) -> &MessagePayload {
        &self.payload
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum GitButlerUpdate {
    /// Update about new commit creation
    CommitCreated(CommitCreatedDetails),
}

/// The actual message payload from different sources.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "source")]
pub enum MessagePayload {
    /// Output from Claude Code CLI stdout stream
    Claude(ClaudeOutput),
    /// Input provided by the user
    User(UserInput),
    /// System message from GitButler about the session
    System(SystemMessage),
    /// Resource update, e.g. a commit was created
    GitButler(GitButlerUpdate),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileAttachment {
    commit_id: Option<String>,
    path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinesAttachment {
    commit_id: Option<String>,
    path: String,
    start: usize,
    end: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommitAttachment {
    commit_id: String,
}

/// Represents a file attachment with full content (used in API input).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PromptAttachment {
    Lines(LinesAttachment),
    File(FileAttachment),
    Commit(CommitAttachment),
}

/// Raw output from Claude API
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeOutput {
    /// Raw JSON value from Claude API streaming output
    pub data: serde_json::Value,
}

/// Represents user input in a Claude session.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    /// The user message
    pub message: String,
    /// Optional attached file references
    #[serde(default)]
    pub attachments: Option<Vec<PromptAttachment>>,
}

/// Details about commits created by Claude.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommitCreatedDetails {
    #[serde(default)]
    pub stack_id: Option<String>,
    #[serde(default)]
    pub branch_name: Option<String>,
    #[serde(default)]
    pub commit_ids: Option<Vec<String>>,
}

/// System messages from GitButler about the Claude session state.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SystemMessage {
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
    /// Deprecated and will be removed, see `GitButlerUpdate::CommitCreated`.
    CommitCreated(CommitCreatedDetails),
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

/// Represents a permission decision with both the action (allow/deny) and scope.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionDecision {
    /// Allow this single request
    AllowOnce,
    /// Allow for the current session
    AllowSession,
    /// Allow for this project
    AllowProject,
    /// Allow globally (always)
    AllowAlways,
    /// Deny this single request
    DenyOnce,
    /// Deny for the current session
    DenySession,
    /// Deny for this project
    DenyProject,
    /// Deny globally (always)
    DenyAlways,
}

impl PermissionDecision {
    /// Returns true if this is an allow decision
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            PermissionDecision::AllowOnce
                | PermissionDecision::AllowSession
                | PermissionDecision::AllowProject
                | PermissionDecision::AllowAlways
        )
    }

    /// Handle the decision by performing the appropriate action based on the variant
    pub fn handle(
        &self,
        request: &ClaudePermissionRequest,
        project_path: &std::path::Path,
        runtime_permissions: &mut crate::permissions::Permissions,
        ctx: Option<&mut gitbutler_command_context::CommandContext>,
        session_id: Option<uuid::Uuid>,
    ) -> anyhow::Result<()> {
        use crate::permissions::{
            Permission, SerializationContext, SettingsKind, add_permission_to_settings,
        };

        // Extract permissions from the request (may be multiple for bash with && or ||)
        let permissions = Permission::from_request(request)?;

        // Build serialization context
        let home_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let global_claude_dir = home_path.join(".claude");

        match self {
            PermissionDecision::AllowOnce => {
                // Single request - no persistence needed
                Ok(())
            }
            PermissionDecision::AllowSession => {
                // Add to runtime permissions
                for permission in &permissions {
                    runtime_permissions.add_approved(permission.clone());
                }

                // Also save to session database if available
                if let (Some(ctx), Some(sess_id)) = (ctx, session_id)
                    && let Ok(Some(session)) = crate::db::get_session_by_current_id(ctx, sess_id)
                {
                    let mut approved = session.approved_permissions().to_vec();
                    approved.extend(permissions);
                    let denied = session.denied_permissions().to_vec();
                    crate::db::update_session_permissions(ctx, session.id, &approved, &denied)?;
                }
                Ok(())
            }
            PermissionDecision::AllowProject => {
                let ctx =
                    SerializationContext::new(&home_path, project_path, &global_claude_dir, false);
                let settings_path = project_path.join(".claude/settings.local.json");

                // Ensure .claude directory exists
                if let Some(parent) = settings_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                for permission in permissions {
                    add_permission_to_settings(
                        &SettingsKind::Allow,
                        &permission,
                        &ctx,
                        &settings_path,
                    )?;
                    runtime_permissions.add_approved(permission);
                }
                Ok(())
            }
            PermissionDecision::AllowAlways => {
                let ctx =
                    SerializationContext::new(&home_path, project_path, &global_claude_dir, true);
                let settings_path = home_path.join(".claude/settings.json");

                // Ensure .claude directory exists
                if let Some(parent) = settings_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                for permission in permissions {
                    add_permission_to_settings(
                        &SettingsKind::Allow,
                        &permission,
                        &ctx,
                        &settings_path,
                    )?;
                    runtime_permissions.add_approved(permission);
                }
                Ok(())
            }
            PermissionDecision::DenyOnce => {
                // Single request - no persistence needed
                Ok(())
            }
            PermissionDecision::DenySession => {
                // Add to runtime permissions
                for permission in &permissions {
                    runtime_permissions.add_denied(permission.clone());
                }

                // Also save to session database if available
                if let (Some(ctx), Some(sess_id)) = (ctx, session_id)
                    && let Ok(Some(session)) = crate::db::get_session_by_current_id(ctx, sess_id)
                {
                    let approved = session.approved_permissions().to_vec();
                    let mut denied = session.denied_permissions().to_vec();
                    denied.extend(permissions);
                    crate::db::update_session_permissions(ctx, session.id, &approved, &denied)?;
                }
                Ok(())
            }
            PermissionDecision::DenyProject => {
                let ctx =
                    SerializationContext::new(&home_path, project_path, &global_claude_dir, false);
                let settings_path = project_path.join(".claude/settings.local.json");

                // Ensure .claude directory exists
                if let Some(parent) = settings_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                for permission in permissions {
                    add_permission_to_settings(
                        &SettingsKind::Deny,
                        &permission,
                        &ctx,
                        &settings_path,
                    )?;
                    runtime_permissions.add_denied(permission);
                }
                Ok(())
            }
            PermissionDecision::DenyAlways => {
                let ctx =
                    SerializationContext::new(&home_path, project_path, &global_claude_dir, true);
                let settings_path = home_path.join(".claude/settings.json");

                // Ensure .claude directory exists
                if let Some(parent) = settings_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                for permission in permissions {
                    add_permission_to_settings(
                        &SettingsKind::Deny,
                        &permission,
                        &ctx,
                        &settings_path,
                    )?;
                    runtime_permissions.add_denied(permission);
                }
                Ok(())
            }
        }
    }
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
    /// The permission decision or None if not yet handled
    pub decision: Option<PermissionDecision>,
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
    content: MessagePayload,
) -> Result<()> {
    let message = db::save_new_message(ctx, session_id, content.clone())?;
    let project_id = ctx.project().id;

    broadcaster.lock().await.send(FrontendEvent {
        name: format!("project://{project_id}/claude/{stack_id}/message_recieved"),
        payload: json!(message),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_input_backwards_compatibility_without_attachments() {
        let json_without_attachments = r#"{
            "message": "foo"
        }"#;

        let user_input: UserInput = serde_json::from_str(json_without_attachments)
            .expect("Failed to deserialize UserInput without attachments field");

        assert_eq!(user_input.message, "foo");
        assert!(
            user_input.attachments.is_none(),
            "Attachments should default to None for backwards compatibility"
        );
    }

    #[test]
    fn test_user_input_with_attachments() {
        let json_with_attachments = r#"{
            "message": "bar",
            "attachments": [
                {
                    "type": "file",
                    "path": "src/main.rs"
                }
            ]
        }"#;

        let user_input: UserInput = serde_json::from_str(json_with_attachments)
            .expect("Failed to deserialize UserInput with attachments field");

        assert_eq!(user_input.message, "bar");
        assert!(user_input.attachments.is_some());
        let attachments = user_input.attachments.unwrap();
        assert_eq!(attachments.len(), 1);
    }
}
