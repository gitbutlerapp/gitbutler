use bstr::ByteSlice;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_stack::StackId;
use std::{path::PathBuf, str::FromStr};

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

    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid, source_branch_id)
        .unwrap();

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
fn multiple_commits() {
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

    std::fs::write(repository.path().join("a.txt"), "This is a").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    // Create a commit on the source branch
    gitbutler_branch_actions::create_commit(project, source_branch_id, "Add a", None, false)
        .unwrap();

    std::fs::write(repository.path().join("b.txt"), "This is b").unwrap();

    // Create a second commit on the source branch, to be moved
    let commit_oid =
        gitbutler_branch_actions::create_commit(project, source_branch_id, "Add b", None, false)
            .unwrap();

    std::fs::write(repository.path().join("c.txt"), "This is c").unwrap();

    // Create a third commit on the source branch

    gitbutler_branch_actions::create_commit(project, source_branch_id, "Add c", None, false)
        .unwrap();

    let target_branch_id = gitbutler_branch_actions::create_virtual_branch(
        project,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    std::fs::write(repository.path().join("d.txt"), "This is d").unwrap();

    // Create a commit on the destination branch
    gitbutler_branch_actions::create_commit(project, target_branch_id, "Add d", None, false)
        .unwrap();

    // Move the top commit from the source branch to the destination branch
    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid, source_branch_id)
        .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    let source_branch = branches.iter().find(|b| b.id == source_branch_id).unwrap();
    let destination_branch = branches.iter().find(|b| b.id == target_branch_id).unwrap();

    assert_eq!(destination_branch.commits.len(), 2);
    assert_eq!(destination_branch.files.len(), 0);
    assert_eq!(
        destination_branch
            .commits
            .clone()
            .into_iter()
            .map(|c| c.description.to_str_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["Add b", "Add d"]
    );

    assert_eq!(source_branch.commits.len(), 2);
    assert_eq!(source_branch.files.len(), 0);
    assert_eq!(
        source_branch
            .commits
            .clone()
            .into_iter()
            .map(|c| c.description.to_str_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["Add c", "Add a"]
    );
}

#[test]
fn multiple_commits_with_diffs() {
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

    std::fs::write(repository.path().join("a.txt"), "This is a").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    // Create a commit on the source branch
    gitbutler_branch_actions::create_commit(project, source_branch_id, "Add a", None, false)
        .unwrap();

    std::fs::write(repository.path().join("b.txt"), "This is b").unwrap();

    // Create as second commit on the source branch, to be moved
    let commit_oid =
        gitbutler_branch_actions::create_commit(project, source_branch_id, "Add b", None, false)
            .unwrap();

    // Uncommitted changes on the source branch
    std::fs::write(repository.path().join("c.txt"), "This is c").unwrap();

    let source_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == source_branch_id)
        .unwrap();

    // State of source branch after the two commits
    assert_eq!(source_branch.commits.len(), 2);
    assert_eq!(source_branch.files.len(), 1);

    let target_branch_id = gitbutler_branch_actions::create_virtual_branch(
        project,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    std::fs::write(repository.path().join("d.txt"), "This is d").unwrap();

    // Create a commit on the destination branch
    gitbutler_branch_actions::create_commit(project, target_branch_id, "Add d", None, false)
        .unwrap();

    // Uncommitted changes on the destination branch
    std::fs::write(repository.path().join("e.txt"), "This is e").unwrap();

    let destination_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    // State of destination branch before the commit is moved
    assert_eq!(destination_branch.commits.len(), 1);
    assert_eq!(destination_branch.files.len(), 1);

    // Move the top commit from the source branch to the destination branch
    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid, source_branch_id)
        .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    let source_branch = branches.iter().find(|b| b.id == source_branch_id).unwrap();
    let destination_branch = branches.iter().find(|b| b.id == target_branch_id).unwrap();

    assert_eq!(destination_branch.commits.len(), 2);
    assert_eq!(destination_branch.files.len(), 1);
    assert_eq!(
        destination_branch
            .commits
            .clone()
            .into_iter()
            .map(|c| c.description.to_str_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["Add b", "Add d"]
    );
    assert_eq!(
        destination_branch.files[0].path,
        PathBuf::from_str("e.txt").unwrap()
    );
    assert_eq!(destination_branch.files[0].hunks.len(), 1);
    assert_eq!(
        destination_branch.files[0].hunks[0].diff.to_str_lossy(),
        "@@ -0,0 +1 @@\n+This is e\n\\ No newline at end of file\n"
    );

    assert_eq!(source_branch.commits.len(), 1);
    assert_eq!(source_branch.files.len(), 1);
    assert_eq!(source_branch.commits[0].description.to_str_lossy(), "Add a");
    assert_eq!(
        source_branch.files[0].path,
        PathBuf::from_str("c.txt").unwrap()
    );
    assert_eq!(source_branch.files[0].hunks.len(), 1);
    assert_eq!(
        source_branch.files[0].hunks[0].diff.to_str_lossy(),
        "@@ -0,0 +1 @@\n+This is c\n\\ No newline at end of file\n"
    );
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

    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid, source_branch_id)
        .unwrap();

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
    assert_eq!(source_branch.files.len(), 1); //
    assert_eq!(
        source_branch.files[0].path,
        PathBuf::from_str("another file.txt").unwrap()
    );
    assert_eq!(source_branch.files[0].hunks.len(), 1);
    assert_eq!(
        source_branch.files[0].hunks[0].diff.to_str_lossy(),
        "@@ -0,0 +1 @@\n+another content\n\\ No newline at end of file\n"
    );
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

    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid, source_branch_id)
        .unwrap();

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
    assert_eq!(destination_branch.files.len(), 1); //
    assert_eq!(
        destination_branch.files[0].path,
        PathBuf::from_str("another file.txt").unwrap()
    );
    assert_eq!(destination_branch.files[0].hunks.len(), 1);
    assert_eq!(
        destination_branch.files[0].hunks[0].diff.to_str_lossy(),
        "@@ -0,0 +1 @@\n+another content\n\\ No newline at end of file\n"
    );
    assert_eq!(source_branch.commits.len(), 0);
    assert_eq!(source_branch.files.len(), 0);
}

#[test]
fn diffs_on_both_branches() {
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

    // Uncommitted changes on the source branch
    std::fs::write(
        repository.path().join("another file.txt"),
        "another content",
    )
    .unwrap();

    // Note: Calling `list_virtual_branches` actually is *needed* to correctly update the state of the virtual branches.
    let source_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == source_branch_id)
        .unwrap();

    // State of source branch after the first commit
    assert_eq!(source_branch.commits.len(), 1);
    assert_eq!(source_branch.files.len(), 1);
    assert_eq!(
        source_branch.files[0].path,
        PathBuf::from_str("another file.txt").unwrap()
    );
    assert_eq!(source_branch.files[0].hunks.len(), 1);
    assert_eq!(
        source_branch.files[0].hunks[0].diff.to_str_lossy(),
        "@@ -0,0 +1 @@\n+another content\n\\ No newline at end of file\n"
    );

    let target_branch_id = gitbutler_branch_actions::create_virtual_branch(
        project,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    // Uncommitted changes on the destination branch
    std::fs::write(
        repository.path().join("yet another file.txt"),
        "yet another content",
    )
    .unwrap();

    let destination_branch = gitbutler_branch_actions::list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == target_branch_id)
        .unwrap();

    // State of the destination branch before the commit is moved
    assert_eq!(destination_branch.commits.len(), 0);
    assert_eq!(destination_branch.files.len(), 1);
    assert_eq!(
        destination_branch.files[0].path,
        PathBuf::from_str("yet another file.txt").unwrap()
    );
    assert_eq!(destination_branch.files[0].hunks.len(), 1);
    assert_eq!(
        destination_branch.files[0].hunks[0].diff.to_str_lossy(),
        "@@ -0,0 +1 @@\n+yet another content\n\\ No newline at end of file\n"
    );

    gitbutler_branch_actions::move_commit(project, target_branch_id, commit_oid, source_branch_id)
        .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    let source_branch = branches.iter().find(|b| b.id == source_branch_id).unwrap();
    let destination_branch = branches.iter().find(|b| b.id == target_branch_id).unwrap();

    assert_eq!(destination_branch.commits.len(), 1);
    assert_eq!(destination_branch.files.len(), 1);
    assert_eq!(
        destination_branch.files[0].path,
        PathBuf::from_str("yet another file.txt").unwrap()
    );
    assert_eq!(destination_branch.files[0].hunks.len(), 1);
    assert_eq!(
        destination_branch.files[0].hunks[0].diff.to_str_lossy(),
        "@@ -0,0 +1 @@\n+yet another content\n\\ No newline at end of file\n"
    );

    assert_eq!(source_branch.commits.len(), 0);
    assert_eq!(source_branch.files.len(), 1);
    assert_eq!(
        source_branch.files[0].path,
        PathBuf::from_str("another file.txt").unwrap()
    );
    assert_eq!(source_branch.files[0].hunks.len(), 1);
    assert_eq!(
        source_branch.files[0].hunks[0].diff.to_str_lossy(),
        "@@ -0,0 +1 @@\n+another content\n\\ No newline at end of file\n"
    );
}

#[test]
fn target_commit_locked_to_ancestors() {
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

    std::fs::write(repository.path().join("a.txt"), "This is a").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    gitbutler_branch_actions::create_commit(project, source_branch_id, "Add a", None, false)
        .unwrap();

    std::fs::write(repository.path().join("a.txt"), "This is a \n\n Updated").unwrap();
    std::fs::write(repository.path().join("b.txt"), "This is b").unwrap();

    let commit_oid = gitbutler_branch_actions::create_commit(
        project,
        source_branch_id,
        "Add b and update b",
        None,
        false,
    )
    .unwrap();

    let target_branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    let result = gitbutler_branch_actions::move_commit(
        project,
        target_branch_id,
        commit_oid,
        source_branch_id,
    );

    assert_eq!(
        result.unwrap_err().to_string(), //
        "the target commit contains hunks locked to its ancestors"
    );
}

#[test]
fn target_commit_locked_to_descendants() {
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

    std::fs::write(repository.path().join("a.txt"), "This is a").unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let source_branch_id = branches[0].id;

    gitbutler_branch_actions::create_commit(project, source_branch_id, "Add a", None, false)
        .unwrap();

    std::fs::write(repository.path().join("b.txt"), "This is b").unwrap();

    let commit_oid = gitbutler_branch_actions::create_commit(
        project,
        source_branch_id,
        "Add b and update b",
        None,
        false,
    )
    .unwrap();

    std::fs::write(repository.path().join("b.txt"), "This is b and an update").unwrap();

    gitbutler_branch_actions::create_commit(project, source_branch_id, "Update b", None, false)
        .unwrap();

    let target_branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
            .unwrap();

    let result = gitbutler_branch_actions::move_commit(
        project,
        target_branch_id,
        commit_oid,
        source_branch_id,
    );

    assert_eq!(
        result.unwrap_err().to_string(), //
        "the target commit contains hunks locked to its descendants"
    );
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
        gitbutler_branch_actions::move_commit(
            project,
            target_branch_id,
            commit_oid,
            source_branch_id
        )
        .unwrap_err() //
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
            git2::Oid::from_str(commit_id_hex).unwrap(),
            source_branch_id,
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

    let id = StackId::generate();
    assert_eq!(
        gitbutler_branch_actions::move_commit(project, id, commit_oid, source_branch_id)
            .unwrap_err()
            .to_string(),
        "Destination branch not found"
    );
}
