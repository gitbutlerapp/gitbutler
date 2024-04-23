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

        fs::write(repository.path().join("file.txt"), "content").unwrap();

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
        assert!(!branch.files[0].hunks[0].locked);
    }

    controller
        .create_commit(project_id, &branch_id, "test", None, false)
        .await
        .unwrap();

    {
        // change in the committed hunks leads to hunk locking
        fs::write(repository.path().join("file.txt"), "updated content").unwrap();

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
    fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
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
        fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
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
        changed_lines[4] = "updated line".to_string();
        fs::write(repository.path().join("file.txt"), changed_lines.join("\n")).unwrap();
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
        assert!(!branch.files[0].hunks[0].locked);
        // cleanup
        fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
    }
    {
        // hunk after the commited part is not locked
        let mut changed_lines = lines.clone();
        changed_lines[20] = "updated line".to_string();
        fs::write(repository.path().join("file.txt"), changed_lines.join("\n")).unwrap();
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
        assert!(!branch.files[0].hunks[0].locked);
        // cleanup
        fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
    }
    {
        // hunk before the commited part but with overlapping context
        let mut changed_lines = lines.clone();
        changed_lines[10] = "updated line".to_string();
        fs::write(repository.path().join("file.txt"), changed_lines.join("\n")).unwrap();
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
        // TODO: We lock this hunk, but can we afford not lock it?
        assert!(branch.files[0].hunks[0].locked);
        // cleanup
        fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
    }
    {
        // hunk after the commited part but with overlapping context
        let mut changed_lines = lines.clone();
        changed_lines[14] = "updated line".to_string();
        fs::write(repository.path().join("file.txt"), changed_lines.join("\n")).unwrap();
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
        // TODO: We lock this hunk, but can we afford not lock it?
        assert!(branch.files[0].hunks[0].locked);
        // cleanup
        fs::write(repository.path().join("file.txt"), lines.clone().join("\n")).unwrap();
    }
}
