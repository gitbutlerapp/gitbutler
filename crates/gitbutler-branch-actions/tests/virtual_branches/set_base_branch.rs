use super::*;

#[test]
fn success() {
    let Test { ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
}

mod error {
    use gitbutler_reference::RemoteRefname;

    use super::*;

    #[test]
    fn missing() {
        let Test { ctx, .. } = &mut Test::default();

        let mut guard = ctx.exclusive_worktree_access();
        assert_eq!(
            gitbutler_branch_actions::set_base_branch(
                ctx,
                &RemoteRefname::from_str("refs/remotes/origin/missing").unwrap(),
                guard.write_permission(),
            )
            .unwrap_err()
            .to_string(),
            "remote branch 'refs/remotes/origin/missing' not found"
        );
    }
}

mod go_back_to_workspace {
    use gitbutler_branch::BranchCreateRequest;
    use gitbutler_testsupport::stack_details;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn should_preserve_applied_vbranches() {
        let Test { repo, ctx, .. } = &mut Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        std::fs::write(repo.path().join("another file.txt"), "content").unwrap();
        super::create_commit(ctx, stack_entry.id, "one").unwrap();

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);

        repo.checkout_commit(oid_one);

        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].0, stack_entry.id);
    }

    #[test]
    fn from_target_branch_index_conflicts() {
        let Test { repo, ctx, .. } = &mut Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        let stacks = stack_details(ctx);
        assert!(stacks.is_empty());

        repo.checkout_commit(oid_one);
        std::fs::write(repo.path().join("file.txt"), "tree").unwrap();

        let mut guard = ctx.exclusive_worktree_access();
        assert!(matches!(
            gitbutler_branch_actions::set_base_branch(
                ctx,
                &"refs/remotes/origin/master".parse().unwrap(),
                guard.write_permission(),
            )
            .unwrap_err()
            .downcast_ref(),
            Some(Marker::ProjectConflict)
        ));
    }

    #[test]
    fn from_target_branch_with_uncommited_conflicting() {
        let Test { repo, ctx, .. } = &mut Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        let stacks = stack_details(ctx);
        assert!(stacks.is_empty());

        repo.checkout_commit(oid_one);
        std::fs::write(repo.path().join("file.txt"), "tree").unwrap();

        let mut guard = ctx.exclusive_worktree_access();
        assert!(matches!(
            gitbutler_branch_actions::set_base_branch(
                ctx,
                &"refs/remotes/origin/master".parse().unwrap(),
                guard.write_permission(),
            )
            .unwrap_err()
            .downcast_ref(),
            Some(Marker::ProjectConflict)
        ));
    }

    #[test]
    fn from_target_branch_with_commit() {
        let Test { repo, ctx, .. } = &mut Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        let mut guard = ctx.exclusive_worktree_access();
        let base = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        let stacks = stack_details(ctx);
        assert!(stacks.is_empty());

        repo.checkout_commit(oid_one);
        std::fs::write(repo.path().join("another file.txt"), "tree").unwrap();
        repo.commit_all("three");

        let mut guard = ctx.exclusive_worktree_access();
        let base_two = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 0);
        assert_eq!(base_two, base);
    }

    #[test]
    fn from_target_branch_without_any_changes() {
        let Test { repo, ctx, .. } = &mut Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        let mut guard = ctx.exclusive_worktree_access();
        let base = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        let stacks = stack_details(ctx);
        assert!(stacks.is_empty());

        repo.checkout_commit(oid_one);

        let mut guard = ctx.exclusive_worktree_access();
        let base_two = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 0);
        assert_eq!(base_two, base);
    }
}
