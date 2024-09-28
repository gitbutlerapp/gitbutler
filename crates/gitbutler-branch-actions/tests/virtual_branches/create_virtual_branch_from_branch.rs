use gitbutler_branch::BranchCreateRequest;
use gitbutler_reference::LocalRefname;

use super::*;

#[test]
fn integration() {
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

    let branch_name = {
        // make a remote branch

        let branch_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest::default(),
        )
        .unwrap();

        std::fs::write(repository.path().join("file.txt"), "first\n").unwrap();
        gitbutler_branch_actions::create_commit(project, branch_id, "first", None, false).unwrap();
        gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

        let branch = gitbutler_branch_actions::list_virtual_branches(project)
            .unwrap()
            .0
            .into_iter()
            .find(|branch| branch.id == branch_id)
            .unwrap();

        let name = branch.upstream.unwrap().name;

        gitbutler_branch_actions::unapply_without_saving_virtual_branch(project, branch_id)
            .unwrap();

        name
    };

    // checkout a existing remote branch
    let branch_id =
        gitbutler_branch_actions::create_virtual_branch_from_branch(project, &branch_name, None)
            .unwrap();

    {
        // add a commit
        std::fs::write(repository.path().join("file.txt"), "first\nsecond").unwrap();

        gitbutler_branch_actions::create_commit(project, branch_id, "second", None, false).unwrap();
    }

    {
        // meanwhile, there is a new commit on master
        repository.checkout(&"refs/heads/master".parse().unwrap());
        std::fs::write(repository.path().join("another.txt"), "").unwrap();
        repository.commit_all("another");
        repository.push_branch(&"refs/heads/master".parse().unwrap());
        repository.checkout(&"refs/heads/gitbutler/workspace".parse().unwrap());
    }

    {
        // merge branch into master
        gitbutler_branch_actions::push_virtual_branch(project, branch_id, false, None).unwrap();

        let branch = gitbutler_branch_actions::list_virtual_branches(project)
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
        gitbutler_branch_actions::fetch_from_remotes(project, None).unwrap();

        let branch = gitbutler_branch_actions::list_virtual_branches(project)
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

#[test]
fn no_conflicts() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repository.checkout(&branch_name);
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        repository.commit_all("first");
        repository.push_branch(&branch_name);
        repository.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert!(branches.is_empty());

    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
    )
    .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, branch_id);
    assert_eq!(branches[0].commits.len(), 1);
    assert_eq!(branches[0].commits[0].description, "first");
}

#[test]
fn conflicts_with_uncommited() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repository.checkout(&branch_name);
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        repository.commit_all("first");
        repository.push_branch(&branch_name);
        repository.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    // create a local branch that conflicts with remote
    {
        std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 1);
    };

    // branch should be created unapplied, because of the conflict

    let new_branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
    )
    .unwrap();
    let new_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|branch| branch.id == new_branch_id)
        .unwrap();
    assert_eq!(new_branch_id, new_branch.id);
    assert_eq!(new_branch.commits.len(), 1);
    assert!(new_branch.upstream.is_some());
}

#[test]
fn conflicts_with_commited() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
        repository.checkout(&branch_name);
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        repository.commit_all("first");
        repository.push_branch(&branch_name);
        repository.checkout(&"refs/heads/master".parse().unwrap());
    }

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    // create a local branch that conflicts with remote
    {
        std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 1);

        gitbutler_branch_actions::create_commit(project, branches[0].id, "hej", None, false)
            .unwrap();
    };

    // branch should be created unapplied, because of the conflict

    let new_branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
    )
    .unwrap();
    let new_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|branch| branch.id == new_branch_id)
        .unwrap();
    assert_eq!(new_branch_id, new_branch.id);
    assert_eq!(new_branch.commits.len(), 1);
    assert!(new_branch.upstream.is_some());
}

#[test]
fn from_default_target() {
    let Test { project, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    // branch should be created unapplied, because of the conflict

    assert_eq!(
        gitbutler_branch_actions::create_virtual_branch_from_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
            None
        )
        .unwrap_err()
        .to_string(),
        "cannot create a branch from default target"
    );
}

#[test]
fn from_non_existent_branch() {
    let Test { project, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    // branch should be created unapplied, because of the conflict

    assert_eq!(
        gitbutler_branch_actions::create_virtual_branch_from_branch(
            project,
            &"refs/remotes/origin/branch".parse().unwrap(),
            None
        )
        .unwrap_err()
        .to_string(),
        "branch refs/remotes/origin/branch was not found"
    );
}

#[test]
fn from_state_remote_branch() {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();

    {
        // create a remote branch
        let branch_name: LocalRefname = "refs/heads/branch".parse().unwrap();
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

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
        project,
        &"refs/remotes/origin/branch".parse().unwrap(),
        None,
    )
    .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, branch_id);
    assert_eq!(branches[0].commits.len(), 1);
    assert!(branches[0].files.is_empty());
    assert_eq!(branches[0].commits[0].description, "branch commit");
}
