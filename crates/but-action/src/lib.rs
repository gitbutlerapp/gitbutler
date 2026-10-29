//! This crate implements various automations that GitButler can perform.

use std::fmt::{Debug, Display};

use anyhow::Context as _;
use but_core::sync::RepoExclusive;
use but_ctx::Context;
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
pub mod reword;
mod simple;
pub use action::{ActionListing, Source, list_actions};
use but_core::ref_metadata::StackId;
use strum::EnumString;

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
            let target_ref: gitbutler_reference::RemoteRefname = ctx
                .project_meta()?
                .target_ref_or_err()?
                .to_string()
                .parse()?;
            gitbutler_branch_actions::set_base_branch(ctx, &target_ref, perm).map(|_| ())
        }
    }
}

fn default_target_setting_if_none(ctx: &Context) -> anyhow::Result<()> {
    let mut project_meta = ctx.project_meta()?;
    if project_meta.target_ref.is_some() {
        return Ok(());
    }

    let repo = ctx.repo.get()?;
    project_meta.target_ref = Some(
        but_workspace::init::infer_default_target_ref(&repo)?
            .context("Could not infer a local target branch; configure one first")?,
    );
    project_meta.target_commit_id = Some(project_meta.resolve_target_commit(&repo)?);
    ctx.set_project_meta(project_meta)?;
    Ok(())
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

#[cfg(test)]
mod target_tests {
    use super::default_target_setting_if_none;

    fn git(path: &std::path::Path, args: &[&str]) -> anyhow::Result<()> {
        let output = std::process::Command::new("git")
            .current_dir(path)
            .args(args)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }

    fn repository() -> anyhow::Result<tempfile::TempDir> {
        let tmp = tempfile::tempdir()?;
        git(tmp.path(), &["init", "-b", "main"])?;
        git(
            tmp.path(),
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "base",
            ],
        )?;
        Ok(tmp)
    }

    #[test]
    fn initializes_local_only_main() -> anyhow::Result<()> {
        let tmp = repository()?;
        let repo = gix::open(tmp.path())?;
        let local = repo.head_id()?.detach();
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        default_target_setting_if_none(&ctx)?;
        let meta = ctx.project_meta()?;
        assert_eq!(
            meta.target_ref_or_err()?.to_string(),
            "refs/heads/main",
            "local main is a valid automation target without any remote"
        );
        assert_eq!(
            meta.target_commit_id,
            Some(local),
            "local-only initialization uses main's tip"
        );
        Ok(())
    }

    #[test]
    fn push_default_does_not_choose_the_automation_target() -> anyhow::Result<()> {
        let tmp = repository()?;
        git(
            tmp.path(),
            &[
                "remote",
                "add",
                "origin",
                "https://example.com/upstream.git",
            ],
        )?;
        git(
            tmp.path(),
            &["remote", "add", "fork", "https://example.com/fork.git"],
        )?;
        git(
            tmp.path(),
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        )?;
        git(
            tmp.path(),
            &["update-ref", "refs/remotes/fork/other", "HEAD"],
        )?;
        git(
            tmp.path(),
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        )?;
        git(
            tmp.path(),
            &[
                "symbolic-ref",
                "refs/remotes/fork/HEAD",
                "refs/remotes/fork/other",
            ],
        )?;
        git(tmp.path(), &["branch", "--track", "other", "fork/other"])?;
        git(
            tmp.path(),
            &["branch", "--set-upstream-to=origin/main", "main"],
        )?;
        git(tmp.path(), &["config", "remote.pushDefault", "fork"])?;
        let repo = gix::open(tmp.path())?;
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        default_target_setting_if_none(&ctx)?;
        assert_eq!(
            ctx.project_meta()?.target_ref_or_err()?.to_string(),
            "refs/heads/main",
            "feature publication settings must not select the integration target"
        );
        Ok(())
    }
}
