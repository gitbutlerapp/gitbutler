use std::path::PathBuf;

use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_core::DiffSpec;
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use gitbutler_branch_actions::hooks;
use gitbutler_repo::{
    FileInfo, RepoCommands,
    hooks::{HookResult, MessageHookResult},
};
use gitbutler_repo_actions::askpass;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn check_signing_settings(ctx: &Context) -> Result<bool> {
    ctx.check_signing_settings()
}

/// NOTE: this function currently needs a tokio runtime to work.
#[but_api]
#[instrument(err(Debug))]
pub async fn git_clone_repository(repository_url: String, target_dir: PathBuf) -> Result<()> {
    gitbutler_git::clone(
        &repository_url,
        &target_dir,
        gitbutler_git::tokio::TokioExecutor,
        handle_git_prompt_clone,
        repository_url.clone(),
    )
    .await?;
    Ok(())
}

async fn handle_git_prompt_clone(prompt: String, url: String) -> Option<String> {
    tracing::info!("received prompt for clone of {url}: {prompt:?}");
    askpass::get_broker()
        .submit_prompt(prompt, askpass::Context::Clone { url })
        .await
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_commit_file(ctx: &Context, relative_path: PathBuf, commit_id: gix::ObjectId) -> Result<FileInfo> {
    ctx.read_file_from_commit(commit_id.to_git2(), &relative_path)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_workspace_file(ctx: &Context, relative_path: PathBuf) -> Result<FileInfo> {
    ctx.read_file_from_workspace(&relative_path)
}

/// Retrieves file content directly from a Git blob object by its blob ID.
///
/// This function is used for displaying image diff previews when the file
/// isn't available in the current workspace or a specific commit (e.g., for
/// deleted files or when comparing against a previous state).
///
/// # Arguments
/// * `blob_id` - Git blob object ID as a hexadecimal string
#[but_api]
#[instrument(err(Debug))]
pub fn get_blob_file(ctx: &but_ctx::Context, relative_path: PathBuf, blob_id: gix::ObjectId) -> Result<FileInfo> {
    let repo = ctx.repo.get()?;
    let object = repo.find_object(blob_id).context("Failed to find blob")?;
    let blob = object.try_into_blob().context("Object is not a blob")?;
    Ok(FileInfo::from_content(&relative_path, &blob.data))
}

#[but_api]
#[instrument(err(Debug))]
pub fn pre_commit_hook_diffspecs(ctx: &but_ctx::Context, changes: Vec<DiffSpec>) -> Result<HookResult> {
    let repo = ctx.repo.get()?;
    let head = repo.head_tree_id_or_empty().context("Failed to get head tree")?;

    let context_lines = ctx.settings.context_lines;

    let mut changes = changes.into_iter().map(Ok).collect::<Vec<_>>();

    let (new_tree, ..) = but_core::tree::apply_worktree_changes(head.detach(), &repo, &mut changes, context_lines)?;

    hooks::pre_commit_with_tree(ctx, new_tree.to_git2())
}
#[but_api]
#[instrument(err(Debug))]
pub fn post_commit_hook(ctx: &but_ctx::Context) -> Result<HookResult> {
    gitbutler_repo::hooks::post_commit(ctx)
}

#[but_api]
#[instrument(err(Debug))]
pub fn message_hook(ctx: &but_ctx::Context, message: String) -> Result<MessageHookResult> {
    gitbutler_repo::hooks::commit_msg(ctx, message)
}

#[but_api]
#[instrument(err(Debug))]
pub fn find_files(ctx: &Context, query: String, limit: Option<usize>) -> Result<Vec<String>> {
    let limit = limit.unwrap_or(10);
    ctx.find_files(&query, limit)
}
