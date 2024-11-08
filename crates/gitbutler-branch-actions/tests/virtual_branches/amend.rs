use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::list_commit_files;
use gitbutler_stack::BranchOwnershipClaims;

use super::*;

#[test]
fn forcepush_allowed() -> anyhow::Result<()> {
    let Test {
        repository,
        project_id,
        project,
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

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
        .unwrap();

    let branch_id =
        gitbutler_branch_actions::create_branch_stack(project, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();

    gitbutler_branch_actions::push_branch_stack(project, branch_id, false, None).unwrap();

    {
        // amend another hunk
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        gitbutler_branch_actions::amend(project, branch_id, commit_id, &to_amend).unwrap();

        let branch = gitbutler_branch_actions::list_branch_stacks(project)
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();
        assert!(branch.requires_force);
        assert_eq!(branch.commits.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(list_commit_files(project, branch.commits[0].id)?.len(), 2);
    }
    Ok(())
}

#[test]
fn forcepush_forbidden() {
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

    let branch_id =
        gitbutler_branch_actions::create_branch_stack(project, &BranchCreateRequest::default())
            .unwrap();

    gitbutler_branch_actions::update_branch_stack(
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
    let commit_oid =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();

    gitbutler_branch_actions::push_branch_stack(project, branch_id, false, None).unwrap();

    {
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        assert_eq!(
            gitbutler_branch_actions::amend(project, branch_id, commit_oid, &to_amend)
                .unwrap_err()
                .to_string(),
            "force-push is not allowed"
        );
    }
}

#[test]
fn non_locked_hunk() -> anyhow::Result<()> {
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

    let branch_id =
        gitbutler_branch_actions::create_branch_stack(project, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();

    let branch = gitbutler_branch_actions::list_branch_stacks(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.files.len(), 0);

    {
        // amend another hunk
        fs::write(repository.path().join("file2.txt"), "content2").unwrap();
        let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        gitbutler_branch_actions::amend(project, branch_id, commit_oid, &to_amend).unwrap();

        let branch = gitbutler_branch_actions::list_branch_stacks(project)
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();
        assert_eq!(branch.commits.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(list_commit_files(project, branch.commits[0].id)?.len(), 2);
    }
    Ok(())
}

#[test]
fn locked_hunk() -> anyhow::Result<()> {
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

    let branch_id =
        gitbutler_branch_actions::create_branch_stack(project, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();

    let branch = gitbutler_branch_actions::list_branch_stacks(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.files.len(), 0);
    assert_eq!(
        list_commit_files(project, branch.commits[0].id)?[0].hunks[0].diff_lines,
        "@@ -0,0 +1 @@\n+content\n\\ No newline at end of file\n"
    );

    {
        // amend another hunk
        fs::write(repository.path().join("file.txt"), "more content").unwrap();
        let to_amend: BranchOwnershipClaims = "file.txt:1-2".parse().unwrap();
        gitbutler_branch_actions::amend(project, branch_id, commit_oid, &to_amend).unwrap();

        let branch = gitbutler_branch_actions::list_branch_stacks(project)
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();

        assert_eq!(branch.commits.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(
            list_commit_files(project, branch.commits[0].id)?[0].hunks[0].diff_lines,
            "@@ -0,0 +1 @@\n+more content\n\\ No newline at end of file\n"
        );
    }
    Ok(())
}

#[test]
fn non_existing_ownership() {
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

    let branch_id =
        gitbutler_branch_actions::create_branch_stack(project, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repository.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None, false)
            .unwrap();

    let branch = gitbutler_branch_actions::list_branch_stacks(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.files.len(), 0);

    {
        // amend non existing hunk
        let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        assert_eq!(
            gitbutler_branch_actions::amend(project, branch_id, commit_oid, &to_amend)
                .unwrap_err()
                .to_string(),
            "target ownership not found"
        );
    }
}
