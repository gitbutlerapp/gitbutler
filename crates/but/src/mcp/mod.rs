use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool,
};

pub mod project;
pub mod stack;
pub mod status;

pub(crate) const UI_CONTEXT_LINES: u32 = 3;

pub(crate) async fn start() -> Result<()> {
    let transport = (tokio::io::stdin(), tokio::io::stdout());

    let service = PublicMcp::new().serve(transport).await?;

    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PublicMcp;

#[tool(tool_box)]
impl PublicMcp {
    pub fn new() -> Self {
        Self
    }

    #[tool(
        description = "Get the status of a project. This contains information about the branches applied and uncommitted file changes."
    )]
    pub fn project_status(&self, #[tool(aggr)] request: ProjectStatusRequest) -> String {
        crate::mcp::status::project_status(&request.project_dir).unwrap_or(format!(
            "Failed to get project status for directory: {}",
            request.project_dir
        ))
    }

    #[tool(
        description = "Add a checkpoint to the project. This store all the file changes made along with information about the prompt used to generate the code as context."
    )]
    pub fn add_checkpoint(&self, #[tool(aggr)] request: AddCheckpointRequest) -> String {
        crate::mcp::stack::add_checkpoint(&request.project_dir, &request.prompt).unwrap_or(format!(
            "Failed to add checkpoint for directory: {} with prompt: {}",
            request.project_dir, request.prompt
        ))
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddCheckpointRequest {
    #[schemars(description = "Absolute path to the project root")]
    pub project_dir: String,
    #[schemars(description = "The prompt used to generate the code to be stored as a checkpoint")]
    pub prompt: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectStatusRequest {
    #[schemars(description = "Absolute path to the project root")]
    pub project_dir: String,
}

#[tool(tool_box)]
impl ServerHandler for PublicMcp {
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
