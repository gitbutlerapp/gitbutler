use gitbutler_core::projects::Controller;
use tempfile::TempDir;

use crate::shared::{self, paths};

pub fn new() -> (Controller, TempDir) {
    let data_dir = paths::data_dir();
    let controller = Controller::from_path(&data_dir);
    (controller, data_dir)
}

mod add {
    use super::*;

    #[test]
    fn success() {
        let (controller, _tmp) = new();
        let repository = shared::TestProject::default();
        let path = repository.path();
        let project = controller.add(path).unwrap();
        assert_eq!(project.path, path);
        assert_eq!(project.title, path.iter().last().unwrap().to_str().unwrap());
    }

    mod error {
        use gitbutler_core::projects::AddError;

        use super::*;

        #[test]
        fn missing() {
            let (controller, _tmp) = new();
            let tmp = tempfile::tempdir().unwrap();
            assert!(matches!(
                controller.add(tmp.path().join("missing")),
                Err(AddError::PathNotFound)
            ));
        }

        #[test]
        fn not_git() {
            let (controller, _tmp) = new();
            let tmp = tempfile::tempdir().unwrap();
            let path = tmp.path();
            std::fs::write(path.join("file.txt"), "hello world").unwrap();
            assert!(matches!(
                controller.add(path),
                Err(AddError::NotAGitRepository)
            ));
        }

        #[test]
        fn empty() {
            let (controller, _tmp) = new();
            let tmp = tempfile::tempdir().unwrap();
            assert!(matches!(
                controller.add(tmp.path()),
                Err(AddError::NotAGitRepository)
            ));
        }

        #[test]
        fn twice() {
            let (controller, _tmp) = new();
            let repository = shared::TestProject::default();
            let path = repository.path();
            controller.add(path).unwrap();
            assert!(matches!(controller.add(path), Err(AddError::AlreadyExists)));
        }
    }
}
