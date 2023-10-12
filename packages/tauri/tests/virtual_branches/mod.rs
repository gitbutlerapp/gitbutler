use std::{fs, str::FromStr};

use gitbutler::{
    git, keys, projects, users,
    virtual_branches::{Controller, ControllerError},
};

use crate::{common::TestProject, paths};

struct Test {
    repository: TestProject,
    project_id: String,
    controller: Controller,
}

impl Default for Test {
    fn default() -> Self {
        let data_dir = paths::data_dir();
        let keys = keys::Controller::from(&data_dir);
        let projects = projects::Controller::from(&data_dir);
        let users = users::Controller::from(&data_dir);

        let test_project = TestProject::default();
        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        Self {
            repository: test_project,
            project_id: project.id,
            controller: Controller::new(&data_dir, &projects, &users, &keys),
        }
    }
}

mod set_base_branch {
    use super::*;

    #[test]
    fn success() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(
                &project_id,
                &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
            )
            .unwrap();
    }

    mod errors {
        use super::*;

        #[test]
        fn missing() {
            let Test {
                project_id,
                controller,
                ..
            } = Test::default();

            assert!(controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/missing").unwrap(),
                )
                .is_err());
        }
    }
}

mod conflicts {
    use gitbutler::virtual_branches::branch::BranchCreateRequest;

    use super::*;

    mod r#virtual {
        use super::*;

        #[tokio::test]
        async fn detect() {
            let Test {
                repository,
                project_id,
                controller,
            } = Test::default();

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch1_id = controller
                .create_virtual_branch(&project_id, &BranchCreateRequest::default())
                .await
                .unwrap();
            fs::write(repository.path().join("file.txt"), "branch one").unwrap();

            controller
                .unapply_virtual_branch(&project_id, &branch1_id)
                .await
                .unwrap();

            controller
                .create_virtual_branch(&project_id, &BranchCreateRequest::default())
                .await
                .unwrap();
            fs::write(repository.path().join("file.txt"), "branch two").unwrap();

            assert!(!controller
                .can_apply_virtual_branch(&project_id, &branch1_id)
                .unwrap());
            assert!(matches!(
                controller
                    .apply_virtual_branch(&project_id, &branch1_id)
                    .await,
                Err(ControllerError::Other(_))
            ));
        }
    }
}
