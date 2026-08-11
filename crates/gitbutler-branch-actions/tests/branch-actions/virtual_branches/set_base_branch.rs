use super::*;
use but_core::RefMetadata as _;
use gitbutler_oplog::OplogExt as _;

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
fn reconfiguring_base_branch_records_snapshot() {
    let Test { ctx, .. } = &mut Test::default();
    let target = "refs/remotes/origin/master".parse().unwrap();
    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(ctx, &target, guard.write_permission()).unwrap();
    gitbutler_branch_actions::set_base_branch(ctx, &target, guard.write_permission()).unwrap();
    drop(guard);

    assert_eq!(
        ctx.snapshots_iter(None, Vec::new(), None).unwrap().count(),
        1,
        "the first call initializes the project without a snapshot because there is no target yet; the second call snapshots that initialized state before reconfiguration"
    );
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
fn switching_the_target_outside_the_workspace_does_not_partially_update_the_project() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let gix_repo = repo.open();
    {
        let head_id = gix_repo.head_id().unwrap();
        gix_repo
            .reference(
                "refs/remotes/origin/other",
                head_id,
                gix::refs::transaction::PreviousValue::Any,
                "test",
            )
            .unwrap();
    }
    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    std::fs::write(repo.path().join("feature.txt"), "feature").unwrap();
    repo.commit_all("feature");
    repo.checkout(&"refs/heads/master".parse().unwrap());

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    repo.checkout(&"refs/heads/some-feature".parse().unwrap());

    let project_meta_before = ctx.project_meta().unwrap();
    let workspace_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into().unwrap();
    let workspace_meta_before = (*ctx
        .meta()
        .unwrap()
        .workspace(workspace_ref.as_ref())
        .unwrap())
    .clone();
    let workspace_ref_before = gix_repo
        .find_reference(&workspace_ref)
        .unwrap()
        .peel_to_id()
        .unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let err = gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/other".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap_err();
    drop(guard);

    assert_eq!(
        err.custom_context().map(|ctx| ctx.code),
        Some(Code::PreconditionFailed),
        "changing targets outside the managed workspace is an unsupported project state"
    );
    assert_eq!(
        err.to_string(),
        "cannot change the target while HEAD is outside the GitButler workspace - return to workspace first",
        "the error explains how to satisfy the target-switch precondition"
    );
    assert_eq!(
        ctx.project_meta().unwrap(),
        project_meta_before,
        "rejecting the target switch must preserve the configured project target"
    );
    assert_eq!(
        *ctx.meta()
            .unwrap()
            .workspace(workspace_ref.as_ref())
            .unwrap(),
        workspace_meta_before,
        "rejecting the target switch must preserve stack metadata"
    );
    assert_eq!(
        gix_repo
            .find_reference(&workspace_ref)
            .unwrap()
            .peel_to_id()
            .unwrap(),
        workspace_ref_before,
        "rejecting the target switch must preserve the existing workspace ref"
    );
}

#[test]
fn switching_a_missing_target_outside_the_workspace_is_rejected() {
    let Test { repo, ctx, .. } = &mut Test::default();

    let gix_repo = repo.open();
    let head_id = gix_repo.head_id().unwrap();
    gix_repo
        .reference(
            "refs/remotes/origin/other",
            head_id,
            gix::refs::transaction::PreviousValue::Any,
            "test",
        )
        .unwrap();
    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    std::fs::write(repo.path().join("feature.txt"), "feature").unwrap();
    repo.commit_all("feature");
    repo.checkout(&"refs/heads/master".parse().unwrap());

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Here is the key - the target we try to set later is deleted.
    gix_repo
        .find_reference("refs/remotes/origin/master")
        .unwrap()
        .delete()
        .unwrap();
    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    let workspace_ref_before = gix_repo
        .find_reference(but_core::WORKSPACE_REF_NAME)
        .unwrap()
        .peel_to_id()
        .unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    let err = gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/other".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap_err();
    drop(guard);

    assert_eq!(
        err.custom_context().map(|ctx| ctx.code),
        Some(Code::PreconditionFailed),
        "a repaired missing target must still enforce the target-switch precondition"
    );
    assert!(
        ctx.project_meta().unwrap().target_ref.is_none(),
        "the replacement target must not be persisted"
    );
    assert!(
        stack_details(ctx).is_empty(),
        "the checked-out branch must not be added to workspace metadata"
    );
    assert_eq!(
        gix_repo
            .find_reference(but_core::WORKSPACE_REF_NAME)
            .unwrap()
            .peel_to_id()
            .unwrap(),
        workspace_ref_before,
        "the existing workspace ref must remain unchanged"
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

    #[test]
    fn missing_remote_url_does_not_mutate_project() {
        let Test { repo, ctx, .. } = &mut Test::default();
        but_core::git_config::edit_repo_config(
            &repo.open(),
            gix::config::Source::Local,
            |config| but_core::git_config::remove_config_value(config, "remote.origin.url"),
        )
        .unwrap();
        ctx.repo.get_mut().unwrap().reload().unwrap();
        let before = ctx.project_meta().unwrap();

        let mut guard = ctx.exclusive_worktree_access();
        let error = gitbutler_branch_actions::set_base_branch(
            ctx,
            &RemoteRefname::from_str("refs/remotes/origin/master").unwrap(),
            guard.write_permission(),
        )
        .unwrap_err();
        drop(guard);

        let message = format!("{error:#}");
        assert!(
            message.contains("failed to get fetch url for remote origin"),
            "{message}"
        );
        assert_eq!(ctx.project_meta().unwrap(), before);
        assert!(stack_details(ctx).is_empty());
    }
}

mod go_back_to_workspace {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn preserves_applied_vbranches() {
        let Test { repo, ctx, .. } = &mut Test::default();

        std::fs::write(repo.path().join("file.txt"), "one").unwrap();
        let oid_one = repo.commit_all("one");
        std::fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("two");
        repo.push();

        repo.checkout(&"refs/heads/some-feature".parse().unwrap());
        std::fs::write(repo.path().join("another file.txt"), "content").unwrap();
        repo.commit_all("feature");

        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        let stack_id = stack_details(ctx)[0].0;
        repo.checkout_commit(oid_one);

        let mut guard = ctx.exclusive_worktree_access();
        gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1, "the applied stack is preserved");
        assert_eq!(stacks[0].0, stack_id, "the preserved stack keeps its id");
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
        let err = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap_err();
        // Going back to the workspace aborts up-front rather than leaving the project conflicted.
        assert_eq!(
            err.custom_context().map(|ctx| ctx.code),
            Some(Code::PreconditionFailed)
        );
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
        let err = gitbutler_branch_actions::set_base_branch(
            ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            guard.write_permission(),
        )
        .unwrap_err();
        // Going back to the workspace aborts up-front rather than leaving the project conflicted.
        assert_eq!(
            err.custom_context().map(|ctx| ctx.code),
            Some(Code::PreconditionFailed)
        );
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
        let mut meta = ctx.meta().unwrap();
        let (repo, mut workspace, _) = ctx
            .workspace_mut_and_db_with_perm(guard.write_permission())
            .unwrap();
        let outcome = but_workspace::branch::apply(
            "refs/heads/C".try_into().unwrap(),
            workspace.clone(),
            &repo,
            &mut meta,
            but_workspace::branch::apply::Options::default(),
        )
        .unwrap();
        assert!(
            outcome.status.persisted_mutation(),
            "branch C must be applied for the multi-stack behind-count scenario"
        );
        *workspace = outcome.workspace;
        drop(workspace);
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
