use std::path::{Path, PathBuf};

use anyhow::Result;
use gitbutler_project::Project;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool,
};

pub mod project;
pub mod status;

pub(crate) const UI_CONTEXT_LINES: u32 = 3;

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

    #[tool(
        description = "Get the status of a project. This contains information about the branches applied and uncommitted file changes."
    )]
    pub fn project_status(&self) -> Result<CallToolResult, rmcp::Error> {
        let status = crate::mcp_internal::status::project_status(&self.project.path)
            .map_err(|e| rmcp::Error::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            status,
        )?]))
    }
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
