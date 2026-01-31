use std::{fs, path, path::PathBuf, str::FromStr};

use but_ctx::Context;
use but_error::Marker;
use but_oxidize::{ObjectIdExt, OidExt};
use but_settings::AppSettings;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::GITBUTLER_WORKSPACE_COMMIT_TITLE;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_project::{self as projects, ProjectId};
use gitbutler_stack::StackId;
use gitbutler_testsupport::{TestProject, VAR_NO_CLEANUP, paths};
use tempfile::TempDir;

struct Test {
    repo: TestProject,
    project_id: ProjectId,
    data_dir: Option<TempDir>,
    ctx: Context,
}

impl Test {
    pub fn new_with_settings(change_settings: fn(&mut AppSettings)) -> Self {
        let data_dir = paths::data_dir();

        let test_project = TestProject::default();
        let outcome =
            gitbutler_project::add_at_app_data_dir(data_dir.as_ref(), test_project.path())
                .expect("failed to add project");
        let project = outcome.unwrap_project();
        let mut settings = AppSettings::default();
        change_settings(&mut settings);
        let ctx = Context::new_from_legacy_project_and_settings(&project, settings);
        Self {
            repo: test_project,
            project_id: project.id,
            data_dir: Some(data_dir),
            ctx,
        }
    }
}

impl Drop for Test {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.data_dir.take().unwrap().keep();
        }
    }
}

impl Default for Test {
    fn default() -> Self {
        Self::new_with_settings(|_settings| {})
    }
}

impl Test {
    /// Consume this instance and keep the temp directory that held the local repository, returning it.
    /// Best used inside a `dbg!(test.debug_local_repo())`
    #[expect(dead_code)]
    pub fn debug_local_repo(&mut self) -> Option<PathBuf> {
        self.repo.debug_local_repo()
    }
}

mod amend;
mod apply_virtual_branch;
mod create_virtual_branch_from_branch;
mod init;
mod list;
mod list_details;
mod move_commit_to_vbranch;
mod oplog;
mod save_and_unapply_virtual_branch;
mod set_base_branch;
mod unapply_without_saving_virtual_branch;
mod undo_commit;
mod update_commit_message;
mod workspace_migration;

pub fn list_commit_files(
    ctx: &Context,
    commit_oid: git2::Oid,
) -> anyhow::Result<Vec<but_core::TreeChange>> {
    let repo = ctx.repo.get()?;
    let commit_id = commit_oid.to_gix();
    but_core::diff::CommitDetails::from_commit_id(
        gix::prelude::ObjectIdExt::attach(commit_id, &repo),
        false,
    )
    .map(|d| d.diff_with_first_parent)
}

pub fn create_commit(
    ctx: &mut Context,
    stack_id: StackId,
    message: &str,
) -> anyhow::Result<git2::Oid> {
    let mut guard = ctx.exclusive_worktree_access();

    let repo = ctx.repo.get()?;
    let worktree = but_core::diff::worktree_changes(&repo)?;
    let file_changes: Vec<but_core::DiffSpec> =
        worktree.changes.iter().map(Into::into).collect::<Vec<_>>();

    let meta = ctx.legacy_meta()?;
    let stacks = but_workspace::legacy::stacks_v3(
        &repo,
        &meta,
        but_workspace::legacy::StacksFilter::InWorkspace,
        None,
    )?;

    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());

    let stack_branch_name = stacks
        .iter()
        .find(|s| s.id == Some(stack_id))
        .and_then(|s| s.heads.first().map(|h| h.name.to_string()))
        .ok_or(anyhow::anyhow!("Could not find associated reference name"))?;

    let outcome = but_workspace::legacy::commit_engine::create_commit_simple(
        ctx,
        stack_id,
        None,
        file_changes,
        message.to_string(),
        stack_branch_name,
        guard.write_permission(),
    );

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_commit_creation(
            snapshot_tree,
            outcome.as_ref().err(),
            message.to_owned(),
            None,
            guard.write_permission(),
        )
    });
    outcome?
        .new_commit
        .map(|c| c.to_git2())
        .ok_or(anyhow::anyhow!("No new commit created"))
}
