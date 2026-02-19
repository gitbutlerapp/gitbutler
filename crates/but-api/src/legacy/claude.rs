use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_claude::{
    Claude, ClaudeCheckResult, ClaudeMessage, ClaudeUserParams, Transcript,
    claude_mcp::{ClaudeConfig, ClaudeProjectConfig},
    claude_settings::ClaudeSettings,
    prompt_templates,
};
use but_core::ref_metadata::StackId;
use but_ctx::{Context, ThreadSafeContext};
use but_settings::AppSettings;
use gitbutler_project::ProjectId;
use serde::Deserialize;
use tracing::instrument;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    #[serde(flatten)]
    pub user_params: ClaudeUserParams,
}

pub async fn claude_send_message(claude: &Claude, params: SendMessageParams) -> Result<()> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    claude
        .instance_by_stack
        .send_message(
            ctx.into_sync(),
            claude.broadcaster.clone(),
            params.stack_id,
            params.user_params,
        )
        .await?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMessagesParams {
    // TODO(ctx): make this `ProjectHandle`.
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub fn claude_get_messages(claude: &Claude, params: GetMessagesParams) -> Result<Vec<ClaudeMessage>> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    let messages = claude.instance_by_stack.get_messages(&ctx, params.stack_id)?;
    Ok(messages)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_get_session_details(
    ctx: ThreadSafeContext,
    session_id: uuid::Uuid,
) -> Result<but_claude::ClaudeSessionDetails> {
    let (worktree_dir, session) = {
        let ctx = ctx.into_thread_local();
        let session = but_claude::db::get_session_by_id(&ctx, session_id)?.context("Could not find session")?;
        let worktree_dir = ctx.workdir_or_fail()?;
        (worktree_dir, session)
    };
    let current_id = Transcript::current_valid_session_id(&worktree_dir, &session).await?;
    if let Some(current_id) = current_id {
        let transcript_path = but_claude::Transcript::get_transcript_path(&worktree_dir, current_id)?;
        let transcript = but_claude::Transcript::from_file(&transcript_path)?;
        Ok(but_claude::ClaudeSessionDetails {
            summary: transcript.summary(),
            last_prompt: transcript.prompt(),
            in_gui: session.in_gui,
        })
    } else {
        Ok(but_claude::ClaudeSessionDetails {
            summary: None,
            last_prompt: None,
            in_gui: session.in_gui,
        })
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_get_user_message(ctx: &but_ctx::Context, offset: Option<i64>) -> Result<Option<ClaudeMessage>> {
    but_claude::db::get_user_message(ctx, offset)
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_list_permission_requests(ctx: &but_ctx::Context) -> Result<Vec<but_claude::ClaudePermissionRequest>> {
    but_claude::db::list_all_permission_requests(ctx)
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_update_permission_request(
    ctx: &mut but_ctx::Context,
    request_id: String,
    decision: but_claude::PermissionDecision,
    use_wildcard: bool,
) -> Result<()> {
    but_claude::db::update_permission_request(ctx, &request_id, decision, use_wildcard)
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_answer_ask_user_question(
    project_id: ProjectId,
    stack_id: StackId,
    answers: std::collections::HashMap<String, String>,
) -> Result<bool> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    but_claude::db::set_ask_user_question_answers_by_stack(&mut ctx, stack_id, answers)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSessionParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_cancel_session(claude: &Claude, params: CancelSessionParams) -> Result<bool> {
    let cancelled = claude.instance_by_stack.cancel_session(params.stack_id).await?;
    Ok(cancelled)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_check_available() -> Result<ClaudeCheckResult> {
    let app_settings = AppSettings::load_from_default_path_creating_without_customization()?;
    let claude_executable = app_settings.claude.executable.clone();
    Ok(but_claude::session::check_claude_available(&claude_executable).await)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsStackActiveParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_is_stack_active(claude: &Claude, params: IsStackActiveParams) -> Result<bool> {
    let is_active = claude.instance_by_stack.is_stack_active(params.stack_id).await;
    Ok(is_active)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactHistoryParams {
    // TODO(ctx): turn into `ProjectHandle`
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_compact_history(claude: &Claude, params: CompactHistoryParams) -> Result<()> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    claude
        .instance_by_stack
        .compact_history(ctx.into_sync(), claude.broadcaster.clone(), params.stack_id)
        .await?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_list_prompt_templates(ctx: &but_ctx::Context) -> Result<Vec<prompt_templates::PromptTemplate>> {
    let templates = prompt_templates::list_templates(&ctx.project_data_dir())?;
    Ok(templates)
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_get_prompt_dirs(ctx: &but_ctx::Context) -> Result<Vec<prompt_templates::PromptDir>> {
    let dirs = prompt_templates::prompt_dirs(&ctx.project_data_dir())?;
    Ok(dirs)
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_maybe_create_prompt_dir(ctx: &but_ctx::Context, path: String) -> Result<()> {
    prompt_templates::maybe_create_dir(&ctx.workdir_or_fail()?, &path)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_get_config(ctx: ThreadSafeContext) -> Result<ClaudeConfig> {
    let worktree_dir = ctx.into_thread_local().workdir_or_fail()?;
    let settings = ClaudeSettings::open(&worktree_dir).await;
    let project_config = ClaudeProjectConfig::open(&settings, &worktree_dir).await;
    Ok(project_config.config())
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_get_sub_agents(ctx: ThreadSafeContext) -> Result<Vec<but_claude::SubAgent>> {
    let workdir = ctx.into_thread_local().workdir_or_fail()?;
    let sub_agents = but_claude::claude_sub_agents::read_claude_sub_agents(&workdir).await;
    Ok(sub_agents)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_verify_path(ctx: ThreadSafeContext, path: String) -> Result<bool> {
    // Check if it's an absolute path first
    let path = if std::path::Path::new(&path).is_absolute() {
        std::path::PathBuf::from(&path)
    } else {
        // If relative, make it relative to project path
        ctx.into_thread_local().workdir_or_fail()?.join(&path)
    };

    // Check if the path exists and is a directory
    match tokio::fs::try_exists(&path).await {
        Ok(exists) => {
            if exists {
                match tokio::fs::metadata(&path).await {
                    Ok(metadata) => Ok(metadata.is_dir()),
                    Err(_) => Ok(false),
                }
            } else {
                Ok(false)
            }
        }
        Err(_) => Ok(false),
    }
}
