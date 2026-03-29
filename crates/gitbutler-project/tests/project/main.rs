use std::path::{Path, PathBuf};

use anyhow::Result;
use but_core::RepositoryExt;
use but_testsupport::{CommandExt, git_at_dir};
use tempfile::TempDir;

#[path = "../support.rs"]
mod support;

pub fn new() -> TempDir {
    support::data_dir()
}

fn repo_path_at(name: &str) -> PathBuf {
    but_testsupport::gix_testtools::scripted_fixture_read_only("various-repositories.sh")
        .unwrap()
        .join(name)
}

fn writable_fixture() -> TempDir {
    but_testsupport::gix_testtools::scripted_fixture_writable("various-repositories.sh").unwrap()
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
    repo.write_local_common_config(&repo.config_snapshot())?;
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

    #[test]
    fn get_recreates_configured_storage_dir() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let configured_repo = set_storage_path_config(repo.path(), "gitbutler-custom")?;
        let expected_gb_dir = configured_repo
            .git_dir()
            .canonicalize()?
            .join("gitbutler-custom");

        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), repo.path())?.unwrap_project();
        let gb_dir = project.open_isolated_repo()?.gitbutler_storage_path()?;
        assert_eq!(gb_dir, expected_gb_dir);
        std::fs::remove_dir_all(&gb_dir)?;
        assert!(!gb_dir.exists(), "sanity check");

        let _project = gitbutler_project::get_with_path(data_dir.path(), project.id)?;
        assert!(gb_dir.exists(), "storage dir should be recreated on get");
        Ok(())
    }

    #[test]
    fn submodule_is_added_as_project() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let fixture = writable_fixture();
        let superproject = fixture.path().join("with-submodule").canonicalize()?;
        let submodule = superproject.join("submodule");
        let expected_git_dir = superproject.join(".git/modules/submodule").canonicalize()?;

        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), &submodule)?.unwrap_project();

        assert_eq!(project.git_dir(), expected_git_dir.as_path());
        assert_eq!(
            serde_json::to_value(&project)?["path"],
            serde_json::Value::String(submodule.display().to_string()),
            "path is the worktree directory (and deprecated, hence the access workaround)"
        );
        assert_eq!(
            project.open_isolated_repo()?.gitbutler_storage_path()?,
            expected_git_dir.join("gitbutler")
        );

        let loaded = gitbutler_project::get_with_path(data_dir.path(), project.id)?;
        assert_eq!(loaded.git_dir(), expected_git_dir.as_path());
        Ok(())
    }

    #[test]
    fn best_effort_adds_submodule_even_if_superproject_exists() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let fixture = writable_fixture();
        let superproject = fixture.path().join("with-submodule").canonicalize()?;
        let submodule = superproject.join("submodule");
        let expected_submodule_git_dir =
            superproject.join(".git/modules/submodule").canonicalize()?;
        let parent = gitbutler_project::add_at_app_data_dir(data_dir.path(), &superproject)?
            .unwrap_project();

        let outcome =
            gitbutler_project::add_with_best_effort_at_app_data_dir(data_dir.path(), &submodule)?;
        let project = match outcome {
            gitbutler_project::AddProjectOutcome::Added(project) => project,
            other => panic!("expected submodule to be added, got {other:?}"),
        };

        assert_ne!(
            project.id, parent.id,
            "parent and super projects are distinct"
        );
        assert_eq!(project.git_dir(), expected_submodule_git_dir.as_path());
        Ok(())
    }

    #[test]
    fn best_effort_adds_parent_repo_from_nested_directory() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let nested_dir = repo.path().join("nested/inside");
        let expected_worktree_dir = repo.path().canonicalize()?;
        let expected_git_dir = repo_git_dir(repo.path())?;
        std::fs::create_dir_all(&nested_dir)?;

        let outcome =
            gitbutler_project::add_with_best_effort_at_app_data_dir(data_dir.path(), &nested_dir)?;
        let project = match outcome {
            gitbutler_project::AddProjectOutcome::Added(project) => project,
            other => panic!("expected parent repo to be added, got {other:?}"),
        };

        assert_eq!(project.git_dir(), expected_git_dir.as_path());
        assert_eq!(
            serde_json::to_value(&project)?["path"],
            serde_json::Value::String(expected_worktree_dir.display().to_string())
        );
        Ok(())
    }

    /// Used in deep-links for instance
    #[test]
    fn best_effort_finds_existing_project_from_file_path() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), repo.path())?.unwrap_project();
        let file_path = repo.path().join("nested/inside/file.txt");
        std::fs::create_dir_all(file_path.parent().expect("file has parent"))?;
        std::fs::write(&file_path, "hello world")?;

        let outcome =
            gitbutler_project::add_with_best_effort_at_app_data_dir(data_dir.path(), &file_path)?;
        let existing_project = match outcome {
            gitbutler_project::AddProjectOutcome::AlreadyExists(project) => project,
            other => panic!("expected existing project to be found, got {other:?}"),
        };

        assert_eq!(
            existing_project.id, project.id,
            "it finds the containing project even if a filepath is given"
        );
        assert_eq!(existing_project.git_dir(), project.git_dir());
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
        fn nested_directory_inside_repo_is_not_added_by_exact_path_but_is_by_best_effort() {
            let data_dir = support::data_dir();
            let repo = support::TestProject::default();
            let nested_dir = repo.path().join("nested/inside");
            std::fs::create_dir_all(&nested_dir).unwrap();
            let project = gitbutler_project::add_at_app_data_dir(data_dir.path(), repo.path())
                .unwrap()
                .unwrap_project();

            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), &nested_dir).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NotAGitRepository(_)));

            let outcome = gitbutler_project::add_with_best_effort_at_app_data_dir(
                data_dir.path(),
                &nested_dir,
            )
            .unwrap();
            let existing_project = match outcome {
                AddProjectOutcome::AlreadyExists(project) => project,
                other => panic!("expected owning project to be found, got {other:?}"),
            };
            assert_eq!(
                existing_project.id, project.id,
                "With best effort, we find the surrounding project, it's like discover"
            );
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
    }
}

mod delete {
    use super::*;
    #[test]
    fn success() {
        let data_dir = support::data_dir();
        let repo = support::TestProject::default();
        let path = repo.path();
        let project = gitbutler_project::add_at_app_data_dir(data_dir.path(), path)
            .unwrap()
            .unwrap_project();
        let project_id = project.id.clone();
        assert!(gitbutler_project::delete_with_path(data_dir.path(), project_id.clone()).is_ok());
        assert!(gitbutler_project::delete_with_path(data_dir.path(), project_id.clone()).is_ok()); // idempotent
        assert!(gitbutler_project::get_with_path(data_dir.path(), project_id).is_err());
        assert!(repo.path().exists());
        assert!(
            !project
                .open_isolated_repo()
                .unwrap()
                .gitbutler_storage_path()
                .unwrap()
                .exists()
        );
    }

    #[test]
    fn submodule_success_without_accidentally_removing_submodule() -> anyhow::Result<()> {
        let data_dir = support::data_dir();
        let fixture = writable_fixture();
        let submodule = fixture
            .path()
            .join("with-submodule")
            .join("submodule")
            .canonicalize()?;
        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), &submodule)?.unwrap_project();
        let project_id = project.id.clone();
        let gb_dir = project.open_isolated_repo()?.gitbutler_storage_path()?;

        assert_eq!(gb_dir, project.git_dir().join("gitbutler"));
        gitbutler_project::delete_with_path(data_dir.path(), project_id.clone())?;
        assert!(submodule.exists());
        assert!(!gb_dir.exists());
        assert!(gitbutler_project::get_with_path(data_dir.path(), project_id).is_err());
        Ok(())
    }

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

        insta::assert_debug_snapshot!(all_refs(&repo)?, @r#"
        [
            "refs/gitbutler/test-ref",
            "refs/heads/gitbutler/workspace",
            "refs/heads/master",
            "refs/heads/unrelated",
            "refs/remotes/origin/master",
        ]
        "#);

        gitbutler_project::delete_with_path(data_dir.path(), project.id)?;

        // Only only sees gitbutler references.
        insta::assert_debug_snapshot!(all_refs(&repo)?, @r#"
        [
            "refs/heads/master",
            "refs/heads/unrelated",
            "refs/remotes/origin/master",
        ]
        "#);
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
        insta::assert_debug_snapshot!(all_refs(&repo)?, @r#"
        [
            "refs/heads/master",
            "refs/heads/unrelated",
            "refs/remotes/origin/master",
        ]
        "#);

        gitbutler_project::delete_with_path(data_dir.path(), project.id)?;

        assert!(repo.path().exists());
        assert!(!repo.gitbutler_storage_path()?.exists());

        // Nothing changed - no reference was touched.
        insta::assert_debug_snapshot!(all_refs(&repo)?, @r#"
        [
            "refs/heads/master",
            "refs/heads/unrelated",
            "refs/remotes/origin/master",
        ]
        "#);

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
