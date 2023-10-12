use gitbutler::projects::Controller;

use crate::{common, paths};

pub fn new() -> Controller {
    let data_dir = paths::data_dir();
    Controller::from(&data_dir)
}

mod add {
    use super::*;

    #[test]
    fn success() {
        let controller = new();
        let repository = common::git_repository();
        let path = repository.workdir().unwrap();
        let project = controller.add(path).unwrap();
        assert_eq!(project.path, path);
        assert_eq!(project.title, path.iter().last().unwrap().to_str().unwrap());
        assert_eq!(project.id.len(), 36);
    }

    mod error {
        use gitbutler::projects::AddError;

        use super::*;

        #[test]
        fn missing() {
            let controller = new();
            let path = tempfile::tempdir().unwrap().into_path();
            assert!(matches!(
                controller.add(&path.join("missing")),
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
                controller.add(&path),
                Err(AddError::NotAGitRepository)
            ));
        }

        #[test]
        fn twice() {
            let controller = new();
            let repository = common::git_repository();
            let path = repository.workdir().unwrap();
            controller.add(path).unwrap();
            assert!(matches!(controller.add(path), Err(AddError::AlreadyExists)));
        }
    }
}
