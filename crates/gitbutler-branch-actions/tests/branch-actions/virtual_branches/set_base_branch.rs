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

#[test]
fn switching_the_target_is_observed_within_the_same_context() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // A second remote branch to switch to.
    {
        let gix_repo = repo.open();
        let head_id = gix_repo.head_id().unwrap().detach();
        gix_repo
            .reference(
                "refs/remotes/origin/other",
                head_id,
                gix::refs::transaction::PreviousValue::Any,
                "test",
            )
            .unwrap();
    }

    let mut guard = ctx.exclusive_worktree_access();
    // The first call ports project metadata into Git config.
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();

    // Once ported, switching again must be observed by reads through the same context.
    let base = gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/other".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    assert_eq!(base.branch_name, "origin/other");

    let project_meta = ctx.project_meta().unwrap();
    assert_eq!(
        project_meta.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/other".to_string())
    );
}

#[test]
fn fills_missing_target_commit_id_from_existing_target_ref() {
    let Test { repo, ctx, .. } = &mut Test::default();
    let target_ref = "refs/remotes/origin/master";
    let expected_target_id = repo
        .open()
        .find_reference(target_ref)
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .id;

    let mut project_meta = ctx.project_meta().unwrap();
    project_meta.target_ref = Some(target_ref.try_into().unwrap());
    project_meta.target_commit_id = None;
    ctx.set_project_meta(project_meta).unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &target_ref.parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    assert_eq!(
        ctx.project_meta().unwrap().target_commit_id,
        Some(expected_target_id)
    );
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

mod behind_count {
    use super::*;

    #[test]
    fn behind_reflects_farthest_behind_stack() {
        let Test { ctx, .. } = &mut Test::from_fixture("scenario/stacks-with-different-bases.sh");

        // HEAD is on branch A (forks from base, 3 behind origin/master).
        // set_base_branch picks up A as a workspace stack automatically.
        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        // Apply C (forks from M2, 1 behind).
        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::create_virtual_branch_from_branch_with_perm(
            ctx,
            &"refs/heads/C".parse().unwrap(),
            None,
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        // Stack A is farthest behind (3 commits behind origin/master).
        // Stack C is 1 commit behind. The behind count should reflect the max.
        let guard = ctx.shared_worktree_access();
        let base =
            gitbutler_branch_actions::base::get_base_branch_data(ctx, guard.read_permission())
                .unwrap();
        drop(guard);
        assert_eq!(
            base.behind, 3,
            "behind count should match the farthest-behind stack (A, which is 3 commits behind)"
        );
    }
}
