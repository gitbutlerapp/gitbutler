use std::path::{Path, PathBuf};

use anyhow::Result;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use rmcp::{
    Error as McpError, ServerHandler, ServiceExt,
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool,
};

pub(crate) async fn start(repo_path: &Path) -> Result<()> {
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = Mcp::new(repo_path.to_path_buf()).serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Mcp {
    project: Project,
}

#[tool(tool_box)]
impl Mcp {
    pub fn new(repo_path: PathBuf) -> Self {
        let project = Project::from_path(&repo_path).expect("Failed to create project from path");
        Self { project }
    }

    #[tool(description = "Handle the changes that are currently uncommitted for the repository.")]
    pub fn handle_changes(
        &self,
        #[tool(aggr)] request: HandleChangesRequest,
    ) -> Result<CallToolResult, McpError> {
        if request.context.is_empty() {
            return Err(McpError::invalid_request(
                "Context cannot be empty".to_string(),
                None,
            ));
        }
        let ctx = &mut CommandContext::open(&self.project, AppSettings::default())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let response = but_auto::handle_changes_simple(ctx, &request.context)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::json(response)?]))
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct HandleChangesRequest {
    #[schemars(
        description = "Information about what has changed and why - i.e. the user prompt etc."
    )]
    pub context: String,
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
