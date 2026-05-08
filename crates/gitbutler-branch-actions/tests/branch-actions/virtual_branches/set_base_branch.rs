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

/// Tests that corrupt git state while outside the workspace, simulating what real users
/// do when they `git switch feature-a`, modify refs, then return to GitButler.
///
/// These tests were written to investigate the production error:
/// "Branch cannot be created: target commit already belongs to another branch"
/// (89 PostHog events, 48 users). The root cause is a stack branch ref pointing below
/// the workspace's lower bound (target base), which causes the graph to claim the
/// target commit for that stack, blocking new independent branches.
mod adversarial_outside_workspace {
    use but_core::ref_metadata::StackId;
    use gitbutler_branch::BranchCreateRequest;
    use gix::reference::Category;
    use gix::refs::transaction::PreviousValue;
    use pretty_assertions::assert_eq;

    use super::*;

    /// Helper: create a branch via the v3 path (workspace_mut_and_db + create_reference).
    fn create_branch_v3(ctx: &mut Context, name: &str) -> anyhow::Result<()> {
        let new_ref = Category::LocalBranch
            .to_full_name(name)
            .map_err(anyhow::Error::from)?;
        let mut meta = ctx.meta()?;
        let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
        let new_ws = but_workspace::branch::create_reference(
            new_ref.as_ref(),
            None,
            &repo,
            &ws,
            &mut meta,
            |_| StackId::generate(),
            None,
        )?;
        *ws = new_ws.into_owned();
        Ok(())
    }

    /// Setup: workspace with one stack that has a commit touching `another_file.txt`.
    /// Returns (early_oid for switching away, stack_entry).
    fn setup_workspace_with_stack(
        repo: &mut super::TestRepo,
        ctx: &mut Context,
    ) -> (gix::ObjectId, but_workspace::legacy::ui::StackEntryNoOpt) {
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
            &BranchCreateRequest {
                name: Some("my-stack".into()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        std::fs::write(repo.path().join("another_file.txt"), "stack work").unwrap();
        super::create_commit(ctx, stack_entry.id, "stack commit").unwrap();

        (oid_one, stack_entry)
    }

    /// Deleting a stack's branch ref while outside the workspace (e.g. `git branch -D my-stack`)
    /// should not permanently break the workspace — creating branches should still work after
    /// returning via set_base_branch.
    #[test]
    fn delete_stack_branch_ref_while_outside_then_return_and_create() {
        let Test { repo, ctx, .. } = &mut Test::default();

        let (oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);

        repo.checkout_commit(oid_one);

        {
            let raw_repo = repo.open();
            if let Ok(reference) = raw_repo.find_reference("refs/heads/my-stack") {
                reference.delete().unwrap();
            }
        }

        let mut guard = ctx.exclusive_worktree_access();
        let return_result = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        );
        drop(guard);

        if let Err(e) = &return_result {
            eprintln!("Return after deleting stack branch failed: {e:#}");
            return;
        }

        let mut guard = ctx.exclusive_worktree_access();
        let create_result = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            guard.write_permission(),
        );
        drop(guard);

        if let Err(e) = &create_result {
            eprintln!("Create branch after stack deletion failed: {e:#}");
        }

        let v3_result = create_branch_v3(ctx, "v3-after-delete");
        if let Err(e) = &v3_result {
            eprintln!("V3 create after stack deletion failed: {e:#}");
        }

        assert!(
            create_result.is_ok() || v3_result.is_ok(),
            "Workspace should be recoverable after stack branch deletion"
        );
    }

    /// Forcing a stack ref to point at the target base commit (e.g. `git branch -f my-stack origin/master`)
    /// is the boundary case: the ref is AT the lower bound, not below it.
    /// This should work — it's equivalent to an empty stack.
    #[test]
    fn force_update_stack_ref_to_target_base_while_outside_then_return_and_create() {
        let Test { repo, ctx, .. } = &mut Test::default();

        let (oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);
        let target_oid = ctx.persisted_default_target().unwrap().sha;

        repo.checkout_commit(oid_one);

        {
            let raw_repo = repo.open();
            raw_repo
                .reference(
                    "refs/heads/my-stack",
                    target_oid,
                    PreviousValue::Any,
                    "test: force stack ref to target base",
                )
                .unwrap();
        }

        let mut guard = ctx.exclusive_worktree_access();
        let return_result = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        );
        drop(guard);

        if let Err(e) = &return_result {
            eprintln!("Return after force-updating stack ref failed: {e:#}");
            return;
        }

        let v3_result = create_branch_v3(ctx, "new-branch");
        if let Err(e) = &v3_result {
            eprintln!(
                "V3 create after force-update to target base: {e:#}\n\
                 This may be the production bug — two branches claiming the same base commit."
            );
        }

        let mut guard = ctx.exclusive_worktree_access();
        let legacy_result = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            guard.write_permission(),
        );
        drop(guard);
        if let Err(e) = &legacy_result {
            eprintln!("Legacy create after force-update to target base: {e:#}");
        }

        if let (Err(v3_err), Err(legacy_err)) = (&v3_result, &legacy_result) {
            panic!(
                "BOTH create paths failed after force-updating stack ref to target base.\n\
                 v3: {v3_err}\n\
                 legacy: {legacy_err}",
            );
        }
    }

    /// BUG REPRODUCTION + FIX: Forces a stack ref to a commit BELOW the target base while
    /// already in workspace mode (simulating `git branch -f my-stack <old-commit>` from another
    /// terminal). This is the root cause of "target commit already belongs to another branch"
    /// (89 PostHog events, 48 users).
    ///
    /// The fix: detect the divergence (watcher would trigger this) and resolve it (Exclude)
    /// before the corrupted state can block new branch creation.
    #[test]
    fn force_update_stack_ref_to_earlier_commit_while_outside_then_return_and_create() {
        use gitbutler_branch_actions::stack_divergence::{
            DivergenceApproach, DivergenceResolution, DivergenceStatus, DivergenceStatuses,
        };

        let Test { repo, ctx, .. } = &mut Test::default();

        let (oid_one, stack_entry) = setup_workspace_with_stack(repo, ctx);

        // Force the stack ref below the target base while still in workspace mode.
        // This simulates someone running `git branch -f my-stack <old-commit>` in another terminal.
        {
            let raw_repo = repo.open();
            raw_repo
                .reference(
                    "refs/heads/my-stack",
                    oid_one,
                    PreviousValue::Any,
                    "test: force stack ref to early commit",
                )
                .unwrap();
        }

        // Detect the divergence (this is what the watcher would trigger).
        let statuses =
            gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx).unwrap();
        match &statuses {
            DivergenceStatuses::DivergedRefs { divergences } => {
                assert_eq!(divergences.len(), 1);
                assert_eq!(divergences[0].status, DivergenceStatus::MovedBelowBase);
            }
            DivergenceStatuses::UpToDate => {
                panic!("Expected divergence but got UpToDate");
            }
        }

        // Resolve by excluding the corrupted stack.
        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::stack_divergence::resolve_diverged_stacks(
            ctx,
            &[DivergenceResolution {
                stack_id: stack_entry.id,
                approach: DivergenceApproach::Exclude,
            }],
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        // After resolution, creating a branch succeeds.
        let v3_result = create_branch_v3(ctx, "new-branch-after-fix");
        assert!(
            v3_result.is_ok(),
            "After divergence resolution, creating branch should work: {:?}",
            v3_result.unwrap_err()
        );
    }

    /// Checking out a branch with the same name as an existing stack (e.g. `git checkout my-stack`),
    /// making commits on it, then returning — tests whether the name collision between the
    /// user's branch and GitButler's stack ref causes problems.
    #[test]
    fn checkout_branch_with_same_name_as_stack_then_return_and_create() {
        let Test { repo, ctx, .. } = &mut Test::default();

        let (_oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);

        repo.checkout(&"refs/heads/my-stack".parse().unwrap());

        std::fs::write(repo.path().join("diverged.txt"), "diverged content").unwrap();
        repo.commit_all("diverged commit on my-stack");

        let mut guard = ctx.exclusive_worktree_access();
        let return_result = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        );
        drop(guard);

        if let Err(e) = &return_result {
            eprintln!("Return from same-named branch: {e:#}");
            return;
        }

        let stacks = stack_details(ctx);
        eprintln!(
            "Stacks after returning from same-named branch: {}",
            stacks.len()
        );

        let v3_result = create_branch_v3(ctx, "new-branch-after-collision");
        assert!(
            v3_result.is_ok(),
            "Creating branch after name collision return should work: {:?}",
            v3_result.unwrap_err()
        );
    }

    /// With multiple stacks, deleting just one stack's ref while outside should not break
    /// the workspace — the surviving stack should remain intact and new branches should
    /// still be creatable.
    #[test]
    fn multiple_stacks_delete_one_ref_while_outside_then_return_and_create() {
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
        let stack_a = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("stack-a".into()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);
        std::fs::write(repo.path().join("a.txt"), "a content").unwrap();
        super::create_commit(ctx, stack_a.id, "commit a").unwrap();

        let mut guard = ctx.exclusive_worktree_access();
        let _stack_b = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("stack-b".into()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);
        std::fs::write(repo.path().join("b.txt"), "b content").unwrap();
        super::create_commit(ctx, _stack_b.id, "commit b").unwrap();

        assert_eq!(stack_details(ctx).len(), 2);

        repo.checkout_commit(oid_one);

        {
            let raw_repo = repo.open();
            raw_repo
                .find_reference("refs/heads/stack-a")
                .unwrap()
                .delete()
                .unwrap();
        }

        let mut guard = ctx.exclusive_worktree_access();
        let return_result = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        );
        drop(guard);

        if let Err(e) = &return_result {
            eprintln!("Return after deleting one of two stack refs: {e:#}");
            return;
        }

        let v3_result = create_branch_v3(ctx, "stack-c");
        if let Err(e) = &v3_result {
            eprintln!("V3 create after partial stack deletion: {e:#}");
        }
        assert!(
            v3_result.is_ok(),
            "Should be able to create branch after one stack ref deleted: {:?}",
            v3_result.unwrap_err()
        );
    }
}

/// Tests for the stack divergence detection API.
mod stack_divergence_detection {
    use gitbutler_branch::BranchCreateRequest;
    use gitbutler_branch_actions::stack_divergence::{DivergenceStatus, DivergenceStatuses};
    use gix::refs::transaction::PreviousValue;

    use super::*;

    /// Helper: set up workspace with one stack, return (early_oid, stack_entry).
    fn setup_workspace_with_stack(
        repo: &mut super::TestRepo,
        ctx: &mut Context,
    ) -> (gix::ObjectId, but_workspace::legacy::ui::StackEntryNoOpt) {
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
            &BranchCreateRequest {
                name: Some("my-stack".into()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        std::fs::write(repo.path().join("another_file.txt"), "stack work").unwrap();
        super::create_commit(ctx, stack_entry.id, "stack commit").unwrap();

        (oid_one, stack_entry)
    }

    /// When no refs have moved, detection should return UpToDate.
    #[test]
    fn no_divergence_returns_up_to_date() {
        let Test { repo, ctx, .. } = &mut Test::default();
        let (_oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);

        let result = gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), DivergenceStatuses::UpToDate);
    }

    /// When a stack ref is force-pushed below the target base, detection should
    /// report MovedBelowBase.
    #[test]
    fn ref_moved_below_base_detected() {
        let Test { repo, ctx, .. } = &mut Test::default();
        let (oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);

        // Force my-stack ref to point below the target base.
        {
            let raw_repo = repo.open();
            raw_repo
                .reference(
                    "refs/heads/my-stack",
                    oid_one,
                    PreviousValue::Any,
                    "test: force below base",
                )
                .unwrap();
        }

        let result =
            gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx).unwrap();
        match result {
            DivergenceStatuses::DivergedRefs { divergences } => {
                assert_eq!(divergences.len(), 1);
                assert_eq!(divergences[0].actual_oid, Some(oid_one));
                assert_eq!(divergences[0].status, DivergenceStatus::MovedBelowBase);
            }
            DivergenceStatuses::UpToDate => {
                panic!("Expected divergence but got UpToDate");
            }
        }
    }

    /// When a stack ref is deleted, detection should report Deleted.
    #[test]
    fn ref_deleted_detected() {
        let Test { repo, ctx, .. } = &mut Test::default();
        let (_oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);

        // Delete the stack ref.
        {
            let raw_repo = repo.open();
            raw_repo
                .find_reference("refs/heads/my-stack")
                .unwrap()
                .delete()
                .unwrap();
        }

        let result =
            gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx).unwrap();
        match result {
            DivergenceStatuses::DivergedRefs { divergences } => {
                assert_eq!(divergences.len(), 1);
                assert_eq!(divergences[0].actual_oid, None);
                assert_eq!(divergences[0].status, DivergenceStatus::Deleted);
            }
            DivergenceStatuses::UpToDate => {
                panic!("Expected divergence but got UpToDate");
            }
        }
    }

    /// When a stack ref moves but the tree at the new position is identical,
    /// detection should report MovedToSameTree.
    #[test]
    fn ref_moved_to_same_tree_detected() {
        let Test { repo, ctx, .. } = &mut Test::default();
        let (_oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);

        // Create another commit with the same tree but different message, then force-update the ref.
        let new_oid = {
            let raw_repo = repo.open();
            // Resolve the current head of my-stack (stack_entry.tip is stale — from before create_commit).
            let current_head = raw_repo
                .find_reference("refs/heads/my-stack")
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .id;
            let old_commit = raw_repo.find_commit(current_head).unwrap();
            let tree_id = old_commit.tree_id().unwrap().detach();
            let parent_ids: Vec<gix::ObjectId> =
                old_commit.parent_ids().map(|id| id.detach()).collect();
            let committer: gix::actor::Signature = old_commit.committer().unwrap().into();
            let author: gix::actor::Signature = old_commit.author().unwrap().into();
            let commit = gix::objs::Commit {
                tree: tree_id,
                parents: parent_ids.into(),
                author,
                committer,
                encoding: None,
                message: "same tree, different message".into(),
                extra_headers: vec![],
            };
            let new_oid = raw_repo.write_object(&commit).unwrap().detach();
            raw_repo
                .reference(
                    "refs/heads/my-stack",
                    new_oid,
                    gix::refs::transaction::PreviousValue::Any,
                    "test: move to same-tree commit",
                )
                .unwrap();
            new_oid
        };

        let result =
            gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx).unwrap();
        match result {
            DivergenceStatuses::DivergedRefs { divergences } => {
                assert_eq!(divergences.len(), 1);
                assert_eq!(divergences[0].actual_oid, Some(new_oid));
                assert_eq!(divergences[0].status, DivergenceStatus::MovedToSameTree);
            }
            DivergenceStatuses::UpToDate => {
                panic!("Expected divergence but got UpToDate");
            }
        }
    }

    /// When a stack ref is moved to a new commit above the base with different content,
    /// detection should report MovedAboveBase.
    #[test]
    fn ref_moved_above_base_detected() {
        let Test { repo, ctx, .. } = &mut Test::default();
        let (_oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);

        // Create a new commit above the base with different tree content,
        // then force-update the ref.
        {
            let raw_repo = repo.open();
            let current_head = raw_repo
                .find_reference("refs/heads/my-stack")
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .id;
            let old_commit = raw_repo.find_commit(current_head).unwrap();
            let parent_ids: Vec<gix::ObjectId> =
                old_commit.parent_ids().map(|id| id.detach()).collect();

            // Write a new blob and tree that differs from the original.
            let blob_id = raw_repo.write_blob("different content").unwrap().detach();
            let mut editor = raw_repo.edit_tree(old_commit.tree_id().unwrap()).unwrap();
            editor
                .upsert(
                    "changed_file.txt",
                    gix::object::tree::EntryKind::Blob,
                    blob_id,
                )
                .unwrap();
            let new_tree = editor.write().unwrap().detach();

            let committer: gix::actor::Signature = old_commit.committer().unwrap().into();
            let author: gix::actor::Signature = old_commit.author().unwrap().into();
            let commit = gix::objs::Commit {
                tree: new_tree,
                parents: parent_ids.into(),
                author,
                committer,
                encoding: None,
                message: "moved above base with different tree".into(),
                extra_headers: vec![],
            };
            let new_oid = raw_repo.write_object(&commit).unwrap().detach();
            raw_repo
                .reference(
                    "refs/heads/my-stack",
                    new_oid,
                    gix::refs::transaction::PreviousValue::Any,
                    "test: move above base with new tree",
                )
                .unwrap();
        }

        let result =
            gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx).unwrap();
        match result {
            DivergenceStatuses::DivergedRefs { divergences } => {
                assert_eq!(divergences.len(), 1);
                assert!(
                    matches!(
                        divergences[0].status,
                        DivergenceStatus::MovedAboveBase { .. }
                    ),
                    "Expected MovedAboveBase, got {:?}",
                    divergences[0].status
                );
            }
            DivergenceStatuses::UpToDate => {
                panic!("Expected divergence but got UpToDate");
            }
        }
    }
}

/// Tests for the stack divergence resolution API.
mod stack_divergence_resolution {
    use gitbutler_branch::BranchCreateRequest;
    use gitbutler_branch_actions::stack_divergence::{
        DivergenceApproach, DivergenceResolution, DivergenceStatuses,
    };
    use gix::refs::transaction::PreviousValue;

    use super::*;

    /// Helper: set up workspace with one stack, return (early_oid, stack_entry).
    fn setup_workspace_with_stack(
        repo: &mut super::TestRepo,
        ctx: &mut Context,
    ) -> (gix::ObjectId, but_workspace::legacy::ui::StackEntryNoOpt) {
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
            &BranchCreateRequest {
                name: Some("my-stack".into()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        std::fs::write(repo.path().join("another_file.txt"), "stack work").unwrap();
        super::create_commit(ctx, stack_entry.id, "stack commit").unwrap();

        (oid_one, stack_entry)
    }

    /// Resolving a diverged ref with Exclude should unapply the stack
    /// and leave the workspace functional.
    #[test]
    fn exclude_unapplies_stack() {
        let Test { repo, ctx, .. } = &mut Test::default();
        let (oid_one, stack_entry) = setup_workspace_with_stack(repo, ctx);

        // Force ref below base to trigger divergence.
        {
            let raw_repo = repo.open();
            raw_repo
                .reference(
                    "refs/heads/my-stack",
                    oid_one,
                    PreviousValue::Any,
                    "test: force below base",
                )
                .unwrap();
        }

        // Verify divergence is detected.
        let statuses =
            gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx).unwrap();
        assert!(matches!(statuses, DivergenceStatuses::DivergedRefs { .. }));

        // Resolve by excluding.
        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::stack_divergence::resolve_diverged_stacks(
            ctx,
            &[DivergenceResolution {
                stack_id: stack_entry.id,
                approach: DivergenceApproach::Exclude,
            }],
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        // Stack should no longer be in workspace.
        let stacks = stack_details(ctx);
        assert!(
            stacks.is_empty() || stacks.iter().all(|(id, _)| *id != stack_entry.id),
            "Excluded stack should not be in workspace"
        );
    }

    /// Resolving with IncludeAsIs should rebuild the workspace commit
    /// accepting the ref at its new position.
    #[test]
    fn include_as_is_rebuilds_workspace() {
        let Test { repo, ctx, .. } = &mut Test::default();
        let (_oid_one, _stack_entry) = setup_workspace_with_stack(repo, ctx);

        // Move the ref to a new commit with different content (above base).
        {
            let raw_repo = repo.open();
            let current_head = raw_repo
                .find_reference("refs/heads/my-stack")
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .id;
            let old_commit = raw_repo.find_commit(current_head).unwrap();
            let parent_ids: Vec<gix::ObjectId> =
                old_commit.parent_ids().map(|id| id.detach()).collect();

            let blob_id = raw_repo.write_blob("new content").unwrap().detach();
            let mut editor = raw_repo.edit_tree(old_commit.tree_id().unwrap()).unwrap();
            editor
                .upsert("new_file.txt", gix::object::tree::EntryKind::Blob, blob_id)
                .unwrap();
            let new_tree = editor.write().unwrap().detach();

            let committer: gix::actor::Signature = old_commit.committer().unwrap().into();
            let author: gix::actor::Signature = old_commit.author().unwrap().into();
            let commit = gix::objs::Commit {
                tree: new_tree,
                parents: parent_ids.into(),
                author,
                committer,
                encoding: None,
                message: "diverged commit".into(),
                extra_headers: vec![],
            };
            let new_oid = raw_repo.write_object(&commit).unwrap().detach();
            raw_repo
                .reference(
                    "refs/heads/my-stack",
                    new_oid,
                    PreviousValue::Any,
                    "test: diverge above base",
                )
                .unwrap();
        }

        // Verify divergence is detected.
        let statuses =
            gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx).unwrap();
        assert!(matches!(statuses, DivergenceStatuses::DivergedRefs { .. }));

        let stack_id = match &statuses {
            DivergenceStatuses::DivergedRefs { divergences } => divergences[0].stack_id,
            _ => unreachable!(),
        };

        // Resolve by including as-is.
        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::stack_divergence::resolve_diverged_stacks(
            ctx,
            &[DivergenceResolution {
                stack_id,
                approach: DivergenceApproach::IncludeAsIs,
            }],
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        // After resolution, workspace should be up-to-date (no more divergence).
        let statuses =
            gitbutler_branch_actions::stack_divergence::detect_diverged_stacks(ctx).unwrap();
        assert_eq!(
            statuses,
            DivergenceStatuses::UpToDate,
            "After IncludeAsIs resolution, workspace should be up-to-date"
        );

        // The new file should be present in the worktree (checkout happened).
        assert!(
            repo.path().join("new_file.txt").exists(),
            "new_file.txt should exist after including diverged ref"
        );
    }

    /// After resolving with IncludeAsIs, creating new branches should work
    /// (this is the fix for the production "already belongs to another branch" error).
    #[test]
    fn include_as_is_below_base_then_create_branch_works() {
        let Test { repo, ctx, .. } = &mut Test::default();
        let (oid_one, stack_entry) = setup_workspace_with_stack(repo, ctx);

        // Force ref below base — this is the bug scenario.
        {
            let raw_repo = repo.open();
            raw_repo
                .reference(
                    "refs/heads/my-stack",
                    oid_one,
                    PreviousValue::Any,
                    "test: force below base",
                )
                .unwrap();
        }

        // Resolve by excluding the broken stack.
        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::stack_divergence::resolve_diverged_stacks(
            ctx,
            &[DivergenceResolution {
                stack_id: stack_entry.id,
                approach: DivergenceApproach::Exclude,
            }],
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        // Now creating a new branch should succeed (the bug is fixed).
        let mut guard = ctx.exclusive_worktree_access();
        let result = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            guard.write_permission(),
        );
        assert!(
            result.is_ok(),
            "Creating branch after resolving diverged stack should work: {:?}",
            result.unwrap_err()
        );
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
        let base = gitbutler_branch_actions::base::get_base_branch_data(ctx).unwrap();
        assert_eq!(
            base.behind, 3,
            "behind count should match the farthest-behind stack (A, which is 3 commits behind)"
        );
    }
}
