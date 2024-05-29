use super::*;

#[tokio::test]
async fn detect_upstream_commits() {
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

    let branch1_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let oid1 = {
        // create first commit
        fs::write(repository.path().join("file.txt"), "content").unwrap();
        controller
            .create_commit(*project_id, branch1_id, "commit", None, false)
            .await
            .unwrap()
    };

    let oid2 = {
        // create second commit
        fs::write(repository.path().join("file.txt"), "content2").unwrap();
        controller
            .create_commit(*project_id, branch1_id, "commit", None, false)
            .await
            .unwrap()
    };

    // push
    controller
        .push_virtual_branch(*project_id, branch1_id, false, None)
        .await
        .unwrap();

    let oid3 = {
        // create third commit
        fs::write(repository.path().join("file.txt"), "content3").unwrap();
        controller
            .create_commit(*project_id, branch1_id, "commit", None, false)
            .await
            .unwrap()
    };

    {
        // should correctly detect pushed commits
        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch1_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let oid1 = {
        // create first commit
        fs::write(repository.path().join("file.txt"), "content").unwrap();
        controller
            .create_commit(*project_id, branch1_id, "commit", None, false)
            .await
            .unwrap()
    };

    let oid2 = {
        // create second commit
        fs::write(repository.path().join("file.txt"), "content2").unwrap();
        controller
            .create_commit(*project_id, branch1_id, "commit", None, false)
            .await
            .unwrap()
    };

    // push
    controller
        .push_virtual_branch(*project_id, branch1_id, false, None)
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
            .find(|b| b.id == branch1_id)
            .unwrap();
        repository.merge(&branch.upstream.as_ref().unwrap().name);
        repository.fetch();
    }

    let oid3 = {
        // create third commit
        fs::write(repository.path().join("file.txt"), "content3").unwrap();
        controller
            .create_commit(*project_id, branch1_id, "commit", None, false)
            .await
            .unwrap()
    };

    {
        // should correctly detect pushed commits
        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
