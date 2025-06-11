use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{CallToolResult, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool,
};

pub mod commit;
pub mod project;
pub mod stack;
pub mod status;

pub(crate) const UI_CONTEXT_LINES: u32 = 3;

pub(crate) async fn start() -> Result<()> {
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let server = Mcp::new();
    let service = server.serve(transport).await?;
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

    #[tool(description = "Get the status of a project.
        This contains information about the branches applied, uncommitted file changes and any uncommitted changes assigned to the branches .")]
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

    #[tool(description = "Commit changes to the repository.
        Applies the given diff spec and creates a commit with the provided message.")]
    pub fn commit(
        &self,
        #[tool(aggr)] params: CommitParams,
    ) -> Result<CallToolResult, rmcp::Error> {
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let outcome = crate::mcp_internal::commit::commit(
            &project_path,
            params.message,
            params.diff_spec,
            params.parent_id,
            params.branch_name,
        )
        .map_err(|e| rmcp::Error::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            outcome,
        )?]))
    }

    #[tool(description = "Amend an existing commit in the repository.
        Updates the commit message and file changes for the specified commit.")]
    pub fn amend(&self, #[tool(aggr)] params: AmendParams) -> Result<CallToolResult, rmcp::Error> {
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let outcome = crate::mcp_internal::commit::amend(
            &project_path,
            params.message,
            params.diff_spec,
            params.commit_id,
            params.branch_name,
        )
        .map_err(|e| rmcp::Error::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            outcome,
        )?]))
    }

    #[tool(description = "Get details for a specific branch in the repository.")]
    pub fn branch_details(
        &self,
        #[tool(aggr)] params: BranchDetailsParams,
    ) -> Result<CallToolResult, rmcp::Error> {
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let details =
            crate::mcp_internal::stack::branch_details(&params.branch_name, &project_path)
                .map_err(|e| rmcp::Error::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            details,
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

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitParams {
    #[schemars(description = "The full root path of the Git project to commit in")]
    pub current_working_directory: String,

    #[schemars(description = "The commit message")]
    pub message: String,

    #[schemars(
        description = "The list of files paths (and optionally their previous paths) to commit.
        If the previous path is provided, it indicates a rename operation."
    )]
    pub diff_spec: Vec<crate::mcp_internal::commit::DiffSpec>,

    #[schemars(description = "Optional parent commit id.
        If provided, the commit will be created as a child of this commit.
        Otherwise, it will be created on top of the specified branch.")]
    pub parent_id: Option<String>,

    #[schemars(description = "The branch name to commit to")]
    pub branch_name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AmendParams {
    #[schemars(description = "The full root path of the Git project to amend in")]
    pub current_working_directory: String,

    #[schemars(description = "The new commit message for the amended commit")]
    pub message: String,

    #[schemars(
        description = "The list of file paths (and optionally their previous paths) to include in the amended commit.
        If the previous path is provided, it indicates a rename operation."
    )]
    pub diff_spec: Vec<crate::mcp_internal::commit::DiffSpec>,

    #[schemars(description = "The commit id of the commit to amend. 
        This is the commit that will be modified with the new message and changes.")]
    pub commit_id: String,

    #[schemars(description = "The branch name to amend the commit on")]
    pub branch_name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BranchDetailsParams {
    #[schemars(description = "The full root path of the Git project to query")]
    pub current_working_directory: String,

    #[schemars(description = "The name of the branch to get details for")]
    pub branch_name: String,
}

#[tool(tool_box)]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("This is the GitButler MCP server.
            This provides tools and other context resources that allow you to interact with your project's version control.
            If enabled, these are the tools that should be used for any Git operations".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "GitButler MCP Server".into(),
                version: "1.0.0".into(),
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }
}
