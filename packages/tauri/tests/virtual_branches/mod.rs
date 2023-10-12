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

            assert!(matches!(
                controller.set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/missing").unwrap(),
                ),
                Err(ControllerError::Other(_))
            ));
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

    mod remote {
        use super::*;

        #[tokio::test]
        async fn detect_and_fix() {
            let Test {
                repository,
                project_id,
                controller,
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            fs::write(repository.path().join("file.txt"), "first").unwrap();
            let first_commit_oid = repository.commit_all("first");
            fs::write(repository.path().join("file.txt"), "second").unwrap();
            repository.commit_all("second");
            repository.push();
            repository.reset_hard(first_commit_oid);

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            let branch1_id = controller
                .create_virtual_branch(&project_id, &BranchCreateRequest::default())
                .await
                .unwrap();
            fs::write(repository.path().join("file.txt"), "conflict").unwrap();

            {
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert!(branches[0].active);
            }

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // there is a conflict now, so the branch should be inactive
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert!(!branches[0].active);
            }

            {
                // when we apply conflicted branch, it has conflict
                controller
                    .apply_virtual_branch(&project_id, &branch1_id)
                    .await
                    .unwrap();

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);

                // and the conflict markers are in the file
                assert_eq!(
                    fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }

            {
                // can't commit conflicts
                assert!(matches!(
                    controller
                        .create_commit(&project_id, &branch1_id, "commit conflicts", None)
                        .await,
                    Err(ControllerError::Conflicting)
                ));
            }

            {
                // fixing the conflict removes conflicted mark
                fs::write(repository.path().join("file.txt"), "resolved").unwrap();
                let commit_oid = controller
                    .create_commit(&project_id, &branch1_id, "resolution", None)
                    .await
                    .unwrap();

                let commit = repository.find_commit(commit_oid).unwrap();
                assert_eq!(commit.parent_count(), 2);

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);
            }
        }
    }
}
