use std::{fs, path, path::PathBuf, str::FromStr};

use but_ctx::{Context, ProjectHandleOrLegacyProjectId, RepoOpenMode};
use but_error::Marker;
use but_project_handle::storage_path_config_key;
use but_settings::AppSettings;
use but_testsupport::gix_testtools::{Creation, scripted_fixture_writable_with_args};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::GITBUTLER_WORKSPACE_COMMIT_TITLE;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_project::{self as projects};
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::StackId;
use tempfile::{TempDir, tempdir};

pub(crate) use crate::support::stack_details;

const VAR_NO_CLEANUP: &str = "GITBUTLER_TESTS_NO_CLEANUP";

struct TestRepo {
    local_repo: git2::Repository,
    temp_dir: Option<TempDir>,
}

impl Default for TestRepo {
    fn default() -> Self {
        let temp_dir = scripted_fixture_writable_with_args(
            "scenario/repo-with-origin.sh",
            None::<String>,
            Creation::Execute,
        )
        .map_err(anyhow::Error::from_boxed)
        .expect("fixture should materialize");
        let local_repo =
            git2::Repository::open(temp_dir.path().join("local")).expect("local repo opens");
        setup_config(&local_repo.config().expect("config exists")).expect("config is writable");
        local_repo
            .remote_set_url(
                "origin",
                temp_dir
                    .path()
                    .join("remote")
                    .to_str()
                    .expect("remote path is valid UTF-8"),
            )
            .expect("origin remote URL can be normalized");
        sync_origin_refs(&local_repo).expect("origin refs are available locally");

        Self {
            local_repo,
            temp_dir: Some(temp_dir),
        }
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.temp_dir.take().map(|tmp| tmp.keep());
        }
    }
}

impl TestRepo {
    pub fn debug_local_repo(&mut self) -> Option<PathBuf> {
        self.temp_dir.take().map(|tmp| tmp.keep().join("local"))
    }

    pub fn path(&self) -> &std::path::Path {
        self.local_repo.workdir().expect("non-bare worktree")
    }

    pub fn push_branch(&self, branch: &LocalRefname) {
        let mut origin = self.local_repo.find_remote("origin").unwrap();
        origin.push(&[&format!("{branch}:{branch}")], None).unwrap();
        sync_origin_refs(&self.local_repo).unwrap();
    }

    pub fn push(&self) {
        let mut origin = self.local_repo.find_remote("origin").unwrap();
        origin
            .push(&["refs/heads/master:refs/heads/master"], None)
            .unwrap();
        sync_origin_refs(&self.local_repo).unwrap();
    }

    pub fn reset_hard(&self, oid: Option<git2::Oid>) {
        let mut index = self.local_repo.index().expect("failed to get index");
        index
            .add_all(["."], git2::IndexAddOption::DEFAULT, None)
            .expect("failed to add all");
        index.write().expect("failed to write index");

        let head = self.local_repo.head().unwrap();
        let commit = oid.map_or(head.peel_to_commit().unwrap(), |oid| {
            self.local_repo.find_commit(oid).unwrap()
        });

        self.local_repo
            .reset(commit.as_object(), git2::ResetType::Hard, None)
            .unwrap();
    }

    pub fn checkout_commit(&self, commit_oid: git2::Oid) {
        let commit = self.local_repo.find_commit(commit_oid).unwrap();
        let commit_tree = commit.tree().unwrap();

        self.local_repo.set_head_detached(commit_oid).unwrap();
        self.local_repo
            .checkout_tree(
                commit_tree.as_object(),
                Some(git2::build::CheckoutBuilder::new().force()),
            )
            .unwrap();
    }

    pub fn checkout(&self, branch: &LocalRefname) {
        let refname: Refname = branch.into();
        let head_commit = self.local_repo.head().unwrap().peel_to_commit().unwrap();
        let tree = match maybe_find_branch_by_refname(&self.local_repo, &refname) {
            Ok(branch) => match branch {
                Some(branch) => branch.get().peel_to_tree().unwrap(),
                None => {
                    self.local_repo
                        .reference(&refname.to_string(), head_commit.id(), false, "new branch")
                        .unwrap();
                    head_commit.tree().unwrap()
                }
            },
            Err(error) => panic!("{error:?}"),
        };
        self.local_repo.set_head(&refname.to_string()).unwrap();
        self.local_repo
            .checkout_tree(
                tree.as_object(),
                Some(git2::build::CheckoutBuilder::new().force()),
            )
            .unwrap();
    }

    pub fn commit_all(&self, message: &str) -> git2::Oid {
        let head = self.local_repo.head().unwrap();
        let mut index = self.local_repo.index().expect("failed to get index");
        index
            .add_all(["."], git2::IndexAddOption::DEFAULT, None)
            .expect("failed to add all");
        index.write().expect("failed to write index");
        let tree_id = index.write_tree().expect("failed to write tree");
        let tree = self.local_repo.find_tree(tree_id).expect("tree exists");
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        let parent = self
            .local_repo
            .find_commit(
                self.local_repo
                    .refname_to_id("HEAD")
                    .expect("failed to get head"),
            )
            .expect("failed to find parent commit");
        self.local_repo
            .commit(
                Some(head.name().expect("head has name")),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent],
            )
            .expect("failed to commit")
    }

    pub fn add_submodule(&self, url: &gitbutler_url::Url, path: &path::Path) {
        let mut submodule = self
            .local_repo
            .submodule(&url.to_string(), path.as_ref(), false)
            .unwrap();
        let repo = submodule.open().unwrap();

        repo.find_remote("origin")
            .unwrap()
            .fetch(&["+refs/heads/*:refs/heads/*"], None, None)
            .unwrap();
        let reference = repo.find_reference("refs/heads/master").unwrap();
        let reference_head = repo.find_commit(reference.target().unwrap()).unwrap();
        repo.checkout_tree(reference_head.tree().unwrap().as_object(), None)
            .unwrap();

        repo.set_head("refs/heads/master").unwrap();
        submodule.add_finalize().unwrap();
    }

    pub fn references(&self) -> Vec<git2::Reference<'_>> {
        self.local_repo
            .references()
            .expect("failed to get references")
            .collect::<Result<Vec<_>, _>>()
            .expect("failed to read references")
    }
}

struct Test {
    repo: TestRepo,
    project_id: ProjectHandleOrLegacyProjectId,
    data_dir: Option<TempDir>,
    ctx: Context,
}

impl Test {
    pub fn new_with_settings(change_settings: fn(&mut AppSettings)) -> Self {
        let data_dir = tempdir().expect("tempdir exists");

        let test_project = TestRepo::default();
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

fn setup_config(config: &git2::Config) -> anyhow::Result<()> {
    match config.open_level(git2::ConfigLevel::Local) {
        Ok(mut local) => {
            local.set_str("commit.gpgsign", "false")?;
            local.set_str("user.name", "gitbutler-test")?;
            local.set_str("user.email", "gitbutler-test@example.com")?;
            local.set_str(storage_path_config_key(), "gitbutler")?;
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

fn sync_origin_refs(repo: &git2::Repository) -> anyhow::Result<()> {
    let remote = repo.find_remote("origin")?;
    let url = remote
        .url()
        .ok_or_else(|| anyhow::anyhow!("origin must have a URL"))?;
    let remote_path = if let Some(path) = url.strip_prefix("file://") {
        PathBuf::from(path)
    } else {
        repo.path().parent().expect("git dir in worktree").join(url)
    };
    let remote_repo = git2::Repository::open(remote_path)?;
    for reference in remote_repo.references_glob("refs/heads/*")? {
        let reference = reference?;
        let name = reference.name().expect("reference has name");
        let short = name
            .strip_prefix("refs/heads/")
            .expect("head ref prefix is present");
        repo.reference(
            &format!("refs/remotes/origin/{short}"),
            reference.target().expect("direct ref target"),
            true,
            "sync origin refs for tests",
        )?;
    }
    Ok(())
}

fn maybe_find_branch_by_refname<'repo>(
    repo: &'repo git2::Repository,
    name: &Refname,
) -> anyhow::Result<Option<git2::Branch<'repo>>> {
    let branch = repo.find_branch(
        &name.simple_name(),
        match name {
            Refname::Virtual(_) | Refname::Local(_) | Refname::Other(_) => git2::BranchType::Local,
            Refname::Remote(_) => git2::BranchType::Remote,
        },
    );
    match branch {
        Ok(branch) => Ok(Some(branch)),
        Err(err) if err.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(err) => Err(err.into()),
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
