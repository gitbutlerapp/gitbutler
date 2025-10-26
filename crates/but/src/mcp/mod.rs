use std::{
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};

mod event;
use crate::metrics::{Event, EventKind, Metrics};
use anyhow::Result;
use but_action::{ActionHandler, Outcome, Source, reword::CommitEvent};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_router,
};

pub(crate) async fn start(app_settings: AppSettings) -> Result<()> {
    // Use `-t` to enable logging
    tracing::info!("Starting MCP server");

    let client_info = Arc::new(Mutex::new(None));
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = Mcp::new(app_settings, client_info.clone())
        .serve(transport)
        .await?;
    let info = service.peer_info();
    if let Ok(mut guard) = client_info.lock() {
        *guard = info.map(|i| i.client_info.clone());
    }
    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Mcp {
    app_settings: AppSettings,
    metrics: Metrics,
    client_info: Arc<Mutex<Option<Implementation>>>,
    event_handler: event::Handler,
    _tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Mcp {
    pub fn new(app_settings: AppSettings, client_info: Arc<Mutex<Option<Implementation>>>) -> Self {
        let metrics = Metrics::new_with_background_handling(&app_settings);
        let event_handler = event::Handler::new_with_background_handling();
        Self {
            app_settings,
            metrics,
            client_info,
            event_handler,
            _tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Update commits on the current branch based on the prompt used to modify the codebase and a summary of the changes made."
    )]
    pub fn gitbutler_update_branches(
        &self,
        request: Parameters<GitButlerUpdateBranchesRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let client_info = self
            .client_info
            .lock()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
            .clone();
        let start_time = std::time::Instant::now();
        let result = self.gitbutler_update_branches_inner(request.0.clone(), &client_info);
        let error = result.as_ref().err().map(|e| e.to_string());
        let updated_branches_count = result
            .as_ref()
            .ok()
            .map(|outcome| outcome.updated_branches.len());
        let commits_count = result.as_ref().ok().and_then(|outcome| {
            outcome
                .updated_branches
                .iter()
                .map(|branch| branch.new_commits.len())
                .sum::<usize>()
                .into()
        });
        let event = &mut Event::new(EventKind::Mcp);
        event.insert_prop("endpoint", "gitbutler_update_branches");
        event.insert_prop("aiCredentialsKind", self.event_handler.credentials_kind());
        event.insert_prop("durationMs", start_time.elapsed().as_millis());
        event.insert_prop("error", error);
        event.insert_prop("updatedBranchesCount", updated_branches_count);
        event.insert_prop("commitsCreatedCount", commits_count);
        event.insert_prop("clientName", client_info.clone().map(|i| i.name));
        event.insert_prop("clientVersion", client_info.clone().map(|i| i.version));
        self.metrics.capture(event);

        result.map(|outcome| Ok(CallToolResult::success(vec![Content::json(outcome)?])))?
    }

    fn gitbutler_update_branches_inner(
        &self,
        request: GitButlerUpdateBranchesRequest,
        client_info: &Option<Implementation>,
    ) -> Result<Outcome, rmcp::ErrorData> {
        if request.changes_summary.is_empty() {
            return Err(rmcp::ErrorData::invalid_request(
                "ChangesSummary cannot be empty".to_string(),
                None,
            ));
        }
        if request.full_prompt.is_empty() {
            return Err(rmcp::ErrorData::invalid_request(
                "FullPrompt cannot be empty".to_string(),
                None,
            ));
        }
        if request.current_working_directory.is_empty() {
            return Err(rmcp::ErrorData::invalid_request(
                "CurrentWorkingDirectory cannot be empty".to_string(),
                None,
            ));
        }

        let repo_path = PathBuf::from(request.current_working_directory.clone());
        let project = Project::from_path(&repo_path).expect("Failed to create project from path");
        let settings = AppSettings::load_from_default_path_creating()
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        let ctx = &mut CommandContext::open(&project, settings)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let (id, outcome) = but_action::handle_changes(
            ctx,
            &request.changes_summary,
            Some(request.full_prompt.clone()),
            ActionHandler::HandleChangesSimple,
            Source::Mcp(client_info.clone().map(Into::into)),
            None,
        )
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        // Trigger commit message generation for newly created commits
        for branch in &outcome.updated_branches {
            for commit in &branch.new_commits {
                if let Ok(commit_id) = gix::ObjectId::from_str(commit) {
                    let commit_event = CommitEvent {
                        external_summary: request.changes_summary.clone(),
                        external_prompt: request.full_prompt.clone(),
                        branch_name: branch.branch_name.clone(),
                        commit_id,
                        project: project.clone(),
                        app_settings: self.app_settings.clone(),
                        trigger: id,
                    };
                    self.event_handler.process_commit(commit_event);
                }
            }
        }
        Ok(outcome)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitButlerUpdateBranchesRequest {
    #[schemars(description = "The exact prompt that the user gave to generate these changes")]
    pub full_prompt: String,
    #[schemars(
        description = "A short bullet list of important things that were changed in the codebase and why"
    )]
    pub changes_summary: String,
    #[schemars(
        description = "The full root path of the Git project the agent is actively working in"
    )]
    pub current_working_directory: String,
}

impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("GitButler MCP server".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "GitButler MCP Server".into(),
                title: None,
                version: "1.0.0".into(),
                icons: None,
                website_url: Some("https://gitbutler.com".into()),
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }
}
