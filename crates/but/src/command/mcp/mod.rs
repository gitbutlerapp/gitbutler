use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use but_workspace::ui::workspace::{DetailedGraphRowData, DetailedGraphWorkspace};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{
        Annotated, CallToolResult, Content, ExtensionCapabilities, Implementation,
        ListResourcesResult, Meta, ProtocolVersion, RawResource, ReadResourceRequestParams,
        ReadResourceResult, Resource, ResourceContents, Root, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

const WORKSPACE_RESOURCE_URI: &str = "ui://gitbutler/workspace/v6.html";
const REVIEW_RESOURCE_URI: &str = "ui://gitbutler/review/v2.html";
const MCP_APP_MIME_TYPE: &str = "text/html;profile=mcp-app";
#[cfg(but_mcp_app_built)]
const WORKSPACE_HTML: &str = include_str!("workspace.html");
#[cfg(but_mcp_app_built)]
const REVIEW_HTML: &str = include_str!("review.html");
#[cfg(not(but_mcp_app_built))]
const WORKSPACE_HTML: &str = MCP_APP_NOT_BUILT_HTML;
#[cfg(not(but_mcp_app_built))]
const REVIEW_HTML: &str = MCP_APP_NOT_BUILT_HTML;
#[cfg(not(but_mcp_app_built))]
const MCP_APP_NOT_BUILT_HTML: &str = r#"<!doctype html>
<html lang="en">
<meta charset="utf-8">
<title>GitButler MCP app unavailable</title>
<p>This development build does not include the GitButler MCP app.</p>
</html>
"#;

/// Serve GitButler's MCP tools over standard input/output.
pub(crate) async fn serve() -> Result<()> {
    tracing::info!("Starting GitButler MCP server");
    let service = Mcp::new()
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Debug, Clone)]
struct Mcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Mcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "gitbutler_workspace",
        title = "View GitButler workspace",
        description = "Returns a GitButler workspace as stacks of branch references and commits. Pass the active repository path when it is known. Omit repository only when the MCP client is known to expose the desired repository as a filesystem root.",
        annotations(
            title = "View GitButler workspace",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        meta = workspace_tool_meta()
    )]
    async fn gitbutler_workspace(
        &self,
        Parameters(request): Parameters<WorkspaceRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let result = workspace_view_for_request(request, context)
            .await
            .and_then(workspace_view_result);
        Ok(match result {
            Ok(result) => result,
            Err(err) => CallToolResult::error(vec![Content::text(format!(
                "Could not read the GitButler workspace: {err:#}"
            ))]),
        })
    }

    #[tool(
        name = "gitbutler_commit_details",
        title = "Load GitButler commit details",
        description = "Loads metadata, changed files, and line statistics for a commit selected in the GitButler workspace view.",
        annotations(
            title = "Load GitButler commit details",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        meta = workspace_action_tool_meta()
    )]
    async fn gitbutler_commit_details(
        &self,
        Parameters(request): Parameters<CommitDetailsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = commit_details(request).and_then(|view| {
            let message = format!("Loaded details for commit {}.", view.details.commit.id);
            structured_tool_result(message, view)
        });
        Ok(match result {
            Ok(result) => result,
            Err(err) => CallToolResult::error(vec![Content::text(format!(
                "Could not load the commit details: {err:#}"
            ))]),
        })
    }

    #[tool(
        name = "gitbutler_branch_details",
        title = "Load GitButler branch details",
        description = "Loads commit, upstream, and push information for a branch selected in the GitButler workspace view.",
        annotations(
            title = "Load GitButler branch details",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        meta = workspace_action_tool_meta()
    )]
    async fn gitbutler_branch_details(
        &self,
        Parameters(request): Parameters<BranchDetailsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = branch_details(request).and_then(|view| {
            let message = format!("Loaded details for branch {}.", view.details.name);
            structured_tool_result(message, view)
        });
        Ok(match result {
            Ok(result) => result,
            Err(err) => CallToolResult::error(vec![Content::text(format!(
                "Could not load the branch details: {err:#}"
            ))]),
        })
    }

    #[tool(
        name = "gitbutler_review_card",
        title = "Show GitButler review",
        description = "Displays cards for pull requests or merge requests. Call this after `but pr new` with the review numbers returned by the command. Omit repository to use the first applicable filesystem root supplied by the MCP client.",
        annotations(
            title = "Show GitButler review",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        ),
        meta = review_card_tool_meta()
    )]
    async fn gitbutler_review_card(
        &self,
        Parameters(request): Parameters<ReviewCardRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let result = review_view_for_request(request, context)
            .await
            .and_then(|view| review_view_result(view, "Loaded"));
        Ok(match result {
            Ok(result) => result,
            Err(err) => CallToolResult::error(vec![Content::text(format!(
                "Could not load the GitButler review: {err:#}"
            ))]),
        })
    }

    #[tool(
        name = "gitbutler_refresh_reviews",
        title = "Refresh GitButler reviews",
        description = "Refreshes pull request or merge request state and CI status for the GitButler review card.",
        annotations(
            title = "Refresh GitButler reviews",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        ),
        meta = review_action_tool_meta()
    )]
    async fn gitbutler_refresh_reviews(
        &self,
        Parameters(request): Parameters<RefreshReviewsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result =
            refresh_reviews(request).and_then(|view| review_view_result(view, "Refreshed"));
        Ok(match result {
            Ok(result) => result,
            Err(err) => CallToolResult::error(vec![Content::text(format!(
                "Could not refresh the GitButler reviews: {err:#}"
            ))]),
        })
    }

    #[tool(
        name = "gitbutler_mark_review_ready",
        title = "Mark review ready",
        description = "Marks an open draft pull request or merge request as ready for review. This tool is called by the GitButler review card.",
        annotations(
            title = "Mark review ready",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        ),
        meta = review_action_tool_meta()
    )]
    async fn gitbutler_mark_review_ready(
        &self,
        Parameters(request): Parameters<MarkReviewReadyRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = mark_review_ready(request)
            .await
            .and_then(|view| review_view_result(view, "Marked as ready"));
        Ok(match result {
            Ok(result) => result,
            Err(err) => CallToolResult::error(vec![Content::text(format!(
                "Could not mark the review as ready: {err:#}"
            ))]),
        })
    }
}

#[tool_handler]
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerInfo {
        let mut extensions = ExtensionCapabilities::new();
        extensions.insert(
            "io.modelcontextprotocol/ui".to_owned(),
            serde_json::from_value(json!({
                "mimeTypes": [MCP_APP_MIME_TYPE]
            }))
            .expect("MCP Apps capability is an object"),
        );

        ServerInfo {
            instructions: Some(
                "Use gitbutler_workspace to inspect a repository's current GitButler workspace. Pass the active repository path when it is available; omit it only when the client is known to expose that repository as a filesystem root. After `but pr new`, call gitbutler_review_card with the returned review numbers so the user can see the created reviews."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_extensions_with(extensions)
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "gitbutler".into(),
                title: Some("GitButler".into()),
                version: option_env!("VERSION").unwrap_or("dev").into(),
                description: Some("GitButler workspace tools and views".into()),
                icons: None,
                website_url: Some("https://gitbutler.com".into()),
            },
            protocol_version: ProtocolVersion::LATEST,
        }
    }

    fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListResourcesResult::with_all_items(vec![
            workspace_resource(),
            review_resource(),
        ])))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        std::future::ready(match request.uri.as_str() {
            WORKSPACE_RESOURCE_URI => Ok(ReadResourceResult {
                contents: vec![workspace_resource_contents()],
            }),
            REVIEW_RESOURCE_URI => Ok(ReadResourceResult {
                contents: vec![review_resource_contents()],
            }),
            _ => Err(McpError::invalid_params(
                format!("Unknown resource URI: {}", request.uri),
                None,
            )),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WorkspaceRequest {
    /// Active repository to inspect. Omit only when the MCP client exposes the
    /// desired repository as a filesystem root.
    repository: Option<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CommitDetailsRequest {
    /// Canonical repository path from the workspace result.
    repository: PathBuf,
    /// Full object ID of the selected commit.
    commit_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct BranchDetailsRequest {
    /// Canonical repository path from the workspace result.
    repository: PathBuf,
    /// Full reference name of the selected branch.
    branch: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ReviewCardRequest {
    /// Repository to inspect when the MCP client's filesystem roots do not
    /// identify the desired repository.
    repository: Option<PathBuf>,
    /// Pull request or merge request numbers returned by `but pr new`.
    review_numbers: Vec<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct RefreshReviewsRequest {
    /// Canonical repository path from the review-card result.
    repository: PathBuf,
    /// Pull request or merge request numbers to refresh.
    review_numbers: Vec<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct MarkReviewReadyRequest {
    /// Canonical repository path from the review-card result.
    repository: PathBuf,
    /// Pull request or merge request number to mark as ready.
    review_number: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceView {
    version: u8,
    repository: RepositoryView,
    summary: WorkspaceSummary,
    workspace: DetailedGraphWorkspace,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryView {
    name: String,
    path: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSummary {
    stacks: usize,
    branches: usize,
    commits: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitDetailsView {
    kind: &'static str,
    repository: RepositoryView,
    details: but_api::diff::json::CommitDetails,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BranchDetailsView {
    kind: &'static str,
    repository: RepositoryView,
    target: Option<String>,
    details: BranchSelectionDetails,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BranchSelectionDetails {
    name: String,
    reference: String,
    remote_tracking_branch: Option<String>,
    tip: String,
    push_status: Option<but_workspace::ui::PushStatus>,
    last_updated_at: Option<i128>,
    commits: usize,
    is_conflicted: bool,
}

struct ResolvedRepository {
    ctx: but_ctx::Context,
    repository: RepositoryView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewView {
    version: u8,
    repository: RepositoryView,
    forge: but_forge::ForgeInfo,
    reviews: Vec<ReviewCard>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewCard {
    number: i64,
    title: String,
    url: String,
    state: ReviewState,
    source_branch: String,
    target_branch: String,
    author: Option<ReviewPerson>,
    reviewers: Vec<ReviewPerson>,
    labels: Vec<String>,
    created_at: Option<String>,
    can_mark_ready: bool,
    ci: ReviewCi,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewPerson {
    login: String,
    name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum ReviewState {
    Draft,
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewCi {
    status: ReviewCiStatus,
    total: usize,
    passing: usize,
    pending: usize,
    failing: usize,
    failing_check_names: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum ReviewCiStatus {
    Unsupported,
    NoChecks,
    InProgress,
    Success,
    Failure,
    ActionRequired,
    Cancelled,
    Unknown,
    Unavailable,
}

async fn workspace_view_for_request(
    request: WorkspaceRequest,
    context: RequestContext<RoleServer>,
) -> Result<WorkspaceView> {
    let resolved = resolve_repository(request.repository, context).await?;
    workspace_view_from_context(&resolved.ctx, &resolved.repository.path)
}

async fn resolve_repository(
    repository: Option<PathBuf>,
    context: RequestContext<RoleServer>,
) -> Result<ResolvedRepository> {
    if let Some(repository) = repository {
        return open_repository(&repository);
    }

    let peer = context.peer;
    let supports_roots = peer
        .peer_info()
        .is_some_and(|client| client.capabilities.roots.is_some());
    if !supports_roots {
        bail!(
            "No repository was provided and this MCP client does not expose filesystem roots. Pass the repository argument explicitly."
        );
    }

    let roots = peer
        .list_roots()
        .await
        .context("Could not request filesystem roots from the MCP client")?;
    repository_from_roots(&roots.roots)
}

fn repository_from_roots(roots: &[Root]) -> Result<ResolvedRepository> {
    if roots.is_empty() {
        bail!(
            "The MCP client did not provide any filesystem roots. Pass the repository argument explicitly."
        );
    }

    let mut failures = Vec::new();
    for root in roots {
        let path = match root_path(root) {
            Ok(path) => path,
            Err(err) => {
                failures.push(err.to_string());
                continue;
            }
        };
        match open_repository(&path) {
            Ok(resolved) => return Ok(resolved),
            Err(err) => failures.push(format!("{}: {err:#}", path.display())),
        }
    }

    bail!(
        "None of the MCP client's filesystem roots identify a Git repository. {}",
        failures.join("; ")
    )
}

fn root_path(root: &Root) -> Result<PathBuf> {
    let url = Url::parse(&root.uri)
        .with_context(|| format!("MCP root is not a valid URI: {}", root.uri))?;
    url.to_file_path()
        .map_err(|()| anyhow::anyhow!("MCP root is not a file URI: {}", root.uri))
}

fn open_repository(repository: &Path) -> Result<ResolvedRepository> {
    let repository = repository
        .canonicalize()
        .with_context(|| format!("Could not resolve repository at {}", repository.display()))?;
    let ctx = but_ctx::Context::discover(&repository)
        .with_context(|| format!("Could not open repository at {}", repository.display()))?;
    let repository = ctx
        .workdir_or_gitdir()
        .context("Could not determine the repository root")?
        .canonicalize()
        .context("Could not resolve the repository root")?;
    let name = repository
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repository")
        .to_owned();
    Ok(ResolvedRepository {
        ctx,
        repository: RepositoryView {
            name,
            path: repository,
        },
    })
}

fn workspace_view_from_context(ctx: &but_ctx::Context, repository: &Path) -> Result<WorkspaceView> {
    let guard = ctx.shared_worktree_access();
    let workspace = but_api::workspace::get_workspace(ctx, guard.read_permission())
        .context("Could not derive the detailed workspace graph")?;

    let mut branches = 0;
    let mut commits = 0;
    for row in workspace.stacks.iter().flat_map(|stack| &stack.rows) {
        match row.data {
            DetailedGraphRowData::Commit(_) => commits += 1,
            DetailedGraphRowData::Reference(_) => branches += 1,
        }
    }

    Ok(WorkspaceView {
        version: 1,
        repository: RepositoryView {
            name: repository
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("repository")
                .to_owned(),
            path: repository.to_owned(),
        },
        summary: WorkspaceSummary {
            stacks: workspace.stacks.len(),
            branches,
            commits,
        },
        workspace,
    })
}

fn workspace_view_result(view: WorkspaceView) -> Result<CallToolResult> {
    let structured_content = serde_json::to_value(&view)
        .context("Could not serialize the GitButler workspace for the MCP client")?;
    let text = format!(
        "GitButler workspace for {}: {} stacks, {} branches, {} commits.",
        view.repository.name, view.summary.stacks, view.summary.branches, view.summary.commits
    );
    Ok(CallToolResult {
        content: vec![Content::text(text)],
        structured_content: Some(structured_content),
        is_error: Some(false),
        meta: None,
    })
}

fn commit_details(request: CommitDetailsRequest) -> Result<CommitDetailsView> {
    let resolved = open_repository(&request.repository)?;
    let commit_id = request
        .commit_id
        .parse::<gix::ObjectId>()
        .with_context(|| format!("Invalid commit ID: {}", request.commit_id))?;
    let details = but_api::diff::commit_details_with_line_stats(&resolved.ctx, commit_id)
        .context("Could not derive commit details")?
        .into();

    Ok(CommitDetailsView {
        kind: "commit",
        repository: resolved.repository,
        details,
    })
}

fn branch_details(request: BranchDetailsRequest) -> Result<BranchDetailsView> {
    let resolved = open_repository(&request.repository)?;
    let branch_name = request
        .branch
        .strip_prefix("refs/heads/")
        .with_context(|| {
            format!(
                "Only local branch references are supported, got {}",
                request.branch
            )
        })?
        .to_owned();
    let target = resolved
        .ctx
        .project_meta()?
        .target_ref
        .map(|name| String::from_utf8_lossy(name.shorten()).into_owned());
    let guard = resolved.ctx.shared_worktree_access();
    let workspace = but_api::workspace::get_workspace(&resolved.ctx, guard.read_permission())
        .context("Could not derive the detailed workspace graph")?;
    let (reference, commits) = workspace
        .stacks
        .iter()
        .find_map(|stack| {
            stack.reference_segments.iter().find_map(|segment| {
                let row = stack.rows.get(segment.reference_idx)?;
                let DetailedGraphRowData::Reference(reference) = &row.data else {
                    return None;
                };
                (reference.ref_name.full_name == request.branch).then(|| {
                    let commits = segment
                        .row_idxs
                        .iter()
                        .filter_map(|row_idx| stack.rows.get(*row_idx))
                        .filter_map(|row| match &row.data {
                            DetailedGraphRowData::Commit(commit) => Some(commit),
                            DetailedGraphRowData::Reference(_) => None,
                        })
                        .collect::<Vec<_>>();
                    (reference, commits)
                })
            })
        })
        .with_context(|| format!("Branch is not present in the workspace: {}", request.branch))?;
    let tip = {
        let repo = resolved.ctx.repo.get()?;
        repo.find_reference(request.branch.as_str())?
            .peel_to_id()?
            .detach()
            .to_string()
    };
    let last_updated_at = commits.iter().map(|commit| commit.committed_at).max();
    let is_conflicted = commits.iter().any(|commit| commit.has_conflicts);
    let details = BranchSelectionDetails {
        name: branch_name.to_owned(),
        reference: request.branch,
        remote_tracking_branch: reference
            .status
            .as_ref()
            .and_then(|status| status.remote_ref.as_ref())
            .map(|remote| remote.full_name.clone()),
        tip,
        push_status: reference.status.as_ref().map(|status| status.push_status),
        last_updated_at,
        commits: commits.len(),
        is_conflicted,
    };

    Ok(BranchDetailsView {
        kind: "branch",
        repository: resolved.repository,
        target,
        details,
    })
}

async fn review_view_for_request(
    request: ReviewCardRequest,
    context: RequestContext<RoleServer>,
) -> Result<ReviewView> {
    if request.review_numbers.is_empty() {
        bail!("At least one review number is required");
    }
    let resolved = resolve_repository(request.repository, context).await?;
    review_view_from_repository(resolved, &request.review_numbers)
}

fn review_view(repository: &Path, review_numbers: &[usize]) -> Result<ReviewView> {
    let resolved = open_repository(repository)?;
    review_view_from_repository(resolved, review_numbers)
}

fn refresh_reviews(request: RefreshReviewsRequest) -> Result<ReviewView> {
    if request.review_numbers.is_empty() {
        bail!("At least one review number is required");
    }
    review_view(&request.repository, &request.review_numbers)
}

fn review_view_from_repository(
    resolved: ResolvedRepository,
    review_numbers: &[usize],
) -> Result<ReviewView> {
    let forge = ForgeRepository::from_context(&resolved.ctx)?;
    let reviews = review_numbers
        .iter()
        .map(|review_number| forge.get_review_card(&resolved.ctx, *review_number))
        .collect::<Result<Vec<_>>>()?;

    Ok(ReviewView {
        version: 2,
        repository: resolved.repository,
        forge: forge.display,
        reviews,
    })
}

async fn mark_review_ready(request: MarkReviewReadyRequest) -> Result<ReviewView> {
    let (preferred_user, repository, storage) = match prepare_mark_review_ready(&request)? {
        MarkReadyPreparation::AlreadyReady(view) => return Ok(view),
        MarkReadyPreparation::Update {
            preferred_user,
            repository,
            storage,
        } => (preferred_user, repository, storage),
    };

    but_forge::set_review_draftiness(
        &preferred_user,
        &repository,
        request.review_number,
        false,
        &storage,
    )
    .await?;

    review_view(&request.repository, &[request.review_number])
}

enum MarkReadyPreparation {
    AlreadyReady(ReviewView),
    Update {
        preferred_user: Option<but_forge::ForgeUser>,
        repository: but_forge::ForgeRepoInfo,
        storage: but_forge_storage::Controller,
    },
}

fn prepare_mark_review_ready(request: &MarkReviewReadyRequest) -> Result<MarkReadyPreparation> {
    let resolved = open_repository(&request.repository)?;
    let forge = ForgeRepository::from_context(&resolved.ctx)?;
    let review = forge.get_review(&resolved.ctx, request.review_number)?;
    if !review.is_open() {
        bail!(
            "{}{} is not open and cannot be marked ready",
            review.unit_symbol,
            review.number
        );
    }
    if !review.draft {
        return review_view_from_repository(resolved, &[request.review_number])
            .map(MarkReadyPreparation::AlreadyReady);
    }
    if !forge.display.capabilities.pr_service {
        bail!(
            "Marking reviews ready is not supported for {:?}",
            forge.display.name
        );
    }

    let ForgeRepository {
        preferred_user,
        repository,
        storage,
        ..
    } = forge;
    Ok(MarkReadyPreparation::Update {
        preferred_user,
        repository,
        storage,
    })
}

struct ForgeRepository {
    preferred_user: Option<but_forge::ForgeUser>,
    repository: but_forge::ForgeRepoInfo,
    display: but_forge::ForgeInfo,
    storage: but_forge_storage::Controller,
}

impl ForgeRepository {
    fn from_context(ctx: &but_ctx::Context) -> Result<Self> {
        let project_meta = ctx.project_meta()?;
        let repo = ctx.repo.get()?;
        let remote_url = project_meta.remote_url_with_fallback(&repo)?;
        let repository = but_forge::derive_forge_repo_info(&remote_url)
            .context("Could not determine a supported forge for this repository")?;
        let display = but_forge::forge_info(&remote_url)
            .context("Could not determine forge display information")?;
        let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
        #[cfg(feature = "legacy")]
        let preferred_user = ctx.legacy_project.preferred_forge_user.clone();
        #[cfg(not(feature = "legacy"))]
        let preferred_user = None;

        Ok(Self {
            preferred_user,
            repository,
            display,
            storage,
        })
    }

    fn get_review(
        &self,
        ctx: &but_ctx::Context,
        review_number: usize,
    ) -> Result<but_forge::ForgeReview> {
        let db = &mut *ctx.db.get_cache_mut()?;
        but_forge::get_forge_review(
            &self.preferred_user,
            &self.repository,
            review_number,
            db,
            &self.storage,
        )
    }

    fn get_review_card(&self, ctx: &but_ctx::Context, review_number: usize) -> Result<ReviewCard> {
        let review = self.get_review(ctx, review_number)?;
        let ci = self.ci_for_ref(ctx, &review.source_branch);
        Ok(ReviewCard::from_review(review, &self.display, ci))
    }

    fn ci_for_ref(&self, ctx: &but_ctx::Context, reference: &str) -> ReviewCi {
        if !self.display.capabilities.checks {
            return ReviewCi::unsupported();
        }

        let result = (|| {
            let db = &mut *ctx.db.get_cache_mut()?;
            but_forge::ci_checks_for_ref_with_cache(
                self.preferred_user.clone(),
                &self.repository,
                &self.storage,
                reference,
                db,
                Some(but_forge::CacheConfig::NoCache),
            )
        })();

        match result {
            Ok(checks) => ReviewCi::from_checks(checks),
            Err(err) => {
                tracing::warn!(
                    review_reference = reference,
                    error = %err,
                    "Could not load CI checks for MCP review card"
                );
                ReviewCi::unavailable()
            }
        }
    }
}

impl ReviewCard {
    fn from_review(
        review: but_forge::ForgeReview,
        forge: &but_forge::ForgeInfo,
        ci: ReviewCi,
    ) -> Self {
        let state = if review.is_merged() {
            ReviewState::Merged
        } else if !review.is_open() {
            ReviewState::Closed
        } else if review.draft {
            ReviewState::Draft
        } else {
            ReviewState::Open
        };
        let can_mark_ready = matches!(state, ReviewState::Draft) && forge.capabilities.pr_service;

        Self {
            number: review.number,
            title: review.title,
            url: review.html_url,
            state,
            source_branch: review.source_branch,
            target_branch: review.target_branch,
            author: review.author.map(|user| ReviewPerson {
                login: user.login,
                name: user.name,
            }),
            reviewers: review
                .reviewers
                .into_iter()
                .map(|user| ReviewPerson {
                    login: user.login,
                    name: user.name,
                })
                .collect(),
            labels: review.labels.into_iter().map(|label| label.name).collect(),
            created_at: review.created_at,
            can_mark_ready,
            ci,
        }
    }
}

impl ReviewCi {
    fn unsupported() -> Self {
        Self::empty(ReviewCiStatus::Unsupported)
    }

    fn no_checks() -> Self {
        Self::empty(ReviewCiStatus::NoChecks)
    }

    fn unavailable() -> Self {
        Self::empty(ReviewCiStatus::Unavailable)
    }

    fn empty(status: ReviewCiStatus) -> Self {
        Self {
            status,
            total: 0,
            passing: 0,
            pending: 0,
            failing: 0,
            failing_check_names: Vec::new(),
        }
    }

    fn from_checks(checks: Vec<but_forge::CiCheck>) -> Self {
        if checks.is_empty() {
            return Self::no_checks();
        }

        let total = checks.len();
        let mut passing = 0;
        let mut pending = 0;
        let mut failing = 0;
        let mut action_required = 0;
        let mut cancelled = 0;
        let mut unknown = 0;
        let mut failing_check_names = Vec::new();

        for check in checks {
            match check.status {
                but_forge::CiStatus::InProgress | but_forge::CiStatus::Queued => pending += 1,
                but_forge::CiStatus::Unknown => unknown += 1,
                but_forge::CiStatus::Complete { conclusion, .. } => match conclusion {
                    but_forge::CiConclusion::Failure | but_forge::CiConclusion::TimedOut => {
                        failing += 1;
                        failing_check_names.push(check.name);
                    }
                    but_forge::CiConclusion::ActionRequired => action_required += 1,
                    but_forge::CiConclusion::Cancelled => cancelled += 1,
                    but_forge::CiConclusion::Success
                    | but_forge::CiConclusion::Neutral
                    | but_forge::CiConclusion::Skipped => passing += 1,
                    but_forge::CiConclusion::Unknown => unknown += 1,
                },
            }
        }

        let status = if failing > 0 {
            ReviewCiStatus::Failure
        } else if action_required > 0 {
            ReviewCiStatus::ActionRequired
        } else if cancelled > 0 {
            ReviewCiStatus::Cancelled
        } else if pending > 0 {
            ReviewCiStatus::InProgress
        } else if passing > 0 {
            ReviewCiStatus::Success
        } else if unknown > 0 {
            ReviewCiStatus::Unknown
        } else {
            ReviewCiStatus::NoChecks
        };

        Self {
            status,
            total,
            passing,
            pending,
            failing,
            failing_check_names,
        }
    }
}

fn review_view_result(view: ReviewView, action: &str) -> Result<CallToolResult> {
    let reviews = view
        .reviews
        .iter()
        .map(|review| {
            format!(
                "{}{} “{}” ({:?})",
                view.forge.unit.symbol, review.number, review.title, review.state
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    structured_tool_result(
        format!(
            "{action} {} review{} for {}: {reviews}.",
            view.forge.unit.name.to_lowercase(),
            if view.reviews.len() == 1 { "" } else { "s" },
            view.repository.name,
        ),
        view,
    )
}

fn structured_tool_result(message: String, value: impl Serialize) -> Result<CallToolResult> {
    Ok(CallToolResult {
        content: vec![Content::text(message)],
        structured_content: Some(
            serde_json::to_value(value).context("Could not serialize structured tool result")?,
        ),
        is_error: Some(false),
        meta: None,
    })
}

fn workspace_tool_meta() -> Meta {
    let mut meta = Meta::new();
    meta.0.insert(
        "ui".to_owned(),
        json!({
            "resourceUri": WORKSPACE_RESOURCE_URI,
            "visibility": ["model", "app"]
        }),
    );
    // Compatibility alias for hosts that implemented ChatGPT Apps before MCP Apps.
    meta.0.insert(
        "openai/outputTemplate".to_owned(),
        json!(WORKSPACE_RESOURCE_URI),
    );
    meta.0
        .insert("openai/widgetAccessible".to_owned(), json!(true));
    meta
}

fn workspace_action_tool_meta() -> Meta {
    let mut meta = Meta::new();
    meta.0.insert(
        "ui".to_owned(),
        json!({
            "visibility": ["app"]
        }),
    );
    meta
}

fn review_card_tool_meta() -> Meta {
    let mut meta = Meta::new();
    meta.0.insert(
        "ui".to_owned(),
        json!({
            "resourceUri": REVIEW_RESOURCE_URI,
            "visibility": ["model", "app"]
        }),
    );
    // Compatibility aliases for hosts that implemented ChatGPT Apps before MCP Apps.
    meta.0.insert(
        "openai/outputTemplate".to_owned(),
        json!(REVIEW_RESOURCE_URI),
    );
    meta.0
        .insert("openai/widgetAccessible".to_owned(), json!(true));
    meta
}

fn review_action_tool_meta() -> Meta {
    let mut meta = Meta::new();
    meta.0.insert(
        "ui".to_owned(),
        json!({
            "visibility": ["app"]
        }),
    );
    meta
}

fn workspace_resource() -> Resource {
    let mut resource = RawResource::new(WORKSPACE_RESOURCE_URI, "gitbutler_workspace");
    resource.title = Some("GitButler workspace".to_owned());
    resource.description =
        Some("Interactive view of GitButler stacks, branches, and commits".into());
    resource.mime_type = Some(MCP_APP_MIME_TYPE.to_owned());
    resource.size = u32::try_from(WORKSPACE_HTML.len()).ok();
    Annotated::new(resource, None)
}

fn review_resource() -> Resource {
    let mut resource = RawResource::new(REVIEW_RESOURCE_URI, "gitbutler_review_card");
    resource.title = Some("GitButler review".to_owned());
    resource.description = Some("Interactive cards for pull requests and merge requests".into());
    resource.mime_type = Some(MCP_APP_MIME_TYPE.to_owned());
    resource.size = u32::try_from(REVIEW_HTML.len()).ok();
    Annotated::new(resource, None)
}

fn workspace_resource_contents() -> ResourceContents {
    let mut meta = Meta::new();
    meta.0.insert(
        "ui".to_owned(),
        json!({
            "prefersBorder": true,
            "permissions": {
                "clipboardWrite": {}
            }
        }),
    );
    ResourceContents::TextResourceContents {
        uri: WORKSPACE_RESOURCE_URI.to_owned(),
        mime_type: Some(MCP_APP_MIME_TYPE.to_owned()),
        text: WORKSPACE_HTML.to_owned(),
        meta: Some(meta),
    }
}

fn review_resource_contents() -> ResourceContents {
    let mut meta = Meta::new();
    meta.0.insert(
        "ui".to_owned(),
        json!({
            "prefersBorder": true
        }),
    );
    ResourceContents::TextResourceContents {
        uri: REVIEW_RESOURCE_URI.to_owned(),
        mime_type: Some(MCP_APP_MIME_TYPE.to_owned()),
        text: REVIEW_HTML.to_owned(),
        meta: Some(meta),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_tool_links_to_mcp_app_resource() {
        let server = Mcp::new();
        let tool = server
            .tool_router
            .get("gitbutler_workspace")
            .expect("workspace tool is registered");
        let serialized = serde_json::to_value(tool).expect("tool serializes");

        assert_eq!(
            serialized["_meta"]["ui"]["resourceUri"], WORKSPACE_RESOURCE_URI,
            "workspace tool points at its MCP App resource"
        );
        assert_eq!(
            serialized["_meta"]["ui"]["visibility"],
            json!(["model", "app"]),
            "the model and app can load the workspace"
        );
        assert_eq!(
            serialized["annotations"]["readOnlyHint"], true,
            "workspace tool is advertised as read-only"
        );
        assert_eq!(
            serialized["inputSchema"]["properties"]["repository"]["type"], "string",
            "workspace tool accepts an optional repository path"
        );
        assert!(
            serialized["inputSchema"]["required"]
                .as_array()
                .is_none_or(|required| !required.iter().any(|field| field == "repository")),
            "repository is not required because MCP roots are preferred"
        );
    }

    #[test]
    fn resources_use_the_mcp_app_contract() {
        let workspace =
            serde_json::to_value(workspace_resource_contents()).expect("resource serializes");
        assert_eq!(workspace["mimeType"], MCP_APP_MIME_TYPE);
        assert_eq!(workspace["_meta"]["ui"]["prefersBorder"], true);
        assert_eq!(
            workspace["_meta"]["ui"]["permissions"]["clipboardWrite"],
            json!({})
        );

        let review = serde_json::to_value(review_resource_contents()).expect("resource serializes");
        assert_eq!(review["mimeType"], MCP_APP_MIME_TYPE);
        assert_eq!(review["_meta"]["ui"]["prefersBorder"], true);
    }

    #[test]
    #[cfg(but_mcp_app_built)]
    fn workspace_resource_is_a_self_contained_mcp_app() {
        let serialized =
            serde_json::to_value(workspace_resource_contents()).expect("resource serializes");

        assert_eq!(
            serialized["mimeType"], MCP_APP_MIME_TYPE,
            "resource uses the MCP App MIME type"
        );
        assert_eq!(
            serialized["_meta"]["ui"]["prefersBorder"], true,
            "resource asks the host for a visible boundary"
        );
        assert_eq!(
            serialized["_meta"]["ui"]["permissions"]["clipboardWrite"],
            json!({}),
            "resource requests clipboard access for copying identifiers"
        );
        assert!(
            WORKSPACE_HTML.contains("ui/initialize"),
            "app performs the standard MCP Apps handshake"
        );
        assert!(
            WORKSPACE_HTML.contains("ui/notifications/tool-result"),
            "app consumes standard MCP tool results"
        );
        assert!(
            WORKSPACE_HTML.contains("ui/update-model-context")
                && WORKSPACE_HTML.contains("ui/message"),
            "workspace selections and actions can be handed to the agent"
        );
        assert!(
            WORKSPACE_HTML.contains("gitbutler_commit_details")
                && WORKSPACE_HTML.contains("gitbutler_branch_details"),
            "workspace view loads selection details through app-only tools"
        );
        assert!(
            WORKSPACE_HTML.contains(r#"name="gitbutler-ui-framework" content="react""#),
            "workspace view is built from the React MCP App source"
        );
        assert!(
            WORKSPACE_HTML.contains("#root{min-width:0;padding:12px}"),
            "padding is applied inside the host-measured React root"
        );
        assert!(
            WORKSPACE_HTML.contains("font-family:Inter")
                && WORKSPACE_HTML.contains("font-family:Geist Mono"),
            "Lite's UI fonts are embedded in the app"
        );
    }

    #[test]
    fn workspace_detail_tools_are_read_only_and_app_only() {
        let server = Mcp::new();
        for tool_name in ["gitbutler_commit_details", "gitbutler_branch_details"] {
            let tool = server
                .tool_router
                .get(tool_name)
                .expect("workspace detail tool is registered");
            let serialized = serde_json::to_value(tool).expect("detail tool serializes");

            assert_eq!(
                serialized["_meta"]["ui"]["visibility"],
                json!(["app"]),
                "detail tools do not clutter the model-visible tool list"
            );
            assert_eq!(
                serialized["annotations"]["readOnlyHint"], true,
                "detail tools only inspect repository state"
            );
            assert_eq!(
                serialized["annotations"]["idempotentHint"], true,
                "reloading details is idempotent"
            );
        }
    }

    #[test]
    fn review_tools_have_separate_model_and_app_visibility() {
        let server = Mcp::new();
        let card = server
            .tool_router
            .get("gitbutler_review_card")
            .expect("review-card tool is registered");
        let card = serde_json::to_value(card).expect("review-card tool serializes");
        let action = server
            .tool_router
            .get("gitbutler_mark_review_ready")
            .expect("review action tool is registered");
        let action = serde_json::to_value(action).expect("review action tool serializes");
        let refresh = server
            .tool_router
            .get("gitbutler_refresh_reviews")
            .expect("review refresh tool is registered");
        let refresh = serde_json::to_value(refresh).expect("review refresh tool serializes");

        assert_eq!(
            card["_meta"]["ui"]["resourceUri"], REVIEW_RESOURCE_URI,
            "review-card tool points at its MCP App resource"
        );
        assert_eq!(
            card["_meta"]["ui"]["visibility"],
            json!(["model", "app"]),
            "the model and the app can load review cards"
        );
        assert_eq!(card["annotations"]["readOnlyHint"], true);
        assert_eq!(
            card["inputSchema"]["properties"]["reviewNumbers"]["type"],
            "array"
        );
        assert_eq!(
            action["_meta"]["ui"]["visibility"],
            json!(["app"]),
            "the mutation is only exposed to the rendered app"
        );
        assert_eq!(action["annotations"]["readOnlyHint"], false);
        assert_eq!(action["annotations"]["idempotentHint"], true);
        assert_eq!(
            refresh["_meta"]["ui"]["visibility"],
            json!(["app"]),
            "the polling tool is only exposed to the rendered app"
        );
        assert_eq!(refresh["annotations"]["readOnlyHint"], true);
        assert_eq!(refresh["annotations"]["idempotentHint"], true);
    }

    #[test]
    #[cfg(but_mcp_app_built)]
    fn review_resource_is_a_self_contained_react_mcp_app() {
        let serialized =
            serde_json::to_value(review_resource_contents()).expect("resource serializes");

        assert_eq!(serialized["mimeType"], MCP_APP_MIME_TYPE);
        assert_eq!(serialized["_meta"]["ui"]["prefersBorder"], true);
        assert!(
            REVIEW_HTML.contains("ui/initialize"),
            "app performs the standard MCP Apps handshake"
        );
        assert!(
            REVIEW_HTML.contains("gitbutler-mcp-view"),
            "review view is the generated review entry point"
        );
        assert!(
            REVIEW_HTML.contains("gitbutler_refresh_reviews"),
            "review view polls the app-only refresh tool"
        );
        assert!(
            REVIEW_HTML.contains(r#"name="gitbutler-ui-framework" content="react""#),
            "review view is built from the React MCP App source"
        );
        assert!(
            REVIEW_HTML.contains("#root{min-width:0;padding:12px}"),
            "padding is applied inside the host-measured React root"
        );
        assert!(
            REVIEW_HTML.contains("font-family:Inter")
                && REVIEW_HTML.contains("font-family:Geist Mono"),
            "Lite's UI fonts are embedded in the app"
        );
    }

    #[test]
    fn review_card_maps_forge_review_state_and_actions() {
        let forge = but_forge::forge_info("https://github.com/gitbutlerapp/gitbutler.git")
            .expect("GitHub is a supported forge");
        let mut review = forge_review_fixture();

        let draft = ReviewCard::from_review(review.clone(), &forge, ReviewCi::no_checks());
        assert!(matches!(draft.state, ReviewState::Draft));
        assert!(
            draft.can_mark_ready,
            "an open draft exposes the ready action"
        );

        review.draft = false;
        let open = ReviewCard::from_review(review.clone(), &forge, ReviewCi::no_checks());
        assert!(matches!(open.state, ReviewState::Open));
        assert!(!open.can_mark_ready);

        review.merged_at = Some("2026-07-24T12:00:00Z".into());
        let merged = ReviewCard::from_review(review, &forge, ReviewCi::no_checks());
        assert!(matches!(merged.state, ReviewState::Merged));
        assert!(!merged.can_mark_ready);
    }

    #[test]
    fn review_ci_aggregates_checks_with_failure_precedence() {
        let checks = vec![
            ci_check_fixture(
                "build",
                but_forge::CiStatus::Complete {
                    conclusion: but_forge::CiConclusion::Success,
                    completed_at: None,
                },
            ),
            ci_check_fixture("test", but_forge::CiStatus::InProgress),
            ci_check_fixture(
                "lint",
                but_forge::CiStatus::Complete {
                    conclusion: but_forge::CiConclusion::Failure,
                    completed_at: None,
                },
            ),
        ];

        let ci = ReviewCi::from_checks(checks);

        assert!(matches!(ci.status, ReviewCiStatus::Failure));
        assert_eq!(ci.total, 3);
        assert_eq!(ci.passing, 1);
        assert_eq!(ci.pending, 1);
        assert_eq!(ci.failing, 1);
        assert_eq!(ci.failing_check_names, ["lint"]);
    }

    #[test]
    fn workspace_view_uses_the_current_workspace_api() -> anyhow::Result<()> {
        let env = but_testsupport::Sandbox::open_or_init_scenario_with_target_and_default_settings(
            "one-stack",
        );
        let ctx = env.context();

        let view = workspace_view_from_context(&ctx, env.projects_root())?;

        assert_eq!(view.version, 1, "workspace response has a stable version");
        assert!(
            view.summary.stacks > 0,
            "one-stack fixture yields a visible stack"
        );
        assert!(
            view.summary.branches > 0,
            "one-stack fixture yields a visible branch"
        );
        let result = workspace_view_result(view)?;
        assert!(
            result.structured_content.is_some(),
            "a successful workspace result always includes structured content"
        );
        Ok(())
    }

    #[test]
    fn commit_details_include_files_and_line_stats() -> anyhow::Result<()> {
        but_testsupport::isolated_app_data_dir(|| {
            let env =
                but_testsupport::Sandbox::open_or_init_scenario_with_target_and_default_settings(
                    "one-stack",
                );
            let ctx = env.context();
            let workspace = workspace_view_from_context(&ctx, env.projects_root())?;
            let commit_id = workspace
                .workspace
                .stacks
                .iter()
                .flat_map(|stack| &stack.rows)
                .find_map(|row| match &row.data {
                    DetailedGraphRowData::Commit(commit) => Some(commit.id.to_string()),
                    DetailedGraphRowData::Reference(_) => None,
                })
                .context("fixture has a commit")?;

            let view = commit_details(CommitDetailsRequest {
                repository: env.projects_root().to_owned(),
                commit_id,
            })?;

            assert_eq!(view.kind, "commit");
            assert!(
                view.details.line_stats.is_some(),
                "commit details compute line statistics for the details panel"
            );
            Ok(())
        })
    }

    #[test]
    fn branch_details_include_commits_and_upstream_state() -> anyhow::Result<()> {
        but_testsupport::isolated_app_data_dir(|| {
            let env =
                but_testsupport::Sandbox::open_or_init_scenario_with_target_and_default_settings(
                    "one-stack",
                );
            let ctx = env.context();
            let workspace = workspace_view_from_context(&ctx, env.projects_root())?;
            let branch = workspace
                .workspace
                .stacks
                .iter()
                .flat_map(|stack| &stack.rows)
                .find_map(|row| match &row.data {
                    DetailedGraphRowData::Commit(_) => None,
                    DetailedGraphRowData::Reference(reference) => {
                        Some(reference.ref_name.full_name.clone())
                    }
                })
                .context("fixture has a branch")?;

            let view = branch_details(BranchDetailsRequest {
                repository: env.projects_root().to_owned(),
                branch,
            })?;

            assert_eq!(view.kind, "branch");
            assert!(
                view.details.commits > 0,
                "branch details include commits for the selected branch"
            );
            Ok(())
        })
    }

    #[test]
    fn workspace_view_uses_the_first_applicable_file_root() -> anyhow::Result<()> {
        but_testsupport::isolated_app_data_dir(|| {
            let env =
                but_testsupport::Sandbox::open_or_init_scenario_with_target_and_default_settings(
                    "one-stack",
                );
            let repository_uri = Url::from_directory_path(env.projects_root()).map_err(|()| {
                anyhow::anyhow!("fixture path cannot be represented as a file URI")
            })?;
            let roots = vec![
                Root {
                    uri: "https://example.com/not-a-filesystem-root".into(),
                    name: Some("Not a file root".into()),
                },
                Root {
                    uri: repository_uri.into(),
                    name: Some("Fixture repository".into()),
                },
            ];

            let resolved = repository_from_roots(&roots)?;
            let view = workspace_view_from_context(&resolved.ctx, &resolved.repository.path)?;

            assert_eq!(
                view.repository.path,
                env.projects_root().canonicalize()?,
                "workspace comes from the first root that identifies a Git repository"
            );
            assert!(
                view.summary.stacks > 0,
                "selected repository root yields its workspace"
            );
            Ok(())
        })
    }

    fn forge_review_fixture() -> but_forge::ForgeReview {
        but_forge::ForgeReview {
            html_url: "https://github.com/gitbutlerapp/gitbutler/pull/42".into(),
            number: 42,
            title: "Show reviews in MCP Apps".into(),
            body: None,
            author: None,
            labels: Vec::new(),
            draft: true,
            source_branch: "feature/mcp-review".into(),
            target_branch: "master".into(),
            sha: "0123456789abcdef".into(),
            integration_commit_shas: Vec::new(),
            created_at: Some("2026-07-24T10:00:00Z".into()),
            modified_at: None,
            merged_at: None,
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: Some("gitbutlerapp".into()),
            head_repo_is_fork: false,
            auto_merge_enabled: false,
            reviewers: Vec::new(),
            unit_symbol: "#".into(),
            last_sync_at: Default::default(),
        }
    }

    fn ci_check_fixture(name: &str, status: but_forge::CiStatus) -> but_forge::CiCheck {
        but_forge::CiCheck {
            id: 1,
            name: name.into(),
            output: Default::default(),
            started_at: None,
            status,
            head_sha: "0123456789abcdef".into(),
            url: String::new(),
            html_url: String::new(),
            details_url: String::new(),
            pull_requests: Vec::new(),
            reference: "feature/mcp-review".into(),
            last_sync_at: Default::default(),
        }
    }
}
