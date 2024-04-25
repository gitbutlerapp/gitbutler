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
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        // by default, hunks are not locked
        write_file(repository, "file.txt", &vec!["content".to_string()]);

        let branch = get_virtual_branch(controller, project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
    }

    controller
        .create_commit(project_id, &branch_id, "test", None, false)
        .await
        .unwrap();

    {
        // change in the committed hunks leads to hunk locking
        write_file(repository, "file.txt", &vec!["updated content".to_string()]);

        let branch = controller
            .list_virtual_branches(project_id)
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
    write_file(repository, "file.txt", &lines);
    repository.commit_all("my commit");
    repository.push();

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    {
        // new hunk in the middle of the file
        lines[12] = "commited stuff".to_string();
        write_file(repository, "file.txt", &lines);
        let branch = get_virtual_branch(controller, project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
    }

    controller
        .create_commit(project_id, &branch_id, "test commit", None, false)
        .await
        .unwrap();
    controller
        .push_virtual_branch(project_id, &branch_id, false, None)
        .await
        .unwrap();

    {
        // hunk before the commited part is not locked
        let mut changed_lines = lines.clone();
        changed_lines[8] = "updated line".to_string();
        write_file(repository, "file.txt", &changed_lines);
        let branch = get_virtual_branch(controller, project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
        // cleanup
        write_file(repository, "file.txt", &lines);
    }
    {
        // hunk after the commited part is not locked
        let mut changed_lines = lines.clone();
        changed_lines[16] = "updated line".to_string();
        write_file(repository, "file.txt", &changed_lines);
        let branch = get_virtual_branch(controller, project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        assert!(!branch.files[0].hunks[0].locked);
        // cleanup
        write_file(repository, "file.txt", &lines);
    }
    {
        // hunk before the commited part but with overlapping context
        let mut changed_lines = lines.clone();
        changed_lines[10] = "updated line".to_string();
        write_file(repository, "file.txt", &changed_lines);
        let branch = get_virtual_branch(controller, project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        // TODO: We lock this hunk, but can we afford not lock it?
        assert!(branch.files[0].hunks[0].locked);
        // cleanup
        write_file(repository, "file.txt", &lines);
    }
    {
        // hunk after the commited part but with overlapping context
        let mut changed_lines = lines.clone();
        changed_lines[14] = "updated line".to_string();
        write_file(repository, "file.txt", &changed_lines);
        let branch = get_virtual_branch(controller, project_id, branch_id).await;
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.files[0].path.display().to_string(), "file.txt");
        assert_eq!(branch.files[0].hunks.len(), 1);
        // TODO: We lock this hunk, but can we afford not lock it?
        assert!(branch.files[0].hunks[0].locked);
        // cleanup
        write_file(repository, "file.txt", &lines);
    }
}

#[tokio::test]
async fn should_double_lock() {
    let Test {
        project_id,
        controller,
        repository,
        ..
    } = &Test::default();

    let mut lines = gen_file(repository, "file.txt", 7);
    write_file(repository, "file.txt", &lines);
    commit_and_push_initial(repository);

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch_id = controller
        .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
        .await
        .unwrap();

    lines[0] = "change 1".to_string();
    write_file(repository, "file.txt", &lines);

    let commit_1 = controller
        .create_commit(project_id, &branch_id, "commit 1", None, false)
        .await
        .unwrap();

    lines[6] = "change 2".to_string();
    write_file(repository, "file.txt", &lines);

    let commit_2 = controller
        .create_commit(project_id, &branch_id, "commit 2", None, false)
        .await
        .unwrap();

    lines[3] = "change3".to_string();
    write_file(repository, "file.txt", &lines);

    let branch = get_virtual_branch(controller, project_id, branch_id).await;
    let locks = &branch.files[0].hunks[0].locked_to.clone().unwrap();

    assert_eq!(locks.len(), 2);
    assert_eq!(locks[0], commit_1);
    assert_eq!(locks[1], commit_2);
}

fn write_file(repository: &TestProject, path: &str, lines: &Vec<String>) {
    fs::write(repository.path().join(path), lines.join("\n")).unwrap()
}

fn gen_file(repository: &TestProject, path: &str, line_count: i32) -> Vec<String> {
    let lines: Vec<_> = (0_i32..line_count).map(|i| format!("line {}", i)).collect();
    write_file(repository, path, &lines);
    lines
}

fn commit_and_push_initial(repository: &TestProject) {
    repository.commit_all("initial commit");
    repository.push();
}

async fn get_virtual_branch(
    controller: &Controller,
    project_id: &ProjectId,
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
