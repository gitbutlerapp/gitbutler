use gitbutler_branch::{BranchCreateRequest, BranchId};

use super::Test;

#[test]
fn no_diffs() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid =
        gitbutler_branch_actions::create_commit(project, source_branch_id, "commit", None, false)
            .unwrap();

    let target_branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid).unwrap();

    let destination_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = gitbutler_branch_actions::list_virtual_branches(project)
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

#[test]
fn diffs_on_source_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid =
        gitbutler_branch_actions::create_commit(project, source_branch_id, "commit", None, false)
            .unwrap();

    std::fs::write(
        repository.path().join("another file.txt"),
        "another content",
    )
    .unwrap();

    let target_branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid).unwrap();

    let destination_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = gitbutler_branch_actions::list_virtual_branches(project)
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

#[test]
fn diffs_on_target_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid =
        gitbutler_branch_actions::create_commit(project, source_branch_id, "commit", None, false)
            .unwrap();

    let target_branch_id = gitbutler_branch_actions::create_virtual_branch(
        project,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    std::fs::write(
        repository.path().join("another file.txt"),
        "another content",
    )
    .unwrap();

    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid).unwrap();

    let destination_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    let source_branch = gitbutler_branch_actions::list_virtual_branches(project)
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

#[test]
fn locked_hunks_on_source_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid =
        gitbutler_branch_actions::create_commit(project, source_branch_id, "commit", None, false)
            .unwrap();

    std::fs::write(repository.path().join("file.txt"), "locked content").unwrap();

    let target_branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    assert_eq!(
        gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid)
            .unwrap_err()
            .to_string(),
        "the source branch contains hunks locked to the target commit"
    );
}

#[test]
fn no_commit() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    gitbutler_branch_actions::create_commit(project, source_branch_id, "commit", None, false)
        .unwrap();

    let target_branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    let commit_id_hex = "a99c95cca7a60f1a2180c2f86fb18af97333c192";
    assert_eq!(
        gitbutler_branch_actions::move_commit(
            project,
            target_branch_id,
            git2::Oid::from_str(commit_id_hex).unwrap()
        )
        .unwrap_err()
        .to_string(),
        format!("commit {commit_id_hex} to be moved could not be found")
    );
}

#[test]
fn no_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    let commit_oid =
        gitbutler_branch_actions::create_commit(project, source_branch_id, "commit", None, false)
            .unwrap();

    let id = BranchId::generate();
    assert_eq!(
        gitbutler_branch_actions::move_commit(project, id, commit_oid)
            .unwrap_err()
            .to_string(),
        format!("branch {id} is not among applied branches")
    );
}
