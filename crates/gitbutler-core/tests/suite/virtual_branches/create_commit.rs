use gitbutler_core::{
    id::Id,
    virtual_branches::{Branch, VirtualBranch},
};

use super::*;

#[tokio::test]
async fn should_lock_updated_hunks() {
    let Test {
        project_id,
        controller,
        repository,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        // by default, hunks are not locked
        repository.write_file("file.txt", &["content".to_string()]);

        let branch = get_virtual_branch(controller, *project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
    }

    controller
        .create_commit(*project_id, branch_id, "test", None, false)
        .await
        .unwrap();

    {
        // change in the committed hunks leads to hunk locking
        repository.write_file("file.txt", &["updated content".to_string()]);

        let branch = controller
            .list_virtual_branches(*project_id)
            .await
            .unwrap()
            .0
            .into_iter()
            .find(|b| b.id == branch_id)
            .unwrap();
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(branch.files[0].hunks[0].locked);
    }
}

#[tokio::test]
async fn should_not_lock_disjointed_hunks() {
    let Test {
        project_id,
        controller,
        repository,
        ..
    } = &Test::default();

    let mut lines: Vec<_> = (0_i32..24_i32).map(|i| format!("line {}", i)).collect();
    repository.write_file("file.txt", &lines);
    repository.commit_all("my commit");
    repository.push();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        // new hunk in the middle of the file
        lines[12] = "commited stuff".to_string();
        repository.write_file("file.txt", &lines);
        let branch = get_virtual_branch(controller, *project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
    }

    controller
        .create_commit(*project_id, branch_id, "test commit", None, false)
        .await
        .unwrap();
    controller
        .push_virtual_branch(*project_id, branch_id, false, None)
        .await
        .unwrap();

    {
        // hunk before the commited part is not locked
        let mut changed_lines = lines.clone();
        changed_lines[8] = "updated line".to_string();
        repository.write_file("file.txt", &changed_lines);
        let branch = get_virtual_branch(controller, *project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
        // cleanup
        repository.write_file("file.txt", &lines);
    }
    {
        // hunk after the commited part is not locked
        let mut changed_lines = lines.clone();
        changed_lines[16] = "updated line".to_string();
        repository.write_file("file.txt", &changed_lines);
        let branch = get_virtual_branch(controller, *project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
        // cleanup
        repository.write_file("file.txt", &lines);
    }
    {
        // hunk before the commited part but with overlapping context
        let mut changed_lines = lines.clone();
        changed_lines[10] = "updated line".to_string();
        repository.write_file("file.txt", &changed_lines);
        let branch = get_virtual_branch(controller, *project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        // TODO: We lock this hunk, but can we afford not lock it?
        assert!(branch.files[0].hunks[0].locked);
        // cleanup
        repository.write_file("file.txt", &lines);
    }
    {
        // hunk after the commited part but with overlapping context
        let mut changed_lines = lines.clone();
        changed_lines[14] = "updated line".to_string();
        repository.write_file("file.txt", &changed_lines);
        let branch = get_virtual_branch(controller, *project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        // TODO: We lock this hunk, but can we afford not lock it?
        assert!(branch.files[0].hunks[0].locked);
        // cleanup
        repository.write_file("file.txt", &lines);
    }
}

#[tokio::test]
async fn should_reset_into_same_branch() {
    let Test {
        project_id,
        controller,
        repository,
        ..
    } = &Test::default();

    let mut lines = repository.gen_file("file.txt", 7);
    commit_and_push_initial(repository);

    let base_branch = controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let branch_2_id = controller
        .create_virtual_branch(
            *project_id,
            &branch::BranchCreateRequest {
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    lines[0] = "change 1".to_string();
    repository.write_file("file.txt", &lines);

    controller
        .create_commit(*project_id, branch_2_id, "commit to branch 2", None, false)
        .await
        .unwrap();

    let files = get_virtual_branch(controller, *project_id, branch_2_id)
        .await
        .files;
    assert_eq!(files.len(), 0);

    // Set target to branch 1 and verify the file resets into branch 2.
    controller
        .update_virtual_branch(
            *project_id,
            branch::BranchUpdateRequest {
                id: branch_2_id,
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    controller
        .reset_virtual_branch(*project_id, branch_2_id, base_branch.base_sha)
        .await
        .unwrap();

    let files = get_virtual_branch(controller, *project_id, branch_2_id)
        .await
        .files;
    assert_eq!(files.len(), 1);
}

#[tokio::test]
async fn should_double_lock() {
    let Test {
        project_id,
        controller,
        repository,
        ..
    } = &Test::default();

    let mut lines = repository.gen_file("file.txt", 7);
    repository.write_file("file.txt", &lines);
    commit_and_push_initial(repository);

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    lines[0] = "change 1".to_string();
    repository.write_file("file.txt", &lines);

    let commit_1 = controller
        .create_commit(*project_id, branch_id, "commit 1", None, false)
        .await
        .unwrap();

    lines[6] = "change 2".to_string();
    repository.write_file("file.txt", &lines);

    let commit_2 = controller
        .create_commit(*project_id, branch_id, "commit 2", None, false)
        .await
        .unwrap();

    lines[3] = "change3".to_string();
    repository.write_file("file.txt", &lines);

    let branch = get_virtual_branch(controller, *project_id, branch_id).await;
    let locks = &branch.files[0].hunks[0].locked_to.clone().unwrap();

    assert_eq!(locks.len(), 2);
    assert_eq!(locks[0].commit_id, commit_1);
    assert_eq!(locks[1].commit_id, commit_2);
}

fn commit_and_push_initial(repository: &TestProject) {
    repository.commit_all("initial commit");
    repository.push();
}

async fn get_virtual_branch(
    controller: &Controller,
    project_id: ProjectId,
    branch_id: Id<Branch>,
) -> VirtualBranch {
    controller
        .list_virtual_branches(project_id)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap()
}
