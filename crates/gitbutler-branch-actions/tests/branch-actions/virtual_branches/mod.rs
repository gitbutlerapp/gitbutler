use std::{fs, path, path::PathBuf, str::FromStr};

use but_ctx::{Context, ProjectHandleOrLegacyProjectId, RepoOpenMode};
use but_error::Marker;
use but_settings::AppSettings;
use but_testsupport::legacy::{TestProject, VAR_NO_CLEANUP, paths};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::GITBUTLER_WORKSPACE_COMMIT_TITLE;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_project::{self as projects};
use gitbutler_stack::StackId;
use tempfile::TempDir;

struct Test {
    repo: TestProject,
    project_id: ProjectHandleOrLegacyProjectId,
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
        let ctx = Context::new_from_legacy_project_and_settings_with_repo_open_mode(
            &project,
            settings,
            RepoOpenMode::Isolated,
        )
        .expect("can create context");
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
mod move_branch;
mod move_commit_to_vbranch;
mod oplog;
mod save_and_unapply_virtual_branch;
mod set_base_branch;
mod unapply_without_saving_virtual_branch;
mod undo_commit;
mod update_commit_message;
mod workspace_migration;

/// Create a raw git commit parented to `parent` that sets `filename` to `content`,
/// bypassing GitButler's stack API. Used in tests to construct competitor commits
/// or to inject a known tree state without going through the worktree.
pub fn make_commit_on_file(
    repo: &gix::Repository,
    parent: gix::ObjectId,
    filename: &str,
    content: &[u8],
) -> anyhow::Result<gix::ObjectId> {
    let blob_id = repo.write_blob(content)?.detach();
    let parent_commit = repo.find_commit(parent)?;
    let mut editor = parent_commit.tree()?.edit()?;
    editor.upsert(filename, gix::object::tree::EntryKind::Blob, blob_id)?;
    let tree_id = editor.write()?.detach();
    Ok(repo
        .write_object(gix::objs::Commit {
            tree: tree_id,
            parents: [parent].into(),
            message: "raw test commit".into(),
            ..parent_commit.decode()?.to_owned()?
        })?
        .detach())
}

/// Cherry-pick `competing_oid` (which shares an ancestor with `onto_oid` and modifies the
/// same content) onto `onto_oid` to produce a conflicted commit, then update the source
/// stack head to that conflicted commit. Returns the conflicted commit's OID.
///
/// The conflicted commit is parented to `onto_oid`, so after this call the source stack
/// history is: merge_base → onto_oid → conflicted.
pub fn push_conflicted_commit_onto(
    ctx: &but_ctx::Context,
    stack_id: gitbutler_stack::StackId,
    onto_oid: gix::ObjectId,
    competing_oid: gix::ObjectId,
) -> anyhow::Result<gix::ObjectId> {
    let (_guard, repo, ws, _db) = ctx.workspace_and_db()?;
    let conflicted_oid = but_rebase::cherry_pick_one(
        &repo,
        onto_oid,
        competing_oid,
        but_rebase::cherry_pick::PickMode::Unconditionally,
        but_rebase::cherry_pick::EmptyCommit::Keep,
    )?;
    assert!(
        but_core::Commit::from_id(repo.find_commit(conflicted_oid)?.id())?.is_conflicted(),
        "cherry_pick_one must have produced a conflicted commit for the test to be meaningful"
    );
    let ref_name = ws
        .stacks
        .iter()
        .find(|s| s.id == Some(stack_id))
        .ok_or_else(|| anyhow::anyhow!("stack not found in workspace"))?
        .ref_name()
        .ok_or_else(|| anyhow::anyhow!("stack has no ref name"))?
        .to_owned();
    repo.reference(
        ref_name.as_ref(),
        conflicted_oid,
        gix::refs::transaction::PreviousValue::Any,
        "test: push conflicted commit",
    )?;
    Ok(conflicted_oid)
}

pub fn list_commit_files(
    ctx: &Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<Vec<but_core::TreeChange>> {
    let repo = ctx.repo.get()?;
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
) -> anyhow::Result<gix::ObjectId> {
    let mut guard = ctx.exclusive_worktree_access();

    let repo = ctx.repo.get()?;
    let worktree = but_core::diff::worktree_changes(&repo)?;
    let file_changes: Vec<but_core::DiffSpec> =
        worktree.changes.iter().map(Into::into).collect::<Vec<_>>();

    let meta = ctx.legacy_meta()?;
    let stacks = {
        let mut cache = ctx.cache.get_cache_mut()?;
        but_workspace::legacy::stacks_v3(
            &repo,
            &meta,
            but_workspace::legacy::StacksFilter::InWorkspace,
            None,
            &mut cache,
        )?
    };

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
        .ok_or(anyhow::anyhow!("No new commit created"))
}
