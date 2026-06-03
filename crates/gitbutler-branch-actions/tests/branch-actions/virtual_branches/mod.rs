#![expect(deprecated, reason = "calls but_workspace::legacy::stacks_v3")]

use std::{fs, path::PathBuf, str::FromStr};

use but_core::{RepositoryExt as _, ref_metadata::StackId};
use but_ctx::{Context, ProjectHandleOrLegacyProjectId, RepoOpenMode};
use but_error::Marker;
use but_rebase::graph_rebase::LookupStep as _;
use but_settings::AppSettings;
use but_testsupport::{
    gix_testtools::{Creation, scripted_fixture_writable_with_args},
    open_repo,
};
use gitbutler_branch_actions::GITBUTLER_WORKSPACE_COMMIT_TITLE;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_repo::{SignaturePurpose, commit_without_signature_gix, signature_gix};
use gix::refs::transaction::PreviousValue;
use tempfile::{TempDir, tempdir};

pub(crate) use crate::support::stack_details;

const VAR_NO_CLEANUP: &str = "GITBUTLER_TESTS_NO_CLEANUP";

struct TestRepo {
    /// The worktree path of the local repository in the fixture.
    local_path: PathBuf,
    fixture_tmp_dir: Option<TempDir>,
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::from_fixture("scenario/repo-with-origin.sh")
    }
}

impl TestRepo {
    fn from_fixture(fixture: &str) -> Self {
        let temp_dir =
            scripted_fixture_writable_with_args(fixture, None::<String>, Creation::Execute)
                .map_err(anyhow::Error::from_boxed)
                .expect("fixture should materialize");
        let local_path = temp_dir.path().join("local");

        Self {
            local_path,
            fixture_tmp_dir: Some(temp_dir),
        }
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.fixture_tmp_dir.take().map(|tmp| tmp.keep());
        }
    }
}

impl TestRepo {
    pub fn debug_local_repo(&mut self) -> Option<PathBuf> {
        self.fixture_tmp_dir
            .take()
            .map(|tmp| tmp.keep().join("local"))
    }

    pub fn path(&self) -> &std::path::Path {
        &self.local_path
    }

    fn open(&self) -> gix::Repository {
        open_repo(&self.local_path).expect("local repo opens")
    }

    /// Simulate pushing `branch` by updating the matching `origin` remote-tracking ref.
    pub fn simulate_push_branch(&self, branch: &LocalRefname) {
        let repo = self.open();
        let local_target = repo
            .find_reference(&branch.to_string())
            .unwrap()
            .peel_to_id()
            .unwrap()
            .detach();
        let short = branch
            .to_string()
            .strip_prefix("refs/heads/")
            .expect("local branch ref")
            .to_owned();
        let remote_ref = format!("refs/remotes/origin/{short}");
        repo.reference(
            remote_ref.as_str(),
            local_target,
            PreviousValue::Any,
            "sync origin refs for tests",
        )
        .unwrap();
    }

    pub fn push(&self) {
        self.simulate_push_branch(&"refs/heads/master".parse().unwrap());
    }

    pub fn reset_hard(&self, oid: Option<gix::ObjectId>) {
        let repo = self.open();
        let current_head = repo.head_id().expect("HEAD peels").detach();
        let target = oid.unwrap_or(current_head);
        but_core::worktree::safe_checkout(
            current_head,
            target,
            &repo,
            but_core::worktree::checkout::Options {
                skip_head_update: true,
                ..Default::default()
            },
        )
        .unwrap();
        update_current_head(&repo, target, "reset hard");
    }

    pub fn checkout_commit(&self, commit_oid: gix::ObjectId) {
        let repo = self.open();
        let current_head = repo.head_id().expect("HEAD peels").detach();
        but_core::worktree::safe_checkout(
            current_head,
            commit_oid,
            &repo,
            but_core::worktree::checkout::Options {
                skip_head_update: true,
                ..Default::default()
            },
        )
        .unwrap();
        set_head_detached(&repo, commit_oid);
    }

    pub fn checkout(&self, branch: &LocalRefname) {
        let repo = self.open();
        let refname: Refname = branch.into();
        let current_head = repo.head_id().expect("HEAD peels").detach();
        let branch_name = refname.to_string();
        let target = match repo.try_find_reference(&branch_name).unwrap() {
            Some(mut reference) => reference.peel_to_id().unwrap().detach(),
            None => {
                repo.reference(
                    branch_name.as_str(),
                    current_head,
                    PreviousValue::Any,
                    "new branch",
                )
                .unwrap();
                current_head
            }
        };
        but_core::worktree::safe_checkout(
            current_head,
            target,
            &repo,
            but_core::worktree::checkout::Options {
                skip_head_update: true,
                ..Default::default()
            },
        )
        .unwrap();
        set_head_symbolic(&repo, branch_name.as_str());
    }

    pub fn commit_all(&self, message: &str) -> gix::ObjectId {
        let repo = self.open();
        let parent = repo.head_id().expect("HEAD peels").detach();
        let tree_id = repo
            .create_wd_tree(0)
            .expect("worktree tree can be written");
        let author = signature_gix(SignaturePurpose::Author);
        let committer = signature_gix(SignaturePurpose::Committer);
        let commit_id = commit_without_signature_gix(
            &repo,
            None,
            author,
            committer,
            message.into(),
            tree_id,
            &[parent],
            None,
        )
        .expect("failed to commit");
        but_core::worktree::safe_checkout(
            parent,
            commit_id,
            &repo,
            but_core::worktree::checkout::Options {
                skip_head_update: true,
                ..Default::default()
            },
        )
        .unwrap();
        update_current_head(&repo, commit_id, "commit");
        commit_id
    }
}

struct Test {
    repo: TestRepo,
    project_id: ProjectHandleOrLegacyProjectId,
    data_dir: Option<TempDir>,
    ctx: Context,
}

impl Test {
    pub fn from_fixture(fixture: &str) -> Self {
        Self::new_with_settings_and_repo(|_settings| {}, TestRepo::from_fixture(fixture))
    }

    pub fn new_with_settings(change_settings: fn(&mut AppSettings)) -> Self {
        Self::new_with_settings_and_repo(change_settings, TestRepo::default())
    }

    fn new_with_settings_and_repo(
        change_settings: fn(&mut AppSettings),
        test_project: TestRepo,
    ) -> Self {
        let data_dir = tempdir().expect("tempdir exists");

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
        .expect("can create context")
        .with_memory_app_cache();
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

fn update_current_head(repo: &gix::Repository, target: gix::ObjectId, message: &str) {
    match repo.head_name().expect("HEAD can be read") {
        Some(name) => {
            let name = name.as_bstr().to_string();
            repo.reference(name.as_str(), target, PreviousValue::Any, message)
                .expect("head reference can be updated");
        }
        None => set_head_detached(repo, target),
    }
}

fn set_head_symbolic(repo: &gix::Repository, target: &str) {
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: b"test: set HEAD".into(),
            },
            expected: PreviousValue::Any,
            new: gix::refs::Target::Symbolic(target.try_into().unwrap()),
        },
        name: "HEAD".try_into().unwrap(),
        deref: false,
    })
    .expect("HEAD can be set");
}

fn set_head_detached(repo: &gix::Repository, target: gix::ObjectId) {
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: b"test: detach HEAD".into(),
            },
            expected: PreviousValue::Any,
            new: gix::refs::Target::Object(target),
        },
        name: "HEAD".try_into().unwrap(),
        deref: false,
    })
    .expect("HEAD can be detached");
}

mod apply_virtual_branch;
mod create_virtual_branch_from_branch;
mod init;
mod list;
mod list_details;
mod oplog;
mod set_base_branch;
mod workspace_migration;

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

    let repo = ctx.repo.get()?.clone();
    let worktree = but_core::diff::worktree_changes(&repo)?;
    let file_changes: Vec<but_core::DiffSpec> =
        worktree.changes.iter().map(Into::into).collect::<Vec<_>>();

    let meta = ctx.legacy_meta()?;
    let stacks = {
        but_workspace::legacy::stacks_v3(
            &repo,
            &meta,
            but_workspace::legacy::StacksFilter::InWorkspace,
            None,
        )?
    };

    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());

    let stack_branch_name = stacks
        .iter()
        .find(|s| s.id == Some(stack_id))
        .and_then(|s| s.heads.first().map(|h| h.name.to_string()))
        .ok_or(anyhow::anyhow!("Could not find associated reference name"))?;

    let mut meta = ctx.meta()?;
    ctx.reload_repo_and_invalidate_workspace(guard.write_permission())?;
    let full_ref_name: gix::refs::FullName =
        format!("refs/heads/{stack_branch_name}").try_into()?;
    let outcome = {
        let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
        let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo)?;
        but_workspace::commit::commit_create(
            editor,
            file_changes,
            but_rebase::graph_rebase::mutate::RelativeToRef::Reference(full_ref_name.as_ref()),
            but_rebase::graph_rebase::mutate::InsertSide::Below,
            message,
            ctx.settings.context_lines,
        )
        .and_then(|outcome| {
            let selector = outcome.commit_selector;
            let materialized = outcome.rebase.materialize()?;
            selector
                .map(|selector| materialized.lookup_pick(selector))
                .transpose()
        })
    };
    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_commit_creation(
            snapshot_tree,
            outcome.as_ref().err(),
            message.to_owned(),
            None,
            guard.write_permission(),
        )
    });
    let new_commit = outcome?.ok_or(anyhow::anyhow!("No new commit created"))?;
    ctx.reload_repo_and_invalidate_workspace(guard.write_permission())?;
    Ok(new_commit)
}
