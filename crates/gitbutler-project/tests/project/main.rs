use std::path::{Path, PathBuf};

use anyhow::Result;
use but_core::RepositoryExt;
use but_testsupport::{CommandExt, git_at_dir};
use tempfile::TempDir;

mod support;

pub fn new() -> TempDir {
    support::data_dir()
}

fn repo_path_at(name: &str) -> PathBuf {
    but_testsupport::gix_testtools::scripted_fixture_read_only("various-repositories.sh")
        .unwrap()
        .join(name)
}

fn repo_git_dir(path: &Path) -> Result<PathBuf> {
    let repo = gix::open_opts(path, gix::open::Options::isolated())?;
    Ok(repo.git_dir().canonicalize()?)
}

fn set_storage_path_config(
    repo_path: &Path,
    value: impl AsRef<std::ffi::OsStr>,
) -> anyhow::Result<gix::Repository> {
    let mut repo = but_testsupport::open_repo(repo_path)?;
    let key = but_project_handle::storage_path_config_key();
    repo.config_snapshot_mut()
        .set_raw_value(key, gix::path::os_str_into_bstr(value.as_ref())?)?;
    let (_config, lock) = repo.local_common_config_for_editing()?;
    repo.write_locked_config(&repo.config_snapshot(), lock)?;
    Ok(repo)
}

mod add {
    use super::*;

    #[test]
    fn success() -> anyhow::Result<()> {
        let tmp = support::data_dir();
        let repo = support::TestProject::default();
        let path = repo.path();
        let project = gitbutler_project::add_at_app_data_dir(tmp.path(), path)
            .unwrap()
            .unwrap_project();
        assert_eq!(
            project.title,
            path.iter().next_back().unwrap().to_str().unwrap()
        );
        Ok(())
    }

    #[test]
    fn creates_configured_storage_dir() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let configured_repo = set_storage_path_config(repo.path(), "gitbutler-custom")?;
        let expected_gb_dir = configured_repo
            .git_dir()
            .canonicalize()?
            .join("gitbutler-custom");

        assert!(!expected_gb_dir.exists());
        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), repo.path())?.unwrap_project();
        let gb_dir = project.open_isolated_repo()?.gitbutler_storage_path()?;
        assert_eq!(gb_dir, expected_gb_dir);
        assert!(gb_dir.exists());
        Ok(())
    }

    mod error {
        use gitbutler_project::AddProjectOutcome;

        use super::*;

        #[test]
        fn non_bare_without_worktree() {
            let tmp = support::data_dir();
            let root = repo_path_at("non-bare-without-worktree");
            let outcome =
                gitbutler_project::add_at_app_data_dir(tmp.path(), root.as_path()).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NoWorkdir));
        }

        #[test]
        fn missing() {
            let data_dir = support::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), tmp.path().join("missing"))
                    .unwrap();
            assert!(matches!(outcome, AddProjectOutcome::PathNotFound));
        }

        #[test]
        fn directory_without_git() {
            let data_dir = support::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let path = tmp.path();
            std::fs::write(path.join("file.txt"), "hello world").unwrap();
            let outcome = gitbutler_project::add_at_app_data_dir(data_dir.path(), path).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NotAGitRepository(_)));
        }

        #[test]
        fn empty() {
            let data_dir = support::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), tmp.path()).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NotAGitRepository(_)));
        }

        #[test]
        fn twice() {
            let data_dir = support::data_dir();
            let repo = support::TestProject::default();
            let path = repo.path();
            gitbutler_project::add_at_app_data_dir(data_dir.path(), path).unwrap();

            let outcome = gitbutler_project::add_at_app_data_dir(data_dir.path(), path).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::AlreadyExists(_)));
        }

        #[test]
        fn bare() {
            let data_dir = support::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let repo_dir = tmp.path().join("bare");

            git_at_dir(tmp.path())
                .args(["init", "--bare"])
                .arg(&repo_dir)
                .run();

            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), repo_dir.as_path())
                    .unwrap();
            assert!(matches!(outcome, AddProjectOutcome::BareRepository));
        }

        #[test]
        fn worktree() {
            let data_dir = support::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let main_worktree_dir = tmp.path().join("main");
            let worktree_dir = tmp.path().join("worktree");

            git_at_dir(tmp.path())
                .args(["init"])
                .arg(&main_worktree_dir)
                .run();
            git_at_dir(&main_worktree_dir)
                .args(["commit", "--allow-empty", "-m", "initial commit"])
                .run();
            git_at_dir(&main_worktree_dir)
                .args(["worktree", "add", "-b", "feature"])
                .arg(&worktree_dir)
                .run();
            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), &worktree_dir).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NonMainWorktree));
        }

        #[test]
        fn reftable_ref_format_is_rejected() {
            let data_dir = support::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let repo_dir = tmp.path().join("reftable");

            let init_output = git_at_dir(tmp.path())
                .args(["init", "--ref-format=reftable"])
                .arg(&repo_dir)
                .output()
                .expect("git can be invoked");
            if !init_output.status.success() {
                eprintln!(
                    "Skipping reftable rejection test because this Git cannot initialize a reftable repository: {}",
                    String::from_utf8_lossy(&init_output.stderr)
                );
                return;
            }

            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), &repo_dir).unwrap();
            assert!(matches!(
                outcome,
                AddProjectOutcome::ReftableRefFormatUnsupported
            ));
        }

        #[test]
        fn worktree_reftable_ref_format_does_not_affect_project_add() {
            let data_dir = support::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let repo_dir = tmp.path().join("repo");

            git_at_dir(tmp.path()).args(["init"]).arg(&repo_dir).run();
            git_at_dir(&repo_dir)
                .args(["config", "extensions.worktreeConfig", "true"])
                .run();
            git_at_dir(&repo_dir)
                .args(["config", "--worktree", "extensions.refStorage", "reftable"])
                .run();

            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), &repo_dir).unwrap();
            assert!(
                matches!(outcome, AddProjectOutcome::Added(_)),
                "worktree configuration doesn't say that reftables should be used, only the common config"
            );
        }
    }
}

mod delete {
    use super::*;
    use snapbox::prelude::*;
    #[test]
    fn deletes_gitbutler_references() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let path = repo.path();
        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), path)?.unwrap_project();

        let repo = project.open_isolated_repo()?;
        let head_id = repo.head_id()?;

        // Create references in both namespaces
        repo.reference(
            "refs/heads/gitbutler/workspace",
            head_id,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "test workspace ref",
        )?;

        let head_id = repo.head_id()?;

        repo.reference(
            "refs/heads/unrelated",
            head_id,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "unrelated workspace ref",
        )?;

        repo.reference(
            "refs/gitbutler/test-ref",
            head_id,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "hidden gitbutler ref",
        )?;

        snapbox::assert_data_eq!(
            all_refs(&repo)?.to_debug(),
            snapbox::str![[r#"
[
    "refs/gitbutler/test-ref",
    "refs/heads/gitbutler/workspace",
    "refs/heads/master",
    "refs/heads/unrelated",
    "refs/remotes/origin/master",
]

"#]]
        );

        gitbutler_project::delete_with_path(data_dir.path(), project.id)?;

        // Only only sees gitbutler references.
        snapbox::assert_data_eq!(
            all_refs(&repo)?.to_debug(),
            snapbox::str![[r#"
[
    "refs/heads/master",
    "refs/heads/unrelated",
    "refs/remotes/origin/master",
]

"#]]
        );
        Ok(())
    }

    #[test]
    fn deletes_project_without_gitbutler_references() -> anyhow::Result<()> {
        // This test ensures that deletion works even when there are no gitbutler references
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let path = repo.path();
        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), path)?.unwrap_project();

        let repo = project.open_isolated_repo()?;
        let head_id = repo.head_id()?;

        repo.reference(
            "refs/heads/unrelated",
            head_id,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "unrelated workspace ref",
        )?;
        snapbox::assert_data_eq!(
            all_refs(&repo)?.to_debug(),
            snapbox::str![[r#"
[
    "refs/heads/master",
    "refs/heads/unrelated",
    "refs/remotes/origin/master",
]

"#]]
        );

        gitbutler_project::delete_with_path(data_dir.path(), project.id)?;

        assert!(repo.path().exists());
        assert!(!repo.gitbutler_storage_path()?.exists());

        // Nothing changed - no reference was touched.
        snapbox::assert_data_eq!(
            all_refs(&repo)?.to_debug(),
            snapbox::str![[r#"
[
    "refs/heads/master",
    "refs/heads/unrelated",
    "refs/remotes/origin/master",
]

"#]]
        );

        Ok(())
    }

    #[test]
    fn removes_configured_storage_dir() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let path = repo.path();
        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), path)?.unwrap_project();
        let gb_dir = project.open_isolated_repo()?.gitbutler_storage_path()?;
        assert!(gb_dir.exists());

        gitbutler_project::delete_with_path(data_dir.path(), project.id)?;
        assert!(!gb_dir.exists());
        Ok(())
    }

    #[test]
    fn refuses_to_delete_git_dir_when_storage_path_points_to_dot_git() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let git_dir = repo_git_dir(repo.path())?;
        let repo_after_config = set_storage_path_config(repo.path(), ".")?;
        assert!(
            repo_after_config.gitbutler_storage_path().is_err(),
            "sanity check: '.' must be rejected as storage path"
        );
        let path = repo.path();
        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), path)?.unwrap_project();

        gitbutler_project::delete_with_path(data_dir.path(), project.id)?;

        assert!(
            git_dir.exists(),
            "the repository .git directory must remain"
        );
        assert!(
            git_dir.join("objects").exists(),
            "git internals must remain after project deletion"
        );
        Ok(())
    }

    fn all_refs(repo: &gix::Repository) -> anyhow::Result<Vec<String>> {
        Ok(repo
            .references()?
            .all()?
            .map(|r| r.unwrap().name().as_bstr().to_string())
            .collect())
    }
}
