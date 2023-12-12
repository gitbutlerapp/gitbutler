//TODO:
#![allow(
    clippy::redundant_closure_for_method_calls,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::dbg_macro
)]

mod common;

use self::common::{paths, TestProject};
use std::{fs, path, str::FromStr};

use gblib::{
    error::Error,
    git, keys,
    projects::{self, ProjectId},
    users,
    virtual_branches::{branch, controller::ControllerError, errors, Controller},
};

struct Test {
    repository: TestProject,
    project_id: ProjectId,
    projects: projects::Controller,
    controller: Controller,
}

impl Default for Test {
    fn default() -> Self {
        let data_dir = paths::data_dir();
        let keys = keys::Controller::from(&data_dir);
        let projects = projects::Controller::from(&data_dir);
        let users = users::Controller::from(&data_dir);
        let helper = git::credentials::Helper::from(&data_dir);

        let test_project = TestProject::default();
        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        Self {
            repository: test_project,
            project_id: project.id,
            controller: Controller::new(&data_dir, &projects, &users, &keys, &helper),
            projects,
        }
    }
}

mod create_commit {
    use super::*;

    #[tokio::test]
    async fn should_lock_updated_hunks() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            // by default, hunks are not locked

            fs::write(repository.path().join("file.txt"), "content").unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(!branch.files[0].hunks[0].locked);
        }

        controller
            .create_commit(&project_id, &branch_id, "test", None)
            .await
            .unwrap();

        {
            // change in the committed hunks leads to hunk locking
            fs::write(repository.path().join("file.txt"), "updated content").unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(branch.files[0].hunks[0].locked);
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
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
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
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
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
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
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
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
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
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
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
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
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

mod delete_virtual_branch {
    use super::*;

    #[tokio::test]
    async fn should_unapply_diff() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // write some
        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();

        controller
            .delete_virtual_branch(&project_id, &branches[0].id)
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 0);
        assert!(!repository.path().join("file.txt").exists());

        let refnames = repository
            .references()
            .into_iter()
            .filter_map(|reference| reference.name().map(|name| name.to_string()))
            .collect::<Vec<_>>();
        assert!(!refnames.contains(&"refs/gitbutler/name".to_string()));
    }

    #[tokio::test]
    async fn should_remove_reference() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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

mod set_base_branch {
    use super::*;

    #[tokio::test]
    async fn success() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();
    }

    mod errors {
        use super::*;

        #[tokio::test]
        async fn missing() {
            let Test {
                project_id,
                controller,
                ..
            } = Test::default();

            assert!(matches!(
                controller
                    .set_base_branch(
                        &project_id,
                        &git::RemoteRefname::from_str("refs/remotes/origin/missing").unwrap(),
                    )
                    .await,
                Err(Error::UserError { .. })
            ));
        }
    }
}

mod unapply {
    use super::*;

    #[tokio::test]
    async fn unapply_with_data() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        controller
            .unapply_virtual_branch(&project_id, &branches[0].id)
            .await
            .unwrap();

        assert!(!repository.path().join("file.txt").exists());

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert!(!branches[0].active);
    }

    #[tokio::test]
    async fn delete_if_empty() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        controller
            .unapply_virtual_branch(&project_id, &branches[0].id)
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 0);
    }
}

mod apply_virtual_branch {
    use super::*;

    #[tokio::test]
    async fn deltect_conflict() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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
                .await
                .unwrap());

            assert!(matches!(
                controller
                    .apply_virtual_branch(&project_id, &branch1_id)
                    .await,
                Err(ControllerError::Action(
                    errors::ApplyBranchError::BranchConflicts(_)
                ))
            ));
        }
    }

    #[tokio::test]
    async fn rebase_commit() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        // make sure we have an undiscovered commit in the remote branch
        {
            fs::write(repository.path().join("file.txt"), "one").unwrap();
            fs::write(repository.path().join("another_file.txt"), "").unwrap();
            let first_commit_oid = repository.commit_all("first");
            fs::write(repository.path().join("file.txt"), "two").unwrap();
            repository.commit_all("second");
            repository.push();
            repository.reset_hard(Some(first_commit_oid));
        }

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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
            ..
        } = Test::default();

        // make sure we have an undiscovered commit in the remote branch
        {
            let first_commit_oid = repository.commit_all("first");
            fs::write(repository.path().join("file.txt"), "").unwrap();
            repository.commit_all("second");
            repository.push();
            repository.reset_hard(Some(first_commit_oid));
        }

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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

#[tokio::test]
async fn resolve_conflict_flow() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = Test::default();

    // make sure we have an undiscovered commit in the remote branch
    {
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        let first_commit_oid = repository.commit_all("first");
        fs::write(repository.path().join("file.txt"), "second").unwrap();
        repository.commit_all("second");
        repository.push();
        repository.reset_hard(Some(first_commit_oid));
    }

    controller
        .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
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
            Err(ControllerError::Action(errors::CommitError::Conflicted(_)))
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

mod update_base_branch {
    use super::*;

    mod unapplied_branch {
        use super::*;

        #[tokio::test]
        async fn conflicts_with_uncommitted_work() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch that is unapplied and contains not commited conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .unapply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // branch should not be changed.

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn commited_conflict_not_pushed() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch with a commit that conflicts with upstream, and work that fixes
                // that conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "conflicting commit", None)
                    .await
                    .unwrap();

                controller
                    .unapply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should not change the branch.

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current);
                assert_eq!(branches[0].files.len(), 0);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn commited_conflict_pushed() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch with a commit that conflicts with upstream, and work that fixes
                // that conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "conflicting commit", None)
                    .await
                    .unwrap();

                controller
                    .push_virtual_branch(&project_id, &branch_id, false)
                    .await
                    .unwrap();

                controller
                    .unapply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should not change the branch.

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current);
                assert_eq!(branches[0].files.len(), 0);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn commited_conflict_not_pushed_fixed_with_more_work() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch with a commit that conflicts with upstream, and work that fixes
                // that conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "conflicting commit", None)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();

                controller
                    .unapply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should rebase upstream, and leave uncommited file as is

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current); // TODO: should be true
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap()); // TODO: should be true
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nfix conflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn commited_conflict_pushed_fixed_with_more_work() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch with a commit that conflicts with upstream, and work that fixes
                // that conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "conflicting commit", None)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();

                controller
                    .unapply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should not touch the branch

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(branches[0].files.len(), 1);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nfix conflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn no_conflicts() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "non conflicting commit", None)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "still no conflicts").unwrap();

                controller
                    .unapply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should update branch base

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "second"
                );
            }
        }

        #[tokio::test]
        async fn integrated_commit_plus_work() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                repository.commit_all("first");
                repository.push();
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "second").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "second", None)
                    .await
                    .unwrap();

                // more local work in the same branch
                fs::write(repository.path().join("file2.txt"), "other").unwrap();

                controller
                    .push_virtual_branch(&project_id, &branch_id, false)
                    .await
                    .unwrap();

                {
                    // merge branch upstream
                    let branch = controller
                        .list_virtual_branches(&project_id)
                        .await
                        .unwrap()
                        .into_iter()
                        .find(|b| b.id == branch_id)
                        .unwrap();

                    repository.merge(&branch.upstream.as_ref().unwrap().name);
                    repository.fetch();
                }

                controller
                    .unapply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                branch_id
            };

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should remove integrated commit, but leave work

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                dbg!(&branches[0]);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert!(controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "second"
                );
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file2.txt")).unwrap(),
                    "other"
                );
            }
        }

        #[tokio::test]
        async fn all_integrated() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "second").unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "second", None)
                    .await
                    .unwrap();

                controller
                    .unapply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
            };

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should remove identical branch

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 0);
            }
        }
    }

    mod applied_branch {
        use super::*;

        #[tokio::test]
        async fn conflicts_with_uncommitted_work() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();

                branch_id
            };

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should stash conflicing branch

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn commited_conflict_not_pushed() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch with a commit that conflicts with upstream, and work that fixes
                // that conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "conflicting commit", None)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should stash the branch.

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current);
                assert_eq!(branches[0].files.len(), 0);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn commited_conflict_pushed() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch with a commit that conflicts with upstream, and work that fixes
                // that conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "conflicting commit", None)
                    .await
                    .unwrap();

                controller
                    .push_virtual_branch(&project_id, &branch_id, false)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should stash the branch.

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current);
                assert_eq!(branches[0].files.len(), 0);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn commited_conflict_not_pushed_fixed_with_more_work() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch with a commit that conflicts with upstream, and work that fixes
                // that conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "conflicting commit", None)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should rebase upstream, and leave uncommited file as is

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current); // TODO: should be true
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap()); // TODO: should be true
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nfix conflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        #[tokio::test]
        async fn commited_conflict_pushed_fixed_with_more_work() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch with a commit that conflicts with upstream, and work that fixes
                // that conflict
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "conflict").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "conflicting commit", None)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should merge upstream, and leave uncommited file as is.

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(!branches[0].base_current); // TODO: should be true
                assert_eq!(branches[0].commits.len(), 1); // TODO: should be 2
                assert_eq!(branches[0].files.len(), 1);
                assert!(!controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap()); // TODO: should be true
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "<<<<<<< ours\nfix conflict\n=======\nsecond\n>>>>>>> theirs\n"
                );
            }
        }

        mod no_conflicts_pushed {
            use super::*;

            #[tokio::test]
            async fn force_push_ok() {
                let Test {
                    repository,
                    project_id,
                    controller,
                    projects,
                    ..
                } = Test::default();

                // make sure we have an undiscovered commit in the remote branch
                {
                    fs::write(repository.path().join("file.txt"), "first").unwrap();
                    let first_commit_oid = repository.commit_all("first");
                    fs::write(repository.path().join("file.txt"), "second").unwrap();
                    repository.commit_all("second");
                    repository.push();
                    repository.reset_hard(Some(first_commit_oid));
                }

                projects
                    .update(&projects::UpdateRequest {
                        id: project_id,
                        ok_with_force_push: Some(true),
                        ..Default::default()
                    })
                    .await
                    .unwrap();

                controller
                    .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                    .await
                    .unwrap();

                let branch_id = {
                    let branch_id = controller
                        .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                        .await
                        .unwrap();

                    fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

                    controller
                        .create_commit(&project_id, &branch_id, "no conflicts", None)
                        .await
                        .unwrap();
                    controller
                        .push_virtual_branch(&project_id, &branch_id, false)
                        .await
                        .unwrap();

                    fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                    branch_id
                };

                {
                    // fetch remote
                    controller.update_base_branch(&project_id).await.unwrap();

                    // rebases branch, since the branch is pushed and force pushing is
                    // allowed

                    let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                    assert_eq!(branches.len(), 1);
                    assert_eq!(branches[0].id, branch_id);
                    assert!(branches[0].active);
                    assert!(branches[0].requires_force);
                    assert!(branches[0].base_current);
                    assert_eq!(branches[0].files.len(), 1);
                    assert_eq!(branches[0].commits.len(), 1);
                    assert!(!branches[0].commits[0].is_remote);
                    assert!(!branches[0].commits[0].is_integrated);
                    assert!(controller
                        .can_apply_virtual_branch(&project_id, &branch_id)
                        .await
                        .unwrap());
                }
            }

            #[tokio::test]
            async fn force_push_not_ok() {
                let Test {
                    repository,
                    project_id,
                    controller,
                    ..
                } = Test::default();

                // make sure we have an undiscovered commit in the remote branch
                {
                    fs::write(repository.path().join("file.txt"), "first").unwrap();
                    let first_commit_oid = repository.commit_all("first");
                    fs::write(repository.path().join("file.txt"), "second").unwrap();
                    repository.commit_all("second");
                    repository.push();
                    repository.reset_hard(Some(first_commit_oid));
                }

                controller
                    .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                    .await
                    .unwrap();

                let branch_id = {
                    let branch_id = controller
                        .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                        .await
                        .unwrap();

                    fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

                    controller
                        .create_commit(&project_id, &branch_id, "no conflicts", None)
                        .await
                        .unwrap();
                    controller
                        .push_virtual_branch(&project_id, &branch_id, false)
                        .await
                        .unwrap();

                    fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                    branch_id
                };

                {
                    // fetch remote
                    controller.update_base_branch(&project_id).await.unwrap();

                    // creates a merge commit, since the branch is pushed

                    let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                    assert_eq!(branches.len(), 1);
                    assert_eq!(branches[0].id, branch_id);
                    assert!(branches[0].active);
                    assert!(branches[0].requires_force);
                    assert!(branches[0].base_current);
                    assert_eq!(branches[0].files.len(), 1);
                    assert_eq!(branches[0].commits.len(), 2);
                    assert!(!branches[0].commits[0].is_remote);
                    assert!(!branches[0].commits[0].is_integrated);
                    assert!(branches[0].commits[1].is_remote);
                    assert!(!branches[0].commits[1].is_integrated);
                    assert!(controller
                        .can_apply_virtual_branch(&project_id, &branch_id)
                        .await
                        .unwrap());
                }
            }
        }

        #[tokio::test]
        async fn no_conflicts() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "no conflicts", None)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                branch_id
            };

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // just rebases branch

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "second"
                );
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file2.txt")).unwrap(),
                    "still no conflict"
                );
            }
        }

        #[tokio::test]
        async fn integrated_commit_plus_work() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                repository.commit_all("first");
                repository.push();
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "second").unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "second", None)
                    .await
                    .unwrap();
                controller
                    .push_virtual_branch(&project_id, &branch_id, false)
                    .await
                    .unwrap();

                {
                    // merge branch upstream
                    let branch = controller
                        .list_virtual_branches(&project_id)
                        .await
                        .unwrap()
                        .into_iter()
                        .find(|b| b.id == branch_id)
                        .unwrap();
                    repository.merge(&branch.upstream.as_ref().unwrap().name);
                    repository.fetch();
                }

                // more local work in the same branch
                fs::write(repository.path().join("file2.txt"), "other").unwrap();

                branch_id
            };

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should remove integrated commit, but leave non integrated work as is

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert!(controller
                    .can_apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap());
            }

            {
                // applying the branch should produce conflict markers
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();
                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                    "second"
                );
                assert_eq!(
                    std::fs::read_to_string(repository.path().join("file2.txt")).unwrap(),
                    "other"
                );
            }
        }

        #[tokio::test]
        async fn integrated_with_locked_hunks() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "first").unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "first", None)
                    .await
                    .unwrap();

                branch_id
            };

            controller
                .push_virtual_branch(&project_id, &branch_id, false)
                .await
                .unwrap();

            // another non-locked hunk
            fs::write(repository.path().join("file.txt"), "first\nsecond").unwrap();

            {
                // push and merge branch remotely
                let branch =
                    controller.list_virtual_branches(&project_id).await.unwrap()[0].clone();
                repository.merge(&branch.upstream.as_ref().unwrap().name);
            }

            repository.fetch();

            {
                controller.update_base_branch(&project_id).await.unwrap();

                // removes integrated commit, leaves non commited work as is

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(branches[0].commits.is_empty());
                assert!(!branches[0].files.is_empty());
            }
        }

        #[tokio::test]
        async fn integrated_with_non_locked_hunks() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "first").unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "first", None)
                    .await
                    .unwrap();

                branch_id
            };

            controller
                .push_virtual_branch(&project_id, &branch_id, false)
                .await
                .unwrap();

            // another non-locked hunk
            fs::write(repository.path().join("another_file.txt"), "first").unwrap();

            {
                // push and merge branch remotely
                let branch =
                    controller.list_virtual_branches(&project_id).await.unwrap()[0].clone();
                repository.merge(&branch.upstream.as_ref().unwrap().name);
            }

            repository.fetch();

            {
                controller.update_base_branch(&project_id).await.unwrap();

                // removes integrated commit, leaves non commited work as is

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].commits.is_empty());
                assert!(!branches[0].files.is_empty());
            }
        }

        #[tokio::test]
        async fn all_integrated() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(repository.path().join("file.txt"), "first").unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(repository.path().join("file.txt"), "second").unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            {
                // make a branch that conflicts with the remote branch, but doesn't know about it yet
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "second").unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "second", None)
                    .await
                    .unwrap();
            };

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // just removes integrated branch

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 0);
            }
        }
    }
}

mod reset_virtual_branch {
    use gblib::virtual_branches::{controller::ControllerError, errors::ResetBranchError};

    use super::*;

    #[tokio::test]
    async fn to_head() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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
            ..
        } = Test::default();

        let base_branch = controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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
            Err(ControllerError::Action(
                ResetBranchError::CommitNotFoundInBranch(_)
            ))
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
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
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
            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch1_id)
                .unwrap();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
            repository.fetch();
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
                ..
            } = Test::default();

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            let commit_one = {
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None)
                    .await
                    .unwrap()
            };

            let commit_two = {
                fs::write(repository.path().join("file.txt"), "content two").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None)
                    .await
                    .unwrap()
            };

            controller
                .push_virtual_branch(&project_id, &branch_id, false)
                .await
                .unwrap();

            controller
                .reset_virtual_branch(&project_id, &branch_id, commit_one)
                .await
                .unwrap();

            repository.reset_hard(None);

            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );

            let cherry_picked_commit_oid = controller
                .cherry_pick(&project_id, &branch_id, commit_two)
                .await
                .unwrap();
            assert!(cherry_picked_commit_oid.is_some());
            assert!(repository.path().join("file.txt").exists());
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content two"
            );

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
            assert_eq!(branches[0].commits.len(), 2);
            assert_eq!(branches[0].commits[0].id, cherry_picked_commit_oid.unwrap());
            assert_eq!(branches[0].commits[1].id, commit_one);
        }

        #[tokio::test]
        async fn to_different_branch() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            let commit_one = {
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None)
                    .await
                    .unwrap()
            };

            let commit_two = {
                fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None)
                    .await
                    .unwrap()
            };

            controller
                .push_virtual_branch(&project_id, &branch_id, false)
                .await
                .unwrap();

            controller
                .reset_virtual_branch(&project_id, &branch_id, commit_one)
                .await
                .unwrap();

            repository.reset_hard(None);

            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );
            assert!(!repository.path().join("file_two.txt").exists());

            let branch_two_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            let cherry_picked_commit_oid = controller
                .cherry_pick(&project_id, &branch_two_id, commit_two)
                .await
                .unwrap();
            assert!(cherry_picked_commit_oid.is_some());

            let branches = controller.list_virtual_branches(&project_id).await.unwrap();
            assert!(repository.path().join("file_two.txt").exists());
            assert_eq!(
                fs::read_to_string(repository.path().join("file_two.txt")).unwrap(),
                "content two"
            );

            assert_eq!(branches.len(), 2);
            assert_eq!(branches[0].id, branch_id);
            assert!(!branches[0].active);
            assert_eq!(branches[0].commits.len(), 1);
            assert_eq!(branches[0].commits[0].id, commit_one);

            assert_eq!(branches[1].id, branch_two_id);
            assert!(branches[1].active);
            assert_eq!(branches[1].commits.len(), 1);
            assert_eq!(branches[1].commits[0].id, cherry_picked_commit_oid.unwrap());
        }

        #[tokio::test]
        async fn non_applied() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
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
                Err(ControllerError::Action(errors::CherryPickError::NotApplied))
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
                ..
            } = Test::default();

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            let commit_one = {
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit one", None)
                    .await
                    .unwrap()
            };

            {
                fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit two", None)
                    .await
                    .unwrap()
            };

            let commit_three = {
                fs::write(repository.path().join("file_three.txt"), "content three").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit three", None)
                    .await
                    .unwrap()
            };

            controller
                .push_virtual_branch(&project_id, &branch_id, false)
                .await
                .unwrap();

            controller
                .reset_virtual_branch(&project_id, &branch_id, commit_one)
                .await
                .unwrap();

            repository.reset_hard(None);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "content"
            );
            assert!(!repository.path().join("file_two.txt").exists());
            assert!(!repository.path().join("file_three.txt").exists());

            // introduce conflict with the remote commit
            fs::write(repository.path().join("file_three.txt"), "conflict").unwrap();

            {
                // cherry picking leads to conflict
                let cherry_picked_commit_oid = controller
                    .cherry_pick(&project_id, &branch_id, commit_three)
                    .await
                    .unwrap();
                assert!(cherry_picked_commit_oid.is_none());

                assert_eq!(
                    fs::read_to_string(repository.path().join("file_three.txt")).unwrap(),
                    "<<<<<<< ours\nconflict\n=======\ncontent three\n>>>>>>> theirs\n"
                );

                let branches = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert_eq!(branches[0].files.len(), 1);
                assert!(branches[0].files[0].conflicted);
                assert_eq!(branches[0].commits.len(), 1);
            }

            {
                // conflict can be resolved
                fs::write(repository.path().join("file_three.txt"), "resolved").unwrap();
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
                assert!(branches[0].requires_force);
                assert!(!branches[0].conflicted);
                assert_eq!(branches[0].commits.len(), 2);
                // resolution commit is there
                assert_eq!(branches[0].commits[0].id, commited_oid);
                assert_eq!(branches[0].commits[1].id, commit_one);
            }
        }

        #[tokio::test]
        async fn non_applied() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            let commit_oid = {
                let first = repository.commit_all("commit");
                fs::write(repository.path().join("file.txt"), "content").unwrap();
                let second = repository.commit_all("commit");
                repository.push();
                repository.reset_hard(Some(first));
                second
            };

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
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
                Err(ControllerError::Action(errors::CherryPickError::NotApplied))
            ));
        }
    }
}

mod amend {
    use super::*;

    #[tokio::test]
    async fn to_default_target() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        // amend without head commit
        fs::write(repository.path().join("file2.txt"), "content").unwrap();
        let to_amend: branch::Ownership = "file2.txt:1-2".parse().unwrap();
        assert!(matches!(
            controller
                .amend(&project_id, &branch_id, &to_amend)
                .await
                .unwrap_err(),
            ControllerError::Action(errors::AmendError::BranchHasNoCommits)
        ));
    }

    #[tokio::test]
    async fn forcepush_allowed() {
        let Test {
            repository,
            project_id,
            controller,
            projects,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        projects
            .update(&projects::UpdateRequest {
                id: project_id,
                ok_with_force_push: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            // create commit
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap();
        };

        controller
            .push_virtual_branch(&project_id, &branch_id, false)
            .await
            .unwrap();

        {
            // amend another hunk
            fs::write(repository.path().join("file2.txt"), "content2").unwrap();
            let to_amend: branch::Ownership = "file2.txt:1-2".parse().unwrap();
            controller
                .amend(&project_id, &branch_id, &to_amend)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert!(branch.requires_force);
            assert_eq!(branch.commits.len(), 1);
            assert_eq!(branch.files.len(), 0);
            assert_eq!(branch.commits[0].files.len(), 2);
        }
    }

    #[tokio::test]
    async fn forcepush_forbidden() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            // create commit
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap();
        };

        controller
            .push_virtual_branch(&project_id, &branch_id, false)
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file2.txt"), "content2").unwrap();
            let to_amend: branch::Ownership = "file2.txt:1-2".parse().unwrap();
            assert!(matches!(
                controller
                    .amend(&project_id, &branch_id, &to_amend)
                    .await
                    .unwrap_err(),
                ControllerError::Action(errors::AmendError::ForcePushNotAllowed(_))
            ));
        }
    }

    #[tokio::test]
    async fn non_locked_hunk() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            // create commit
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.commits.len(), 1);
            assert_eq!(branch.files.len(), 0);
            assert_eq!(branch.commits[0].files.len(), 1);
        };

        {
            // amend another hunk
            fs::write(repository.path().join("file2.txt"), "content2").unwrap();
            let to_amend: branch::Ownership = "file2.txt:1-2".parse().unwrap();
            controller
                .amend(&project_id, &branch_id, &to_amend)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.commits.len(), 1);
            assert_eq!(branch.files.len(), 0);
            assert_eq!(branch.commits[0].files.len(), 2);
        }
    }

    #[tokio::test]
    async fn locked_hunk() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            // create commit
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.commits.len(), 1);
            assert_eq!(branch.files.len(), 0);
            assert_eq!(branch.commits[0].files.len(), 1);
            assert_eq!(
                branch.commits[0].files[0].hunks[0].diff,
                "@@ -0,0 +1 @@\n+content\n\\ No newline at end of file\n"
            );
        };

        {
            // amend another hunk
            fs::write(repository.path().join("file.txt"), "more content").unwrap();
            let to_amend: branch::Ownership = "file.txt:1-2".parse().unwrap();
            controller
                .amend(&project_id, &branch_id, &to_amend)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();

            assert_eq!(branch.commits.len(), 1);
            assert_eq!(branch.files.len(), 0);
            assert_eq!(branch.commits[0].files.len(), 1);
            assert_eq!(
                branch.commits[0].files[0].hunks[0].diff,
                "@@ -0,0 +1 @@\n+more content\n\\ No newline at end of file\n"
            );
        }
    }

    #[tokio::test]
    async fn non_existing_ownership() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            // create commit
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.commits.len(), 1);
            assert_eq!(branch.files.len(), 0);
            assert_eq!(branch.commits[0].files.len(), 1);
        };

        {
            // amend non existing hunk
            let to_amend: branch::Ownership = "file2.txt:1-2".parse().unwrap();
            assert!(matches!(
                controller
                    .amend(&project_id, &branch_id, &to_amend)
                    .await
                    .unwrap_err(),
                ControllerError::Action(errors::AmendError::TargetOwnerhshipNotFound(_))
            ));
        }
    }
}

mod init {
    use super::*;

    #[tokio::test]
    async fn twice() {
        let data_dir = paths::data_dir();
        let keys = keys::Controller::from(&data_dir);
        let projects = projects::Controller::from(&data_dir);
        let users = users::Controller::from(&data_dir);
        let helper = git::credentials::Helper::from(&data_dir);

        let test_project = TestProject::default();

        let controller = Controller::new(&data_dir, &projects, &users, &keys, &helper);

        {
            let project = projects
                .add(test_project.path())
                .expect("failed to add project");
            controller
                .set_base_branch(&project.id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();
            assert!(controller
                .list_virtual_branches(&project.id)
                .await
                .unwrap()
                .is_empty());
            projects.delete(&project.id).await.unwrap();
            controller
                .list_virtual_branches(&project.id)
                .await
                .unwrap_err();
        }

        {
            let project = projects.add(test_project.path()).unwrap();
            controller
                .set_base_branch(&project.id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            // even though project is on gitbutler/integration, we should not import it
            assert!(controller
                .list_virtual_branches(&project.id)
                .await
                .unwrap()
                .is_empty());
        }
    }

    #[tokio::test]
    async fn dirty_non_target() {
        // a situation when you initialize project while being on the local verison of the master
        // that has uncommited changes.
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        repository.checkout("refs/heads/some-feature".parse().unwrap());

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(branches[0].files[0].hunks.len(), 1);
        assert!(branches[0].upstream.is_none());
        assert_eq!(branches[0].name, "some-feature");
    }

    #[tokio::test]
    async fn dirty_target() {
        // a situation when you initialize project while being on the local verison of the master
        // that has uncommited changes.
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(branches[0].files[0].hunks.len(), 1);
        assert!(branches[0].upstream.is_none());
        assert_eq!(branches[0].name, "master");
    }

    #[tokio::test]
    async fn commit_on_non_target_local() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        repository.checkout("refs/heads/some-feature".parse().unwrap());
        fs::write(repository.path().join("file.txt"), "content").unwrap();
        repository.commit_all("commit on target");

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        dbg!(&branches);
        assert_eq!(branches.len(), 1);
        assert!(branches[0].files.is_empty());
        assert_eq!(branches[0].commits.len(), 1);
        assert!(branches[0].upstream.is_none());
        assert_eq!(branches[0].name, "some-feature");
    }

    #[tokio::test]
    async fn commit_on_non_target_remote() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        repository.checkout("refs/heads/some-feature".parse().unwrap());
        fs::write(repository.path().join("file.txt"), "content").unwrap();
        repository.commit_all("commit on target");
        repository.push_branch(&"refs/heads/some-feature".parse().unwrap());

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        dbg!(&branches);
        assert_eq!(branches.len(), 1);
        assert!(branches[0].files.is_empty());
        assert_eq!(branches[0].commits.len(), 1);
        assert!(branches[0].upstream.is_some());
        assert_eq!(branches[0].name, "some-feature");
    }

    #[tokio::test]
    async fn commit_on_target() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        fs::write(repository.path().join("file.txt"), "content").unwrap();
        repository.commit_all("commit on target");

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert!(branches[0].files.is_empty());
        assert_eq!(branches[0].commits.len(), 1);
        assert!(branches[0].upstream.is_none());
        assert_eq!(branches[0].name, "master");
    }

    #[tokio::test]
    async fn submodule() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        let submodule_url: git::Url = TestProject::default()
            .path()
            .display()
            .to_string()
            .parse()
            .unwrap();
        repository.add_submodule(&submodule_url, path::Path::new("submodule"));

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branches = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(branches[0].files[0].hunks.len(), 1);
    }
}

mod squash {
    use super::*;

    #[tokio::test]
    async fn head() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None)
                .await
                .unwrap()
        };

        let commit_four_oid = {
            fs::write(repository.path().join("file four.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit four", None)
                .await
                .unwrap()
        };

        controller
            .squash(&project_id, &branch_id, commit_four_oid)
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();

        let descriptions = branch
            .commits
            .iter()
            .map(|c| c.description.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            descriptions,
            vec!["commit three\ncommit four", "commit two", "commit one"]
        );
    }

    #[tokio::test]
    async fn middle() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        let commit_two_oid = {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file four.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit four", None)
                .await
                .unwrap()
        };

        controller
            .squash(&project_id, &branch_id, commit_two_oid)
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();

        let descriptions = branch
            .commits
            .iter()
            .map(|c| c.description.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            descriptions,
            vec!["commit four", "commit three", "commit one\ncommit two"]
        );
    }

    #[tokio::test]
    async fn forcepush_allowed() {
        let Test {
            repository,
            project_id,
            controller,
            projects,
            ..
        } = Test::default();

        projects
            .update(&projects::UpdateRequest {
                id: project_id,
                ok_with_force_push: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        controller
            .push_virtual_branch(&project_id, &branch_id, false)
            .await
            .unwrap();

        let commit_two_oid = {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file four.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit four", None)
                .await
                .unwrap()
        };

        controller
            .squash(&project_id, &branch_id, commit_two_oid)
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();

        let descriptions = branch
            .commits
            .iter()
            .map(|c| c.description.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            descriptions,
            vec!["commit four", "commit three", "commit one\ncommit two"]
        );
        assert!(branch.requires_force);
    }

    #[tokio::test]
    async fn forcepush_forbidden() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        controller
            .push_virtual_branch(&project_id, &branch_id, false)
            .await
            .unwrap();

        let commit_two_oid = {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file four.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit four", None)
                .await
                .unwrap()
        };

        assert!(matches!(
            controller
                .squash(&project_id, &branch_id, commit_two_oid)
                .await
                .unwrap_err(),
            ControllerError::Action(errors::SquashError::ForcePushNotAllowed(_))
        ));
    }

    #[tokio::test]
    async fn root() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one_oid = {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        assert!(matches!(
            controller
                .squash(&project_id, &branch_id, commit_one_oid)
                .await
                .unwrap_err(),
            ControllerError::Action(errors::SquashError::CantSquashRootCommit)
        ));
    }
}

mod update_commit_message {
    use super::*;

    #[tokio::test]
    async fn head() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None)
                .await
                .unwrap()
        };

        let commit_three_oid = {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None)
                .await
                .unwrap()
        };

        controller
            .update_commit_message(
                &project_id,
                &branch_id,
                commit_three_oid,
                "commit three updated",
            )
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();

        let descriptions = branch
            .commits
            .iter()
            .map(|c| c.description.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            descriptions,
            vec!["commit three updated", "commit two", "commit one"]
        );
    }

    #[tokio::test]
    async fn middle() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        let commit_two_oid = {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None)
                .await
                .unwrap()
        };

        controller
            .update_commit_message(
                &project_id,
                &branch_id,
                commit_two_oid,
                "commit two updated",
            )
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();

        let descriptions = branch
            .commits
            .iter()
            .map(|c| c.description.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            descriptions,
            vec!["commit three", "commit two updated", "commit one"]
        );
    }

    #[tokio::test]
    async fn forcepush_allowed() {
        let Test {
            repository,
            project_id,
            controller,
            projects,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        projects
            .update(&projects::UpdateRequest {
                id: project_id,
                ok_with_force_push: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one_oid = {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        controller
            .push_virtual_branch(&project_id, &branch_id, false)
            .await
            .unwrap();

        controller
            .update_commit_message(
                &project_id,
                &branch_id,
                commit_one_oid,
                "commit one updated",
            )
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();

        let descriptions = branch
            .commits
            .iter()
            .map(|c| c.description.clone())
            .collect::<Vec<_>>();
        assert_eq!(descriptions, vec!["commit one updated"]);
        assert!(branch.requires_force);
    }

    #[tokio::test]
    async fn forcepush_forbidden() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one_oid = {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        controller
            .push_virtual_branch(&project_id, &branch_id, false)
            .await
            .unwrap();

        assert!(matches!(
            controller
                .update_commit_message(
                    &project_id,
                    &branch_id,
                    commit_one_oid,
                    "commit one updated",
                )
                .await
                .unwrap_err(),
            ControllerError::Action(errors::UpdateCommitMessageError::ForcePushNotAllowed(_))
        ));
    }

    #[tokio::test]
    async fn root() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one_oid = {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None)
                .await
                .unwrap()
        };

        controller
            .update_commit_message(
                &project_id,
                &branch_id,
                commit_one_oid,
                "commit one updated",
            )
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();

        let descriptions = branch
            .commits
            .iter()
            .map(|c| c.description.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            descriptions,
            vec!["commit three", "commit two", "commit one updated"]
        );
    }

    #[tokio::test]
    async fn empty() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one_oid = {
            fs::write(repository.path().join("file one.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit one", None)
                .await
                .unwrap()
        };

        assert!(matches!(
            controller
                .update_commit_message(&project_id, &branch_id, commit_one_oid, "",)
                .await,
            Err(ControllerError::Action(
                errors::UpdateCommitMessageError::EmptyMessage
            ))
        ));
    }
}
