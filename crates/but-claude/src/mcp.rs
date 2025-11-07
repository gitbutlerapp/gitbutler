use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use but_db::poll::ItemKind;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
};

use crate::permissions::{PermissionCheck, Permissions};

pub async fn start(repo_path: &Path, session_id_str: &str) -> Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let client_info = Arc::new(Mutex::new(None));
    let transport = (tokio::io::stdin(), tokio::io::stdout());

    // Parse the current session ID from Claude Code
    let current_session_id = uuid::Uuid::parse_str(session_id_str)
        .map_err(|e| anyhow::anyhow!("Invalid session ID '{}': {}", session_id_str, e))?;

    // Look up the session by current_id to get the stable session ID
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let ctx = &mut CommandContext::open(&project, app_settings)?;
    let session = crate::db::get_session_by_current_id(ctx, current_session_id)?
        .ok_or_else(|| anyhow::anyhow!("Session not found in database: {}", current_session_id))?;

    tracing::info!(
        "Starting MCP server for session {} (current_id: {})",
        session.id,
        current_session_id
    );

    // Use the stable session.id, not the current_id
    let server = Mcp {
        project,
        tool_router: Mcp::tool_router(),
        runtime_permissions: Default::default(),
        session_id: session.id,
    };
    let service = server.serve(transport).await?;
    let info = service.peer_info();
    if let Ok(mut guard) = client_info.lock() {
        *guard = info.map(|i| i.client_info.clone());
    }
    service.waiting().await?;
    // serve_server(server, transport).await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Mcp {
    project: Project,
    tool_router: ToolRouter<Self>,
    runtime_permissions: Arc<Mutex<Permissions>>,
    session_id: uuid::Uuid,
}

#[tool_router(vis = "pub")]
impl Mcp {
    #[tool(
        name = "approval_prompt",
        description = "Permission check for tool calls"
    )]
    pub fn approval_prompt(
        &self,
        request: Parameters<McpPermissionRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let approved = self
            .approval_inner(
                request.0.clone().into(),
                std::time::Duration::from_secs(60 * 60 * 24),
            )
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let result = Ok(McpPermissionResponse {
            behavior: if approved {
                Behavior::Allow
            } else {
                Behavior::Deny
            },
            updated_input: Some(request.0.input.clone()),
            message: if approved {
                None
            } else {
                Some("Rejected by user".to_string())
            },
        });
        result.map(|outcome| Ok(CallToolResult::success(vec![Content::json(outcome)?])))?
    }

    fn approval_inner(
        &self,
        req: crate::ClaudePermissionRequest,
        timeout: std::time::Duration,
    ) -> anyhow::Result<bool> {
        let app_settings = AppSettings::load_from_default_path_creating()?;
        let ctx = &mut CommandContext::open(&self.project, app_settings)?;

        // Load session permissions from database (using stable session ID)
        let session = crate::db::get_session_by_id(ctx, self.session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", self.session_id))?;

        // Merge runtime and session permissions
        let runtime_perms = self.runtime_permissions.lock().unwrap();
        let session_perms =
            Permissions::from_slices(session.approved_permissions(), session.denied_permissions());
        let combined_perms = Permissions::merge([&*runtime_perms, &session_perms]);
        drop(runtime_perms); // Release the lock

        // Check the combined permissions
        let result = combined_perms.check(&req).unwrap_or_default();
        match result {
            PermissionCheck::Approved => return Ok(true),
            PermissionCheck::Denied => return Ok(false),
            PermissionCheck::Ask => (), // Continue to ask the user
        }

        // Send notification for permission request
        if let Err(e) =
            crate::notifications::notify_permission_request(ctx.app_settings(), &req.tool_name)
        {
            tracing::warn!("Failed to send permission request notification: {}", e);
        }

        // Create a record that will be seen by the user in the UI
        ctx.db()?
            .claude_permission_requests()
            .insert(req.clone().try_into()?)?;
        // Poll for user approval
        let rx = ctx.db()?.poll_changes(
            ItemKind::Actions
                | ItemKind::Workflows
                | ItemKind::Assignments
                | ItemKind::Rules
                | ItemKind::ClaudePermissionRequests,
            std::time::Duration::from_millis(500),
        )?;
        let mut approved_state = false;
        let start_time = std::time::Instant::now();

        for item in rx {
            if start_time.elapsed() > timeout {
                eprintln!("Timeout waiting for permission approval (1 day)");
                break;
            }
            match item {
                Ok(ItemKind::ClaudePermissionRequests) => {
                    if let Some(updated) = ctx.db()?.claude_permission_requests().get(&req.id)? {
                        if let Some(decision_str) = updated.decision {
                            let decision: crate::PermissionDecision =
                                serde_json::from_str(&decision_str)?;
                            approved_state = decision.is_allowed();

                            // Handle the decision - persist to settings/session/database and update runtime permissions
                            let project_path = self.project.worktree_dir()?.canonicalize()?;
                            let mut runtime_perms = self.runtime_permissions.lock().unwrap();

                            if let Err(e) = decision.handle(
                                &req,
                                &project_path,
                                &mut runtime_perms,
                                Some(ctx),
                                Some(self.session_id),
                            ) {
                                tracing::warn!("Failed to handle permission decision: {}", e);
                            }

                            break;
                        }
                    } else {
                        eprintln!("Permission request not found: {}", req.id);
                        break;
                    }
                }
                Ok(_) => continue, // Ignore other item kinds
                Err(e) => {
                    eprintln!("Error polling for changes: {e}");
                    break;
                }
            }
        }
        Ok(approved_state)
    }
}

impl From<McpPermissionRequest> for crate::ClaudePermissionRequest {
    fn from(request: McpPermissionRequest) -> Self {
        crate::ClaudePermissionRequest {
            id: request.tool_use_id,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            tool_name: request.tool_name,
            input: request.input,
            decision: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct McpPermissionRequest {
    #[schemars(description = "The name of the tool requesting permission")]
    tool_name: String,
    #[schemars(description = "The input for the tool")]
    input: serde_json::Value,
    #[schemars(description = "The unique tool use request ID")]
    tool_use_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Behavior {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPermissionResponse {
    behavior: Behavior,
    updated_input: Option<serde_json::Value>,
    message: Option<String>,
}

#[tool_handler]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("GitButler CC Security MCP server".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                title: None,
                name: "GitButler CC Security MCP server".into(),
                version: "1.0.0".into(),
                icons: None,
                website_url: Some("https://gitbutler.com".into()),
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }
}
