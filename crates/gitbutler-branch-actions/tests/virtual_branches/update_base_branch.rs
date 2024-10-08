use super::*;

mod applied_branch {
    use gitbutler_stack::BranchCreateRequest;

    use super::*;

    #[test]
    fn conflicts_with_uncommitted_work() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        {
            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
        }

        let unapplied_branch = {
            // fetch remote
            let unapplied_branches = gitbutler_branch_actions::update_base_branch(project).unwrap();
            assert_eq!(unapplied_branches.len(), 1);

            // should stash conflicting branch

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 0);

            Refname::from_str(unapplied_branches[0].as_str()).unwrap()
        };

        {
            // applying the branch should produce conflict markers
            gitbutler_branch_actions::create_virtual_branch_from_branch(
                project,
                &unapplied_branch,
                None,
            )
            .unwrap();
            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
            assert!(branches[0].conflicted);
            assert!(branches[0].base_current);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(
                std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
            );
        }
    }

    #[test]
    fn commited_conflict_not_pushed() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        {
            // make a branch with a commit that conflicts with upstream, and work that fixes
            // that conflict
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
            gitbutler_branch_actions::create_commit(
                project,
                branch_id,
                "conflicting commit",
                None,
                false,
            )
            .unwrap();
        }

        let unapplied_branch = {
            // when fetching remote
            let unapplied_branches = gitbutler_branch_actions::update_base_branch(project).unwrap();
            assert_eq!(unapplied_branches.len(), 1);

            // should stash the branch.

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 0);

            Refname::from_str(unapplied_branches[0].as_str()).unwrap()
        };

        {
            // applying the branch should produce conflict markers
            gitbutler_branch_actions::create_virtual_branch_from_branch(
                project,
                &unapplied_branch,
                None,
            )
            .unwrap();
            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
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

    #[test]
    fn commited_conflict_pushed() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        {
            // make a branch with a commit that conflicts with upstream, and work that fixes
            // that conflict
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
            gitbutler_branch_actions::create_commit(
                project,
                branch_id,
                "conflicting commit",
                None,
                false,
            )
            .unwrap();

            gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();
        }

        let unapplied_branch = {
            // when fetching remote
            let unapplied_branches = gitbutler_branch_actions::update_base_branch(project).unwrap();
            assert_eq!(unapplied_branches.len(), 1);

            // should stash the branch.

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 0);

            Refname::from_str(unapplied_branches[0].as_str()).unwrap()
        };

        {
            // applying the branch should produce conflict markers
            gitbutler_branch_actions::create_virtual_branch_from_branch(
                project,
                &unapplied_branch,
                None,
            )
            .unwrap();
            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
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

    #[test]
    fn commited_conflict_not_pushed_fixed_with_more_work() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        {
            // make a branch with a commit that conflicts with upstream, and work that fixes
            // that conflict
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
            gitbutler_branch_actions::create_commit(
                project,
                branch_id,
                "conflicting commit",
                None,
                false,
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();
        }

        let unapplied_branch = {
            // when fetching remote
            let unapplied_branches = gitbutler_branch_actions::update_base_branch(project).unwrap();
            assert_eq!(unapplied_branches.len(), 1);

            // should rebase upstream, and leave uncommited file as is

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 0);

            Refname::from_str(unapplied_branches[0].as_str()).unwrap()
        };

        {
            // applying the branch should produce conflict markers
            gitbutler_branch_actions::create_virtual_branch_from_branch(
                project,
                &unapplied_branch,
                None,
            )
            .unwrap();
            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
            assert!(branches[0].conflicted);
            assert!(branches[0].base_current);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(branches[0].commits.len(), 2);
            assert_eq!(
                std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "<<<<<<< ours\nfix conflict\n=======\nsecond\n>>>>>>> theirs\n"
            );
        }
    }

    #[test]
    fn commited_conflict_pushed_fixed_with_more_work() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        {
            // make a branch with a commit that conflicts with upstream, and work that fixes
            // that conflict
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "conflict").unwrap();
            gitbutler_branch_actions::create_commit(
                project,
                branch_id,
                "conflicting commit",
                None,
                false,
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "fix conflict").unwrap();
        }

        let unapplied_branch = {
            // when fetching remote
            let unapplied_branches = gitbutler_branch_actions::update_base_branch(project).unwrap();
            assert_eq!(unapplied_branches.len(), 1);

            // should merge upstream, and leave uncommited file as is.

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 0);

            Refname::from_str(unapplied_branches[0].as_str()).unwrap()
        };

        {
            // applying the branch should produce conflict markers
            gitbutler_branch_actions::create_virtual_branch_from_branch(
                project,
                &unapplied_branch,
                None,
            )
            .unwrap();
            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
            assert!(branches[0].conflicted);
            assert!(branches[0].base_current);
            assert_eq!(branches[0].files.len(), 1);
            assert_eq!(branches[0].commits.len(), 2);
            assert_eq!(
                std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "<<<<<<< ours\nfix conflict\n=======\nsecond\n>>>>>>> theirs\n"
            );
        }
    }

    mod no_conflicts_pushed {
        use gitbutler_stack::BranchUpdateRequest;

        use super::*;

        #[test]
        fn force_push_ok() {
            let Test {
                repository,
                project,
                project_id,
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
                    ..Default::default()
                })
                .unwrap();

            gitbutler_branch_actions::set_base_branch(
                project,
                &"refs/remotes/origin/master".parse().unwrap(),
            )
            .unwrap();

            let branch_id = {
                let branch_id = gitbutler_branch_actions::create_virtual_branch(
                    project,
                    &BranchCreateRequest::default(),
                )
                .unwrap();

                fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

                gitbutler_branch_actions::create_commit(
                    project,
                    branch_id,
                    "no conflicts",
                    None,
                    false,
                )
                .unwrap();
                gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None)
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                branch_id
            };

            {
                // fetch remote
                gitbutler_branch_actions::update_base_branch(project).unwrap();

                // rebases branch, since the branch is pushed and force pushing is
                // allowed

                let (branches, _) =
                    gitbutler_branch_actions::list_virtual_branches(project).unwrap();
                assert_eq!(branches.len(), 1);
                assert_eq!(branches[0].id, branch_id);
                assert!(branches[0].active);
                assert!(branches[0].requires_force);
                assert!(branches[0].base_current);
                assert_eq!(branches[0].files.len(), 1);
                assert_eq!(branches[0].commits.len(), 1);
                assert!(!branches[0].commits[0].is_remote);
                assert!(
                    branches[0].commits[0].copied_from_remote_id.is_some(),
                    "it's copied, which displays things differently in the \
                     UI which knows what remote commit it relates to"
                );
                assert!(!branches[0].commits[0].is_integrated);
            }
        }

        #[test]
        fn force_push_not_ok() {
            let Test {
                repository,
                project,
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

            gitbutler_branch_actions::set_base_branch(
                project,
                &"refs/remotes/origin/master".parse().unwrap(),
            )
            .unwrap();

            let branch_id = {
                let branch_id = gitbutler_branch_actions::create_virtual_branch(
                    project,
                    &BranchCreateRequest::default(),
                )
                .unwrap();

                fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

                gitbutler_branch_actions::create_commit(
                    project,
                    branch_id,
                    "no conflicts",
                    None,
                    false,
                )
                .unwrap();
                gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None)
                    .unwrap();

                fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

                branch_id
            };

            gitbutler_branch_actions::update_virtual_branch(
                project,
                BranchUpdateRequest {
                    id: branch_id,
                    allow_rebasing: Some(false),
                    ..Default::default()
                },
            )
            .unwrap();

            {
                // fetch remote
                gitbutler_branch_actions::update_base_branch(project).unwrap();

                // creates a merge commit, since the branch is pushed

                let (branches, _) =
                    gitbutler_branch_actions::list_virtual_branches(project).unwrap();
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
            }
        }
    }

    #[test]
    fn no_conflicts() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch_id = {
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file2.txt"), "no conflict").unwrap();

            gitbutler_branch_actions::create_commit(
                project,
                branch_id,
                "no conflicts",
                None,
                false,
            )
            .unwrap();

            fs::write(repository.path().join("file2.txt"), "still no conflict").unwrap();

            branch_id
        };

        {
            // fetch remote
            gitbutler_branch_actions::update_base_branch(project).unwrap();

            // just rebases branch

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
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

    #[test]
    fn integrated_commit_plus_work() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        {
            fs::write(repository.path().join("file.txt"), "first").unwrap();
            repository.commit_all("first");
            repository.push();
        }

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch_id = {
            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "second").unwrap();

            gitbutler_branch_actions::create_commit(project, branch_id, "second", None, false)
                .unwrap();
            gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

            {
                // merge branch upstream
                let branch = gitbutler_branch_actions::list_virtual_branches(project)
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
            gitbutler_branch_actions::update_base_branch(project).unwrap();

            // should remove integrated commit, but leave non integrated work as is

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
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

    #[test]
    fn integrated_with_locked_conflicting_hunks() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        // branch has no conflict
        let branch_id = {
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(
                repository.path().join("file.txt"),
                "1\n2\n3\n4\n5\n6\n7\n8\n19\n10\n11\n12\n",
            )
            .unwrap();

            gitbutler_branch_actions::create_commit(project, branch_id, "fourth", None, false)
                .unwrap();

            branch_id
        };

        // push the branch
        gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

        // another locked conflicting hunk
        fs::write(
            repository.path().join("file.txt"),
            "1\n2\n3\n4\n5\n6\n77\n8\n19\n10\n11\n12\n",
        )
        .unwrap();

        {
            // merge branch remotely
            let branch = gitbutler_branch_actions::list_virtual_branches(project)
                .unwrap()
                .0[0]
                .clone();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
        }

        repository.fetch();

        let unapplied_refname = {
            let unapplied_refnames = gitbutler_branch_actions::update_base_branch(project).unwrap();
            assert_eq!(unapplied_refnames.len(), 1);

            // removes integrated commit, leaves non commited work as is

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 0);
            assert_eq!(
                fs::read_to_string(repository.path().join("file.txt")).unwrap(),
                "1\n2\n3\n4\n5\n6\n17\n8\n19\n10\n11\n12\n"
            );

            unapplied_refnames[0].clone()
        };

        {
            gitbutler_branch_actions::create_virtual_branch_from_branch(
                project,
                &Refname::from_str(unapplied_refname.as_str()).unwrap(),
                None,
            )
            .unwrap();

            let vb_state = VirtualBranchesHandle::new(project.gb_dir());
            let ctx = CommandContext::open(project).unwrap();
            update_workspace_commit(&vb_state, &ctx).unwrap();
            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
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
        }
    }

    #[test]
    fn integrated_with_locked_hunks() {
        let Test {
            repository,
            project,
            project_id,
            projects,
            ..
        } = &Test::default();

        projects
            .update(&projects::UpdateRequest {
                id: *project_id,
                ..Default::default()
            })
            .unwrap();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch_id = {
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "first").unwrap();

            gitbutler_branch_actions::create_commit(project, branch_id, "first", None, false)
                .unwrap();

            branch_id
        };

        gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

        // another non-locked hunk
        fs::write(repository.path().join("file.txt"), "first\nsecond").unwrap();

        {
            // push and merge branch remotely
            let branch = gitbutler_branch_actions::list_virtual_branches(project)
                .unwrap()
                .0[0]
                .clone();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
        }

        repository.fetch();

        {
            gitbutler_branch_actions::update_base_branch(project).unwrap();

            // removes integrated commit, leaves non commited work as is

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
            assert!(branches[0].commits.is_empty());
            assert!(branches[0].upstream.is_none());
            assert_eq!(branches[0].files.len(), 1);
        }
    }

    #[test]
    fn integrated_with_non_locked_hunks() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch_id = {
            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "first").unwrap();

            gitbutler_branch_actions::create_commit(project, branch_id, "first", None, false)
                .unwrap();

            branch_id
        };

        gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

        // another non-locked hunk
        fs::write(repository.path().join("another_file.txt"), "first").unwrap();

        {
            // push and merge branch remotely
            let branch = gitbutler_branch_actions::list_virtual_branches(project)
                .unwrap()
                .0[0]
                .clone();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
        }

        repository.fetch();

        {
            gitbutler_branch_actions::update_base_branch(project).unwrap();

            // removes integrated commit, leaves non commited work as is

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 1);
            assert_eq!(branches[0].id, branch_id);
            assert!(branches[0].active);
            assert!(branches[0].commits.is_empty());
            assert!(branches[0].upstream.is_none());
            assert_eq!(branches[0].files.len(), 1);
        }
    }

    #[test]
    fn all_integrated() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        {
            // make a branch that conflicts with the remote branch, but doesn't know about it yet
            let branch_id = gitbutler_branch_actions::create_virtual_branch(
                project,
                &BranchCreateRequest::default(),
            )
            .unwrap();

            fs::write(repository.path().join("file.txt"), "second").unwrap();

            gitbutler_branch_actions::create_commit(project, branch_id, "second", None, false)
                .unwrap();
        };

        {
            // fetch remote
            gitbutler_branch_actions::update_base_branch(project).unwrap();

            // just removes integrated branch

            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 0);
        }
    }

    #[test]
    fn integrate_work_while_being_behind() {
        let Test {
            repository,
            project,
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

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest::default(),
        )
        .unwrap();

        {
            // open pr
            fs::write(repository.path().join("file2.txt"), "new file").unwrap();
            gitbutler_branch_actions::create_commit(project, branch_id, "second", None, false)
                .unwrap();
            gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();
        }

        {
            // merge pr
            let branch = gitbutler_branch_actions::list_virtual_branches(project)
                .unwrap()
                .0[0]
                .clone();
            repository.merge(&branch.upstream.as_ref().unwrap().name);
            repository.fetch();
        }

        {
            // fetch remote
            gitbutler_branch_actions::update_base_branch(project).unwrap();

            // just removes integrated branch
            let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
            assert_eq!(branches.len(), 0);
        }
    }

    // Ensure integrated branch is dropped and that a commit on another
    // branch does not lead to the introduction of gost/phantom diffs.
    #[test]
    fn should_not_get_confused_by_commits_in_other_branches() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        fs::write(repository.path().join("file-1.txt"), "one").unwrap();
        let first_commit_oid = repository.commit_all("first");
        fs::write(repository.path().join("file-2.txt"), "two").unwrap();
        repository.commit_all("second");
        repository.push();
        repository.reset_hard(Some(first_commit_oid));

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let branch_1_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest::default(),
        )
        .unwrap();

        fs::write(repository.path().join("file-3.txt"), "three").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_1_id, "third", None, false)
            .unwrap();

        let branch_2_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest {
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        fs::write(repository.path().join("file-4.txt"), "four").unwrap();

        gitbutler_branch_actions::create_commit(project, branch_2_id, "fourth", None, false)
            .unwrap();

        gitbutler_branch_actions::push_virtual_branch(project, branch_2_id, false, None).unwrap();

        let branch = gitbutler_branch_actions::list_virtual_branches(project)
            .unwrap()
            .0[1]
            .clone();

        repository.merge(&branch.upstream.as_ref().unwrap().name);
        repository.fetch();

        // TODO(mg): Figure out why test fails without listing first.
        gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        gitbutler_branch_actions::update_base_branch(project).unwrap();

        // Verify we have only the first branch left, and that no files
        // are present.
        let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].files.len(), 0);
    }
}
