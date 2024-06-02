use gitbutler_core::projects::Controller;
use tempfile::TempDir;

use gitbutler_testsupport::{self, paths};

pub fn new() -> (Controller, TempDir) {
    let data_dir = paths::data_dir();
    let controller = Controller::from_path(data_dir.path());
    (controller, data_dir)
}

mod add {
    use super::*;

    #[test]
    fn success() {
        let (controller, _tmp) = new();
        let repository = gitbutler_testsupport::TestProject::default();
        let path = repository.path();
        let project = controller.add(path).unwrap();
        assert_eq!(project.path, path);
        assert_eq!(project.title, path.iter().last().unwrap().to_str().unwrap());
    }

    mod error {
        use super::*;

        #[test]
        fn missing() {
            let (controller, _tmp) = new();
            let tmp = tempfile::tempdir().unwrap();
            assert_eq!(
                controller
                    .add(tmp.path().join("missing"))
                    .unwrap_err()
                    .to_string(),
                "path not found"
            );
        }

        #[test]
        fn directory_without_git() {
            let (controller, _tmp) = new();
            let tmp = tempfile::tempdir().unwrap();
            let path = tmp.path();
            std::fs::write(path.join("file.txt"), "hello world").unwrap();
            assert_eq!(
                controller.add(path).unwrap_err().to_string(),
                "must be a Git repository"
            );
        }

        #[test]
        fn empty() {
            let (controller, _tmp) = new();
            let tmp = tempfile::tempdir().unwrap();
            let err = controller.add(tmp.path()).unwrap_err();
            assert_eq!(err.to_string(), "must be a Git repository");
        }

        #[test]
        fn twice() {
            let (controller, _tmp) = new();
            let repository = gitbutler_testsupport::TestProject::default();
            let path = repository.path();
            controller.add(path).unwrap();
            assert_eq!(
                controller.add(path).unwrap_err().to_string(),
                "project already exists"
            );
        }

        #[test]
        fn bare() {
            let (controller, _tmp) = new();
            let tmp = tempfile::tempdir().unwrap();
            let repo_dir = tmp.path().join("bare");

            let repo = git2::Repository::init_bare(&repo_dir).unwrap();
            create_initial_commit(&repo);

            let err = controller.add(repo_dir).unwrap_err();
            assert_eq!(err.to_string(), "bare repositories are unsupported");
        }

        #[test]
        fn worktree() {
            let (controller, _tmp) = new();
            let tmp = tempfile::tempdir().unwrap();
            let main_worktree_dir = tmp.path().join("main");
            let worktree_dir = tmp.path().join("worktree");

            let repo = git2::Repository::init(main_worktree_dir).unwrap();
            create_initial_commit(&repo);

            let worktree = repo.worktree("feature", &worktree_dir, None).unwrap();
            let err = controller.add(worktree.path()).unwrap_err();
            assert_eq!(err.to_string(), "can only work in main worktrees");
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
    #[tokio::test]
    async fn success() {
        let (controller, _tmp) = new();
        let repository = gitbutler_testsupport::TestProject::default();
        let path = repository.path();
        let project = controller.add(path).unwrap();
        assert!(controller.delete(project.id).await.is_ok());
        assert!(controller.delete(project.id).await.is_ok()); // idempotent
        assert!(controller.get(project.id).is_err());
        assert!(!project.gb_dir().exists());
        assert!(!project.path.join(".gitbutler.json").exists());
    }
}
