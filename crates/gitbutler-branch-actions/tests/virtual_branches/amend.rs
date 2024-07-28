use gitbutler_branch::BranchOwnershipClaims;
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};

use super::*;

#[test]
fn forcepush_allowed() {
    let Test {
        repository,
        project_id,
        project,
        controller,
        projects,
        ..
    } = &Test::default();

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
        .unwrap();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_id = controller
        .create_commit(project, branch_id, "commit one", None, false)
        .unwrap();

    controller
        .push_virtual_branch(project, branch_id, false, None)
        .unwrap();

    {
        // amend another hunk
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        controller
            .amend(project, branch_id, commit_id, &to_amend)
            .unwrap();

        let branch = controller
            .list_virtual_branches(project)
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

#[test]
fn forcepush_forbidden() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    controller
        .update_virtual_branch(
            project,
            BranchUpdateRequest {
                id: branch_id,
                allow_rebasing: Some(false),
                ..Default::default()
            },
        )
        .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid = controller
        .create_commit(project, branch_id, "commit one", None, false)
        .unwrap();

    controller
        .push_virtual_branch(project, branch_id, false, None)
        .unwrap();

    {
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        assert_eq!(
            controller
                .amend(project, branch_id, commit_oid, &to_amend)
                .unwrap_err()
                .to_string(),
            "force-push is not allowed"
        );
    }
}

#[test]
fn non_locked_hunk() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid = controller
        .create_commit(project, branch_id, "commit one", None, false)
        .unwrap();

    let branch = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits[0].files.len(), 1);

    {
        // amend another hunk
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        controller
            .amend(project, branch_id, commit_oid, &to_amend)
            .unwrap();

        let branch = controller
            .list_virtual_branches(project)
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

#[test]
fn locked_hunk() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid = controller
        .create_commit(project, branch_id, "commit one", None, false)
        .unwrap();

    let branch = controller
        .list_virtual_branches(project)
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

    {
        // amend another hunk
        fs::write(repository.path().join("file.txt"), "more content").unwrap();
        let to_amend: BranchOwnershipClaims = "file.txt:1-2".parse().unwrap();
        controller
            .amend(project, branch_id, commit_oid, &to_amend)
            .unwrap();

        let branch = controller
            .list_virtual_branches(project)
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

#[test]
fn non_existing_ownership() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid = controller
        .create_commit(project, branch_id, "commit one", None, false)
        .unwrap();

    let branch = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits[0].files.len(), 1);

    {
        // amend non existing hunk
        let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        assert_eq!(
            controller
                .amend(project, branch_id, commit_oid, &to_amend)
                .unwrap_err()
                .to_string(),
            "target ownership not found"
        );
    }
}
