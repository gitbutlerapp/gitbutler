use std::sync::{Arc, Mutex};

use anyhow::Result;
use but_settings::AppSettings;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::{
        router::{prompt::PromptRouter, tool::ToolRouter},
        wrapper::Parameters,
    },
    model::{
        CallToolResult, GetPromptRequestParam, GetPromptResult, Implementation, ListPromptsResult,
        PaginatedRequestParam, PromptMessage, PromptMessageContent,
        PromptMessageRole, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    prompt, prompt_handler, prompt_router, schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use tracing_subscriber::{self, EnvFilter};

use crate::metrics::{Event, EventKind, Metrics};

pub mod commit;
pub mod project;
pub mod stack;
pub mod status;

pub(crate) async fn start(app_settings: AppSettings) -> Result<()> {
    // Initialize the tracing subscriber with file and stdout logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting MCP server");

    let client_info = Arc::new(Mutex::new(None));
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = Mcp::new(app_settings, client_info.clone())
        .serve(transport)
        .await?;
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
    metrics: Metrics,
    client_info: Arc<Mutex<Option<Implementation>>>,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl Mcp {
    pub fn new(app_settings: AppSettings, client_info: Arc<Mutex<Option<Implementation>>>) -> Self {
        let metrics = Metrics::new_with_background_handling(&app_settings);
        Self {
            metrics,
            client_info,
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }
}

#[tool_router]
impl Mcp {
    #[tool(description = "Get the status of a project.
        This contains information about the branches applied, uncommitted file changes and any uncommitted changes assigned to the branches .")]
    pub fn project_status(
        &self,
        Parameters(params): Parameters<ProjectStatusParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let client_info = self
            .client_info
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            .clone();

        let start_time = std::time::Instant::now();
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let status = crate::mcp_internal::status::project_status(&project_path)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let event = &mut Event::new(EventKind::McpInternal);
        event.insert_prop("endpoint", "project_status");
        event.insert_prop("durationMs", start_time.elapsed().as_millis());
        event.insert_prop("clientName", client_info.clone().map(|i| i.name));
        event.insert_prop("clientVersion", client_info.clone().map(|i| i.version));
        self.metrics.capture(event);

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            status,
        )?]))
    }

    #[tool(description = "Commit changes to the repository.
        Applies the given diff spec and creates a commit with the provided message.")]
    pub fn commit(
        &self,
        Parameters(params): Parameters<CommitParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let client_info = self
            .client_info
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            .clone();

        let start_time = std::time::Instant::now();
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let outcome = crate::mcp_internal::commit::commit(
            &project_path,
            params.message,
            params.diff_spec,
            params.parent_id,
            params.branch_name,
        )
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let event = &mut Event::new(EventKind::McpInternal);
        event.insert_prop("endpoint", "commit");
        event.insert_prop("durationMs", start_time.elapsed().as_millis());
        event.insert_prop("clientName", client_info.clone().map(|i| i.name));
        event.insert_prop("clientVersion", client_info.clone().map(|i| i.version));
        self.metrics.capture(event);

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            outcome,
        )?]))
    }

    #[tool(description = "Amend an existing commit in the repository.
        Updates the commit message and file changes for the specified commit.")]
    pub fn amend(&self, Parameters(params): Parameters<AmendParams>) -> Result<CallToolResult, rmcp::ErrorData> {
        let client_info = self
            .client_info
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            .clone();

        let start_time = std::time::Instant::now();
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let outcome = crate::mcp_internal::commit::amend(
            &project_path,
            params.message,
            params.diff_spec,
            params.commit_id,
            params.branch_name,
        )
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let event = &mut Event::new(EventKind::McpInternal);
        event.insert_prop("endpoint", "amend");
        event.insert_prop("durationMs", start_time.elapsed().as_millis());
        event.insert_prop("clientName", client_info.clone().map(|i| i.name));
        event.insert_prop("clientVersion", client_info.clone().map(|i| i.version));
        self.metrics.capture(event);

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            outcome,
        )?]))
    }

    #[tool(description = "Get details for a specific branch in the repository.")]
    pub fn branch_details(
        &self,
        Parameters(params): Parameters<BranchDetailsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let client_info = self
            .client_info
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            .clone();

        let start_time = std::time::Instant::now();
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let details =
            crate::mcp_internal::stack::branch_details(&params.branch_name, &project_path)
                .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let event = &mut Event::new(EventKind::McpInternal);
        event.insert_prop("endpoint", "branch_details");
        event.insert_prop("durationMs", start_time.elapsed().as_millis());
        event.insert_prop("clientName", client_info.clone().map(|i| i.name));
        event.insert_prop("clientVersion", client_info.clone().map(|i| i.version));
        self.metrics.capture(event);

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            details,
        )?]))
    }

    #[tool(description = "Create a new branch in the repository.
        This will create a new stack containing only a branch with the given name and description.")]
    pub fn create_branch(
        &self,
        Parameters(params): Parameters<CreateBranchParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let client_info = self
            .client_info
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            .clone();

        let start_time = std::time::Instant::now();
        let project_path = std::path::PathBuf::from(&params.current_working_directory);
        let stack_entry = crate::mcp_internal::stack::create_stack_with_branch(
            &params.branch_name,
            &params.description,
            &project_path,
        )
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let event = &mut Event::new(EventKind::McpInternal);
        event.insert_prop("endpoint", "create_branch");
        event.insert_prop("durationMs", start_time.elapsed().as_millis());
        event.insert_prop("clientName", client_info.clone().map(|i| i.name));
        event.insert_prop("clientVersion", client_info.clone().map(|i| i.version));
        self.metrics.capture(event);

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            stack_entry,
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

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchParams {
    #[schemars(description = "The full root path of the Git project to create the branch in")]
    pub current_working_directory: String,

    #[schemars(description = "The name of the new branch to create")]
    pub branch_name: String,

    #[schemars(description = "Description of the branch.
    It's important to be detailed about the branch's purpose and any relevant context so that it's easy to determine where changes should be applied.")]
    pub description: String,
}

#[prompt_router]
impl Mcp {
    #[prompt(
        name = "handle_changes",
        description = "Contains the recommended steps to handle file changes in the project in order to commit them"
    )]
    async fn handle_changes_prompt(&self) -> Result<GetPromptResult, rmcp::ErrorData> {
        let prompt = "Handle the file changes following the steps below:
1. Take a look at the **project status**. Understand the branches applied (if any), the uncommitted file changes and the files assigned to them.
2. Determine which file changes should be committed together. Try to be granular and commit only the changes that are related to each other.
3. Determine which file changes belong to which branch. Do this by looking at the file changes and the branch names and descriptions. If no branch matches the changes create a new branch with a descriptive name and a detailed description.
4. Determine if some changes should be **amended** to an existing commit. Do this by looking a the **branch details** and its commits. If so, use the amend tool to update the commit with the new changes. Otherwise, use the commit tool to create a new commit with the changes.
4. Be descriptive in your commit messages. Explain what the changes are, not why.
5. If you are not sure about the changes, ask for clarification. Otherwise, proceed with committing the changes.
                ";

        Ok(GetPromptResult {
            description: None,
            messages: vec![PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text(prompt),
            }],
        })
    }

    #[prompt(
        name = "handle_changes_interactively",
        description = "Interactively handle file changes in the project to commit them"
    )]
    async fn handle_changes_interactively_prompt(&self) -> Result<GetPromptResult, rmcp::ErrorData> {
        let prompt = "Handle the file changes following the steps below:
1. Take a look at the **project status**. Understand the branches applied (if any), the uncommitted file changes and the files assigned to them.
2. If there are no branches applied, ask me about the intent of the changes and create a new branch with a descriptive name and a detailed description.
3. Ask me questions about the file changes. Focus on understanding the intent of the changes, and match that alongside the changes to the branches applied (if any).
4. Based on the answers, determine which file changes should be committed together. Try to be granular and commit only the changes that are related to each other.
5. If the changes are related to an existing branch, take a look at the **branch details** and its commits. Determine which commit the changes should be applied to.
6. Propose a commit message that describes the changes. The message should explain what the changes are, not why.
7. If accepted, commit (or amend) the changes using the respective tool.
8. Go back to step 1 and continue until all uncommitted changes have been handled.
                ";

        Ok(GetPromptResult {
            description: None,
            messages: vec![PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text(prompt),
            }],
        })
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("This is the GitButler MCP server.
            This provides tools and other context resources that allow you to interact with your project's version control.
            If enabled, these are the tools that should be used for any Git operations".into()),
            capabilities: ServerCapabilities::builder().enable_tools().enable_prompts().build(),
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
