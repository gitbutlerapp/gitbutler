use super::*;

mod applied_branch {

    use super::*;

    #[tokio::test]
    async fn conflicts_with_uncommitted_work() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();

            branch_id
        };

        {
            // fetch remote
            controller.update_base_branch(*project_id).await.unwrap();

            // should stash conflicting branch

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(!branches[0].active);
            assert!(!branches[0].base_current);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(branches[0].commits.len(), 0);
            assert!(!controller
                .can_apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap());
        }

        {
            // applying the branch should produce conflict markers
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();
            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            // make a branch with a commit that conflicts with upstream, and work that fixes
            // that conflict
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
            controller
                .create_commit(*project_id, branch_id, "conflicting commit", None, false)
                .await
                .unwrap();

            branch_id
        };

        {
            // when fetching remote
            controller.update_base_branch(*project_id).await.unwrap();

            // should stash the branch.

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(!branches[0].active);
            assert!(!branches[0].base_current);
            assert_eq!(branches[0].files.len(), 0);
            assert_eq!(branches[0].commits.len(), 1);
            assert!(!controller
                .can_apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap());
        }

        {
            // applying the branch should produce conflict markers
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();
            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            // make a branch with a commit that conflicts with upstream, and work that fixes
            // that conflict
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
            controller
                .create_commit(*project_id, branch_id, "conflicting commit", None, false)
                .await
                .unwrap();

            controller
                .push_virtual_branch(*project_id, branch_id, false, None)
                .await
                .unwrap();

            branch_id
        };

        {
            // when fetching remote
            controller.update_base_branch(*project_id).await.unwrap();

            // should stash the branch.

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(!branches[0].active);
            assert!(!branches[0].base_current);
            assert_eq!(branches[0].files.len(), 0);
            assert_eq!(branches[0].commits.len(), 1);
            assert!(!controller
                .can_apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap());
        }

        {
            // applying the branch should produce conflict markers
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();
            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            // make a branch with a commit that conflicts with upstream, and work that fixes
            // that conflict
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
            controller
                .create_commit(*project_id, branch_id, "conflicting commit", None, false)
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();

            branch_id
        };

        {
            // when fetching remote
            controller.update_base_branch(*project_id).await.unwrap();

            // should rebase upstream, and leave uncommited file as is

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(!branches[0].active);
            assert!(!branches[0].base_current); // TODO: should be true
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(branches[0].commits.len(), 1);
            assert!(!controller
                .can_apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap()); // TODO: should be true
        }

        {
            // applying the branch should produce conflict markers
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();
            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            // make a branch with a commit that conflicts with upstream, and work that fixes
            // that conflict
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
            controller
                .create_commit(*project_id, branch_id, "conflicting commit", None, false)
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();

            branch_id
        };

        {
            // when fetching remote
            controller.update_base_branch(*project_id).await.unwrap();

            // should merge upstream, and leave uncommited file as is.

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(!branches[0].active);
            assert!(!branches[0].base_current); // TODO: should be true
            assert_eq!(branches[0].commits.len(), 1); // TODO: should be 2
            assert_eq!(branches[0].files.len(), 1);
            assert!(!controller
                .can_apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap()); // TODO: should be true
        }

        {
            // applying the branch should produce conflict markers
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();
            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
            } = &Test::default();

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
                    id: *project_id,
                    ok_with_force_push: Some(true),
                    ..Default::default()
                })
                .await
                .unwrap();

            controller
                .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                let branch_id = controller
                    .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

                controller
                    .create_commit(*project_id, branch_id, "no conflicts", None, false)
                    .await
                    .unwrap();
                controller
                    .push_virtual_branch(*project_id, branch_id, false, None)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                branch_id
            };

            {
                // fetch remote
                controller.update_base_branch(*project_id).await.unwrap();

                // rebases branch, since the branch is pushed and force pushing is
                // allowed

                let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
                    .can_apply_virtual_branch(*project_id, branch_id)
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
            } = &Test::default();

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
                .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
                .await
                .unwrap();

            let branch_id = {
                let branch_id = controller
                    .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

                controller
                    .create_commit(*project_id, branch_id, "no conflicts", None, false)
                    .await
                    .unwrap();
                controller
                    .push_virtual_branch(*project_id, branch_id, false, None)
                    .await
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                branch_id
            };

            projects
                .update(&projects::UpdateRequest {
                    id: *project_id,
                    ok_with_force_push: Some(false),
                    ..Default::default()
                })
                .await
                .unwrap();

            {
                // fetch remote
                controller.update_base_branch(*project_id).await.unwrap();

                // creates a merge commit, since the branch is pushed

                let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
                    .can_apply_virtual_branch(*project_id, branch_id)
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
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

            controller
                .create_commit(*project_id, branch_id, "no conflicts", None, false)
                .await
                .unwrap();

            fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

            branch_id
        };

        {
            // fetch remote
            controller.update_base_branch(*project_id).await.unwrap();

            // just rebases branch

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
            assert!(branches[0].base_current);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(branches[0].commits.len(), 1);
            assert!(controller
                .can_apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap());
        }

        {
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();
            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

        {
            fs::write(repository.path().join("file.txt"), "first").unwrap();
            repository.commit_all("first");
            repository.push();
        }

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "second").unwrap();

            controller
                .create_commit(*project_id, branch_id, "second", None, false)
                .await
                .unwrap();
            controller
                .push_virtual_branch(*project_id, branch_id, false, None)
                .await
                .unwrap();

            {
                // merge branch upstream
                let branch = controller
                    .list_virtual_branches(*project_id)
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
            controller.update_base_branch(*project_id).await.unwrap();

            // should remove integrated commit, but leave non integrated work as is

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
            assert!(branches[0].base_current);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(branches[0].commits.len(), 0);
            assert!(controller
                .can_apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap());
        }

        {
            // applying the branch should produce conflict markers
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();
            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        // branch has no conflict
        let branch_id = {
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(
                repository.path().join("file.txt"),
                "1\n2\n3\n4\n5\n6\n7\n8\n19\n10\n11\n12\n",
            )
            .unwrap();

            controller
                .create_commit(*project_id, branch_id, "fourth", None, false)
                .await
                .unwrap();

            branch_id
        };

        // push the branch
        controller
            .push_virtual_branch(*project_id, branch_id, false, None)
            .await
            .unwrap();

        // another locked conflicting hunk
        fs::write(
            repository.path().join("file.txt"),
            "1\n2\n3\n4\n5\n6\n77\n8\n19\n10\n11\n12\n",
        )
        .unwrap();

        {
            // merge branch remotely
            let branch = controller
                .list_virtual_branches(*project_id)
                .await
                .unwrap()
                .0[0]
                .clone();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
        }

        repository.fetch();

        {
            controller.update_base_branch(*project_id).await.unwrap();

            // removes integrated commit, leaves non commited work as is

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(!branches[0].active);
            assert!(branches[0].commits.is_empty());
            assert!(!branches[0].files.is_empty());
        }

        {
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert!(branches[0].active);
            assert!(branches[0].conflicted);
            assert!(branches[0].base_current);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(branches[0].files[0].hunks.len(), 1);
            assert_eq!(
                branches[0].files[0].hunks[0].diff,
                "@@ -4,7 +4,11 @@\n 4\n 5\n 6\n-7\n+<<<<<<< ours\n+77\n+=======\n+17\n+>>>>>>> theirs\n 8\n 19\n 10\n"
            );
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
        } = &Test::default();

        projects
            .update(&projects::UpdateRequest {
                id: *project_id,
                ok_with_force_push: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "first").unwrap();

            controller
                .create_commit(*project_id, branch_id, "first", None, false)
                .await
                .unwrap();

            branch_id
        };

        controller
            .push_virtual_branch(*project_id, branch_id, false, None)
            .await
            .unwrap();

        // another non-locked hunk
        fs::write(repository.path().join("file.txt"), "first\nsecond").unwrap();

        {
            // push and merge branch remotely
            let branch = controller
                .list_virtual_branches(*project_id)
                .await
                .unwrap()
                .0[0]
                .clone();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
        }

        repository.fetch();

        {
            controller.update_base_branch(*project_id).await.unwrap();

            // removes integrated commit, leaves non commited work as is

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
            assert!(branches[0].commits.is_empty());
            assert!(branches[0].upstream.is_none());
            assert_eq!(branches[0].files.len(), 1);
        }

        {
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = {
            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "first").unwrap();

            controller
                .create_commit(*project_id, branch_id, "first", None, false)
                .await
                .unwrap();

            branch_id
        };

        controller
            .push_virtual_branch(*project_id, branch_id, false, None)
            .await
            .unwrap();

        // another non-locked hunk
        fs::write(repository.path().join("another_file.txt"), "first").unwrap();

        {
            // push and merge branch remotely
            let branch = controller
                .list_virtual_branches(*project_id)
                .await
                .unwrap()
                .0[0]
                .clone();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
        }

        repository.fetch();

        {
            controller.update_base_branch(*project_id).await.unwrap();

            // removes integrated commit, leaves non commited work as is

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
            assert!(branches[0].commits.is_empty());
            assert!(branches[0].upstream.is_none());
            assert!(!branches[0].files.is_empty());
        }

        {
            controller
                .apply_virtual_branch(*project_id, branch_id)
                .await
                .unwrap();

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        {
            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            let branch_id = controller
                .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
                .await
                .unwrap();

            fs::write(repository.path().join("file.txt"), "second").unwrap();

            controller
                .create_commit(*project_id, branch_id, "second", None, false)
                .await
                .unwrap();
        };

        {
            // fetch remote
            controller.update_base_branch(*project_id).await.unwrap();

            // just removes integrated branch

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

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
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        {
            // open pr
            fs::write(repository.path().join("file2.txt"), "new file").unwrap();
            controller
                .create_commit(*project_id, branch_id, "second", None, false)
                .await
                .unwrap();
            controller
                .push_virtual_branch(*project_id, branch_id, false, None)
                .await
                .unwrap();
        }

        {
            // merge pr
            let branch = controller
                .list_virtual_branches(*project_id)
                .await
                .unwrap()
                .0[0]
                .clone();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
            repository.fetch();
        }

        {
            // fetch remote
            controller.update_base_branch(*project_id).await.unwrap();

            // just removes integrated branch
            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
            assert_eq!(branches.len(), 0);
        }
    }

    // Ensure integrated branch is dropped and that a commit on another
    // branch does not lead to the introduction of gost/phantom diffs.
    #[tokio::test]
    async fn should_not_get_confused_by_commits_in_other_branches() {
        let Test {
            repository,
            project_id,
            controller,
            ..
        } = &Test::default();

        fs::write(repository.path().join("file-1.txt"), "one").unwrap();
        let first_commit_oid = repository.commit_all("first");
        fs::write(repository.path().join("file-2.txt"), "two").unwrap();
        repository.commit_all("second");
        repository.push();
        repository.reset_hard(Some(first_commit_oid));

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_1_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        fs::write(repository.path().join("file-3.txt"), "three").unwrap();
        controller
            .create_commit(*project_id, branch_1_id, "third", None, false)
            .await
            .unwrap();

        let branch_2_id = controller
            .create_virtual_branch(
                *project_id,
                &branch::BranchCreateRequest {
                    selected_for_changes: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        fs::write(repository.path().join("file-4.txt"), "four").unwrap();

        controller
            .create_commit(*project_id, branch_2_id, "fourth", None, false)
            .await
            .unwrap();

        controller
            .push_virtual_branch(*project_id, branch_2_id, false, None)
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(*project_id)
            .await
            .unwrap()
            .0[1]
            .clone();

        repository.merge(&branch.upstream.as_ref().unwrap().name);
        repository.fetch();

        // TODO(mg): Figure out why test fails without listing first.
        controller.list_virtual_branches(*project_id).await.unwrap();
        controller.update_base_branch(*project_id).await.unwrap();

        // Verify we have only the first branch left, and that no files
        // are present.
        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].files.len(), 0);
    }
}
