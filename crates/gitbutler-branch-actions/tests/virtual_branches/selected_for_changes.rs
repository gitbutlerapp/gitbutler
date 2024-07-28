use super::*;
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};

#[test]
fn unapplying_selected_branch_selects_anther() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    std::fs::write(repository.path().join("file one.txt"), "").unwrap();

    // first branch should be created as default
    let b_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    // if default branch exists, new branch should not be created as default
    let b2_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();

    let b = branches.iter().find(|b| b.id == b_id).unwrap();

    let b2 = branches.iter().find(|b| b.id == b2_id).unwrap();

    assert!(b.selected_for_changes);
    assert!(!b2.selected_for_changes);

    controller.convert_to_real_branch(project, b_id).unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();

    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, b2.id);
    assert!(branches[0].selected_for_changes);
    assert!(branches[0].active);
}

#[test]
fn deleting_selected_branch_selects_anther() {
    let Test {
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    // first branch should be created as default
    let b_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    // if default branch exists, new branch should not be created as default
    let b2_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();

    let b = branches.iter().find(|b| b.id == b_id).unwrap();

    let b2 = branches.iter().find(|b| b.id == b2_id).unwrap();

    assert!(b.selected_for_changes);
    assert!(!b2.selected_for_changes);

    controller.delete_virtual_branch(project, b_id).unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();

    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, b2.id);
    assert!(branches[0].selected_for_changes);
}

#[test]
fn create_virtual_branch_should_set_selected_for_changes() {
    let Test {
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    // first branch should be created as default
    let b_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();
    let branch = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b_id)
        .unwrap();
    assert!(branch.selected_for_changes);

    // if default branch exists, new branch should not be created as default
    let b_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();
    let branch = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b_id)
        .unwrap();
    assert!(!branch.selected_for_changes);

    // explicitly don't make this one default
    let b_id = controller
        .create_virtual_branch(
            project,
            &BranchCreateRequest {
                selected_for_changes: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    let branch = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b_id)
        .unwrap();
    assert!(!branch.selected_for_changes);

    // explicitly make this one default
    let b_id = controller
        .create_virtual_branch(
            project,
            &BranchCreateRequest {
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    let branch = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b_id)
        .unwrap();
    assert!(branch.selected_for_changes);
}

#[test]
fn update_virtual_branch_should_reset_selected_for_changes() {
    let Test {
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let b1_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();
    let b1 = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b1_id)
        .unwrap();
    assert!(b1.selected_for_changes);

    let b2_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();
    let b2 = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b2_id)
        .unwrap();
    assert!(!b2.selected_for_changes);

    controller
        .update_virtual_branch(
            project,
            BranchUpdateRequest {
                id: b2_id,
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

    let b1 = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b1_id)
        .unwrap();
    assert!(!b1.selected_for_changes);

    let b2 = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b2_id)
        .unwrap();
    assert!(b2.selected_for_changes);
}

#[test]
fn unapply_virtual_branch_should_reset_selected_for_changes() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let b1_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();
    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let b1 = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b1_id)
        .unwrap();
    assert!(b1.selected_for_changes);

    let b2_id = controller
        .create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    let b2 = controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b2_id)
        .unwrap();
    assert!(!b2.selected_for_changes);

    controller.convert_to_real_branch(project, b1_id).unwrap();

    assert!(controller
        .list_virtual_branches(project)
        .unwrap()
        .0
        .into_iter()
        .any(|b| b.selected_for_changes && b.id != b1_id))
}

#[test]
fn hunks_distribution() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].files.len(), 1);

    controller
        .create_virtual_branch(
            project,
            &BranchCreateRequest {
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    std::fs::write(repository.path().join("another_file.txt"), "content").unwrap();
    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[1].files.len(), 1);
}

#[test]
fn applying_first_branch() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    let unapplied_branch = controller
        .convert_to_real_branch(project, branches[0].id)
        .unwrap();
    let unapplied_branch = Refname::from_str(&unapplied_branch).unwrap();
    controller
        .create_virtual_branch_from_branch(project, &unapplied_branch, None)
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].active);
    assert!(branches[0].selected_for_changes);
}

// This test was written in response to issue #4148, to ensure the appearence
// of a locked hunk doesn't drag along unrelated hunks to its branch.
#[test]
fn new_locked_hunk_without_modifying_existing() {
    let Test {
        repository,
        project,
        controller,
        ..
    } = &Test::default();

    let mut lines = repository.gen_file("file.txt", 9);
    repository.commit_all("first commit");
    repository.push();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    lines[0] = "modification 1".to_string();
    repository.write_file("file.txt", &lines);

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].files.len(), 1);

    controller
        .create_commit(project, branches[0].id, "second commit", None, false)
        .expect("failed to create commit");

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].files.len(), 0);
    assert_eq!(branches[0].commits.len(), 1);

    controller
        .create_virtual_branch(
            project,
            &BranchCreateRequest {
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

    lines[8] = "modification 2".to_string();
    repository.write_file("file.txt", &lines);

    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].files.len(), 0);
    assert_eq!(branches[1].files.len(), 1);

    lines[0] = "modification 3".to_string();
    repository.write_file("file.txt", &lines);
    let (branches, _) = controller.list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[1].files.len(), 1);
}
