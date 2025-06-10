use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
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
pub struct Mcp {}

#[tool(tool_box)]
impl Mcp {
    pub fn new() -> Self {
        Self {}
    }

    #[tool(
        description = "Get the status of a project. This contains information about the branches applied and uncommitted file changes."
    )]
    pub fn project_status(
        &self,
        #[tool(aggr)] params: ProjectStatusParams,
    ) -> Result<CallToolResult, rmcp::Error> {
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let status = crate::mcp_internal::status::project_status(&project_path)
            .map_err(|e| rmcp::Error::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            status,
        )?]))
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatusParams {
    #[schemars(
        description = "The full root path of the Git project the agent is actively working in"
    )]
    pub current_working_directory: String,
}

#[tool(tool_box)]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("This is the GitButler MCP server. This provides tools and other context resources that allow you to interact with your project's version control. If enabled, these are the tools that should be used for any Git operations".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "GitButler MCP Server".into(),
                version: "1.0.0".into(),
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }
}
