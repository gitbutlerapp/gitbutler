use super::*;

#[test]
fn success() {
    let Test { ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();
}

mod error {
    use gitbutler_reference::RemoteRefname;

    use super::*;

    #[test]
    fn missing() {
        let Test { ctx, .. } = &Test::default();

        assert_eq!(
            gitbutler_branch_actions::set_base_branch(
                ctx,
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
        let Test { repo, ctx, .. } = &Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let stack_entry =
            gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
                .unwrap();

        std::fs::write(repo.path().join("another file.txt"), "content").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "one", None).unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;

        assert_eq!(branches.len(), 1);

        repo.checkout_commit(oid_one);

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, stack_entry.id);
        assert!(branches[0].active);
    }

    #[test]
    fn from_target_branch_index_conflicts() {
        let Test { repo, ctx, .. } = &Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;

        assert!(branches.is_empty());

        repo.checkout_commit(oid_one);
        std::fs::write(repo.path().join("file.txt"), "tree").unwrap();

        assert!(matches!(
            gitbutler_branch_actions::set_base_branch(
                ctx,
                &"refs/remotes/origin/master".parse().unwrap()
            )
            .unwrap_err()
            .downcast_ref(),
            Some(Marker::ProjectConflict)
        ));
    }

    #[test]
    fn from_target_branch_with_uncommited() {
        let Test { repo, ctx, .. } = &Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert!(branches.is_empty());

        repo.checkout_commit(oid_one);
        std::fs::write(repo.path().join("another file.txt"), "tree").unwrap();

        assert!(matches!(
            gitbutler_branch_actions::set_base_branch(
                ctx,
                &"refs/remotes/origin/master".parse().unwrap()
            )
            .unwrap_err()
            .downcast_ref(),
            Some(Marker::ProjectConflict)
        ));
    }

    #[test]
    fn from_target_branch_with_commit() {
        let Test { repo, ctx, .. } = &Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        let base = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert!(branches.is_empty());

        repo.checkout_commit(oid_one);
        std::fs::write(repo.path().join("another file.txt"), "tree").unwrap();
        repo.commit_all("three");

        let base_two = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 0);
        assert_eq!(base_two, base);
    }

    #[test]
    fn from_target_branch_without_any_changes() {
        let Test { repo, ctx, .. } = &Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        let base = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert!(branches.is_empty());

        repo.checkout_commit(oid_one);

        let base_two = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
        )
        .unwrap();

        let list_result = gitbutler_branch_actions::list_virtual_branches(ctx).unwrap();
        let branches = list_result.branches;
        assert_eq!(branches.len(), 0);
        assert_eq!(base_two, base);
    }
}
