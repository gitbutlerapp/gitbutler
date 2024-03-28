use gitbutler_app::projects::Controller;

use crate::common::{self, paths};

pub fn new() -> Controller {
    let data_dir = paths::data_dir();
    Controller::from_path(data_dir)
}

mod add {
    use super::*;

    #[test]
    fn success() {
        let controller = new();
        let repository = common::TestProject::default();
        let path = repository.path();
        let project = controller.add(path).unwrap();
        assert_eq!(project.path, path);
        assert_eq!(project.title, path.iter().last().unwrap().to_str().unwrap());
    }

    mod error {
        use gitbutler_app::projects::AddError;

        use super::*;

        #[test]
        fn missing() {
            let controller = new();
            let path = tempfile::tempdir().unwrap().into_path();
            assert!(matches!(
                controller.add(path.join("missing")),
                Err(AddError::PathNotFound)
            ));
        }

        #[test]
        fn not_git() {
            let controller = new();
            let path = tempfile::tempdir().unwrap().into_path();
            std::fs::write(path.join("file.txt"), "hello world").unwrap();
            assert!(matches!(
                controller.add(&path),
                Err(AddError::NotAGitRepository)
            ));
        }

        #[test]
        fn empty() {
            let controller = new();
            let path = tempfile::tempdir().unwrap().into_path();
            assert!(matches!(
                controller.add(path),
                Err(AddError::NotAGitRepository)
            ));
        }

        #[test]
        fn twice() {
            let controller = new();
            let repository = common::TestProject::default();
            let path = repository.path();
            controller.add(path).unwrap();
            assert!(matches!(controller.add(path), Err(AddError::AlreadyExists)));
        }
    }
}
