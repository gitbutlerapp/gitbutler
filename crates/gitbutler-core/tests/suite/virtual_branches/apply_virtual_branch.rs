use super::*;

#[tokio::test]
async fn rebase_commit() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let mut branch1_id = {
        // create a branch with some commited work
        let branch1_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();
        fs::write(repository.path().join("another_file.txt"), "virtual").unwrap();

        controller
            .create_commit(*project_id, branch1_id, "virtual commit", None, false)
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert!(branches[0].active);
        assert_eq!(branches[0].files.len(), 0);
        assert_eq!(branches[0].commits.len(), 1);

        branch1_id
    };

    let unapplied_branch = {
        // unapply first vbranch
        let unapplied_branch = controller
            .convert_to_real_branch(*project_id, branch1_id, Default::default())
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

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 0);

        git::Refname::from_str(&unapplied_branch).unwrap()
    };

    {
        // fetch remote
        controller.update_base_branch(*project_id).await.unwrap();

        // branch is stil unapplied
        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 0);

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
        branch1_id = controller
            .create_virtual_branch_from_branch(*project_id, &unapplied_branch)
            .await
            .unwrap();

        // it should be rebased
        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
    } = &Test::default();

    // make sure we have an undiscovered commit in the remote branch
    {
        let first_commit_oid = repository.commit_all("first");
        fs::write(repository.path().join("file.txt"), "").unwrap();
        repository.commit_all("second");
        repository.push();
        repository.reset_hard(Some(first_commit_oid));
    }

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let mut branch1_id = {
        // make a branch with some work
        let branch1_id = controller
            .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();
        fs::write(repository.path().join("another_file.txt"), "").unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert!(branches[0].active);
        assert_eq!(branches[0].files.len(), 1);
        assert_eq!(branches[0].commits.len(), 0);

        branch1_id
    };

    let unapplied_branch = {
        // unapply first vbranch
        let unapplied_branch = controller
            .convert_to_real_branch(*project_id, branch1_id, Default::default())
            .await
            .unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 0);

        assert!(!repository.path().join("another_file.txt").exists());
        assert!(!repository.path().join("file.txt").exists());

        git::Refname::from_str(&unapplied_branch).unwrap()
    };

    {
        // fetch remote
        controller.update_base_branch(*project_id).await.unwrap();

        // first branch is stil unapplied
        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 0);

        assert!(!repository.path().join("another_file.txt").exists());
        assert!(repository.path().join("file.txt").exists());
    }

    {
        // apply first vbranch again
        branch1_id = controller
            .create_virtual_branch_from_branch(*project_id, &unapplied_branch)
            .await
            .unwrap();

        // workdir should be rebased, and work should be restored
        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
