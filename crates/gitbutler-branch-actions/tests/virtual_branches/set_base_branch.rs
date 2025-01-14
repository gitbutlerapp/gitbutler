use super::*;

#[test]
fn success() {
    let Test { project, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();
}

mod error {
    use gitbutler_reference::RemoteRefname;

    use super::*;

    #[test]
    fn missing() {
        let Test { project, .. } = &Test::default();

        assert_eq!(
            gitbutler_branch_actions::set_base_branch(
                project,
                &RemoteRefname::from_str("refs/remotes/origin/missing").unwrap(),
            )
            .unwrap_err()
            .to_string(),
            "remote branch 'refs/remotes/origin/missing' not found"
        );
    }
}

mod go_back_to_workspace {
    use gitbutler_branch::BranchCreateRequest;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn should_preserve_applied_vbranches() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let vbranch_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest::default(),
        )
        .unwrap();

        std::fs::write(repository.path().join("another file.txt"), "content").unwrap();
        gitbutler_branch_actions::create_commit(project, vbranch_id, "one", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;

        assert_eq!(branches.len(), 1);

        repository.checkout_commit(oid_one);

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, vbranch_id);
        assert!(branches[0].active);
    }

    #[test]
    fn from_target_branch_index_conflicts() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;

        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);
        std::fs::write(repository.path().join("file.txt"), "tree").unwrap();

        assert!(matches!(
            gitbutler_branch_actions::set_base_branch(
                project,
                &"refs/remotes/origin/master".parse().unwrap()
            )
            .unwrap_err()
            .downcast_ref(),
            Some(Marker::ProjectConflict)
        ));
    }

    #[test]
    fn from_target_branch_with_uncommited() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);
        std::fs::write(repository.path().join("another file.txt"), "tree").unwrap();

        assert!(matches!(
            gitbutler_branch_actions::set_base_branch(
                project,
                &"refs/remotes/origin/master".parse().unwrap()
            )
            .unwrap_err()
            .downcast_ref(),
            Some(Marker::ProjectConflict)
        ));
    }

    #[test]
    fn from_target_branch_with_commit() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        let base = gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);
        std::fs::write(repository.path().join("another file.txt"), "tree").unwrap();
        repository.commit_all("three");

        let base_two = gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 0);
        assert_eq!(base_two, base);
    }

    #[test]
    fn from_target_branch_without_any_changes() {
        let Test {
            repository,
            project,
            ..
        } = &Test::default();

        std::fs::write(repository.path().join("file.txt"), "one").unwrap();
        let oid_one = repository.commit_all("one");
        std::fs::write(repository.path().join("file.txt"), "two").unwrap();
        repository.commit_all("two");
        repository.push();

        let base = gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert!(branches.is_empty());

        repository.checkout_commit(oid_one);

        let base_two = gitbutler_branch_actions::set_base_branch(
            project,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(project).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 0);
        assert_eq!(base_two, base);
    }
}
