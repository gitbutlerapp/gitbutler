use gitbutler_branch::branch::Branch;
use gitbutler_id::id::Id;
use gitbutler_virtual::VirtualBranch;

use super::*;

#[tokio::test]
async fn should_lock_updated_hunks() {
    let Test {
        project,
        controller,
        repository,
        ..
    } = &Test::default();

    controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        // by default, hunks are not locked
        repository.write_file("file.txt", &["content".to_string()]);

        let branch = get_virtual_branch(controller, project, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
    }

    controller
        .create_commit(project, branch_id, "test", None, false)
        .await
        .unwrap();

    {
        // change in the committed hunks leads to hunk locking
        repository.write_file("file.txt", &["updated content".to_string()]);

        let branch = controller
            .list_virtual_branches(project)
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
async fn should_reset_into_same_branch() {
    let Test {
        project,
        controller,
        repository,
        ..
    } = &Test::default();

    let mut lines = repository.gen_file("file.txt", 7);
    commit_and_push_initial(repository);

    let base_branch = controller
        .set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    controller
        .create_virtual_branch(project, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    let branch_2_id = controller
        .create_virtual_branch(
            project,
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
        .create_commit(project, branch_2_id, "commit to branch 2", None, false)
        .await
        .unwrap();

    let files = get_virtual_branch(controller, project, branch_2_id)
        .await
        .files;
    assert_eq!(files.len(), 0);

    // Set target to branch 1 and verify the file resets into branch 2.
    controller
        .update_virtual_branch(
            project,
            branch::BranchUpdateRequest {
                id: branch_2_id,
                selected_for_changes: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    controller
        .reset_virtual_branch(project, branch_2_id, base_branch.base_sha)
        .await
        .unwrap();

    let files = get_virtual_branch(controller, project, branch_2_id)
        .await
        .files;
    assert_eq!(files.len(), 1);
}

fn commit_and_push_initial(repository: &TestProject) {
    repository.commit_all("initial commit");
    repository.push();
}

async fn get_virtual_branch(
    controller: &Controller,
    project: &Project,
    branch_id: Id<Branch>,
) -> VirtualBranch {
    controller
        .list_virtual_branches(project)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap()
}
