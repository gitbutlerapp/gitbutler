//! This crate implements various automations that GitButler can perform.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use but_core::sync::RepoExclusive;
use but_ctx::Context;
use but_meta::virtual_branches_legacy_types::Target;
use but_workspace::legacy::ui::StackEntry;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod action;
pub mod cli;
pub mod commit_format;
mod generate;
pub mod rename_branch;
pub mod reword;
mod simple;
mod workflow;
pub use action::{ActionListing, Source, list_actions};
use but_core::ref_metadata::StackId;
use strum::EnumString;
pub use workflow::{WorkflowList, list_workflows};

/// React to detected worktree changes by creating commits on the appropriate workspace stacks.
///
/// This is the action-side callback for watcher or external tool flows that have noticed
/// uncommitted changes and want GitButler to absorb them. It first prepares the repository state,
/// then collects hunk assignments, creates a stack if none exists, groups changes by target stack,
/// and creates commits with the requested message.
///
/// `ctx` provides the repository, workspace, database, metadata, settings, default-target setup,
/// and operating-mode state. `change_summary` is the generated or user-provided summary used in the
/// commit message. `external_prompt` is optional additional prompt text to prepend to the commit
/// message. `handler` selects the concrete handle-changes implementation. `exclusive_stack` limits
/// commits to one stack when set. `perm` is the caller-held exclusive worktree permission used for
/// every repository and workspace access in this function.
pub fn on_uncommitted_changes(
    ctx: &mut Context,
    change_summary: &str,
    external_prompt: Option<String>,
    handler: ActionHandler,
    exclusive_stack: Option<StackId>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<Outcome> {
    prepare_handle_changes(ctx, perm)?;
    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    match handler {
        ActionHandler::HandleChangesSimple => simple::handle_changes(
            change_summary,
            external_prompt,
            exclusive_stack,
            perm,
            &repo,
            &mut ws,
            &mut db,
            &mut meta,
            context_lines,
        ),
    }
}

/// Record and run an uncommitted-changes reaction while reusing the caller's worktree permission.
///
/// `ctx` provides the repository, workspace, database, metadata, settings, snapshots, and
/// operating-mode state. `change_summary` is the generated or user-provided summary used in the
/// commit message and action record. `external_prompt` is optional additional prompt text to
/// prepend to the commit message and persist with the action. `handler` selects the concrete
/// handle-changes implementation. `source` records where the action originated. `exclusive_stack`
/// limits commits to one stack when set. `perm` is the caller-held exclusive worktree permission
/// used for snapshots, repository access, workspace access, and rule preparation.
pub fn record_uncommitted_changes_with_perm(
    ctx: &mut Context,
    change_summary: &str,
    external_prompt: Option<String>,
    handler: ActionHandler,
    source: Source,
    exclusive_stack: Option<StackId>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<(Uuid, Outcome)> {
    let snapshot_before = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::AutoHandleChangesBefore),
        perm,
    )?;
    let response = on_uncommitted_changes(
        ctx,
        change_summary,
        external_prompt.clone(),
        handler,
        exclusive_stack,
        perm,
    );
    let snapshot_after = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::AutoHandleChangesAfter),
        perm,
    )?;
    let id = {
        let mut db = ctx.db.get_cache_mut()?;
        action::record_handle_changes_action(
            &mut db,
            change_summary,
            external_prompt,
            handler,
            source,
            snapshot_before,
            snapshot_after,
            &response,
        )?
    };
    response.map(|outcome| (id, outcome))
}

/// Prepare repository state for handle-changes.
///
/// This preserves the legacy preconditions from the old context-owning action flow:
/// ensure a default target exists, reject edit mode, and switch back to the workspace
/// branch when the repository is currently outside the workspace. Callers must hold
/// exclusive worktree access and pass its permission through so no nested guard is
/// acquired here.
fn prepare_handle_changes(ctx: &mut Context, perm: &mut RepoExclusive) -> anyhow::Result<()> {
    default_target_setting_if_none(ctx)?;
    match gitbutler_operating_modes::operating_mode(ctx, perm.read_permission())? {
        OperatingMode::OpenWorkspace => Ok(()),
        OperatingMode::Edit(_) => Err(anyhow::anyhow!(
            "Cannot handle changes while in edit mode. Please exit edit mode first."
        )),
        OperatingMode::OutsideWorkspace(_) => {
            let default_target = ctx.persisted_default_target()?;
            gitbutler_branch_actions::set_base_branch(ctx, &default_target.branch, perm).map(|_| ())
        }
    }
}

fn default_target_setting_if_none(ctx: &Context) -> anyhow::Result<()> {
    if ctx.project_meta()?.target_ref.is_some() {
        return Ok(());
    }
    // Lets do the equivalent of `git symbolic-ref refs/remotes/origin/HEAD --short` to guess the default target.

    let repo = ctx.repo.get()?;
    let remote_name = repo
        .remote_default_name(gix::remote::Direction::Push)
        .ok_or_else(|| anyhow::anyhow!("No push remote set or more than one remote"))?
        .to_string();

    let mut head_ref = repo
        .find_reference(&format!("refs/remotes/{remote_name}/HEAD"))
        .map_err(|_| anyhow::anyhow!("No HEAD reference found for remote {remote_name}"))?;
    let target_ref_name = head_ref
        .target()
        .try_name()
        .ok_or_else(|| anyhow::anyhow!("Remote HEAD for {remote_name} is not symbolic"))?
        .to_owned();

    let head_commit = head_ref.peel_to_commit()?;

    let remote_refname =
        gitbutler_reference::RemoteRefname::from_str(&target_ref_name.as_bstr().to_string())?;

    let target = Target {
        branch: remote_refname.clone(),
        remote_url: "".to_string(),
        sha: head_commit.id,
        push_remote_name: None,
    };

    let mut project_meta = ctx.project_meta()?;
    project_meta.target_ref = Some(remote_refname.to_string().try_into()?);
    project_meta.target_commit_id = Some(head_commit.id);
    project_meta.push_remote = None;
    project_meta.persist_to_local_config(&repo)?;
    ctx.legacy_meta()?.set_default_target(target)?;
    ctx.invalidate_workspace_cache()?;
    Ok(())
}

#[expect(deprecated, reason = "calls but_workspace::legacy::stacks_v3")]
fn stacks(ctx: &Context, repo: &gix::Repository) -> anyhow::Result<Vec<StackEntry>> {
    let meta = ctx.legacy_meta()?;
    but_workspace::legacy::stacks_v3(
        repo,
        &meta,
        but_workspace::legacy::StacksFilter::InWorkspace,
        None,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, EnumString, Default)]
#[serde(rename_all = "camelCase")]
pub enum ActionHandler {
    #[default]
    HandleChangesSimple,
}

impl Display for ActionHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub updated_branches: Vec<UpdatedBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedBranch {
    pub stack_id: StackId,
    pub branch_name: String,
    pub new_commits: Vec<String>,
}
