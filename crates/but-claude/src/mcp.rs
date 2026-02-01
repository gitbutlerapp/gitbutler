use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use but_ctx::{Context, ThreadSafeContext};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};

use crate::permissions::{PermissionCheck, Permissions};

pub async fn start(repo_path: &Path, session_id_str: &str) -> Result<()> {
    let ctx = Context::open(repo_path)?;
    let client_info = Arc::new(Mutex::new(None));
    let transport = (tokio::io::stdin(), tokio::io::stdout());

    // Parse the current session ID from Claude Code
    let current_session_id = uuid::Uuid::parse_str(session_id_str)
        .map_err(|e| anyhow::anyhow!("Invalid session ID '{}': {}", session_id_str, e))?;

    // Look up the session by current_id to get the stable session ID
    let session = crate::db::get_session_by_current_id(&ctx, current_session_id)?
        .ok_or_else(|| anyhow::anyhow!("Session not found in database: {}", current_session_id))?;

    tracing::info!(
        "Starting MCP server for session {} (current_id: {})",
        session.id,
        current_session_id
    );

    // Use the stable session.id, not the current_id
    let server = Mcp {
        ctx: ctx.into_sync(),
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
    ctx: ThreadSafeContext,
    tool_router: ToolRouter<Self>,
    runtime_permissions: Arc<Mutex<Permissions>>,
    session_id: uuid::Uuid,
}

#[tool_router(vis = "pub")]
impl Mcp {
    #[tool(name = "approval_prompt", description = "Permission check for tool calls")]
    pub fn approval_prompt(
        &self,
        request: Parameters<McpPermissionRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        // Check if this is an AskUserQuestion request
        if request.0.tool_name == "AskUserQuestion" {
            return self.handle_ask_user_question(request);
        }

        let approved = self
            .approval_inner(request.0.clone().into(), std::time::Duration::from_secs(60 * 60 * 24))
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let result = Ok(McpPermissionResponse {
            behavior: if approved { Behavior::Allow } else { Behavior::Deny },
            updated_input: Some(request.0.input.clone()),
            message: if approved {
                None
            } else {
                Some("Rejected by user".to_string())
            },
        });
        result.map(|outcome| Ok(CallToolResult::success(vec![Content::json(outcome)?])))?
    }

    /// Handle AskUserQuestion tool call
    fn handle_ask_user_question(
        &self,
        request: Parameters<McpPermissionRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let answers = self
            .ask_user_question_inner(
                &request.0.tool_use_id,
                &request.0.input,
                std::time::Duration::from_secs(60), // 60 second timeout for questions
            )
            .map_err(|e| {
                tracing::error!("AskUserQuestion failed: {}", e);
                rmcp::ErrorData::internal_error(e.to_string(), None)
            })?;

        // Build the updated input with answers
        let mut updated_input = request.0.input.clone();
        if let Some(obj) = updated_input.as_object_mut() {
            obj.insert("answers".to_string(), serde_json::json!(answers));
        }

        let result = Ok(McpPermissionResponse {
            behavior: Behavior::Allow,
            updated_input: Some(updated_input),
            message: None,
        });
        result.map(|outcome| Ok(CallToolResult::success(vec![Content::json(outcome)?])))?
    }

    /// Inner handler for AskUserQuestion that stores the request and waits for answers
    fn ask_user_question_inner(
        &self,
        id: &str,
        input: &serde_json::Value,
        _timeout: std::time::Duration,
    ) -> anyhow::Result<HashMap<String, String>> {
        // Parse questions from input
        let questions: Vec<crate::AskUserQuestion> = input
            .get("questions")
            .map(|q| serde_json::from_value(q.clone()))
            .transpose()?
            .unwrap_or_default();

        let now = chrono::Utc::now().naive_utc();
        let request = crate::ClaudeAskUserQuestionRequest {
            id: id.to_string(),
            created_at: now,
            updated_at: now,
            questions,
            answers: None,
            stack_id: None, // MCP path doesn't have stack context
        };

        // Store in-memory and get receiver for response
        let receiver = crate::pending_requests::pending_requests()
            .insert_question(request, self.session_id);

        // Wait for user answers with timeout (blocking)
        match receiver.blocking_recv() {
            Ok(answers) => Ok(answers),
            Err(_) => {
                // Sender dropped (session cancelled or timeout)
                crate::pending_requests::pending_requests().remove_question(id);
                anyhow::bail!("Question request cancelled or timed out")
            }
        }
    }

    fn approval_inner(
        &self,
        req: crate::ClaudePermissionRequest,
        _timeout: std::time::Duration,
    ) -> anyhow::Result<bool> {
        let mut ctx = self.ctx.clone().into_thread_local();

        // Load session permissions from database (using stable session ID)
        let session = crate::db::get_session_by_id(&ctx, self.session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", self.session_id))?;

        // Merge runtime and session permissions
        let runtime_perms = self.runtime_permissions.lock().unwrap();
        let session_perms = Permissions::from_slices(session.approved_permissions(), session.denied_permissions());
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
        if let Err(e) = crate::notifications::notify_permission_request(&ctx.settings, &req.tool_name) {
            tracing::warn!("Failed to send permission request notification: {}", e);
        }

        // Store in-memory and get receiver for response
        let receiver = crate::pending_requests::pending_requests()
            .insert_permission(req.clone(), self.session_id);

        // Wait for user decision (blocking)
        match receiver.blocking_recv() {
            Ok(decision) => {
                let approved = decision.is_allowed();

                // Handle the decision - persist to settings/session/database and update runtime permissions
                let mut runtime_perms = self.runtime_permissions.lock().unwrap();
                if let Err(e) = decision.handle(&req, &mut runtime_perms, &mut ctx, self.session_id) {
                    tracing::warn!("Failed to handle permission decision: {}", e);
                }

                Ok(approved)
            }
            Err(_) => {
                // Sender dropped (session cancelled)
                crate::pending_requests::pending_requests().remove_permission(&req.id);
                Ok(false)
            }
        }
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
            use_wildcard: false, // Default to false for requests coming from MCP
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
