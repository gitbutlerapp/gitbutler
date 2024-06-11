use super::*;

#[tokio::test]
async fn unapplying_selected_branch_selects_anther() {
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

    std::fs::write(repository.path().join("file one.txt"), "").unwrap();

    // first branch should be created as default
    let b_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    // if default branch exists, new branch should not be created as default
    let b2_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();

    let b = branches.iter().find(|b| b.id == b_id).unwrap();

    let b2 = branches.iter().find(|b| b.id == b2_id).unwrap();

    assert!(b.selected_for_changes);
    assert!(!b2.selected_for_changes);

    controller
        .convert_to_real_branch(*project_id, b_id, Default::default())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();

    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].id, b.id);
    assert!(!branches[0].selected_for_changes);
    assert!(!branches[0].active);
    assert_eq!(branches[1].id, b2.id);
    assert!(branches[1].selected_for_changes);
    assert!(branches[1].active);
}

#[tokio::test]
async fn deleting_selected_branch_selects_anther() {
    let Test {
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    // first branch should be created as default
    let b_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    // if default branch exists, new branch should not be created as default
    let b2_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();

    let b = branches.iter().find(|b| b.id == b_id).unwrap();

    let b2 = branches.iter().find(|b| b.id == b2_id).unwrap();

    assert!(b.selected_for_changes);
    assert!(!b2.selected_for_changes);

    controller
        .delete_virtual_branch(*project_id, b_id)
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();

    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].id, b2.id);
    assert!(branches[0].selected_for_changes);
}

#[tokio::test]
async fn create_virtual_branch_should_set_selected_for_changes() {
    let Test {
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    // first branch should be created as default
    let b_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();
    let branch = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b_id)
        .unwrap();
    assert!(branch.selected_for_changes);

    // if default branch exists, new branch should not be created as default
    let b_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();
    let branch = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b_id)
        .unwrap();
    assert!(!branch.selected_for_changes);

    // explicitly don't make this one default
    let b_id = controller
        .create_virtual_branch(
            *project_id,
            &branch::BranchCreateRequest {
                selected_for_changes: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let branch = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b_id)
        .unwrap();
    assert!(!branch.selected_for_changes);

    // explicitly make this one default
    let b_id = controller
        .create_virtual_branch(
            *project_id,
            &branch::BranchCreateRequest {
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let branch = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b_id)
        .unwrap();
    assert!(branch.selected_for_changes);
}

#[tokio::test]
async fn update_virtual_branch_should_reset_selected_for_changes() {
    let Test {
        project_id,
        controller,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let b1_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();
    let b1 = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b1_id)
        .unwrap();
    assert!(b1.selected_for_changes);

    let b2_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();
    let b2 = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b2_id)
        .unwrap();
    assert!(!b2.selected_for_changes);

    controller
        .update_virtual_branch(
            *project_id,
            branch::BranchUpdateRequest {
                id: b2_id,
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let b1 = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b1_id)
        .unwrap();
    assert!(!b1.selected_for_changes);

    let b2 = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b2_id)
        .unwrap();
    assert!(b2.selected_for_changes);
}

#[tokio::test]
async fn unapply_virtual_branch_should_reset_selected_for_changes() {
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

    let b1_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();
    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let b1 = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b1_id)
        .unwrap();
    assert!(b1.selected_for_changes);

    controller
        .convert_to_real_branch(*project_id, b1_id, Default::default())
        .await
        .unwrap();

    let b1 = controller
        .list_virtual_branches(*project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == b1_id)
        .unwrap();
    assert!(!b1.selected_for_changes);
}

#[tokio::test]
async fn hunks_distribution() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches[0].files.len(), 1);

    controller
        .create_virtual_branch(
            *project_id,
            &branch::BranchCreateRequest {
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    std::fs::write(repository.path().join("another_file.txt"), "content").unwrap();
    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[1].files.len(), 1);
}

#[tokio::test]
async fn applying_first_branch() {
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

    std::fs::write(repository.path().join("file.txt"), "content").unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);

    controller
        .convert_to_real_branch(*project_id, branches[0].id, Default::default())
        .await
        .unwrap();
    controller
        .apply_virtual_branch(*project_id, branches[0].id)
        .await
        .unwrap();

    let (branches, _) = controller.list_virtual_branches(*project_id).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].active);
    assert!(branches[0].selected_for_changes);
}
