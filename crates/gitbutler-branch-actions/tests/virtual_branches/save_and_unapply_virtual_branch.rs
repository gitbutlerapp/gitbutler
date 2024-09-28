use super::*;

#[test]
fn unapply_with_data() {
    let Test {
        project,
        repository,
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

    gitbutler_branch_actions::save_and_unapply_virutal_branch(project, branches[0].id).unwrap();

    assert!(!repository.path().join("file.txt").exists());

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 0);
}

#[test]
fn conflicting() {
    let Test {
        project,
        repository,
        ..
    } = &Test::default();

    // make sure we have an undiscovered commit in the remote branch
    {
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        let first_commit_oid = repository.commit_all("first");
        fs::write(repository.path().join("file.txt"), "second").unwrap();
        repository.commit_all("second");
        repository.push();
        repository.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let unapplied_branch = {
        // make a conflicting branch, and stash it

        std::fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        assert_eq!(branches.len(), 1);
        let branch = &branches[0];
        assert_eq!(
            branch.name, "Virtual branch",
            "the auto-created branch gets the default name"
        );
        assert!(branch.base_current);
        assert!(branch.active);
        assert_eq!(
            branch.files[0].hunks[0].diff,
            "@@ -1 +1 @@\n-first\n\\ No newline at end of file\n+conflict\n\\ No newline at end of file\n"
        );

        let unapplied_branch =
            gitbutler_branch_actions::save_and_unapply_virutal_branch(project, branch.id).unwrap();

        Refname::from_str(&unapplied_branch).unwrap()
    };

    {
        // update base branch, causing conflict
        gitbutler_branch_actions::update_base_branch(project).unwrap();

        assert_eq!(
            std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "second"
        );
    }

    let branch_id = {
        // apply branch, it should conflict
        let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
            project,
            &unapplied_branch,
            None,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
        );

        let vb_state = VirtualBranchesHandle::new(project.gb_dir());
        let ctx = CommandContext::open(project).unwrap();
        update_workspace_commit(&vb_state, &ctx).unwrap();
        let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();

        assert_eq!(branches.len(), 1);
        let branch = &branches[0];
        assert!(branch.conflicted);
        assert_eq!(
            branch.files[0].hunks[0].diff,
            "@@ -1 +1,5 @@\n-first\n\\ No newline at end of file\n+<<<<<<< ours\n+conflict\n+=======\n+second\n+>>>>>>> theirs\n"
        );

        branch_id
    };

    {
        // Converting the branch to a real branch should put us back in an unconflicted state
        gitbutler_branch_actions::save_and_unapply_virutal_branch(project, branch_id).unwrap();

        assert_eq!(
            std::fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "second"
        );
    }
}

#[test]
fn delete_if_empty() {
    let Test { project, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())
        .unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 1);

    gitbutler_branch_actions::save_and_unapply_virutal_branch(project, branches[0].id).unwrap();

    let (branches, _) = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
    assert_eq!(branches.len(), 0);
}
