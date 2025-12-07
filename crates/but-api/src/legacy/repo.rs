use std::path::PathBuf;

use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_core::DiffSpec;
use but_ctx::Context;
use but_meta::virtual_branches_legacy_types::BranchOwnershipClaims;
use but_oxidize::ObjectIdExt;
use gitbutler_branch_actions::{RemoteBranchFile, hooks};
use gitbutler_project::ProjectId;
use gitbutler_repo::{
    FileInfo, RepoCommands,
    hooks::{HookResult, MessageHookResult},
};
use gitbutler_repo_actions::askpass;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn git_get_local_config(project_id: ProjectId, key: String) -> Result<Option<String>> {
    let project = gitbutler_project::get(project_id)?;
    project.get_local_config(&key)
}

#[but_api]
#[instrument(err(Debug))]
pub fn git_set_local_config(project_id: ProjectId, key: String, value: String) -> Result<()> {
    let project = gitbutler_project::get(project_id)?;
    project.set_local_config(&key, &value)
}

#[but_api]
#[instrument(err(Debug))]
pub fn check_signing_settings(project_id: ProjectId) -> Result<bool> {
    let project = gitbutler_project::get(project_id)?;
    project.check_signing_settings()
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
pub fn get_uncommitted_files(project_id: ProjectId) -> Result<Vec<RemoteBranchFile>> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_branch_actions::get_uncommitted_files(&ctx)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_commit_file(
    project_id: ProjectId,
    relative_path: PathBuf,
    commit_id: String,
) -> Result<FileInfo> {
    let project = gitbutler_project::get(project_id)?;

    let commit_oid = if let Ok(oid) = git2::Oid::from_str(&commit_id) {
        oid
    } else {
        // Support refs like "HEAD" by resolving with gix
        let repo = project.open_repo()?;
        repo.rev_parse_single(commit_id.as_bytes())?.to_git2()
    };

    project.read_file_from_commit(commit_oid, &relative_path)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_workspace_file(project_id: ProjectId, relative_path: PathBuf) -> Result<FileInfo> {
    let project = gitbutler_project::get(project_id)?;
    project.read_file_from_workspace(&relative_path)
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
pub fn get_blob_file(
    project_id: ProjectId,
    relative_path: PathBuf,
    blob_id: String,
) -> Result<FileInfo> {
    let project = gitbutler_project::get(project_id)?;
    let repo = project.open_repo()?;

    let oid = gix::ObjectId::from_hex(blob_id.as_bytes()).context("Failed to parse blob ID")?;

    let object = repo.find_object(oid).context("Failed to find blob")?;
    let blob = object.try_into_blob()?;

    Ok(FileInfo::from_content(&relative_path, &blob.data))
}

#[but_api]
#[instrument(err(Debug))]
pub fn pre_commit_hook(
    project_id: ProjectId,
    ownership: BranchOwnershipClaims,
) -> Result<HookResult> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let claim = ownership.into();
    hooks::pre_commit(&ctx, &claim)
}

#[but_api]
#[instrument(err(Debug))]
pub fn pre_commit_hook_diffspecs(
    project_id: ProjectId,
    changes: Vec<DiffSpec>,
) -> Result<HookResult> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;

    let repository = ctx.repo.get()?;
    let head = repository
        .head_tree_id_or_empty()
        .context("Failed to get head tree")?;

    let context_lines = ctx.settings().context_lines;

    let mut changes = changes.into_iter().map(Ok).collect::<Vec<_>>();

    let (new_tree, ..) = but_core::tree::apply_worktree_changes(
        head.detach(),
        &repository,
        &mut changes,
        context_lines,
    )?;

    hooks::pre_commit_with_tree(&ctx, new_tree.to_git2())
}

#[but_api]
#[instrument(err(Debug))]
pub fn post_commit_hook(project_id: ProjectId) -> Result<HookResult> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_repo::hooks::post_commit(&ctx)
}

#[but_api]
#[instrument(err(Debug))]
pub fn message_hook(project_id: ProjectId, message: String) -> Result<MessageHookResult> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    gitbutler_repo::hooks::commit_msg(&ctx, message)
}

#[but_api]
#[instrument(err(Debug))]
pub fn find_files(
    project_id: ProjectId,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<String>> {
    let project = gitbutler_project::get(project_id)?;
    let limit = limit.unwrap_or(10);
    project.find_files(&query, limit)
}
