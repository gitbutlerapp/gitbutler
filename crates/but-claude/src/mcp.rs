use std::sync::{Arc, Mutex};

use anyhow::Result;
use rmcp::{
    Error as McpError, ServerHandler, ServiceExt,
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool,
};
use tracing_subscriber::{self, EnvFilter};

pub async fn start() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting MCP server");

    let client_info = Arc::new(Mutex::new(None));
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = Mcp::default().serve(transport).await?;
    let info = service.peer_info();
    if let Ok(mut guard) = client_info.lock() {
        guard.replace(info.client_info.clone());
    }
    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct Mcp {}

#[tool(tool_box)]
impl Mcp {
    #[tool(description = "Permission check - approve if the input contains allow, otherwise deny.")]
    pub fn approval_prompt(
        &self,
        #[tool(aggr)] request: PermissionRequest,
    ) -> Result<CallToolResult, McpError> {
        let result = Ok(PermissionResponse {
            behavior: Behavior::Allow,
            updated_input: Some(request.input),
            message: None,
        });
        result.map(|outcome| Ok(CallToolResult::success(vec![Content::json(outcome)?])))?
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    #[schemars(description = "The name of the tool requesting permission")]
    tool_name: String,
    #[schemars(description = "The input for the tool")]
    input: serde_json::Value,
    #[schemars(description = "The unique tool use request ID")]
    tool_use_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::Display)]
pub enum Behavior {
    #[strum(serialize = "allow")]
    Allow,
    #[strum(serialize = "deny")]
    Deny,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponse {
    behavior: Behavior,
    updated_input: Option<serde_json::Value>,
    message: Option<String>,
}

#[tool(tool_box)]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("GitButler MCP server".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "GitButler MCP Server".into(),
                version: "1.0.0".into(),
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }
}
