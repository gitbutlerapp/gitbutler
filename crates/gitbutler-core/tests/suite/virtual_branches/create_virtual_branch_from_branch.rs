use super::*;

#[tokio::test]
async fn integration() {
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

    let branch_name = {
        // make a remote branch

        let branch_id = controller
            .create_virtual_branch(*project_id, &super::branch::BranchCreateRequest::default())
            .await
            .unwrap();

        std::fs::write(repository.path().join("file.txt"), "first\n").unwrap();
        controller
            .create_commit(*project_id, branch_id, "first", None, false)
            .await
            .unwrap();
        controller
            .push_virtual_branch(*project_id, branch_id, false, None)
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(*project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|branch| branch.id == branch_id)
            .unwrap();

        let name = branch.upstream.unwrap().name;

        controller
            .delete_virtual_branch(*project_id, branch_id)
            .await
            .unwrap();

        name
    };

    // checkout a existing remote branch
    let branch_id = controller
        .create_virtual_branch_from_branch(*project_id, &branch_name)
        .await
        .unwrap();

    {
        // add a commit
        std::fs::write(repository.path().join("file.txt"), "first\nsecond").unwrap();

        controller
            .create_commit(*project_id, branch_id, "second", None, false)
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
            .push_virtual_branch(*project_id, branch_id, false, None)
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(*project_id)
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
        controller
            .fetch_from_remotes(*project_id, None)
            .await
            .unwrap();

        let branch = controller
            .list_virtual_branches(*project_id)
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
    } = &Test::default();

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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert!(branches.is_empty());

    let branch_id = controller
        .create_virtual_branch_from_branch(
            *project_id,
            &"refs/remotes/origin/branch".parse().unwrap(),
        )
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
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
    } = &Test::default();

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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    // create a local branch that conflicts with remote
    {
        std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
    };

    // branch should be created unapplied, because of the conflict

    let new_branch_id = controller
        .create_virtual_branch_from_branch(
            *project_id,
            &"refs/remotes/origin/branch".parse().unwrap(),
        )
        .await
        .unwrap();
    let new_branch = controller
        .list_virtual_branches(*project_id)
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
    } = &Test::default();

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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    // create a local branch that conflicts with remote
    {
        std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
        assert_eq!(branches.len(), 1);

        controller
            .create_commit(*project_id, branches[0].id, "hej", None, false)
            .await
            .unwrap();
    };

    // branch should be created unapplied, because of the conflict

    let new_branch_id = controller
        .create_virtual_branch_from_branch(
            *project_id,
            &"refs/remotes/origin/branch".parse().unwrap(),
        )
        .await
        .unwrap();
    let new_branch = controller
        .list_virtual_branches(*project_id)
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
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    // branch should be created unapplied, because of the conflict

    assert_eq!(
        controller
            .create_virtual_branch_from_branch(
                *project_id,
                &"refs/remotes/origin/master".parse().unwrap(),
            )
            .await
            .unwrap_err()
            .to_string(),
        "cannot create a branch from default target"
    );
}

#[tokio::test]
async fn from_non_existent_branch() {
    let Test {
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    // branch should be created unapplied, because of the conflict

    assert_eq!(
        controller
            .create_virtual_branch_from_branch(
                *project_id,
                &"refs/remotes/origin/branch".parse().unwrap(),
            )
            .await
            .unwrap_err()
            .to_string(),
        "branch refs/remotes/origin/branch was not found"
    );
}

#[tokio::test]
async fn from_state_remote_branch() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

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
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch_from_branch(
            *project_id,
            &"refs/remotes/origin/branch".parse().unwrap(),
        )
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, branch_id);
    assert_eq!(branches[0].commits.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(branches[0].commits[0].description, "branch commit");
}
