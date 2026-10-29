use std::fs;

use anyhow::Context as _;
use but_core::{GitConfigSettings, RepositoryExt as _};
use but_ctx::Context;
use but_testsupport::Sandbox;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gitbutler_oplog::{OplogExt, RestoreKind};
use gix::bstr::ByteSlice as _;

#[test]
fn restore_ad_hoc_refs_and_redo_refs_no_longer_reachable_from_head() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    repo.invoke_bash(
        r#"
        git checkout main
        git update-ref -d refs/heads/gitbutler/workspace
        git checkout -b bottom
        git commit --allow-empty -m bottom
        git checkout -b middle
        git commit --allow-empty -m middle
        git checkout -b top
        git commit --allow-empty -m top
    "#,
    );
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    ctx.reload_repo_and_invalidate_workspace(perm)?;
    let snapshot =
        ctx.create_snapshot(SnapshotDetails::new(OperationKind::OnDemandSnapshot), perm)?;

    // Split both lower refs away from HEAD, as failed transaction materialization can do.
    let new_bottom =
        repo.invoke_git("commit-tree origin/main^{tree} -p origin/main -m rewritten-bottom");
    let new_middle = repo.invoke_git(&format!(
        "commit-tree origin/main^{{tree}} -p {new_bottom} -m rewritten-middle"
    ));
    repo.invoke_git(&format!("update-ref refs/heads/bottom {new_bottom}"));
    repo.invoke_git(&format!("update-ref refs/heads/middle {new_middle}"));
    ctx.reload_repo_and_invalidate_workspace(perm)?;
    let undo = ctx.restore_snapshot(snapshot, RestoreKind::RestoreFromSnapshotViaUndo, perm)?;

    // Restore must recover the entire original stack, not merely top's checkout identity.
    snapbox::assert_data_eq!(
        repo.git_log(),
        snapbox::str![[r#"
* 931a0f6 (HEAD -> top) top
* 43b38f2 (middle) middle
* 4a2b5dc (bottom) bottom
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );

    ctx.reload_repo_and_invalidate_workspace(perm)?;
    ctx.restore_snapshot(undo, RestoreKind::RestoreFromSnapshotViaRedo, perm)?;
    // The before-restore snapshot must preserve bottom and middle even outside HEAD's graph.
    snapbox::assert_data_eq!(
        repo.git_log(),
        snapbox::str![[r#"
* fa9408d (middle) rewritten-middle
* c2f5013 (bottom) rewritten-bottom
| * 931a0f6 (HEAD -> top) top
| * 43b38f2 middle
| * 4a2b5dc bottom
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
    Ok(())
}

#[test]
fn restore_legacy_single_additional_ref() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let reference: gix::refs::FullName = "refs/heads/extra".try_into()?;
    repo.invoke_git("branch extra A");
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let tree_id = ctx.prepare_snapshot_with_refs(&[reference.as_ref()], perm.read_permission())?;
    let legacy_tree = {
        let git = repo.open_repo();
        let tree = git.find_tree(tree_id)?;
        let entry = tree
            .lookup_entry_by_path("additional-refs/0")?
            .expect("the explicit ref is captured");
        let mut editor = tree.edit()?;
        editor.upsert(
            "additional-ref",
            gix::objs::tree::EntryKind::Tree,
            entry.id(),
        )?;
        editor.remove("additional-refs")?;
        editor.write()?.detach()
    };
    let snapshot = ctx.commit_snapshot(
        legacy_tree,
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        perm,
    )?;
    repo.invoke_git("update-ref refs/heads/extra origin/main");
    ctx.restore_snapshot(snapshot, RestoreKind::RestoreFromSnapshotViaUndo, perm)?;

    // The old singular additional-ref encoding still restores the saved branch tip.
    snapbox::assert_data_eq!(
        repo.invoke_git("rev-parse extra A"),
        snapbox::str![[r#"
4bed59b96e6cdb81015812875e82c36c1ff20b68
4bed59b96e6cdb81015812875e82c36c1ff20b68
"#]]
    );
    Ok(())
}

#[test]
fn restore_additional_refs_rejects_other_worktrees() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let reference: gix::refs::FullName = "refs/heads/extra".try_into()?;
    repo.invoke_git("branch extra A");
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let tree = ctx.prepare_snapshot_with_refs(&[reference.as_ref()], perm.read_permission())?;
    let snapshot = ctx.commit_snapshot(
        tree,
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        perm,
    )?;
    repo.invoke_git("update-ref refs/heads/extra origin/main");
    let worktree = tempfile::tempdir()?;
    repo.invoke_git(&format!(
        "worktree add '{}' extra",
        worktree.path().display()
    ));

    let error = ctx
        .restore_snapshot(snapshot, RestoreKind::RestoreFromSnapshotViaUndo, perm)
        .unwrap_err();
    // Restoring our checkout is allowed, but moving a ref checked out elsewhere is not.
    snapbox::assert_data_eq!(
        error.to_string(),
        snapbox::str![[
            r#"Cannot restore branch 'extra' because it is checked out in worktrees: [..]"#
        ]]
    );
    // The rejected restore leaves the other worktree's branch at its current target.
    snapbox::assert_data_eq!(
        repo.invoke_git("rev-parse extra origin/main"),
        snapbox::str![[r#"
0dc37334a458df421bf67ea806103bf5004845dd
0dc37334a458df421bf67ea806103bf5004845dd
"#]]
    );
    Ok(())
}

#[test]
fn redo_rejects_a_ref_checked_out_elsewhere_only_after_the_snapshot() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let reference: gix::refs::FullName = "refs/heads/extra".try_into()?;
    repo.invoke_git("branch extra A");
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let tree = ctx.prepare_snapshot_with_refs(&[reference.as_ref()], perm.read_permission())?;
    let snapshot = ctx.commit_snapshot(
        tree,
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        perm,
    )?;
    let worktree = tempfile::tempdir()?;
    repo.invoke_git(&format!(
        "worktree add '{}' extra",
        worktree.path().display()
    ));
    // `extra` didn't move, so undo has nothing to refuse.
    let redo = ctx.restore_snapshot(snapshot, RestoreKind::RestoreFromSnapshotViaUndo, perm)?;
    repo.invoke_git("update-ref refs/heads/extra origin/main");

    let error = ctx
        .restore_snapshot(redo, RestoreKind::RestoreFromSnapshotViaRedo, perm)
        .unwrap_err();
    // The worktree checked `extra` out after the first snapshot was taken, so redo
    // must not move it either, although it had it checked out when undo ran.
    snapbox::assert_data_eq!(
        error.to_string(),
        snapbox::str![[
            r#"Cannot restore branch 'extra' because it is checked out in worktrees: [..]"#
        ]]
    );
    Ok(())
}

#[test]
fn snapshot_with_additional_ref_includes_branch_order() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    set_branch_order(ctx, &["refs/heads/A", "refs/heads/B"])?;
    let additional_ref: gix::refs::FullName = "refs/heads/A".try_into()?;
    let mut guard = ctx.exclusive_worktree_access();
    let tree_id =
        ctx.prepare_snapshot_with_refs(&[additional_ref.as_ref()], guard.read_permission())?;
    let snapshot_id = ctx.commit_snapshot(
        tree_id,
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    assert!(
        snapshot_blob(&repo.open_repo(), snapshot_id, "branch_order.toml")?
            .contains("refs/heads/A"),
        "the additional-ref snapshot path should include branch-order metadata"
    );
    Ok(())
}

#[test]
fn restore_snapshot_replaces_branch_order() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    set_branch_order(ctx, &["refs/heads/A", "refs/heads/B"])?;
    let expected = ctx.db.get_cache()?.branch_order().get_snapshot()?;
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    assert!(
        snapshot_blob(&repo.open_repo(), snapshot_id, "branch_order.toml")?
            .contains("refs/heads/A"),
        "the oplog snapshot should contain branch-order metadata"
    );

    set_branch_order(ctx, &["refs/heads/C", "refs/heads/D"])?;
    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert_eq!(
        ctx.db.get_cache()?.branch_order().get_snapshot()?,
        expected,
        "restoring should replace the complete branch-order table"
    );
    Ok(())
}

#[test]
fn restore_snapshot_restores_explicitly_empty_branch_order() -> anyhow::Result<()> {
    let Test { ctx, .. } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    set_branch_order(ctx, &["refs/heads/A", "refs/heads/B"])?;

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert!(
        ctx.db
            .get_cache()?
            .branch_order()
            .get_snapshot()?
            .entries
            .is_empty(),
        "an explicitly empty snapshot should clear branch order"
    );
    Ok(())
}

#[test]
fn restore_legacy_snapshot_preserves_current_branch_order() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let legacy_snapshot_id = snapshot_without_branch_order(&repo.open_repo(), snapshot_id)?;
    set_branch_order(ctx, &["refs/heads/A", "refs/heads/B"])?;
    let expected = ctx.db.get_cache()?.branch_order().get_snapshot()?;

    ctx.restore_snapshot(
        legacy_snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert_eq!(
        ctx.db.get_cache()?.branch_order().get_snapshot()?,
        expected,
        "a snapshot without branch-order data should preserve current metadata"
    );
    Ok(())
}

#[test]
fn malformed_branch_order_fails_before_restore_mutates_state() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    set_branch_order(ctx, &["refs/heads/A", "refs/heads/B"])?;
    let expected = ctx.db.get_cache()?.branch_order().get_snapshot()?;
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let malformed_snapshot_id =
        snapshot_with_branch_order(&repo.open_repo(), snapshot_id, b"not valid = [")?;
    let oplog_head = ctx.oplog_head()?;

    let error = ctx
        .restore_snapshot(
            malformed_snapshot_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )
        .expect_err("malformed branch-order metadata must fail restore");

    assert!(
        error
            .to_string()
            .contains("failed to parse branch_order.toml"),
        "restore should identify malformed branch-order data: {error:#}"
    );
    assert_eq!(
        ctx.db.get_cache()?.branch_order().get_snapshot()?,
        expected,
        "failed validation should leave branch order unchanged"
    );
    assert_eq!(
        ctx.oplog_head()?,
        oplog_head,
        "failed validation should not advance the oplog"
    );
    Ok(())
}

#[test]
fn invalid_branch_order_fails_before_restore_mutates_worktree() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let invalid_snapshot_id = snapshot_with_branch_order(
        &repo.open_repo(),
        snapshot_id,
        br#"
[[entries]]
branch_ref_name = "refs/heads/A"
parent_ref_name = "refs/heads/B"

[[entries]]
branch_ref_name = "refs/heads/C"
parent_ref_name = "refs/heads/B"
"#,
    )?;
    let changed_path = repo.projects_root().join("first");
    fs::write(&changed_path, "changed after snapshot")?;

    let error = ctx
        .restore_snapshot(
            invalid_snapshot_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )
        .expect_err("invalid branch-order metadata must fail restore");

    assert!(
        error.to_string().contains("invalid branch_order.toml"),
        "restore should identify invalid branch-order data: {error:#}"
    );
    assert_eq!(
        fs::read_to_string(changed_path)?,
        "changed after snapshot",
        "validation should happen before the worktree is restored"
    );
    Ok(())
}

#[test]
fn restore_snapshot_reverts_the_target() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    // A second remote branch to switch to.
    {
        let gix_repo = repo.open_repo();
        let head_id = gix_repo.head_id()?.detach();
        gix_repo.reference(
            "refs/heads/other",
            head_id,
            gix::refs::transaction::PreviousValue::Any,
            "test",
        )?;
    }

    configure_default_target(ctx)?;
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    let mut changed = ctx.project_meta()?;
    changed.target_ref = Some(but_core::ref_metadata::LocalRefName::new(
        "refs/heads/other".try_into()?,
    )?);
    ctx.set_project_meta(changed)?;
    assert_eq!(
        ctx.project_meta()?.target_ref.map(|name| name.to_string()),
        Some("refs/heads/other".to_string()),
        "the target change is visible before restoring the snapshot"
    );

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        ctx.project_meta()?.target_ref.map(|name| name.to_string()),
        Some("refs/heads/main".to_string()),
        "undoing a base-branch switch reverts the target everywhere, not just in the TOML"
    );
    Ok(())
}

#[test]
fn snapshot_creation_works_with_unmerged_index() -> anyhow::Result<()> {
    use gix::index::entry::{Flags, Mode, Stage};

    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;

    // Simulate what a workspace update leaves behind when an uncommitted file
    // conflicts: conflict markers in the worktree and unmerged entries in the index.
    // `deleted.txt` has no 'ours' stage, as when the local side deleted the file.
    let (base_blob, ours_blob, theirs_blob) = {
        let repo = ctx.repo.get()?;
        let ours_blob = repo.write_blob(b"ours\n")?.detach();
        let base_blob = repo.write_blob(b"base\n")?.detach();
        let theirs_blob = repo.write_blob(b"theirs\n")?.detach();
        let mut index = repo.open_index()?;
        for (path, stage, blob) in [
            ("conflicted.txt", Stage::Base, base_blob),
            ("conflicted.txt", Stage::Ours, ours_blob),
            ("conflicted.txt", Stage::Theirs, theirs_blob),
            ("deleted.txt", Stage::Base, base_blob),
            ("deleted.txt", Stage::Theirs, theirs_blob),
            ("df", Stage::Ours, ours_blob),
            ("df/child", Stage::Theirs, theirs_blob),
        ] {
            index.dangerously_push_entry(
                Default::default(),
                blob,
                Flags::from_stage(stage),
                Mode::FILE,
                path.into(),
            );
        }
        #[cfg(unix)]
        for (stage, blob) in [
            (Stage::Base, base_blob),
            (Stage::Ours, ours_blob),
            (Stage::Theirs, theirs_blob),
        ] {
            index.dangerously_push_entry(
                Default::default(),
                blob,
                Flags::from_stage(stage),
                Mode::FILE,
                b"invalid-\xff.txt".as_bstr(),
            );
        }
        index.sort_entries();
        index.write(Default::default())?;
        (base_blob, ours_blob, theirs_blob)
    };
    fs::write(
        repo.projects_root().join("conflicted.txt"),
        "<<<<<<< ours\nours\n||||||| base\nbase\n=======\ntheirs\n>>>>>>> theirs\n",
    )?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    // Undo must also work while the currently checked-out index remains unresolved.
    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    {
        let repo = ctx.repo.get()?;
        let mut index = repo.open_index()?;
        index.remove_entries(|_, _, _| true);
        index.write(Default::default())?;
        assert!(
            index.entries().is_empty(),
            "the conflict is cleared before restore"
        );
    }
    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    let index = ctx.repo.get()?.open_index()?;
    for (path, stage, expected) in [
        ("conflicted.txt", Stage::Base, base_blob),
        ("conflicted.txt", Stage::Ours, ours_blob),
        ("conflicted.txt", Stage::Theirs, theirs_blob),
        ("deleted.txt", Stage::Base, base_blob),
        ("deleted.txt", Stage::Theirs, theirs_blob),
        ("df", Stage::Ours, ours_blob),
        ("df/child", Stage::Theirs, theirs_blob),
    ] {
        assert_eq!(
            index
                .entry_by_path_and_stage(path.into(), stage)
                .map(|entry| entry.id),
            Some(expected),
            "restore preserves stage {stage:?} for {path}"
        );
    }
    assert!(
        index
            .entry_by_path_and_stage("deleted.txt".into(), Stage::Ours)
            .is_none(),
        "restore preserves the missing ours stage for a local deletion"
    );
    #[cfg(unix)]
    for (stage, expected) in [
        (Stage::Base, base_blob),
        (Stage::Ours, ours_blob),
        (Stage::Theirs, theirs_blob),
    ] {
        assert_eq!(
            index
                .entry_by_path_and_stage(b"invalid-\xff.txt".as_bstr(), stage)
                .map(|entry| entry.id),
            Some(expected),
            "restore preserves non-UTF-8 conflict paths at stage {stage:?}"
        );
    }
    Ok(())
}

#[test]
fn snapshot_has_authoritative_meta_and_omits_legacy_target() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let mut expected = ctx.project_meta()?;
    expected.push_remote = Some("origin".to_owned());
    ctx.set_project_meta(expected.clone())?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    let repo = repo.open_repo();
    let project_meta = snapshot_blob(&repo, snapshot_id, "project_meta.toml")?;
    let virtual_branches = snapshot_blob(&repo, snapshot_id, "virtual_branches.toml")?;

    assert!(
        project_meta.contains(&format!(
            "targetRef = \"{}\"",
            expected.target_ref.as_ref().unwrap()
        )),
        "the authoritative snapshot metadata stores the target ref"
    );
    assert!(
        project_meta.contains(&format!(
            "targetCommitId = \"{}\"",
            expected.target_commit_id.unwrap()
        )),
        "the authoritative snapshot metadata stores the target commit"
    );
    assert!(
        project_meta.contains("pushRemote = \"origin\""),
        "the authoritative snapshot metadata stores the push remote"
    );
    assert!(
        !virtual_branches.contains("[default_target]"),
        "the snapshot TOML omits the legacy target"
    );
    Ok(())
}

#[test]
fn restore_unset_cutoff_after_upstream_moves_and_redo() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    repo.invoke_bash(
        r#"
        git config branch.main.remote origin
        git config branch.main.merge refs/heads/main
        "#,
    );
    let mut original = ctx.project_meta()?;
    original.target_commit_id = None;
    ctx.set_project_meta(original.clone())?;
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    ctx.reload_repo_and_invalidate_workspace(perm)?;
    let snapshot =
        ctx.create_snapshot(SnapshotDetails::new(OperationKind::OnDemandSnapshot), perm)?;
    let captured = repo
        .open_repo()
        .find_commit(snapshot)?
        .tree()?
        .lookup_entry_by_path("target_tree")?
        .context("snapshot has a target tree")?
        .id()
        .detach();

    repo.invoke_bash(
        r#"
        git update-ref refs/remotes/origin/main "$(git commit-tree "$(git mktree </dev/null)" -p origin/main -m upstream-moved)"
        echo changed > after-snapshot.txt
        "#,
    );
    ctx.reload_repo_and_invalidate_workspace(perm)?;
    let mut changed = original.clone();
    let upstream = repo
        .open_repo()
        .find_reference("refs/remotes/origin/main")?
        .peel_to_commit()?
        .id;
    changed.target_commit_id = Some(upstream);
    ctx.set_project_meta(changed.clone())?;

    let undo = ctx.restore_snapshot(snapshot, RestoreKind::RestoreFromSnapshotViaUndo, perm)?;
    assert_eq!(
        ctx.project_meta()?,
        original,
        "undo restores an unset cutoff, not the captured or current upstream tip"
    );
    let repo_view = repo.open_repo();
    assert!(
        !repo_view
            .workdir()
            .unwrap()
            .join("after-snapshot.txt")
            .exists(),
        "undo restores captured worktree content"
    );
    assert_ne!(
        captured,
        repo_view.find_commit(upstream)?.tree_id()?.detach(),
        "the upstream tree changed after capture"
    );
    assert_eq!(
        repo_view
            .find_reference("refs/remotes/origin/main")?
            .peel_to_commit()?
            .id,
        upstream,
        "undo does not rewind the upstream"
    );

    ctx.reload_repo_and_invalidate_workspace(perm)?;
    ctx.restore_snapshot(undo, RestoreKind::RestoreFromSnapshotViaRedo, perm)?;
    assert_eq!(
        ctx.project_meta()?,
        changed,
        "redo restores the explicit cutoff from before undo"
    );
    assert_eq!(
        fs::read_to_string(repo_view.workdir().unwrap().join("after-snapshot.txt"))?,
        "changed\n",
        "redo restores the worktree from before undo"
    );
    Ok(())
}

#[test]
fn snapshot_with_only_a_target_commit_omits_target_ref() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let mut target_commit_only = ctx.project_meta()?;
    target_commit_only.target_ref = None;
    let target_commit_id = target_commit_only.target_commit_id.unwrap();
    ctx.set_project_meta(target_commit_only)?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let repo = repo.open_repo();
    let project_meta = snapshot_blob(&repo, snapshot_id, "project_meta.toml")?;

    assert!(
        !project_meta.contains("targetRef"),
        "an absent target ref remains absent in the authoritative metadata"
    );
    assert!(
        project_meta.contains(&format!("targetCommitId = \"{target_commit_id}\"")),
        "the target commit remains in the authoritative metadata"
    );
    Ok(())
}

#[test]
fn restore_falls_back_to_the_legacy_target_in_old_snapshots() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    repo.invoke_git("branch --set-upstream-to=origin/main main");
    let mut guard = ctx.exclusive_worktree_access();
    ctx.reload_repo_and_invalidate_workspace(guard.write_permission())?;
    configure_default_target(ctx)?;
    let original = ctx.project_meta()?;
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let old_snapshot_id = snapshot_as_legacy(
        &repo.open_repo(),
        snapshot_id,
        original.target_commit_id.unwrap(),
    )?;

    let mut changed = original.clone();
    changed.target_ref = None;
    ctx.set_project_meta(changed)?;
    ctx.restore_snapshot(
        old_snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert_eq!(
        ctx.project_meta()?,
        original,
        "legacy snapshot metadata restores the original target"
    );
    Ok(())
}

#[test]
fn restore_reverts_a_target_commit_only_change() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let original = ctx.project_meta()?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let repo = repo.open_repo();
    let original_commit = repo.find_commit(original.target_commit_id.unwrap())?;
    let alternate_target = repo
        .write_object(gix::objs::Commit {
            message: "alternate target".into(),
            parents: [original_commit.id].into(),
            ..original_commit.decode()?.to_owned()?
        })?
        .detach();
    let mut changed = original.clone();
    changed.target_commit_id = Some(alternate_target);
    ctx.set_project_meta(changed)?;

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        ctx.project_meta()?,
        original,
        "restoring the snapshot reverts a target-commit-only change"
    );
    Ok(())
}

#[test]
fn malformed_project_meta_fails_before_restore_mutates_state() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let malformed_snapshot_id = snapshot_with_project_meta(
        &repo.open_repo(),
        snapshot_id,
        b"targetCommitId = 'not-an-object-id'\n",
    )?;
    let before_meta = ctx.project_meta()?;
    let before_oplog_head = ctx.oplog_head()?;
    let live_path = ctx.project_data_dir().join("virtual_branches.toml");
    let before_toml = fs::read(&live_path)?;

    let error = ctx
        .restore_snapshot(
            malformed_snapshot_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )
        .expect_err("malformed authoritative metadata must fail restore");
    assert!(
        error.to_string().contains("invalid targetCommitId"),
        "restore reports the malformed authoritative field: {error:#}"
    );
    assert_eq!(
        ctx.project_meta()?,
        before_meta,
        "malformed snapshot metadata does not change project metadata"
    );
    assert_eq!(
        ctx.oplog_head()?,
        before_oplog_head,
        "a failed restore does not advance the oplog head"
    );
    assert_eq!(
        fs::read(live_path)?,
        before_toml,
        "a failed restore does not rewrite virtual_branches.toml"
    );
    Ok(())
}

#[test]
fn oplog_snapshots_ignore_commit_signing_configuration() -> anyhow::Result<()> {
    let Test { ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    enable_failing_commit_signing(ctx)?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let repo = ctx.repo.get()?;

    assert!(
        !has_signature(&repo, snapshot_id)?,
        "oplog snapshots must stay unsigned even when user commit signing is enabled"
    );
    Ok(())
}

#[test]
fn head_corrupt_is_recreated_automatically() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    let mut guard = ctx.exclusive_worktree_access();
    ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(snapshots.len(), 1, "the baseline creates one snapshot");

    // overwrite oplog head with a non-commit sha
    let oplog_path = ctx.project_data_dir().join("operations-log.toml");
    fs::write(
        oplog_path,
        "head_sha = \"758d54f587227fba3da3b61fbb54a99c17903d59\"",
    )?;

    fs::write(repo.projects_root().join("changed.txt"), "changed")?;
    let replacement = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(
        snapshots.len(),
        1,
        "it should have just reset the oplog head, so only 1, not 2"
    );
    assert_eq!(
        snapshots[0].commit_id, replacement,
        "the recreated oplog starts at the replacement snapshot"
    );
    Ok(())
}

#[test]
fn restore_snapshot_with_empty_branch_in_workspace() -> anyhow::Result<()> {
    let Test { ctx, .. } = &mut Test::from_scenario("two-stacks-one-empty", &["A", "B"]);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert_eq!(
        ctx.snapshots_iter(None, Vec::new(), None)?.count(),
        2,
        "restoring a snapshot containing an empty branch records the restore"
    );
    Ok(())
}

#[test]
fn restore_reconstitutes_missing_commit() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    let gix_repo = repo.open_repo();
    let second = gix_repo.rev_parse_single("A")?.detach();
    let first = gix_repo.rev_parse_single("A~1")?.detach();
    let workspace_one = gix_repo.rev_parse_single("test-workspace-one")?.detach();
    gix_repo.reference(
        "refs/heads/A",
        first,
        gix::refs::transaction::PreviousValue::Any,
        "rewind test branch",
    )?;
    gix_repo.reference(
        but_core::WORKSPACE_REF_NAME,
        workspace_one,
        gix::refs::transaction::PreviousValue::Any,
        "rewind test workspace",
    )?;

    let hex = second.to_string();
    let loose_object = gix_repo
        .git_dir()
        .join("objects")
        .join(&hex[..2])
        .join(&hex[2..]);
    assert!(loose_object.is_file(), "fixture commit is stored loose");
    fs::remove_file(&loose_object)?;
    assert!(
        !gix_repo.has_object(second),
        "the commit is absent before restore"
    );

    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert!(
        gix_repo.has_object(second),
        "restore recreates the missing commit"
    );
    assert_eq!(
        gix_repo.find_reference("refs/heads/A")?.peel_to_id()?,
        second,
        "restore moves the branch back to the recreated commit"
    );
    Ok(())
}

#[test]
fn restore_restores_conflict_sidecars() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    let git_dir = repo.open_repo().git_dir().to_owned();
    fs::write(git_dir.join("conflicts"), "conflict A")?;
    fs::write(git_dir.join("base_merge_parent"), "parent A")?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    fs::remove_file(git_dir.join("conflicts"))?;
    fs::remove_file(git_dir.join("base_merge_parent"))?;

    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert_eq!(
        fs::read_to_string(git_dir.join("conflicts"))?,
        "conflict A",
        "restore recreates the conflicts sidecar"
    );
    assert_eq!(
        fs::read_to_string(git_dir.join("base_merge_parent"))?,
        "parent A",
        "restore recreates the base merge parent sidecar"
    );
    Ok(())
}

#[test]
fn restore_repoints_workspace_and_worktree() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let gix_repo = repo.open_repo();
    let original_workspace = gix_repo.rev_parse_single("test-workspace-two")?.detach();
    let workspace_one = gix_repo.rev_parse_single("test-workspace-one")?.detach();
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    gix_repo.reference(
        but_core::WORKSPACE_REF_NAME,
        workspace_one,
        gix::refs::transaction::PreviousValue::Any,
        "move workspace before restore",
    )?;
    fs::remove_file(repo.projects_root().join("second"))?;

    let index_before = fs::read(gix_repo.index_path())?;
    let lock_path = gix_repo.git_dir().join("index.lock");
    fs::write(&lock_path, b"another writer\n")?;
    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )
    .expect_err("restore must respect the existing index lock");
    assert_eq!(
        fs::read(gix_repo.index_path())?,
        index_before,
        "a locked index is unchanged"
    );
    assert_eq!(
        gix_repo.head_id()?.detach(),
        workspace_one,
        "a locked index prevents moving HEAD"
    );
    assert!(
        !repo.projects_root().join("second").exists(),
        "a locked index prevents worktree changes"
    );
    assert_eq!(
        fs::read(&lock_path)?,
        b"another writer\n",
        "another writer's lock is preserved"
    );
    fs::remove_file(lock_path)?;

    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert_eq!(
        gix_repo
            .find_reference(but_core::WORKSPACE_REF_NAME)?
            .peel_to_id()?,
        original_workspace,
        "restore repoints the workspace ref to the snapshotted commit"
    );
    assert_eq!(
        fs::read_to_string(repo.projects_root().join("second"))?,
        "second\n",
        "restore checks out the snapshotted worktree content"
    );
    assert_eq!(
        ctx.snapshots_iter(None, Vec::new(), None)?.count(),
        2,
        "restoring the snapshot records a second oplog entry"
    );
    Ok(())
}

#[test]
fn restore_keeps_index_stat_of_untouched_files() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    // Age the files so their refreshed stat isn't racy, which restoring must not trust.
    let an_hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for name in ["M", "first", "second"] {
        fs::File::options()
            .write(true)
            .open(repo.projects_root().join(name))?
            .set_modified(an_hour_ago)?;
    }
    repo.invoke_git("update-index --refresh");
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    fs::remove_file(repo.projects_root().join("second"))?;

    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    // `diff-files` lists entries whose stat doesn't match the file. Only the file the
    // restore re-created needs to be hashed again.
    snapbox::assert_data_eq!(
        repo.invoke_git("diff-files --name-only"),
        snapbox::str!["second"]
    );
    Ok(())
}

#[test]
fn restore_keeps_a_sparse_checkout_sparse() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    // As `git sparse-checkout` leaves a file out of the worktree.
    repo.invoke_git("update-index --skip-worktree first");
    fs::remove_file(repo.projects_root().join("first"))?;
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    // Unmarked, the file would show as deleted, and committing everything would delete it.
    snapbox::assert_data_eq!(
        repo.invoke_git("ls-files -v first"),
        snapbox::str!["S first"]
    );
    assert!(
        !repo.projects_root().join("first").exists(),
        "the file stays out of the worktree"
    );
    Ok(())
}

#[test]
fn restore_round_trips_workspace_and_ad_hoc_checkouts() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    let repo = repo.open_repo();
    let workspace_ref: &gix::refs::FullNameRef = but_core::WORKSPACE_REF_NAME.try_into()?;
    let original_workspace = repo.find_reference(workspace_ref)?.peel_to_id()?.detach();
    let ad_hoc_ref = gix::refs::FullName::try_from("refs/heads/ad-hoc")?;
    let ad_hoc_commit = ctx.project_meta()?.target_commit_id_or_err()?;

    let mut guard = ctx.exclusive_worktree_access();
    let workspace_snapshot = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    but_core::worktree::safe_checkout_from_head(
        ad_hoc_commit,
        &repo,
        but_core::worktree::checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    )?;
    repo.reference(
        ad_hoc_ref.as_ref(),
        ad_hoc_commit,
        gix::refs::transaction::PreviousValue::Any,
        "test ad-hoc checkout",
    )?;
    but_core::update_head_reference(
        &repo,
        gix::refs::Target::Symbolic(ad_hoc_ref.clone()),
        false,
        "test",
        b"leave workspace".as_bstr(),
        0,
    )?;
    repo.find_reference(workspace_ref)?.delete()?;

    let ad_hoc_snapshot = ctx.restore_snapshot(
        workspace_snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        repo.head_name()?
            .expect("restored workspace HEAD is symbolic")
            .as_bstr(),
        workspace_ref.as_bstr(),
        "undoing a transition to ad-hoc mode must check out the managed workspace"
    );
    assert_eq!(
        repo.find_reference(workspace_ref)?.peel_to_id()?,
        original_workspace,
        "undoing a transition to ad-hoc mode must recreate the original workspace ref"
    );

    ctx.restore_snapshot(
        ad_hoc_snapshot,
        RestoreKind::RestoreFromSnapshotViaRedo,
        guard.write_permission(),
    )?;
    assert_eq!(
        repo.head_name()?.expect("restored ad-hoc HEAD is symbolic"),
        ad_hoc_ref,
        "redoing the transition must return to the ad-hoc branch"
    );
    assert_eq!(
        repo.head_id()?,
        ad_hoc_commit,
        "redoing the transition must restore the ad-hoc commit"
    );
    assert!(
        repo.try_find_reference(workspace_ref)?.is_none(),
        "redoing a transition that removed the managed workspace must remove that ref again"
    );
    Ok(())
}

#[test]
fn snapshot_history_orders_and_paginates() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    let mut guard = ctx.exclusive_worktree_access();
    let first = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::CreateBranch),
        guard.write_permission(),
    )?;
    fs::write(repo.projects_root().join("one"), "one")?;
    let second = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::CreateCommit),
        guard.write_permission(),
    )?;
    fs::write(repo.projects_root().join("two"), "two")?;
    let third = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::GenericBranchUpdate),
        guard.write_permission(),
    )?;

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(
        snapshots
            .iter()
            .map(|snapshot| snapshot.commit_id)
            .collect::<Vec<_>>(),
        [third, second, first],
        "snapshot history is returned newest first"
    );
    let after_second = ctx
        .snapshots_iter(Some(second), Vec::new(), None)?
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(
        after_second
            .iter()
            .map(|snapshot| snapshot.commit_id)
            .collect::<Vec<_>>(),
        [first],
        "pagination after the second snapshot returns only older entries"
    );
    Ok(())
}

#[test]
fn first_snapshot_diff_works() -> anyhow::Result<()> {
    let Test { ctx, .. } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let mut guard = ctx.exclusive_worktree_access();
    let first_snapshot = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    drop(guard);

    ctx.snapshot_diff(first_snapshot, None)?;
    Ok(())
}

#[test]
fn snapshot_diff_of_an_ad_hoc_checkout_shows_what_was_committed_there() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    // A branch checked out on its own, the managed workspace's ref left as it was.
    repo.invoke_git("checkout -q -b ad-hoc");
    let paths = |diff: Vec<but_core::TreeChange>| -> Vec<String> {
        diff.into_iter().map(|c| c.path.to_string()).collect()
    };
    let before = snapshot(ctx)?;
    repo.invoke_bash("echo new >new-file && git add new-file && git commit -qm new");

    assert_eq!(
        paths(ctx.snapshot_diff(before, None)?),
        ["new-file"],
        "the newest operation, against what's checked out now"
    );
    let after = snapshot(ctx)?;
    assert_eq!(
        paths(ctx.snapshot_diff(before, Some(after))?),
        ["new-file"],
        "an older one, against what the next snapshot had checked out"
    );
    Ok(())
}

#[test]
fn snapshot_diff_while_a_commit_is_edited_reads_the_workspace() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    let before = snapshot(ctx)?;
    // Editing a commit checks it out on a branch of GitButler's own, which the workspace
    // doesn't match.
    repo.invoke_bash(
        "git checkout -q -b gitbutler/edit && echo edited >>M && git commit -qam edit",
    );
    assert_eq!(
        ctx.snapshot_diff(before, None)?.len(),
        0,
        "the workspace didn't change, which editing a commit goes back to"
    );
    Ok(())
}

fn snapshot(ctx: &mut Context) -> anyhow::Result<gix::ObjectId> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::CreateCommit),
        guard.write_permission(),
    )
}

struct Test {
    repo: Sandbox,
    ctx: Context,
}

impl Default for Test {
    fn default() -> Self {
        let repo =
            Sandbox::init_scenario_with_target_and_default_settings("metadata-free-workspace");
        let ctx = Context::from_repo_for_testing(repo.open_repo())
            .expect("fixture repository opens")
            .with_memory_app_cache();
        Self { repo, ctx }
    }
}

impl Test {
    fn from_scenario(name: &str, branches: &[&str]) -> Self {
        let repo = Sandbox::init_scenario_with_target_and_default_settings(name);
        repo.setup_metadata(branches);
        let ctx = Context::from_repo_for_testing(repo.open_repo())
            .expect("fixture repository opens")
            .with_memory_app_cache();
        Self { repo, ctx }
    }
}

fn configure_default_target(ctx: &Context) -> anyhow::Result<()> {
    assert!(
        ctx.project_meta()?.target_commit_id.is_some(),
        "the fixture initializes target metadata"
    );
    Ok(())
}

fn snapshot_blob(
    repo: &gix::Repository,
    snapshot_id: gix::ObjectId,
    path: &str,
) -> anyhow::Result<String> {
    let tree = repo.find_commit(snapshot_id)?.tree()?;
    let entry = tree
        .lookup_entry_by_path(path)?
        .with_context(|| format!("snapshot contains {path}"))?;
    Ok(repo.find_blob(entry.id())?.data.to_str()?.to_owned())
}

fn set_branch_order(ctx: &Context, refs: &[&str]) -> anyhow::Result<()> {
    ctx.db.get_cache_mut()?.branch_order_mut()?.set_order(
        &refs
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
    )?;
    Ok(())
}

fn snapshot_without_branch_order(
    repo: &gix::Repository,
    snapshot_id: gix::ObjectId,
) -> anyhow::Result<gix::ObjectId> {
    let snapshot = repo.find_commit(snapshot_id)?;
    let mut tree = snapshot.tree()?.edit()?;
    tree.remove("branch_order.toml")?;
    Ok(repo
        .write_object(gix::objs::Commit {
            tree: tree.write()?.detach(),
            ..snapshot.decode()?.to_owned()?
        })?
        .detach())
}

fn snapshot_with_branch_order(
    repo: &gix::Repository,
    snapshot_id: gix::ObjectId,
    contents: &[u8],
) -> anyhow::Result<gix::ObjectId> {
    let snapshot = repo.find_commit(snapshot_id)?;
    let mut tree = snapshot.tree()?.edit()?;
    tree.upsert(
        "branch_order.toml",
        gix::object::tree::EntryKind::Blob,
        repo.write_blob(contents)?,
    )?;
    Ok(repo
        .write_object(gix::objs::Commit {
            tree: tree.write()?.detach(),
            ..snapshot.decode()?.to_owned()?
        })?
        .detach())
}

fn snapshot_as_legacy(
    repo: &gix::Repository,
    snapshot_id: gix::ObjectId,
    target_id: gix::ObjectId,
) -> anyhow::Result<gix::ObjectId> {
    let snapshot = repo.find_commit(snapshot_id)?;
    let mut tree = snapshot.tree()?.edit()?;
    tree.remove("project_meta.toml")?;
    let mut virtual_branches = snapshot_blob(repo, snapshot_id, "virtual_branches.toml")?;
    virtual_branches.push_str(&format!(
        "\n[default_target]\nbranchName = \"main\"\nremoteName = \"origin\"\nremoteUrl = \"\"\nsha = \"{target_id}\"\npushRemoteName = \"origin\"\n"
    ));
    tree.upsert(
        "virtual_branches.toml",
        gix::object::tree::EntryKind::Blob,
        repo.write_blob(virtual_branches.as_bytes())?,
    )?;
    Ok(repo
        .write_object(gix::objs::Commit {
            tree: tree.write()?.detach(),
            ..snapshot.decode()?.to_owned()?
        })?
        .detach())
}

fn snapshot_with_project_meta(
    repo: &gix::Repository,
    snapshot_id: gix::ObjectId,
    contents: &[u8],
) -> anyhow::Result<gix::ObjectId> {
    let snapshot = repo.find_commit(snapshot_id)?;
    let mut tree = snapshot.tree()?.edit()?;
    let blob = repo.write_blob(contents)?;
    tree.upsert(
        "project_meta.toml",
        gix::object::tree::EntryKind::Blob,
        blob,
    )?;
    Ok(repo
        .write_object(gix::objs::Commit {
            tree: tree.write()?.detach(),
            ..snapshot.decode()?.to_owned()?
        })?
        .detach())
}

fn enable_failing_commit_signing(ctx: &Context) -> anyhow::Result<()> {
    ctx.repo.get()?.set_git_settings(&GitConfigSettings {
        gitbutler_sign_commits: Some(true),
        signing_key: Some("definitely-no-such-signing-key".into()),
        ..Default::default()
    })
}

fn has_signature(repo: &gix::Repository, id: gix::ObjectId) -> anyhow::Result<bool> {
    Ok(repo
        .find_commit(id)?
        .decode()?
        .extra_headers()
        .pgp_signature()
        .is_some())
}
