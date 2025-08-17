use std::sync::{Arc, Mutex};

use anyhow::Result;
use rmcp::{
    Error as McpError, ServerHandler, ServiceExt,
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool,
};

pub async fn start() -> Result<()> {
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
    #[tool(description = "Permission check for tool calls")]
    pub fn approval_prompt(
        &self,
        #[tool(aggr)] request: McpPermissionRequest,
    ) -> Result<CallToolResult, McpError> {
        let result = Ok(McpPermissionResponse {
            behavior: Behavior::Allow,
            updated_input: Some(request.input),
            message: None,
        });
        result.map(|outcome| Ok(CallToolResult::success(vec![Content::json(outcome)?])))?
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
