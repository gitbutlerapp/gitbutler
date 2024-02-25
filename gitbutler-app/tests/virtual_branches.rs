//TODO:
#![allow(
    clippy::redundant_closure_for_method_calls,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::dbg_macro
)]

mod common;

use std::{fs, path, str::FromStr};

use gblib::{
    error::Error,
    git, keys,
    projects::{self, ProjectId},
    users,
    virtual_branches::{branch, controller::ControllerError, errors, Controller},
};

use self::common::{paths, TestProject};

struct Test {
    repository: TestProject,
    project_id: ProjectId,
    projects: projects::Controller,
    controller: Controller,
}

impl Default for Test {
    fn default() -> Self {
        let data_dir = paths::data_dir();
        let keys = keys::Controller::try_from(&data_dir).unwrap();
        let projects = projects::Controller::try_from(&data_dir).unwrap();
        let users = users::Controller::try_from(&data_dir).unwrap();
        let helper = git::credentials::Helper::try_from(&data_dir).unwrap();

        let test_project = TestProject::default();
        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        Self {
            repository: test_project,
            project_id: project.id,
            controller: Controller::new(data_dir, projects.clone(), users, keys, helper),
            projects,
        }
    }
}

mod unapply_ownership {
    use gblib::virtual_branches::branch::Ownership;

    use super::*;

    #[tokio::test]
    async fn should_unapply_with_commits() {
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

        fs::write(
            repository.path().join("file.txt"),
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n",
        )
        .unwrap();
        controller
            .create_commit(&project_id, &branch_id, "test", None, false)
            .await
            .unwrap();

        // change in the committed hunks leads to hunk locking
        fs::write(
            repository.path().join("file.txt"),
            "_\n2\n3\n4\n5\n6\n7\n8\n9\n_\n",
        )
        .unwrap();

        controller
            .unapply_ownership(
                &project_id,
                &"file.txt:1-2,10-11".parse::<Ownership>().unwrap(),
            )
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();
        assert!(branch.files.is_empty());
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
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(!branch.files[0].hunks[0].locked);
        }

        controller
            .create_commit(&project_id, &branch_id, "test", None, false)
            .await
            .unwrap();

        {
            // change in the committed hunks leads to hunk locking
            fs::write(repository.path().join("file.txt"), "updated content").unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(branch.files[0].hunks[0].locked);
        }
    }

    #[tokio::test]
    async fn should_not_lock_disjointed_hunks() {
        let Test {
            project_id,
            controller,
            repository,
            ..
        } = Test::default();

        let mut lines: Vec<_> = (0_i32..24_i32).map(|i| format!("line {}", i)).collect();
        fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
        repository.commit_all("my commit");
        repository.push();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            // new hunk in the middle of the file
            lines[12] = "commited stuff".to_string();
            fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(!branch.files[0].hunks[0].locked);
        }

        controller
            .create_commit(&project_id, &branch_id, "test commit", None, false)
            .await
            .unwrap();
        controller
            .push_virtual_branch(&project_id, &branch_id, false)
            .await
            .unwrap();

        {
            // hunk before the commited part is not locked
            let mut changed_lines = lines.clone();
            changed_lines[0] = "updated line\nwith extra line".to_string();
            fs::write(repository.path().join("file.txt"), changed_lines.join("\n")).unwrap();
            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(!branch.files[0].hunks[0].locked);
            // cleanup
            fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
        }
        {
            // hunk after the commited part is not locked
            let mut changed_lines = lines.clone();
            changed_lines[23] = "updated line".to_string();
            fs::write(repository.path().join("file.txt"), changed_lines.join("\n")).unwrap();
            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(!branch.files[0].hunks[0].locked);
            // cleanup
            fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
        }
        {
            // hunk before the commited part but with overlapping context
            let mut changed_lines = lines.clone();
            changed_lines[10] = "updated line".to_string();
            fs::write(repository.path().join("file.txt"), changed_lines.join("\n")).unwrap();
            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(!branch.files[0].hunks[0].locked);
            // cleanup
            fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
        }
        {
            // hunk after the commited part but with overlapping context
            let mut changed_lines = lines.clone();
            changed_lines[14] = "updated line".to_string();
            fs::write(repository.path().join("file.txt"), changed_lines.join("\n")).unwrap();
            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert_eq!(branch.files.len(), 1);
            assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
            assert_eq!(branch.files[0].hunks.len(), 1);
            assert!(!branch.files[0].hunks[0].locked);
            // cleanup
            fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert_eq!(branches[0].name, "Virtual branch");

            let refnames = repository
                .references()
                .into_iter()
                .filter_map(|reference| reference.name().map(|name| name.to_string()))
                .collect::<Vec<_>>();
            assert!(refnames.contains(&"refs/gitbutler/Virtual-branch".to_string()));
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "test", None, false)
                .await
                .unwrap();
            controller
                .push_virtual_branch(&project_id, &branch1_id, false)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch1_id, "test", None, false)
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
                    .create_commit(&project_id, &branch2_id, "test", None, false)
                    .await
                    .unwrap();
                controller
                    .push_virtual_branch(&project_id, &branch2_id, false)
                    .await
                    .unwrap();
                branch2_id
            };

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();

        controller
            .delete_virtual_branch(&project_id, &branches[0].id)
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

    use pretty_assertions::assert_eq;

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

    #[tokio::test]
    async fn go_back_to_integration() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        println!("{}", repository.path().display());

        let base = controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);

        let base_two = controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        assert_eq!(base_two, base);
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        controller
            .unapply_virtual_branch(&project_id, &branches[0].id)
            .await
            .unwrap();

        assert!(!repository.path().join("file.txt").exists());

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert!(!branches[0].active);
    }

    #[tokio::test]
    async fn conflicting() {
        let Test {
            project_id,
            controller,
            repository,
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
            // make a conflicting branch, and stash it

            std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert!(branches[0].base_current);
            assert!(branches[0].active);
            assert_eq!(branches[0].files[0].hunks[0].diff, "@@ -1,1 +1,1 @@\n-first\n\\ No newline at end of file\n+conflict\n\\ No newline at end of file\n");

            controller
                .unapply_virtual_branch(&project_id, &branches[0].id)
                .await
                .unwrap();

            branches[0].id
        };

        {
            // update base branch, causing conflict
            controller.update_base_branch(&project_id).await.unwrap();

            assert_eq!(
                std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "second"
            );

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|branch| branch.id == branch_id)
                .unwrap();
            assert!(!branch.base_current);
            assert!(!branch.active);
        }

        {
            // apply branch, it should conflict
            controller
                .apply_virtual_branch(&project_id, &branch_id)
                .await
                .unwrap();

            assert_eq!(
                std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
            );

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert!(branch.base_current);
            assert!(branch.conflicted);
            assert_eq!(branch.files[0].hunks[0].diff, "@@ -1,1 +1,5 @@\n-first\n\\ No newline at end of file\n+<<<<<<< ours\n+conflict\n+=======\n+second\n+>>>>>>> theirs\n");
        }

        {
            controller
                .unapply_virtual_branch(&project_id, &branch_id)
                .await
                .unwrap();

            assert_eq!(
                std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "second"
            );

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|b| b.id == branch_id)
                .unwrap();
            assert!(!branch.active);
            assert!(!branch.base_current);
            assert!(!branch.conflicted);
            assert_eq!(branch.files[0].hunks[0].diff, "@@ -1,1 +1,1 @@\n-first\n\\ No newline at end of file\n+conflict\n\\ No newline at end of file\n");
        }
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        controller
            .unapply_virtual_branch(&project_id, &branches[0].id)
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "virtual commit", None, false)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert!(branches[0].active);

        branch1_id
    };

    {
        // fetch remote
        controller.update_base_branch(&project_id).await.unwrap();

        // there is a conflict now, so the branch should be inactive
        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "commit conflicts", None, false)
                .await,
            Err(ControllerError::Action(errors::CommitError::Conflicted(_)))
        ));
    }

    {
        // fixing the conflict removes conflicted mark
        fs::write(repository.path().join("file.txt"), "resolved").unwrap();
        let commit_oid = controller
            .create_commit(&project_id, &branch1_id, "resolution", None, false)
            .await
            .unwrap();

        let commit = repository.find_commit(commit_oid).unwrap();
        assert_eq!(commit.parent_count(), 2);

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert!(branches[0].active);
        assert!(!branches[0].conflicted);
    }
}

mod fetch_from_target {
    use super::*;

    #[tokio::test]
    async fn should_update_last_fetched() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let before_fetch = controller.get_base_branch_data(&project_id).await.unwrap();
        assert!(before_fetch.unwrap().last_fetched_ms.is_none());

        let fetch = controller.fetch_from_target(&project_id).await.unwrap();
        assert!(fetch.last_fetched_ms.is_some());

        let after_fetch = controller.get_base_branch_data(&project_id).await.unwrap();
        assert!(after_fetch.as_ref().unwrap().last_fetched_ms.is_some());
        assert_eq!(fetch.last_fetched_ms, after_fetch.unwrap().last_fetched_ms);

        let second_fetch = controller.fetch_from_target(&project_id).await.unwrap();
        assert!(second_fetch.last_fetched_ms.is_some());
        assert_ne!(fetch.last_fetched_ms, second_fetch.last_fetched_ms);

        let after_second_fetch = controller.get_base_branch_data(&project_id).await.unwrap();
        assert!(after_second_fetch
            .as_ref()
            .unwrap()
            .last_fetched_ms
            .is_some());
        assert_eq!(
            second_fetch.last_fetched_ms,
            after_second_fetch.unwrap().last_fetched_ms
        );
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "conflicting commit", None, false)
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "conflicting commit", None, false)
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "conflicting commit", None, false)
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "conflicting commit", None, false)
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(
                        &project_id,
                        &branch_id,
                        "non conflicting commit",
                        None,
                        false,
                    )
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(branches[0].upstream.is_none());
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "second", None, false)
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
                        .0
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
                assert!(branches[0].upstream.is_none());
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "second", None, false)
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 0);
            }
        }

        #[tokio::test]
        async fn integrate_work_while_being_behind() {
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

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            {
                // open pr
                fs::write(repository.path().join("file2.txt"), "new file").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "second", None, false)
                    .await
                    .unwrap();
                controller
                    .push_virtual_branch(&project_id, &branch_id, false)
                    .await
                    .unwrap();
            }

            controller
                .unapply_virtual_branch(&project_id, &branch_id)
                .await
                .unwrap();

            {
                // merge pr
                let branch = controller
                    .list_virtual_branches(&project_id)
                    .await
                    .unwrap()
                    .0[0]
                    .clone();
                repository.merge(&branch.upstream.as_ref().unwrap().name);
                repository.fetch();
            }

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // just removes integrated branch
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "conflicting commit", None, false)
                    .await
                    .unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should stash the branch.

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "conflicting commit", None, false)
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "conflicting commit", None, false)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should rebase upstream, and leave uncommited file as is

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "conflicting commit", None, false)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();

                branch_id
            };

            {
                // when fetching remote
                controller.update_base_branch(&project_id).await.unwrap();

                // should merge upstream, and leave uncommited file as is.

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                        .create_commit(&project_id, &branch_id, "no conflicts", None, false)
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

                    let (branches, _) =
                        controller.list_virtual_branches(&project_id).await.unwrap();
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
                        .create_commit(&project_id, &branch_id, "no conflicts", None, false)
                        .await
                        .unwrap();
                    controller
                        .push_virtual_branch(&project_id, &branch_id, false)
                        .await
                        .unwrap();

                    fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                    branch_id
                };

                projects
                    .update(&projects::UpdateRequest {
                        id: project_id,
                        ok_with_force_push: Some(false),
                        ..Default::default()
                    })
                    .await
                    .unwrap();

                {
                    // fetch remote
                    controller.update_base_branch(&project_id).await.unwrap();

                    // creates a merge commit, since the branch is pushed

                    let (branches, _) =
                        controller.list_virtual_branches(&project_id).await.unwrap();
                    assert_eq!(branches.len(), 1);
                    assert_eq!(branches[0].id, branch_id);
                    assert!(branches[0].active);
                    assert!(!branches[0].requires_force);
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
                    .create_commit(&project_id, &branch_id, "no conflicts", None, false)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                branch_id
            };

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // just rebases branch

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "second", None, false)
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
                        .0
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
        async fn integrated_with_locked_conflicting_hunks() {
            let Test {
                repository,
                project_id,
                controller,
                ..
            } = Test::default();

            // make sure we have an undiscovered commit in the remote branch
            {
                fs::write(
                    repository.path().join("file.txt"),
                    "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n",
                )
                .unwrap();
                let first_commit_oid = repository.commit_all("first");
                fs::write(
                    repository.path().join("file.txt"),
                    "1\n2\n3\n4\n5\n6\n17\n8\n9\n10\n11\n12\n",
                )
                .unwrap();
                repository.commit_all("second");
                repository.push();
                repository.reset_hard(Some(first_commit_oid));
            }

            controller
                .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            // branch has no conflict
            let branch_id = {
                let branch_id = controller
                    .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(
                    repository.path().join("file.txt"),
                    "1\n2\n3\n4\n5\n6\n7\n8\n19\n10\n11\n12\n",
                )
                .unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "first", None, false)
                    .await
                    .unwrap();

                branch_id
            };

            // push the branch
            controller
                .push_virtual_branch(&project_id, &branch_id, false)
                .await
                .unwrap();

            // another locked conflicing hunk
            fs::write(
                repository.path().join("file.txt"),
                "1\n2\n3\n4\n5\n6\n77\n8\n19\n10\n11\n12\n",
            )
            .unwrap();

            {
                // merge branch remotely
                let branch = controller
                    .list_virtual_branches(&project_id)
                    .await
                    .unwrap()
                    .0[0]
                    .clone();
                repository.merge(&branch.upstream.as_ref().unwrap().name);
            }

            repository.fetch();

            {
                controller.update_base_branch(&project_id).await.unwrap();

                // removes integrated commit, leaves non commited work as is

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(!branches[0].active);
                assert!(branches[0].commits.is_empty());
                assert!(!branches[0].files.is_empty());
            }

            {
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert!(branches[0].active);
                assert!(branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].files[0].hunks.len(), 1);
                assert_eq!(branches[0].files[0].hunks[0].diff, "@@ -4,7 +4,11 @@\n 4\n 5\n 6\n-7\n+<<<<<<< ours\n+77\n+=======\n+17\n+>>>>>>> theirs\n 8\n 19\n 10\n");
                assert_eq!(branches[0].commits.len(), 0);
            }
        }

        #[tokio::test]
        async fn integrated_with_locked_hunks() {
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
                    ok_with_force_push: Some(false),
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

                fs::write(repository.path().join("file.txt"), "first").unwrap();

                controller
                    .create_commit(&project_id, &branch_id, "first", None, false)
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
                let branch = controller
                    .list_virtual_branches(&project_id)
                    .await
                    .unwrap()
                    .0[0]
                    .clone();
                repository.merge(&branch.upstream.as_ref().unwrap().name);
            }

            repository.fetch();

            {
                controller.update_base_branch(&project_id).await.unwrap();

                // removes integrated commit, leaves non commited work as is

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].commits.is_empty());
                assert!(branches[0].upstream.is_none());
                assert_eq!(branches[0].files.len(), 1);
            }

            {
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0); // no merge commit
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
                    .create_commit(&project_id, &branch_id, "first", None, false)
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
                let branch = controller
                    .list_virtual_branches(&project_id)
                    .await
                    .unwrap()
                    .0[0]
                    .clone();
                repository.merge(&branch.upstream.as_ref().unwrap().name);
            }

            repository.fetch();

            {
                controller.update_base_branch(&project_id).await.unwrap();

                // removes integrated commit, leaves non commited work as is

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].commits.is_empty());
                assert!(branches[0].upstream.is_none());
                assert!(!branches[0].files.is_empty());
            }

            {
                controller
                    .apply_virtual_branch(&project_id, &branch_id)
                    .await
                    .unwrap();

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 1);
                assert!(branches[0].active);
                assert!(!branches[0].conflicted);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 0);
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
                    .create_commit(&project_id, &branch_id, "second", None, false)
                    .await
                    .unwrap();
            };

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // just removes integrated branch

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
                assert_eq!(branches.len(), 0);
            }
        }

        #[tokio::test]
        async fn integrate_work_while_being_behind() {
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

            let branch_id = controller
                .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            {
                // open pr
                fs::write(repository.path().join("file2.txt"), "new file").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "second", None, false)
                    .await
                    .unwrap();
                controller
                    .push_virtual_branch(&project_id, &branch_id, false)
                    .await
                    .unwrap();
            }

            {
                // merge pr
                let branch = controller
                    .list_virtual_branches(&project_id)
                    .await
                    .unwrap()
                    .0[0]
                    .clone();
                repository.merge(&branch.upstream.as_ref().unwrap().name);
                repository.fetch();
            }

            {
                // fetch remote
                controller.update_base_branch(&project_id).await.unwrap();

                // just removes integrated branch
                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap()
        };

        let oid2 = {
            // create second commit
            fs::write(repository.path().join("file.txt"), "content2").unwrap();
            controller
                .create_commit(&project_id, &branch1_id, "commit", None, false)
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap()
        };

        {
            // should correctly detect pushed commits
            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap()
        };

        let oid2 = {
            // create second commit
            fs::write(repository.path().join("file.txt"), "content2").unwrap();
            controller
                .create_commit(&project_id, &branch1_id, "commit", None, false)
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
                .0
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
                .create_commit(&project_id, &branch1_id, "commit", None, false)
                .await
                .unwrap()
        };

        {
            // should correctly detect pushed commits
            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "commit", None, false)
                    .await
                    .unwrap()
            };

            let commit_two = {
                fs::write(repository.path().join("file.txt"), "content two").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None, false)
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "commit", None, false)
                    .await
                    .unwrap()
            };

            let commit_two = {
                fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None, false)
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

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "commit", None, false)
                    .await
                    .unwrap()
            };

            {
                fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None, false)
                    .await
                    .unwrap()
            };

            let commit_three_oid = {
                fs::write(repository.path().join("file_three.txt"), "content three").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit", None, false)
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
                    .create_commit(&project_id, &branch_id, "commit one", None, false)
                    .await
                    .unwrap()
            };

            {
                fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit two", None, false)
                    .await
                    .unwrap()
            };

            let commit_three = {
                fs::write(repository.path().join("file_three.txt"), "content three").unwrap();
                controller
                    .create_commit(&project_id, &branch_id, "commit three", None, false)
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

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                    .create_commit(&project_id, &branch_id, "resolution", None, false)
                    .await
                    .unwrap();

                let commit = repository.find_commit(commited_oid).unwrap();
                assert_eq!(commit.parent_count(), 2);

                let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        projects
            .update(&projects::UpdateRequest {
                id: project_id,
                ok_with_force_push: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();

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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
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
                .0
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
                ok_with_force_push: Some(false),
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
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
                .0
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
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
                .0
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
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
        let keys = keys::Controller::try_from(&data_dir).unwrap();
        let projects = projects::Controller::try_from(&data_dir).unwrap();
        let users = users::Controller::try_from(&data_dir).unwrap();
        let helper = git::credentials::Helper::try_from(&data_dir).unwrap();

        let test_project = TestProject::default();

        let controller = Controller::new(data_dir, projects.clone(), users, keys, helper);

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
                .0
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
                .0
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

        repository.checkout(&"refs/heads/some-feature".parse().unwrap());

        fs::write(repository.path().join("file.txt"), "content").unwrap();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        repository.checkout(&"refs/heads/some-feature".parse().unwrap());
        fs::write(repository.path().join("file.txt"), "content").unwrap();
        repository.commit_all("commit on target");

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        repository.checkout(&"refs/heads/some-feature".parse().unwrap());
        fs::write(repository.path().join("file.txt"), "content").unwrap();
        repository.commit_all("commit on target");
        repository.push_branch(&"refs/heads/some-feature".parse().unwrap());

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None, false)
                .await
                .unwrap()
        };

        let commit_four_oid = {
            fs::write(repository.path().join("file four.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit four", None, false)
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
            .0
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap()
        };

        let commit_two_oid = {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file four.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit four", None, false)
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
            .0
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
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
                .create_commit(&project_id, &branch_id, "commit two", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file four.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit four", None, false)
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
            .0
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
            projects,
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap()
        };

        controller
            .push_virtual_branch(&project_id, &branch_id, false)
            .await
            .unwrap();

        projects
            .update(&projects::UpdateRequest {
                id: project_id,
                ok_with_force_push: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();

        let commit_two_oid = {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file four.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit four", None, false)
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None, false)
                .await
                .unwrap()
        };

        let commit_three_oid = {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None, false)
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
            .0
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap()
        };

        let commit_two_oid = {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None, false)
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
            .0
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
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
            .0
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
                ok_with_force_push: Some(false),
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file two.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit two", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file three.txt"), "").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "commit three", None, false)
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
            .0
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
                .create_commit(&project_id, &branch_id, "commit one", None, false)
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

mod create_virtual_branch_from_branch {
    use super::*;

    #[tokio::test]
    async fn integration() {
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

        let branch_name = {
            // make a remote branch

            let branch_id = controller
                .create_virtual_branch(&project_id, &super::branch::BranchCreateRequest::default())
                .await
                .unwrap();

            std::fs::write(repository.path().join("file.txt"), "first\n").unwrap();
            controller
                .create_commit(&project_id, &branch_id, "first", None, false)
                .await
                .unwrap();
            controller
                .push_virtual_branch(&project_id, &branch_id, false)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|branch| branch.id == branch_id)
                .unwrap();

            let name = branch.upstream.unwrap().name;

            controller
                .delete_virtual_branch(&project_id, &branch_id)
                .await
                .unwrap();

            name
        };

        // checkout a existing remote branch
        let branch_id = controller
            .create_virtual_branch_from_branch(&project_id, &branch_name)
            .await
            .unwrap();

        {
            // add a commit
            std::fs::write(repository.path().join("file.txt"), "first\nsecond").unwrap();

            controller
                .create_commit(&project_id, &branch_id, "second", None, false)
                .await
                .unwrap();
        }

        {
            // meanwhile, there is a new commit on master
            repository.checkout(&"refs/heads/master".parse().unwrap());
            std::fs::write(repository.path().join("another.txt"), "").unwrap();
            repository.commit_all("another");
            repository.push_branch(&"refs/heads/master".parse().unwrap());
            repository.checkout(&"refs/heads/gitbutler/integration".parse().unwrap());
        }

        {
            // merge branch into master
            controller
                .push_virtual_branch(&project_id, &branch_id, false)
                .await
                .unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|branch| branch.id == branch_id)
                .unwrap();

            assert!(branch.commits[0].is_remote);
            assert!(!branch.commits[0].is_integrated);
            assert!(branch.commits[1].is_remote);
            assert!(!branch.commits[1].is_integrated);

            repository.rebase_and_merge(&branch_name);
        }

        {
            // should mark commits as integrated
            controller.fetch_from_target(&project_id).await.unwrap();

            let branch = controller
                .list_virtual_branches(&project_id)
                .await
                .unwrap()
                .0
                .into_iter()
                .find(|branch| branch.id == branch_id)
                .unwrap();

            assert!(branch.commits[0].is_remote);
            assert!(branch.commits[0].is_integrated);
            assert!(branch.commits[1].is_remote);
            assert!(branch.commits[1].is_integrated);
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

        {
            // create a remote branch
            let branch_name: git::LocalRefname = "refs/heads/branch".parse().unwrap();
            repository.checkout(&branch_name);
            fs::write(repository.path().join("file.txt"), "first").unwrap();
            repository.commit_all("first");
            repository.push_branch(&branch_name);
            repository.checkout(&"refs/heads/master".parse().unwrap());
        }

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert!(branches.is_empty());

        let branch_id = controller
            .create_virtual_branch_from_branch(
                &project_id,
                &"refs/remotes/origin/branch".parse().unwrap(),
            )
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch_id);
        assert_eq!(branches[0].commits.len(), 1);
        assert_eq!(branches[0].commits[0].description, "first");
    }

    #[tokio::test]
    async fn conflicts_with_uncommited() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        {
            // create a remote branch
            let branch_name: git::LocalRefname = "refs/heads/branch".parse().unwrap();
            repository.checkout(&branch_name);
            fs::write(repository.path().join("file.txt"), "first").unwrap();
            repository.commit_all("first");
            repository.push_branch(&branch_name);
            repository.checkout(&"refs/heads/master".parse().unwrap());
        }

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // create a local branch that conflicts with remote
        {
            std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
        };

        // branch should be created unapplied, because of the conflict

        let new_branch_id = controller
            .create_virtual_branch_from_branch(
                &project_id,
                &"refs/remotes/origin/branch".parse().unwrap(),
            )
            .await
            .unwrap();
        let new_branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|branch| branch.id == new_branch_id)
            .unwrap();
        assert!(!new_branch.active);
        assert_eq!(new_branch.commits.len(), 1);
        assert!(new_branch.upstream.is_some());
    }

    #[tokio::test]
    async fn conflicts_with_commited() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        {
            // create a remote branch
            let branch_name: git::LocalRefname = "refs/heads/branch".parse().unwrap();
            repository.checkout(&branch_name);
            fs::write(repository.path().join("file.txt"), "first").unwrap();
            repository.commit_all("first");
            repository.push_branch(&branch_name);
            repository.checkout(&"refs/heads/master".parse().unwrap());
        }

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // create a local branch that conflicts with remote
        {
            std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

            let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
            assert_eq!(branches.len(), 1);

            controller
                .create_commit(&project_id, &branches[0].id, "hej", None, false)
                .await
                .unwrap();
        };

        // branch should be created unapplied, because of the conflict

        let new_branch_id = controller
            .create_virtual_branch_from_branch(
                &project_id,
                &"refs/remotes/origin/branch".parse().unwrap(),
            )
            .await
            .unwrap();
        let new_branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|branch| branch.id == new_branch_id)
            .unwrap();
        assert!(!new_branch.active);
        assert_eq!(new_branch.commits.len(), 1);
        assert!(new_branch.upstream.is_some());
    }

    #[tokio::test]
    async fn from_default_target() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // branch should be created unapplied, because of the conflict

        assert!(matches!(
            controller
                .create_virtual_branch_from_branch(
                    &project_id,
                    &"refs/remotes/origin/master".parse().unwrap(),
                )
                .await
                .unwrap_err(),
            ControllerError::Action(
                errors::CreateVirtualBranchFromBranchError::CantMakeBranchFromDefaultTarget
            )
        ));
    }

    #[tokio::test]
    async fn from_non_existent_branch() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // branch should be created unapplied, because of the conflict

        assert!(matches!(
            controller
                .create_virtual_branch_from_branch(
                    &project_id,
                    &"refs/remotes/origin/branch".parse().unwrap(),
                )
                .await
                .unwrap_err(),
            ControllerError::Action(errors::CreateVirtualBranchFromBranchError::BranchNotFound(
                _
            ))
        ));
    }

    #[tokio::test]
    async fn from_state_remote_branch() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = Test::default();

        {
            // create a remote branch
            let branch_name: git::LocalRefname = "refs/heads/branch".parse().unwrap();
            repository.checkout(&branch_name);
            fs::write(repository.path().join("file.txt"), "branch commit").unwrap();
            repository.commit_all("branch commit");
            repository.push_branch(&branch_name);
            repository.checkout(&"refs/heads/master".parse().unwrap());

            // make remote branch stale
            std::fs::write(repository.path().join("antoher_file.txt"), "master commit").unwrap();
            repository.commit_all("master commit");
            repository.push();
        }

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch_from_branch(
                &project_id,
                &"refs/remotes/origin/branch".parse().unwrap(),
            )
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch_id);
        assert_eq!(branches[0].commits.len(), 1);
        assert!(branches[0].files.is_empty());
        assert_eq!(branches[0].commits[0].description, "branch commit");
    }
}

mod selected_for_changes {
    use super::*;

    #[tokio::test]
    async fn unapplying_selected_branch_selects_anther() {
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

        std::fs::write(repository.path().join("file one.txt"), "").unwrap();

        // first branch should be created as default
        let b_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        // if default branch exists, new branch should not be created as default
        let b2_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();

        let b = branches.iter().find(|b| b.id == b_id).unwrap();

        let b2 = branches.iter().find(|b| b.id == b2_id).unwrap();

        assert!(b.selected_for_changes);
        assert!(!b2.selected_for_changes);

        controller
            .unapply_virtual_branch(&project_id, &b_id)
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();

        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].id, b.id);
        assert!(!branches[0].selected_for_changes);
        assert!(!branches[0].active);
        assert_eq!(branches[1].id, b2.id);
        assert!(branches[1].selected_for_changes);
        assert!(branches[1].active);
    }

    #[tokio::test]
    async fn deleting_selected_branch_selects_anther() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // first branch should be created as default
        let b_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        // if default branch exists, new branch should not be created as default
        let b2_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();

        let b = branches.iter().find(|b| b.id == b_id).unwrap();

        let b2 = branches.iter().find(|b| b.id == b2_id).unwrap();

        assert!(b.selected_for_changes);
        assert!(!b2.selected_for_changes);

        controller
            .delete_virtual_branch(&project_id, &b_id)
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();

        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, b2.id);
        assert!(branches[0].selected_for_changes);
    }

    #[tokio::test]
    async fn create_virtual_branch_should_set_selected_for_changes() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // first branch should be created as default
        let b_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();
        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b_id)
            .unwrap();
        assert!(branch.selected_for_changes);

        // if default branch exists, new branch should not be created as default
        let b_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();
        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b_id)
            .unwrap();
        assert!(!branch.selected_for_changes);

        // explicitly don't make this one default
        let b_id = controller
            .create_virtual_branch(
                &project_id,
                &branch::BranchCreateRequest {
                    selected_for_changes: Some(false),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b_id)
            .unwrap();
        assert!(!branch.selected_for_changes);

        // explicitly make this one default
        let b_id = controller
            .create_virtual_branch(
                &project_id,
                &branch::BranchCreateRequest {
                    selected_for_changes: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b_id)
            .unwrap();
        assert!(branch.selected_for_changes);
    }

    #[tokio::test]
    async fn update_virtual_branch_should_reset_selected_for_changes() {
        let Test {
            project_id,
            controller,
            ..
        } = Test::default();

        controller
            .set_base_branch(&project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let b1_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();
        let b1 = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b1_id)
            .unwrap();
        assert!(b1.selected_for_changes);

        let b2_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();
        let b2 = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b2_id)
            .unwrap();
        assert!(!b2.selected_for_changes);

        controller
            .update_virtual_branch(
                &project_id,
                branch::BranchUpdateRequest {
                    id: b2_id,
                    selected_for_changes: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let b1 = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b1_id)
            .unwrap();
        assert!(!b1.selected_for_changes);

        let b2 = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b2_id)
            .unwrap();
        assert!(b2.selected_for_changes);
    }

    #[tokio::test]
    async fn unapply_virtual_branch_should_reset_selected_for_changes() {
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

        let b1_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();
        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let b1 = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b1_id)
            .unwrap();
        assert!(b1.selected_for_changes);

        controller
            .unapply_virtual_branch(&project_id, &b1_id)
            .await
            .unwrap();

        let b1 = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == b1_id)
            .unwrap();
        assert!(!b1.selected_for_changes);
    }

    #[tokio::test]
    async fn hunks_distribution() {
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

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches[0].files.len(), 1);

        controller
            .create_virtual_branch(
                &project_id,
                &branch::BranchCreateRequest {
                    selected_for_changes: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        std::fs::write(repository.path().join("another_file.txt"), "content").unwrap();
        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(branches[1].files.len(), 1);
    }

    #[tokio::test]
    async fn applying_first_branch() {
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

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        controller
            .unapply_virtual_branch(&project_id, &branches[0].id)
            .await
            .unwrap();
        controller
            .apply_virtual_branch(&project_id, &branches[0].id)
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert!(branches[0].active);
        assert!(branches[0].selected_for_changes);
    }
}

mod move_commit_to_vbranch {
    use gblib::virtual_branches::BranchId;

    use super::*;

    #[tokio::test]
    async fn no_diffs() {
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

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        let source_branch_id = branches[0].id;

        let commit_oid = controller
            .create_commit(&project_id, &source_branch_id, "commit", None, false)
            .await
            .unwrap();

        let target_branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        controller
            .move_commit(&project_id, &target_branch_id, commit_oid)
            .await
            .unwrap();

        let destination_branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == target_branch_id)
            .unwrap();

        let source_branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == source_branch_id)
            .unwrap();

        assert_eq!(destination_branch.commits.len(), 1);
        assert_eq!(destination_branch.files.len(), 0);
        assert_eq!(source_branch.commits.len(), 0);
        assert_eq!(source_branch.files.len(), 0);
    }

    #[tokio::test]
    async fn diffs_on_source_branch() {
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

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        let source_branch_id = branches[0].id;

        let commit_oid = controller
            .create_commit(&project_id, &source_branch_id, "commit", None, false)
            .await
            .unwrap();

        std::fs::write(
            repository.path().join("another file.txt"),
            "another content",
        )
        .unwrap();

        let target_branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        controller
            .move_commit(&project_id, &target_branch_id, commit_oid)
            .await
            .unwrap();

        let destination_branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == target_branch_id)
            .unwrap();

        let source_branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == source_branch_id)
            .unwrap();

        assert_eq!(destination_branch.commits.len(), 1);
        assert_eq!(destination_branch.files.len(), 0);
        assert_eq!(source_branch.commits.len(), 0);
        assert_eq!(source_branch.files.len(), 1);
    }

    #[tokio::test]
    async fn diffs_on_target_branch() {
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

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        let source_branch_id = branches[0].id;

        let commit_oid = controller
            .create_commit(&project_id, &source_branch_id, "commit", None, false)
            .await
            .unwrap();

        let target_branch_id = controller
            .create_virtual_branch(
                &project_id,
                &branch::BranchCreateRequest {
                    selected_for_changes: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        std::fs::write(
            repository.path().join("another file.txt"),
            "another content",
        )
        .unwrap();

        controller
            .move_commit(&project_id, &target_branch_id, commit_oid)
            .await
            .unwrap();

        let destination_branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == target_branch_id)
            .unwrap();

        let source_branch = controller
            .list_virtual_branches(&project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == source_branch_id)
            .unwrap();

        assert_eq!(destination_branch.commits.len(), 1);
        assert_eq!(destination_branch.files.len(), 1);
        assert_eq!(source_branch.commits.len(), 0);
        assert_eq!(source_branch.files.len(), 0);
    }

    #[tokio::test]
    async fn locked_hunks_on_source_branch() {
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

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        let source_branch_id = branches[0].id;

        let commit_oid = controller
            .create_commit(&project_id, &source_branch_id, "commit", None, false)
            .await
            .unwrap();

        std::fs::write(repository.path().join("file.txt"), "locked content").unwrap();

        let target_branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        assert!(matches!(
            controller
                .move_commit(&project_id, &target_branch_id, commit_oid)
                .await
                .unwrap_err(),
            ControllerError::Action(errors::MoveCommitError::SourceLocked)
        ));
    }

    #[tokio::test]
    async fn no_commit() {
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

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        let source_branch_id = branches[0].id;

        controller
            .create_commit(&project_id, &source_branch_id, "commit", None, false)
            .await
            .unwrap();

        let target_branch_id = controller
            .create_virtual_branch(&project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        assert!(matches!(
            controller
                .move_commit(
                    &project_id,
                    &target_branch_id,
                    git::Oid::from_str("a99c95cca7a60f1a2180c2f86fb18af97333c192").unwrap()
                )
                .await
                .unwrap_err(),
            ControllerError::Action(errors::MoveCommitError::CommitNotFound(_))
        ));
    }

    #[tokio::test]
    async fn no_branch() {
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

        std::fs::write(repository.path().join("file.txt"), "content").unwrap();

        let (branches, _) = controller.list_virtual_branches(&project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        let source_branch_id = branches[0].id;

        let commit_oid = controller
            .create_commit(&project_id, &source_branch_id, "commit", None, false)
            .await
            .unwrap();

        assert!(matches!(
            controller
                .move_commit(&project_id, &BranchId::generate(), commit_oid)
                .await
                .unwrap_err(),
            ControllerError::Action(errors::MoveCommitError::BranchNotFound(_))
        ));
    }
}
