use std::path::PathBuf;

use but_core::RepositoryExt;
use gitbutler_testsupport::{self, paths};
use tempfile::TempDir;

pub fn new() -> TempDir {
    paths::data_dir()
}

fn storage_key() -> String {
    but_project_handle::storage_path_config_key().to_owned()
}

fn repo_path_at(name: &str) -> PathBuf {
    gitbutler_testsupport::gix_testtools::scripted_fixture_read_only("various-repositories.sh")
        .unwrap()
        .join(name)
}

fn writable_fixture() -> TempDir {
    gitbutler_testsupport::gix_testtools::scripted_fixture_writable("various-repositories.sh")
        .unwrap()
}

mod add {
    use super::*;

    #[test]
    fn success() -> anyhow::Result<()> {
        let tmp = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
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
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
        let key = storage_key();
        git2::Repository::open(repo.path())?
            .config()?
            .set_str(&key, "gitbutler-custom")?;

        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), repo.path())?.unwrap_project();
        let gb_dir = project.open_isolated_repo()?.gitbutler_storage_path()?;
        assert!(gb_dir.exists());
        Ok(())
    }

    #[test]
    fn get_recreates_configured_storage_dir() -> anyhow::Result<()> {
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
        let key = storage_key();
        git2::Repository::open(repo.path())?
            .config()?
            .set_str(&key, "gitbutler-custom")?;

        let project =
            gitbutler_project::add_at_app_data_dir(data_dir.path(), repo.path())?.unwrap_project();
        let gb_dir = project.open_isolated_repo()?.gitbutler_storage_path()?;
        std::fs::remove_dir_all(&gb_dir)?;
        assert!(!gb_dir.exists(), "sanity check");

        let _project = gitbutler_project::get_with_path(data_dir.path(), project.id)?;
        assert!(gb_dir.exists(), "storage dir should be recreated on get");
        Ok(())
    }

    #[test]
    fn submodule_is_added_as_project() -> anyhow::Result<()> {
        let data_dir = paths::data_dir();
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
        let data_dir = paths::data_dir();
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
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
        let nested_dir = repo.path().join("nested/inside");
        let expected_worktree_dir = repo.path().canonicalize()?;
        let expected_git_dir = git2::Repository::open(repo.path())?.path().canonicalize()?;
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
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
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
            let tmp = paths::data_dir();
            let root = repo_path_at("non-bare-without-worktree");
            let outcome =
                gitbutler_project::add_at_app_data_dir(tmp.path(), root.as_path()).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NoWorkdir));
        }

        #[test]
        fn missing() {
            let data_dir = paths::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), tmp.path().join("missing"))
                    .unwrap();
            assert!(matches!(outcome, AddProjectOutcome::PathNotFound));
        }

        #[test]
        fn directory_without_git() {
            let data_dir = paths::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let path = tmp.path();
            std::fs::write(path.join("file.txt"), "hello world").unwrap();
            let outcome = gitbutler_project::add_at_app_data_dir(data_dir.path(), path).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NotAGitRepository(_)));
        }

        #[test]
        fn empty() {
            let data_dir = paths::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), tmp.path()).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NotAGitRepository(_)));
        }

        #[test]
        fn nested_directory_inside_repo_is_not_added_by_exact_path_but_is_by_best_effort() {
            let data_dir = paths::data_dir();
            let repo = gitbutler_testsupport::TestProject::default();
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
            let data_dir = paths::data_dir();
            let repo = gitbutler_testsupport::TestProject::default();
            let path = repo.path();
            gitbutler_project::add_at_app_data_dir(data_dir.path(), path).unwrap();

            let outcome = gitbutler_project::add_at_app_data_dir(data_dir.path(), path).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::AlreadyExists(_)));
        }

        #[test]
        fn bare() {
            let data_dir = paths::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let repo_dir = tmp.path().join("bare");

            let repo = git2::Repository::init_bare(&repo_dir).unwrap();
            create_initial_commit(&repo);

            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), repo_dir.as_path())
                    .unwrap();
            assert!(matches!(outcome, AddProjectOutcome::BareRepository));
        }

        #[test]
        fn worktree() {
            let data_dir = paths::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let main_worktree_dir = tmp.path().join("main");
            let worktree_dir = tmp.path().join("worktree");

            let repo = git2::Repository::init(main_worktree_dir).unwrap();
            create_initial_commit(&repo);

            let worktree = repo.worktree("feature", &worktree_dir, None).unwrap();
            let outcome =
                gitbutler_project::add_at_app_data_dir(data_dir.path(), worktree.path()).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NonMainWorktree));
        }

        fn create_initial_commit(repo: &git2::Repository) -> git2::Oid {
            let signature = git2::Signature::now("test", "test@email.com").unwrap();

            let mut index = repo.index().unwrap();
            let oid = index.write_tree().unwrap();

            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                "initial commit",
                &repo.find_tree(oid).unwrap(),
                &[],
            )
            .unwrap()
        }
    }
}

mod delete {
    use super::*;
    #[test]
    fn success() {
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
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
        let data_dir = paths::data_dir();
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
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
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
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
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
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
        let key = storage_key();
        git2::Repository::open(repo.path())?
            .config()?
            .set_str(&key, "gitbutler-custom")?;
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
        let data_dir = paths::data_dir();
        let repo = gitbutler_testsupport::TestProject::default();
        let key = storage_key();
        let git_dir = git2::Repository::open(repo.path())?.path().to_path_buf();
        git2::Repository::open(repo.path())?
            .config()?
            .set_str(&key, ".")?;
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
