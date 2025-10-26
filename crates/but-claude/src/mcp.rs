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
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};

pub async fn start(repo_path: &Path) -> Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let client_info = Arc::new(Mutex::new(None));
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let server = Mcp::new(project);
    let service = server.serve(transport).await?;
    if let Some(info) = service.peer_info() {
        if let Ok(mut guard) = client_info.lock() {
            guard.replace(info.client_info.clone());
        }
    }
    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Mcp {
    project: Project,
    tool_router: ToolRouter<Self>,
}

impl Mcp {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl Mcp {
    #[tool(description = "Permission check for tool calls")]
    pub fn approval_prompt(
        &self,
        Parameters(request): Parameters<McpPermissionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let approved = self
            .approval_inner(
                request.clone().into(),
                std::time::Duration::from_secs(60 * 60 * 24),
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let result = Ok(McpPermissionResponse {
            behavior: if approved {
                Behavior::Allow
            } else {
                Behavior::Deny
            },
            updated_input: Some(request.input),
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

        // Send notification for permission request
        if let Err(e) =
            crate::notifications::notify_permission_request(&app_settings, &req.tool_name)
        {
            tracing::warn!("Failed to send permission request notification: {}", e);
        }

        let ctx = &mut CommandContext::open(&self.project, app_settings)?;
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
                        if let Some(approved) = updated.approved {
                            approved_state = approved;
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
            approved: None,
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
            instructions: Some("GitButler MCP server".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "GitButler MCP Server".into(),
                version: "1.0.0".into(),
                title: None,
                icons: None,
                website_url: None,
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }
}
