use anyhow::Result;
use but_action::OpenAiProvider;
use but_api_macros::api_cmd;
use but_core::TreeChange;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use std::sync::Arc;
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[instrument(err(Debug))]
pub fn list_actions(
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> Result<but_action::ActionListing, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_action::list_actions(ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[api_cmd]
#[instrument(err(Debug))]
pub fn handle_changes(
    project_id: ProjectId,
    change_summary: String,
    handler: but_action::ActionHandler,
) -> Result<but_action::Outcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_action::handle_changes(
        ctx,
        &change_summary,
        None,
        handler,
        but_action::Source::GitButler,
        None,
    )
    .map(|(_id, outcome)| outcome)
    .map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[api_cmd]
#[instrument(err(Debug))]
pub fn list_workflows(
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> Result<but_action::WorkflowList, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_action::list_workflows(ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

// For the functions that require emitters (auto_commit, auto_branch_changes, absorb, freestyle),
// we need to handle them differently as they take emitter callbacks which are tauri-specific
#[instrument(skip(emitter), err(Debug))]
pub fn auto_commit_internal(
    emitter: Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));

    match openai {
        Some(openai) => but_action::auto_commit(emitter, ctx, &openai, changes).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}

#[instrument(skip(emitter), err(Debug))]
pub fn auto_branch_changes_internal(
    emitter: Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));

    match openai {
        Some(openai) => but_action::branch_changes(emitter, ctx, &openai, changes).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}

#[instrument(skip(emitter), err(Debug))]
pub fn absorb_internal(
    emitter: Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));

    match openai {
        Some(openai) => but_action::absorb(emitter, ctx, &openai, changes).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}

#[instrument(skip(emitter), err(Debug))]
pub fn freestyle_internal(
    emitter: Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>,
    project_id: ProjectId,
    message_id: String,
    chat_messages: Vec<but_action::ChatMessage>,
    model: Option<String>,
) -> Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));
    match openai {
        Some(openai) => but_action::freestyle(project_id, message_id, emitter, ctx, &openai, chat_messages, model).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}