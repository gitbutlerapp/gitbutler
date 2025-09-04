use gitbutler_testsupport::{self, paths};
use tempfile::TempDir;

pub fn new() -> TempDir {
    paths::data_dir()
}

mod add {
    use super::*;

    #[test]
    fn success() {
        let tmp = paths::data_dir();
        let repository = gitbutler_testsupport::TestProject::default();
        let path = repository.path();
        let project = gitbutler_project::add_with_path(tmp.path(), path)
            .unwrap()
            .unwrap_project();
        assert_eq!(project.path, path);
        assert_eq!(
            project.title,
            path.iter().next_back().unwrap().to_str().unwrap()
        );
    }

    mod error {
        use super::*;
        use gitbutler_project::AddProjectOutcome;
        use std::path::PathBuf;

        #[test]
        fn non_bare_without_worktree() {
            let tmp = paths::data_dir();
            let root = repo_path_at("non-bare-without-worktree");
            let outcome = gitbutler_project::add_with_path(tmp.path(), root.as_path()).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NoWorkdir));
        }

        #[test]
        fn submodule() {
            let tmp = paths::data_dir();
            let root = repo_path_at("with-submodule").join("submodule");
            let outcome = gitbutler_project::add_with_path(tmp.path(), root.as_path()).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NoDotGitDirectory));
        }

        #[test]
        fn missing() {
            let data_dir = paths::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let outcome =
                gitbutler_project::add_with_path(data_dir.path(), &tmp.path().join("missing"))
                    .unwrap();
            assert!(matches!(outcome, AddProjectOutcome::PathNotFound));
        }

        #[test]
        fn directory_without_git() {
            let data_dir = paths::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let path = tmp.path();
            std::fs::write(path.join("file.txt"), "hello world").unwrap();
            let outcome = gitbutler_project::add_with_path(data_dir.path(), path).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NotAGitRepository(_)));
        }

        #[test]
        fn empty() {
            let data_dir = paths::data_dir();
            let tmp = tempfile::tempdir().unwrap();
            let outcome = gitbutler_project::add_with_path(data_dir.path(), tmp.path()).unwrap();
            assert!(matches!(outcome, AddProjectOutcome::NotAGitRepository(_)));
        }

        #[test]
        fn twice() {
            let data_dir = paths::data_dir();
            let repository = gitbutler_testsupport::TestProject::default();
            let path = repository.path();
            gitbutler_project::add_with_path(data_dir.path(), path).unwrap();

            let outcome = gitbutler_project::add_with_path(data_dir.path(), path).unwrap();
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
                gitbutler_project::add_with_path(data_dir.path(), repo_dir.as_path()).unwrap();
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
                gitbutler_project::add_with_path(data_dir.path(), worktree.path()).unwrap();
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

        fn repo_path_at(name: &str) -> PathBuf {
            gitbutler_testsupport::gix_testtools::scripted_fixture_read_only(
                "various-repositories.sh",
            )
            .unwrap()
            .join(name)
        }
    }
}

mod delete {
    use super::*;
    #[test]
    fn success() {
        let data_dir = paths::data_dir();
        let repository = gitbutler_testsupport::TestProject::default();
        let path = repository.path();
        let project = gitbutler_project::add_with_path(data_dir.path(), path)
            .unwrap()
            .unwrap_project();
        assert!(gitbutler_project::delete_with_path(data_dir.path(), project.id).is_ok());
        assert!(gitbutler_project::delete_with_path(data_dir.path(), project.id).is_ok()); // idempotent
        assert!(gitbutler_project::get_with_path(data_dir.path(), project.id).is_err());
        assert!(!project.gb_dir().exists());
    }
}
