use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool,
};

pub mod project;
pub mod status;

pub(crate) const UI_CONTEXT_LINES: u32 = 3;

pub(crate) async fn start() -> Result<()> {
    let transport = (tokio::io::stdin(), tokio::io::stdout());

    let service = Mcp::new().serve(transport).await?;

    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Mcp;

#[tool(tool_box)]
impl Mcp {
    pub fn new() -> Self {
        Self
    }

    #[tool(
        description = "Get the status of a project. This contains information about the branches applied and uncommitted file changes."
    )]
    pub fn project_status(&self, #[tool(aggr)] request: ProjectStatusRequest) -> String {
        crate::mcp_internal::status::project_status(&request.project_dir).unwrap_or(format!(
            "Failed to get project status for directory: {}",
            request.project_dir
        ))
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectStatusRequest {
    #[schemars(description = "Absolute path to the project root")]
    pub project_dir: String,
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
