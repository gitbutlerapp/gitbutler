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
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one = {
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit", None, false)
                .await
                .unwrap()
        };

        let commit_two = {
            fs::write(repository.path().join("file.txt"), "content two").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit", None, false)
                .await
                .unwrap()
        };

        controller
            .push_virtual_branch(*project_id, branch_id, false, None)
            .await
            .unwrap();

        controller
            .reset_virtual_branch(*project_id, branch_id, commit_one)
            .await
            .unwrap();

        repository.reset_hard(None);

        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );

        let cherry_picked_commit_oid = controller
            .cherry_pick(*project_id, branch_id, commit_two)
            .await
            .unwrap();
        assert!(cherry_picked_commit_oid.is_some());
        assert!(repository.path().join("file.txt").exists());
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content two"
        );

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one = {
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit", None, false)
                .await
                .unwrap()
        };

        let commit_two = {
            fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit", None, false)
                .await
                .unwrap()
        };

        controller
            .push_virtual_branch(*project_id, branch_id, false, None)
            .await
            .unwrap();

        controller
            .reset_virtual_branch(*project_id, branch_id, commit_one)
            .await
            .unwrap();

        repository.reset_hard(None);

        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "content"
        );
        assert!(!repository.path().join("file_two.txt").exists());

        let branch_two_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let cherry_picked_commit_oid = controller
            .cherry_pick(*project_id, branch_two_id, commit_two)
            .await
            .unwrap();
        assert!(cherry_picked_commit_oid.is_some());

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one_oid = {
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit", None, false)
                .await
                .unwrap()
        };

        let commit_three_oid = {
            fs::write(repository.path().join("file_three.txt"), "content three").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit", None, false)
                .await
                .unwrap()
        };

        controller
            .reset_virtual_branch(*project_id, branch_id, commit_one_oid)
            .await
            .unwrap();

        controller
            .convert_to_real_branch(*project_id, branch_id, Default::default())
            .await
            .unwrap();

        assert_eq!(
            controller
                .cherry_pick(*project_id, branch_id, commit_three_oid)
                .await
                .unwrap_err()
                .to_string(),
            "can not cherry pick a branch that is not applied"
        );
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
        } = &Test::default();

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        let commit_one = {
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit one", None, false)
                .await
                .unwrap()
        };

        {
            fs::write(repository.path().join("file_two.txt"), "content two").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit two", None, false)
                .await
                .unwrap()
        };

        let commit_three = {
            fs::write(repository.path().join("file_three.txt"), "content three").unwrap();
            controller
                .create_commit(*project_id, branch_id, "commit three", None, false)
                .await
                .unwrap()
        };

        controller
            .push_virtual_branch(*project_id, branch_id, false, None)
            .await
            .unwrap();

        controller
            .reset_virtual_branch(*project_id, branch_id, commit_one)
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
                .cherry_pick(*project_id, branch_id, commit_three)
                .await
                .unwrap();
            assert!(cherry_picked_commit_oid.is_none());

            assert_eq!(
                fs::read_to_string(repository.path().join("file_three.txt")).unwrap(),
                "<<<<<<< ours\nconflict\n=======\ncontent three\n>>>>>>> theirs\n"
            );

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
                .create_commit(*project_id, branch_id, "resolution", None, false)
                .await
                .unwrap();

            let commit = repository.find_commit(commited_oid).unwrap();
            assert_eq!(commit.parent_count(), 2);

            let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
        } = &Test::default();

        let commit_oid = {
            let first = repository.commit_all("commit");
            fs::write(repository.path().join("file.txt"), "content").unwrap();
            let second = repository.commit_all("commit");
            repository.push();
            repository.reset_hard(Some(first));
            second
        };

        controller
            .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
            .await
            .unwrap();

        let branch_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();

        // introduce conflict with the remote commit
        fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        controller
            .convert_to_real_branch(*project_id, branch_id, Default::default())
            .await
            .unwrap();

        assert_eq!(
            controller
                .cherry_pick(*project_id, branch_id, commit_oid)
                .await
                .unwrap_err()
                .to_string(),
            "can not cherry pick a branch that is not applied"
        );
    }
}
