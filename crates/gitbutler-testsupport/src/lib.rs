#![forbid(rust_2018_idioms)]
pub const VAR_NO_CLEANUP: &str = "GITBUTLER_TESTS_NO_CLEANUP";

mod test_project;
pub use test_project::TestProject;

mod suite;
pub use suite::*;

pub mod paths {
    use tempfile::TempDir;

    use super::temp_dir;

    pub fn data_dir() -> TempDir {
        temp_dir()
    }
}

pub mod virtual_branches {
    use gitbutler_branch::{Target, VirtualBranchesHandle};
    use gitbutler_command_context::CommandContext;

    use crate::empty_bare_repository;

    pub fn set_test_target(ctx: &CommandContext) -> anyhow::Result<()> {
        let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
        let (remote_repo, _tmp) = empty_bare_repository();
        let mut remote = ctx
            .repository()
            .remote("origin", remote_repo.path().to_str().unwrap())
            .expect("failed to add remote");
        remote.push(&["refs/heads/master:refs/heads/master"], None)?;

        vb_state
            .set_default_target(Target {
                branch: "refs/remotes/origin/master".parse().unwrap(),
                remote_url: remote_repo.path().to_str().unwrap().parse().unwrap(),
                sha: remote_repo.head().unwrap().target().unwrap(),
                push_remote_name: None,
            })
            .expect("failed to write target");

        gitbutler_branch_actions::update_workspace_commit(&vb_state, ctx)
            .expect("failed to update workspace");

        Ok(())
    }
}

pub fn init_opts() -> git2::RepositoryInitOptions {
    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("master");
    opts
}

pub fn init_opts_bare() -> git2::RepositoryInitOptions {
    let mut opts = init_opts();
    opts.bare(true);
    opts
}

pub mod writable {
    use crate::DRIVER;
    use gitbutler_command_context::CommandContext;
    use gitbutler_project::{Project, ProjectId};
    use tempfile::TempDir;

    pub fn fixture(
        script_name: &str,
        project_directory: &str,
    ) -> anyhow::Result<(CommandContext, TempDir)> {
        let (project, tempdir) = fixture_project(script_name, project_directory)?;
        let ctx = CommandContext::open(&project)?;
        Ok((ctx, tempdir))
    }
    pub fn fixture_project(
        script_name: &str,
        project_directory: &str,
    ) -> anyhow::Result<(Project, TempDir)> {
        let root = gix_testtools::scripted_fixture_writable_with_args(
            script_name,
            Some(DRIVER.display().to_string()),
            gix_testtools::Creation::ExecuteScript,
        )
        .expect("script execution always succeeds");

        let project = Project {
            id: ProjectId::generate(),
            title: project_directory.to_owned(),
            path: root.path().join(project_directory),
            ..Default::default()
        };
        Ok((project, root))
    }
}

pub mod read_only {
    use crate::DRIVER;
    use gitbutler_command_context::CommandContext;
    use gitbutler_project::{Project, ProjectId};
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
    use std::collections::BTreeSet;

    /// Execute the script at `script_name.sh` (assumed to be located in `tests/fixtures/<script_name>`)
    /// and make the command-line application available to it. That way the script can perform GitButler
    /// operations and leave relevant files around statically.
    /// Use `project_directory` to define where the project is located within the directory containing
    /// the output of `script_name`.
    ///
    /// Returns the project that is strictly for read-only use.
    pub fn fixture(script_name: &str, project_directory: &str) -> anyhow::Result<CommandContext> {
        let project = fixture_project(script_name, project_directory)?;
        CommandContext::open(&project)
    }

    /// Like [`fixture()`], but will return only the `Project` at `project_directory` after executing `script_name`.
    pub fn fixture_project(script_name: &str, project_directory: &str) -> anyhow::Result<Project> {
        static IS_VALID_PROJECT: Lazy<Mutex<BTreeSet<(String, String)>>> =
            Lazy::new(|| Mutex::new(Default::default()));

        let root = gix_testtools::scripted_fixture_read_only_with_args(
            script_name,
            Some(DRIVER.display().to_string()),
        )
        .expect("script execution always succeeds");

        let mut is_valid_guard = IS_VALID_PROJECT.lock();
        let was_inserted =
            is_valid_guard.insert((script_name.to_owned(), project_directory.to_owned()));
        let project_worktree_dir = root.join(project_directory);
        // Assure the project is valid the first time.
        let project = if was_inserted {
            let tmp = tempfile::TempDir::new()?;
            gitbutler_project::Controller::from_path(tmp.path()).add(project_worktree_dir)?
        } else {
            Project {
                id: ProjectId::generate(),
                title: project_directory.to_owned(),
                path: project_worktree_dir,
                ..Default::default()
            }
        };
        Ok(project)
    }
}

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
pub(crate) static DRIVER: Lazy<PathBuf> = Lazy::new(|| {
    let mut cargo = std::process::Command::new(env!("CARGO"));
    let res = cargo
        .args(["build", "-p=gitbutler-cli"])
        .status()
        .expect("cargo should run fine");
    assert!(res.success(), "cargo invocation should be successful");

    let path = Path::new("../../target")
        .join("debug")
        .join(if cfg!(windows) {
            "gitbutler-cli.exe"
        } else {
            "gitbutler-cli"
        });
    assert!(
        path.is_file(),
        "Expecting driver to be located at {path:?} - we also assume a certain crate location"
    );
    path.canonicalize()
        .expect("canonicalization works as the CWD is valid and there are no symlinks to resolve")
});

/// A secrets store to prevent secrets to be written into the systems own store.
///
/// Note that this can't be used if secrets themselves are under test as it' doesn't act
/// like a real store, i.e. stored secrets can't be retrieved anymore.
pub mod secrets;
