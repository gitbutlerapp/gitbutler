pub const VAR_NO_CLEANUP: &str = "GITBUTLER_TESTS_NO_CLEANUP";

use but_ctx::Context;
use but_meta::VirtualBranchesTomlMetadata;
use but_workspace::{legacy::StacksFilter, ui::StackDetails};
use gix::bstr::BStr;
/// Direct access to lower-level utilities for cases where this is enough.
///
/// Prefer to use [`read_only`] and [`writable`] otherwise.
pub use gix_testtools;

mod test_project;
pub use test_project::TestProject;

mod suite;
pub use suite::*;

pub mod testing_repository;

pub mod paths {
    use tempfile::TempDir;

    use super::temp_dir;

    pub fn data_dir() -> TempDir {
        temp_dir()
    }
}

pub mod virtual_branches {
    use but_ctx::Context;
    use gitbutler_stack::{Target, VirtualBranchesHandle};

    use crate::empty_bare_repository;

    pub fn set_test_target(ctx: &Context) -> anyhow::Result<()> {
        let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
        let (remote_repo, _tmp) = empty_bare_repository();
        let git2_repo = &*ctx.git2_repo.get()?;
        let mut remote = git2_repo
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

        gitbutler_branch_actions::update_workspace_commit(&vb_state, ctx, false)
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
    use but_ctx::Context;
    use but_settings::AppSettings;
    use gitbutler_project::Project;
    use tempfile::TempDir;

    use crate::{BUT_DRIVER, DRIVER};

    pub fn fixture(
        script_name: &str,
        project_directory: &str,
    ) -> anyhow::Result<(Context, TempDir)> {
        fixture_with_settings(script_name, project_directory, |_| {})
    }
    pub fn fixture_with_settings(
        script_name: &str,
        project_directory: &str,
        change_settings: fn(&mut AppSettings),
    ) -> anyhow::Result<(Context, TempDir)> {
        let (project, tempdir) = fixture_project(script_name, project_directory)?;
        let mut settings = AppSettings::default();
        change_settings(&mut settings);
        let ctx = Context::new_from_legacy_project_and_settings(&project, settings);
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

        let project = Project::new_for_gitbutler_testsupport(
            project_directory.to_owned(),
            root.path().join(project_directory),
        );
        Ok((project, root))
    }

    /// Use the `but` CLI instead of `gitbutler-cli` for fixtures that need stacking support.
    pub fn but_fixture(
        script_name: &str,
        project_directory: &str,
    ) -> anyhow::Result<(Context, TempDir)> {
        but_fixture_with_settings(script_name, project_directory, |_| {})
    }

    /// Use the `but` CLI instead of `gitbutler-cli` for fixtures that need stacking support.
    pub fn but_fixture_with_settings(
        script_name: &str,
        project_directory: &str,
        change_settings: fn(&mut AppSettings),
    ) -> anyhow::Result<(Context, TempDir)> {
        let (project, tempdir) = but_fixture_project(script_name, project_directory)?;
        let mut settings = AppSettings::default();
        change_settings(&mut settings);
        let ctx = Context::new_from_legacy_project_and_settings(&project, settings);
        Ok((ctx, tempdir))
    }

    /// Use the `but` CLI instead of `gitbutler-cli` for fixtures that need stacking support.
    pub fn but_fixture_project(
        script_name: &str,
        project_directory: &str,
    ) -> anyhow::Result<(Project, TempDir)> {
        let root = gix_testtools::scripted_fixture_writable_with_args(
            script_name,
            Some(BUT_DRIVER.display().to_string()),
            gix_testtools::Creation::ExecuteScript,
        )
        .expect("script execution always succeeds");

        let project = Project::new_for_gitbutler_testsupport(
            project_directory.to_owned(),
            root.path().join(project_directory),
        );
        Ok((project, root))
    }
}
/// Display a Git tree in the style of the `tree` CLI program, but include blob contents and usful Git metadata.
pub fn visualize_gix_tree(tree_id: gix::Id<'_>) -> termtree::Tree<String> {
    fn visualize_tree(
        id: gix::Id<'_>,
        name_and_mode: Option<(&BStr, gix::object::tree::EntryMode)>,
    ) -> anyhow::Result<termtree::Tree<String>> {
        fn short_id(id: &gix::hash::oid) -> String {
            id.to_hex_with_len(7).to_string()
        }
        let repo = id.repo;
        let entry_name =
            |id: &gix::hash::oid, name: Option<(&BStr, gix::object::tree::EntryMode)>| -> String {
                match name {
                    None => short_id(id),
                    Some((name, mode)) => {
                        format!(
                            "{name}:{mode}{} {}",
                            short_id(id),
                            match repo.find_blob(id) {
                                Ok(blob) => format!("{:?}", blob.data.as_bstr()),
                                Err(_) => "".into(),
                            },
                            mode = if mode.is_tree() {
                                "".into()
                            } else {
                                format!("{:o}:", mode.value())
                            }
                        )
                    }
                }
            };

        let mut tree = termtree::Tree::new(entry_name(&id, name_and_mode));
        for entry in repo.find_tree(id)?.iter() {
            let entry = entry?;
            if entry.mode().is_tree() {
                tree.push(visualize_tree(
                    entry.id(),
                    Some((entry.filename(), entry.mode())),
                )?);
            } else {
                tree.push(entry_name(
                    entry.oid(),
                    Some((entry.filename(), entry.mode())),
                ));
            }
        }
        Ok(tree)
    }
    visualize_tree(tree_id.object().unwrap().peel_to_tree().unwrap().id(), None).unwrap()
}

/// Visualize a git2 tree, otherwise just like [`visualize_gix_tree()`].
pub fn visualize_git2_tree(tree_id: git2::Oid, repo: &git2::Repository) -> termtree::Tree<String> {
    let repo = gix::open_opts(repo.path(), gix::open::Options::isolated()).unwrap();
    visualize_gix_tree(git2_to_gix_object_id(tree_id).attach(&repo))
}

pub fn stack_details(ctx: &Context) -> Vec<(StackId, StackDetails)> {
    let repo = ctx.clone_repo_for_merging_non_persisting().unwrap();
    let stacks = {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project_data_dir().join("virtual_branches.toml"),
        )
        .unwrap();
        but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None)
    }
    .unwrap();
    let mut details = vec![];
    for stack in stacks {
        let stack_id = stack
            .id
            .expect("BUG(opt-stack-id): test code shouldn't trigger this");
        details.push((
            stack_id,
            {
                let meta = VirtualBranchesTomlMetadata::from_path(
                    ctx.project_data_dir().join("virtual_branches.toml"),
                )
                .unwrap();
                but_workspace::legacy::stack_details_v3(stack_id.into(), &repo, &meta)
            }
            .unwrap(),
        ));
    }
    details
}

pub mod read_only {
    use std::collections::BTreeSet;

    use but_ctx::Context;
    use but_settings::{AppSettings, app_settings::FeatureFlags};
    use gitbutler_project::Project;
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;

    use crate::DRIVER;

    /// Execute the script at `script_name.sh` (assumed to be located in `tests/fixtures/<script_name>`)
    /// and make the command-line application available to it. That way the script can perform GitButler
    /// operations and leave relevant files around statically.
    /// Use `project_directory` to define where the project is located within the directory containing
    /// the output of `script_name`.
    ///
    /// Returns the project that is strictly for read-only use.
    pub fn fixture(script_name: &str, project_directory: &str) -> anyhow::Result<Context> {
        let project = fixture_project(script_name, project_directory)?;
        Ok(Context::new_from_legacy_project_and_settings(
            &project,
            AppSettings::default(),
        ))
    }

    /// As [fixture()], but allows setting `features` in the app settings
    pub fn fixture_with_features(
        script_name: &str,
        project_directory: &str,
        features: FeatureFlags,
    ) -> anyhow::Result<Context> {
        let project = fixture_project(script_name, project_directory)?;
        Ok(Context::new_from_legacy_project_and_settings(
            &project,
            AppSettings {
                feature_flags: features,
                ..Default::default()
            },
        ))
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
            let outcome =
                gitbutler_project::add_at_app_data_dir(tmp.path(), project_worktree_dir.as_path())?;
            outcome.try_project()?
        } else {
            Project::new_for_gitbutler_testsupport(
                project_directory.to_owned(),
                project_worktree_dir,
            )
        };
        Ok(project)
    }
}

use std::path::{Path, PathBuf};

use but_oxidize::git2_to_gix_object_id;
use gitbutler_stack::StackId;
use gix::{bstr::ByteSlice, prelude::ObjectIdExt};
use once_cell::sync::Lazy;

pub(crate) static DRIVER: Lazy<PathBuf> = Lazy::new(|| {
    let mut cargo = std::process::Command::new(env!("CARGO"));
    let res = cargo
        .args(["build", "-p=gitbutler-cli", "--features=testing"])
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

pub(crate) static BUT_DRIVER: Lazy<PathBuf> = Lazy::new(|| {
    let mut cargo = std::process::Command::new(env!("CARGO"));
    let res = cargo
        .args(["build", "-p=but"])
        .status()
        .expect("cargo should run fine");
    assert!(res.success(), "cargo invocation should be successful");

    let path = Path::new("../../target")
        .join("debug")
        .join(if cfg!(windows) { "but.exe" } else { "but" });
    assert!(
        path.is_file(),
        "Expecting but driver to be located at {path:?} - we also assume a certain crate location"
    );
    path.canonicalize()
        .expect("canonicalization works as the CWD is valid and there are no symlinks to resolve")
});

/// A secrets store to prevent secrets to be written into the systems own store.
///
/// Note that this can't be used if secrets themselves are under test as it' doesn't act
/// like a real store, i.e. stored secrets can't be retrieved anymore.
pub mod secrets;
