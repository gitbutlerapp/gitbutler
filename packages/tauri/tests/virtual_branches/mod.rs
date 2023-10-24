//TODO:
#![allow(
    clippy::redundant_closure_for_method_calls,
    clippy::rest_pat_in_fully_bound_structs
)]

use std::{fs, str::FromStr};

use gblib::{
    git, keys,
    projects::{self, ProjectId},
    users,
    virtual_branches::{branch, Controller, ControllerError},
};

use crate::{common::TestProject, paths};

struct Test {
    repository: TestProject,
    project_id: ProjectId,
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

mod references {
    use super::*;

    mod create_virtual_branch {
        use super::*;

        #[tokio::test]
        async fn simple() {
            let Test {
                project_id,
                controller,
                repository,
                ..
            } = Test::default();

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert_eq!(branches[0].name, "Virtual branch");

            let refnames = repository
                .references()
                .into_iter()
                .filter_map(|reference| reference.name().map(|name| name.to_string()))
                .collect::<Vec<_>>();
            assert!(refnames.contains(&"refs/gitbutler/virtual-branch".to_string()));
        }

        #[tokio::test]
        async fn duplicate_name() {
            let Test {
                project_id,
                controller,
                repository,
                ..
            } = Test::default();

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch1_id = controller
                .create_virtual_branch(
                    &project_id,
                    &gblib::virtual_branches::branch::BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            let branch2_id = controller
                .create_virtual_branch(
                    &project_id,
                    &gblib::virtual_branches::branch::BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 2);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].name, "name");
            assert_eq!(branches[1].id, branch2_id);
            assert_eq!(branches[1].name, "name 1");

            let refnames = repository
                .references()
                .into_iter()
                .filter_map(|reference| reference.name().map(|name| name.to_string()))
                .collect::<Vec<_>>();
            assert!(refnames.contains(&"refs/gitbutler/name".to_string()));
            assert!(refnames.contains(&"refs/gitbutler/name-1".to_string()));
        }
    }

    mod update_virtual_branch {
        use super::*;

        #[tokio::test]
        async fn simple() {
            let Test {
                project_id,
                controller,
                repository,
                ..
            } = Test::default();

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch_id = controller
                .create_virtual_branch(
                    &project_id,
                    &branch::BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            controller
                .update_virtual_branch(
                    &project_id,
                    branch::BranchUpdateRequest {
                        id: branch_id,
                        name: Some("new name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert_eq!(branches[0].name, "new name");

            let refnames = repository
                .references()
                .into_iter()
                .filter_map(|reference| reference.name().map(|name| name.to_string()))
                .collect::<Vec<_>>();
            assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
            assert!(refnames.contains(&"refs/gitbutler/new-name".to_string()));
        }

        #[tokio::test]
        async fn duplicate_name() {
            let Test {
                project_id,
                controller,
                repository,
                ..
            } = Test::default();

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch1_id = controller
                .create_virtual_branch(
                    &project_id,
                    &branch::BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            let branch2_id = controller
                .create_virtual_branch(
                    &project_id,
                    &branch::BranchCreateRequest {
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            controller
                .update_virtual_branch(
                    &project_id,
                    branch::BranchUpdateRequest {
                        id: branch2_id,
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 2);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].name, "name");
            assert_eq!(branches[1].id, branch2_id);
            assert_eq!(branches[1].name, "name 1");

            let refnames = repository
                .references()
                .into_iter()
                .filter_map(|reference| reference.name().map(|name| name.to_string()))
                .collect::<Vec<_>>();
            assert!(refnames.contains(&"refs/gitbutler/name".to_string()));
            assert!(refnames.contains(&"refs/gitbutler/name-1".to_string()));
        }
    }

    mod delete_virtual_branch {
        use super::*;

        #[tokio::test]
        async fn simple() {
            let Test {
                project_id,
                controller,
                repository,
                ..
            } = Test::default();

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let id = controller
                .create_virtual_branch(
                    &project_id,
                    &branch::BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            controller
                .delete_virtual_branch(&project_id, &id)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 0);

            let refnames = repository
                .references()
                .into_iter()
                .filter_map(|reference| reference.name().map(|name| name.to_string()))
                .collect::<Vec<_>>();
            assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
        }
    }

    mod push_virtual_branch {
        use super::*;

        #[tokio::test]
        async fn simple() {
            let Test {
                project_id,
                controller,
                repository,
                ..
            } = Test::default();

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch1_id = controller
                .create_virtual_branch(
                    &project_id,
                    &branch::BranchCreateRequest {
                        name: Some("name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "content").unwrap();

            controller
                .create_commit(&project_id, &branch1_id, "test", None)
                .await
                .unwrap();
            controller
                .push_virtual_branch(&project_id, &branch1_id, false)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].name, "name");
            assert_eq!(
                branches[0].upstream.as_ref().unwrap().name.to_string(),
                "refs/remotes/origin/name"
            );

            let refnames = repository
                .references()
                .into_iter()
                .filter_map(|reference| reference.name().map(|name| name.to_string()))
                .collect::<Vec<_>>();
            assert!(refnames.contains(&branches[0].upstream.clone().unwrap().name.to_string()));
        }

        #[tokio::test]
        async fn duplicate_names() {
            let Test {
                project_id,
                controller,
                repository,
                ..
            } = Test::default();

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch1_id = {
                // create and push branch with some work
                let branch1_id = controller
                    .create_virtual_branch(
                        &project_id,
                        &branch::BranchCreateRequest {
                            name: Some("name".to_string()),
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap();
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                controller
                    .create_commit(&project_id, &branch1_id, "test", None)
                    .await
                    .unwrap();
                controller
                    .push_virtual_branch(&project_id, &branch1_id, false)
                    .await
                    .unwrap();
                branch1_id
            };

            // rename first branch
            controller
                .update_virtual_branch(
                    &project_id,
                    branch::BranchUpdateRequest {
                        id: branch1_id,
                        name: Some("updated name".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            let branch2_id = {
                // create another branch with first branch's old name and push it
                let branch2_id = controller
                    .create_virtual_branch(
                        &project_id,
                        &branch::BranchCreateRequest {
                            name: Some("name".to_string()),
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap();
                fs::write(repository.path().join("file.txt"), "updated content").unwrap();
                controller
                    .create_commit(&project_id, &branch2_id, "test", None)
                    .await
                    .unwrap();
                controller
                    .push_virtual_branch(&project_id, &branch2_id, false)
                    .await
                    .unwrap();
                branch2_id
            };

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 2);
            // first branch is pushing to old ref remotely
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].name, "updated name");
            assert_eq!(
                branches[0].upstream.as_ref().unwrap().name,
                "refs/remotes/origin/name".parse().unwrap()
            );
            // new branch is pushing to new ref remotely
            assert_eq!(branches[1].id, branch2_id);
            assert_eq!(branches[1].name, "name");
            assert_eq!(
                branches[1].upstream.as_ref().unwrap().name,
                "refs/remotes/origin/name-1".parse().unwrap()
            );

            let refnames = repository
                .references()
                .into_iter()
                .filter_map(|reference| reference.name().map(|name| name.to_string()))
                .collect::<Vec<_>>();
            assert!(refnames.contains(&branches[0].upstream.clone().unwrap().name.to_string()));
            assert!(refnames.contains(&branches[1].upstream.clone().unwrap().name.to_string()));
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
    use super::*;

    mod apply_virtual_branch {
        use super::*;

        #[tokio::test]
        async fn deltect_conflict() {
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

            let branch1_id = {
                let branch1_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();
                fs::write(repository.path().join("file.txt"), "branch one").unwrap();

                branch1_id
            };

            // unapply first vbranch
            controller
                .unapply_virtual_branch(&project_id, &branch1_id)
                .await
                .unwrap();

            {
                // create another vbranch that conflicts with the first one
                controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();
                fs::write(repository.path().join("file.txt"), "branch two").unwrap();
            }

            {
                // it should not be possible to apply the first branch
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

        #[tokio::test]
        async fn rebase_commit() {
            let Test {
                repository,
                project_id,
                controller,
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "one").unwrap();
                fs::write(repository.path().join("another_file.txt"), "").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "two").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(first_commit_oid);
            }

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch1_id = {
                // create a branch with some commited work
                let branch1_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();
                fs::write(repository.path().join("another_file.txt"), "virtual").unwrap();

                controller
                    .create_commit(&project_id, &branch1_id, "virtual commit", None)
                    .await
                    .unwrap();

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert!(branches[0].active);
                assert_eq!(branches[0].files.len(), 0);
                assert_eq!(branches[0].commits.len(), 1);

                branch1_id
            };

            {
                // unapply first vbranch
                controller
                    .unapply_virtual_branch(&project_id, &branch1_id)
                    .await
                    .unwrap();

                assert_eq!(
                    fs::read_to_string(repository.path().join("another_file.txt")).unwrap(),
                    ""
                );
                assert_eq!(
                    fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "one"
                );

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert_eq!(branches[0].files.len(), 0);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!branches[0].active);
            }

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // branch is stil unapplied
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert_eq!(branches[0].files.len(), 0);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!branches[0].active);
                assert!(!branches[0].conflicted);

                assert_eq!(
                    fs::read_to_string(repository.path().join("another_file.txt")).unwrap(),
                    ""
                );
                assert_eq!(
                    fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "two"
                );
            }

            {
                // apply first vbranch again
                controller
                    .apply_virtual_branch(&project_id, &branch1_id)
                    .await
                    .unwrap();

                // it should be rebased
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert_eq!(branches[0].files.len(), 0);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);

                assert_eq!(
                    fs::read_to_string(repository.path().join("another_file.txt")).unwrap(),
                    "virtual"
                );

                assert_eq!(
                    fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "two"
                );
            }
        }

        #[tokio::test]
        async fn rebase_work() {
            let Test {
                repository,
                project_id,
                controller,
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(first_commit_oid);
            }

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch1_id = {
                // make a branch with some work
                let branch1_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();
                fs::write(repository.path().join("another_file.txt"), "").unwrap();

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert!(branches[0].active);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);

                branch1_id
            };

            {
                // unapply first vbranch
                controller
                    .unapply_virtual_branch(&project_id, &branch1_id)
                    .await
                    .unwrap();

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert!(!branches[0].active);

                assert!(!repository.path().join("another_file.txt").exists());
                assert!(!repository.path().join("file.txt").exists());
            }

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // first branch is stil unapplied
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert!(!branches[0].active);
                assert!(!branches[0].conflicted);

                assert!(!repository.path().join("another_file.txt").exists());
                assert!(repository.path().join("file.txt").exists());
            }

            {
                // apply first vbranch again
                controller
                    .apply_virtual_branch(&project_id, &branch1_id)
                    .await
                    .unwrap();

                // workdir should be rebased, and work should be restored
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);

                assert!(repository.path().join("another_file.txt").exists());
                assert!(repository.path().join("file.txt").exists());
            }
        }
    }

    mod update_base_branch {
        use super::*;

        #[tokio::test]
        async fn detect_resolve_conflict() {
            let Test {
                repository,
                project_id,
                controller,
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(first_commit_oid);
            }

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch1_id = {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch1_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();
                fs::write(repository.path().join("file.txt"), "conflict").unwrap();

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch1_id);
                assert!(branches[0].active);

                branch1_id
            };

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

mod reset_virtual_branch {
    use super::*;

    #[tokio::test]
    async fn to_head() {
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
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let oid = {
            fs::write(repository.path().join("file.txt"), "content").unwrap();

            // commit changes
            let oid = controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 1);
            assert_eq!(branches[0].commits[0].id, oid);
            assert_eq!(branches[0].files.len(), 0);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );

            oid
        };

        {
            // reset changes to head
            controller
                .reset_virtual_branch(&project_id, &branch1_id, oid)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 1);
            assert_eq!(branches[0].commits[0].id, oid);
            assert_eq!(branches[0].files.len(), 0);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );
        }
    }

    #[tokio::test]
    async fn to_target() {
        let Test {
            repository,
            project_id,
            controller,
        } = Test::default();

        let base_branch = controller
            .set_base_branch(
                &project_id,
                &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
            )
            .unwrap();

        let branch1_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file.txt"), "content").unwrap();

            // commit changes
            let oid = controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 1);
            assert_eq!(branches[0].commits[0].id, oid);
            assert_eq!(branches[0].files.len(), 0);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );
        }

        {
            // reset changes to head
            controller
                .reset_virtual_branch(&project_id, &branch1_id, base_branch.base_sha)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 0);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );
        }
    }

    #[tokio::test]
    async fn to_commit() {
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
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let first_commit_oid = {
            // commit some changes

            fs::write(repository.path().join("file.txt"), "content").unwrap();

            let oid = controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 1);
            assert_eq!(branches[0].commits[0].id, oid);
            assert_eq!(branches[0].files.len(), 0);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );

            oid
        };

        {
            // commit some more
            fs::write(repository.path().join("file.txt"), "more content").unwrap();

            let second_commit_oid = controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 2);
            assert_eq!(branches[0].commits[0].id, second_commit_oid);
            assert_eq!(branches[0].commits[1].id, first_commit_oid);
            assert_eq!(branches[0].files.len(), 0);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "more content"
            );
        }

        {
            // reset changes to the first commit
            controller
                .reset_virtual_branch(&project_id, &branch1_id, first_commit_oid)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 1);
            assert_eq!(branches[0].commits[0].id, first_commit_oid);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "more content"
            );
        }
    }

    #[tokio::test]
    async fn to_non_existing() {
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
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file.txt"), "content").unwrap();

            // commit changes
            let oid = controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap();

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 1);
            assert_eq!(branches[0].commits[0].id, oid);
            assert_eq!(branches[0].files.len(), 0);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );

            oid
        };

        assert!(matches!(
            controller
                .reset_virtual_branch(
                    &project_id,
                    &branch1_id,
                    "fe14df8c66b73c6276f7bb26102ad91da680afcb".parse().unwrap()
                )
                .await,
            Err(ControllerError::Other(_))
        ));
    }
}

mod upstream {
    use super::*;

    #[tokio::test]
    async fn detect_upstream_commits() {
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
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let oid1 = {
            // create first commit
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap()
        };

        let oid2 = {
            // create second commit
            fs::write(repository.path().join("file.txt"), "content2").unwrap();
            controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap()
        };

        // push
        controller
            .push_virtual_branch(&project_id, &branch1_id, false)
            .await
            .unwrap();

        let oid3 = {
            // create third commit
            fs::write(repository.path().join("file.txt"), "content3").unwrap();
            controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap()
        };

        {
            // should correctly detect pushed commits
            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 3);
            assert_eq!(branches[0].commits[0].id, oid3);
            assert!(!branches[0].commits[0].is_remote);
            assert_eq!(branches[0].commits[1].id, oid2);
            assert!(branches[0].commits[1].is_remote);
            assert_eq!(branches[0].commits[2].id, oid1);
            assert!(branches[0].commits[2].is_remote);
        }
    }

    #[tokio::test]
    async fn detect_integrated_commits() {
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
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let oid1 = {
            // create first commit
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap()
        };

        let oid2 = {
            // create second commit
            fs::write(repository.path().join("file.txt"), "content2").unwrap();
            controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap()
        };

        // push
        controller
            .push_virtual_branch(&project_id, &branch1_id, false)
            .await
            .unwrap();

        {
            // merge branch upstream
            let branch = controller.list_virtual_branches(&project_id).await.unwrap().into_iter().find(|b| b.id == branch1_id).unwrap();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
            repository.fetch()
        }

        let oid3 = {
            // create third commit
            fs::write(repository.path().join("file.txt"), "content3").unwrap();
            controller
                .create_commit(&project_id, &branch1_id, "commit", None)
                .await
                .unwrap()
        };

        {
            // should correctly detect pushed commits
            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch1_id);
            assert_eq!(branches[0].commits.len(), 3);
            assert_eq!(branches[0].commits[0].id, oid3);
            assert!(!branches[0].commits[0].is_integrated);
            assert_eq!(branches[0].commits[1].id, oid2);
            assert!(branches[0].commits[1].is_integrated);
            assert_eq!(branches[0].commits[2].id, oid1);
            assert!(branches[0].commits[2].is_integrated);
        }
    }
}

mod cherry_pick {
    use super::*;

    mod cleanly {
        use super::*;

        #[tokio::test]
        async fn applied() {
            let Test {
                repository,
                project_id,
                controller,
            } = Test::default();

            let commit_oid = {
                let first = repository.commit_all("commit");
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                let second = repository.commit_all("commit");
                repository.push();
                repository.reset_hard(first);
                second
            };

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            let cherry_picked_commit_oid = controller
                .cherry_pick(&project_id, &branch_id, commit_oid)
                .await
                .unwrap();
            assert!(cherry_picked_commit_oid.is_some());
            assert!(repository.path().join("file.txt").exists());

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
            assert_eq!(branches[0].commits.len(), 1);
            assert_eq!(branches[0].commits[0].id, cherry_picked_commit_oid.unwrap());
        }

        #[tokio::test]
        async fn non_applied() {
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

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            let commit_one_oid = {
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None)
                    .await
                    .unwrap()
            };

            {
                fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None)
                    .await
                    .unwrap()
            };

            let commit_three_oid = {
                fs::write(repository.path().join("file_three.txt"), "content three").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None)
                    .await
                    .unwrap()
            };

            controller
                .reset_virtual_branch(&project_id, &branch_id, commit_one_oid)
                .await
                .unwrap();

            controller
                .unapply_virtual_branch(&project_id, &branch_id)
                .await
                .unwrap();

            assert!(matches!(
                controller
                    .cherry_pick(&project_id, &branch_id, commit_three_oid)
                    .await,
                Err(ControllerError::Other(_))
            ));
        }
    }

    mod with_conflicts {
        use super::*;

        #[tokio::test]
        async fn applied() {
            let Test {
                repository,
                project_id,
                controller,
            } = Test::default();

            let commit_oid = {
                let first = repository.commit_all("commit");
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                let second = repository.commit_all("commit");
                repository.push();
                repository.reset_hard(first);
                second
            };

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            // introduce conflict with the remote commit
            fs::write(repository.path().join("file.txt"), "conflict").unwrap();

            {
                // cherry picking leads to conflict
                let cherry_picked_commit_oid = controller
                    .cherry_pick(&project_id, &branch_id, commit_oid)
                    .await
                    .unwrap();
                assert!(cherry_picked_commit_oid.is_none());

                assert_eq!(
                    fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\ncontent\n>>>>>>> theirs\n"
                );

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert_eq!(branches[0].files.len(), 1);
                assert!(branches[0].files[0].conflicted);
                assert_eq!(branches[0].commits.len(), 0);
            }

            {
                // conflict can be resolved
                fs::write(repository.path().join("file.txt"), "resolved").unwrap();
                let commited_oid = controller
                    .create_commit(&project_id, &branch_id, "resolution", None)
                    .await
                    .unwrap();

                let commit = repository.find_commit(commited_oid).unwrap();
                assert_eq!(commit.parent_count(), 2);

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);
                assert_eq!(branches[0].commits.len(), 2);
                // resolution commit is there
                assert_eq!(branches[0].commits[0].id, commited_oid);
                // cherry picked commit is there
                assert_eq!(
                    branches[0].commits[1].files[0].hunks[0].diff,
                    "@@ -0,0 +1 @@\n+content\n\\ No newline at end of file\n"
                );
            }
        }

        #[tokio::test]
        async fn non_applied() {
            let Test {
                repository,
                project_id,
                controller,
            } = Test::default();

            let commit_oid = {
                let first = repository.commit_all("commit");
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                let second = repository.commit_all("commit");
                repository.push();
                repository.reset_hard(first);
                second
            };

            controller
                .set_base_branch(
                    &project_id,
                    &git::RemoteBranchName::from_str("refs/remotes/origin/master").unwrap(),
                )
                .unwrap();

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            // introduce conflict with the remote commit
            fs::write(repository.path().join("file.txt"), "conflict").unwrap();

            controller
                .unapply_virtual_branch(&project_id, &branch_id)
                .await
                .unwrap();

            assert!(matches!(
                controller
                    .cherry_pick(&project_id, &branch_id, commit_oid)
                    .await,
                Err(ControllerError::Other(_))
            ));
        }
    }
}
