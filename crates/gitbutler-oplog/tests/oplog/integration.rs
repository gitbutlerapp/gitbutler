use std::fs;

use anyhow::Context as _;
use but_core::{GitConfigSettings, RepositoryExt as _};
use but_ctx::Context;
use but_testsupport::{Sandbox, visualize_commit_graph_all, visualize_index};
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gitbutler_oplog::{OplogExt, RestoreKind};
use gix::bstr::ByteSlice as _;
use snapbox::IntoData as _;

#[test]
fn snapshot_with_additional_ref_includes_branch_order() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    set_branch_order(ctx, &["refs/heads/A", "refs/heads/B"])?;
    let additional_ref: gix::refs::FullName = "refs/heads/A".try_into()?;
    let mut guard = ctx.exclusive_worktree_access();
    let tree_id =
        ctx.prepare_snapshot_with_ref(additional_ref.as_ref(), guard.read_permission())?;
    let snapshot_id = ctx.commit_snapshot(
        tree_id,
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    // The additional-ref snapshot path writes branch-order metadata like a plain snapshot does.
    snapbox::assert_data_eq!(
        snapshot_blob(&repo.open_repo(), snapshot_id, "branch_order.toml")?,
        snapbox::str![[r#"
[[entries]]
branch_ref_name = "refs/heads/A"
parent_ref_name = "refs/heads/B"

[[entries]]
branch_ref_name = "refs/heads/B"

"#]]
    );
    Ok(())
}

#[test]
fn restore_snapshot_replaces_branch_order() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    set_branch_order(ctx, &["refs/heads/A", "refs/heads/B"])?;
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    snapbox::assert_data_eq!(
        snapshot_blob(&repo.open_repo(), snapshot_id, "branch_order.toml")?,
        snapbox::str![[r#"
[[entries]]
branch_ref_name = "refs/heads/A"
parent_ref_name = "refs/heads/B"

[[entries]]
branch_ref_name = "refs/heads/B"

"#]]
    );

    set_branch_order(ctx, &["refs/heads/C", "refs/heads/D"])?;
    // `set_order` only replaces the chains it touches, so C/D are added next to A/B.
    snapbox::assert_data_eq!(
        branch_order(ctx)?,
        snapbox::str![[r#"
[
    "refs/heads/A -> refs/heads/B",
    "refs/heads/B",
    "refs/heads/C -> refs/heads/D",
    "refs/heads/D",
]
"#]]
    );

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    // Restoring replaces the complete table, so the C/D rows added after the snapshot are gone.
    snapbox::assert_data_eq!(
        branch_order(ctx)?,
        snapbox::str![[r#"
[
    "refs/heads/A -> refs/heads/B",
    "refs/heads/B",
]
"#]]
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
    snapbox::assert_data_eq!(
        branch_order(ctx)?,
        snapbox::str![[r#"
[
    "refs/heads/A -> refs/heads/B",
    "refs/heads/B",
]
"#]]
    );

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    // The snapshot was taken while the table was empty, so restoring it clears the order.
    snapbox::assert_data_eq!(branch_order(ctx)?, snapbox::str!["[]"]);
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

    ctx.restore_snapshot(
        legacy_snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    // A snapshot without branch-order data leaves the current table alone.
    snapbox::assert_data_eq!(
        branch_order(ctx)?,
        snapbox::str![[r#"
[
    "refs/heads/A -> refs/heads/B",
    "refs/heads/B",
]
"#]]
    );
    Ok(())
}

#[test]
fn malformed_branch_order_fails_before_restore_mutates_state() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    set_branch_order(ctx, &["refs/heads/A", "refs/heads/B"])?;
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

    snapbox::assert_data_eq!(
        format!("{error:#}"),
        snapbox::str![[r#"
failed to parse branch_order.toml: TOML parse error at line 1, column 5
  |
1 | not valid = [
  |     ^
key with no value, expected `=`

"#]]
    );
    // Validation fails before anything is written, so the order set before the snapshot survives.
    snapbox::assert_data_eq!(
        branch_order(ctx)?,
        snapbox::str![[r#"
[
    "refs/heads/A -> refs/heads/B",
    "refs/heads/B",
]
"#]]
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
    fs::write(repo.projects_root().join("first"), "changed after snapshot")?;
    snapbox::assert_data_eq!(
        repo.git_status(),
        snapbox::str![[r#"
 M first

"#]]
    );

    let error = ctx
        .restore_snapshot(
            invalid_snapshot_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )
        .expect_err("invalid branch-order metadata must fail restore");

    snapbox::assert_data_eq!(
        format!("{error:#}"),
        snapbox::str!["invalid branch_order.toml: duplicate parent reference 'refs/heads/B'"]
    );
    // Validation happens before the worktree is restored, so the modification is still there.
    snapbox::assert_data_eq!(
        repo.git_status(),
        snapbox::str![[r#"
 M first

"#]]
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
            "refs/remotes/origin/other",
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
    changed.target_ref = Some("refs/remotes/origin/other".try_into()?);
    ctx.set_project_meta(changed)?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/other
target_commit_id: 0dc3733
push_remote: origin

"#]]
    );

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    // Undoing a base-branch switch reverts the target everywhere, not just in the TOML.
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/main
target_commit_id: 0dc3733
push_remote: origin

"#]]
    );
    Ok(())
}

#[test]
#[expect(deprecated, reason = "libgit2 index compatibility boundary")]
fn snapshot_creation_works_with_unmerged_index() -> anyhow::Result<()> {
    fn conflict_index_entry(
        path: impl AsRef<[u8]>,
        stage: u16,
        blob: git2::Oid,
    ) -> git2::IndexEntry {
        let path = path.as_ref();
        let path_len = path.len().min(0x0fff) as u16;
        git2::IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: 0o100644,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: blob,
            flags: stage << 12 | path_len,
            flags_extended: 0,
            path: path.into(),
        }
    }

    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;

    // Simulate what a workspace update leaves behind when an uncommitted file
    // conflicts: conflict markers in the worktree and unmerged entries in the index.
    // `deleted.txt` has no 'ours' stage, as when the local side deleted the file.
    {
        let git2_repo = ctx.git2_repo.get()?;
        let ours_blob = git2_repo.blob(b"ours\n")?;
        let base_blob = git2_repo.blob(b"base\n")?;
        let theirs_blob = git2_repo.blob(b"theirs\n")?;
        let mut index = git2_repo.index()?;
        for (path, stage, blob) in [
            ("conflicted.txt", 1, base_blob),
            ("conflicted.txt", 2, ours_blob),
            ("conflicted.txt", 3, theirs_blob),
            ("deleted.txt", 1, base_blob),
            ("deleted.txt", 3, theirs_blob),
            ("df", 2, ours_blob),
            ("df/child", 3, theirs_blob),
        ] {
            index.add(&conflict_index_entry(path, stage, blob))?;
        }
        #[cfg(unix)]
        for (stage, blob) in [(1, base_blob), (2, ours_blob), (3, theirs_blob)] {
            index.add(&conflict_index_entry(b"invalid-\xff.txt", stage, blob))?;
        }
        index.write()?;
    }
    fs::write(
        repo.projects_root().join("conflicted.txt"),
        "<<<<<<< ours\nours\n||||||| base\nbase\n=======\ntheirs\n>>>>>>> theirs\n",
    )?;
    let gix_repo = repo.open_repo();
    let unmerged_index = index_entries(&gix_repo)?;
    #[cfg(unix)]
    snapbox::assert_data_eq!(
        unmerged_index.as_str(),
        snapbox::str![[r#"
100644:ab77689 M
100644:df967b9 conflicted.txt:1
100644:b19a1e9 conflicted.txt:2
100644:950b81b conflicted.txt:3
100644:df967b9 deleted.txt:1
100644:950b81b deleted.txt:3
100644:b19a1e9 df:2
100644:950b81b df/child:3
100644:df967b9 invalid-�.txt:1
100644:b19a1e9 invalid-�.txt:2
100644:950b81b invalid-�.txt:3

"#]]
    );
    #[cfg(not(unix))]
    snapbox::assert_data_eq!(
        unmerged_index.as_str(),
        snapbox::str![[r#"
100644:ab77689 M
100644:df967b9 conflicted.txt:1
100644:b19a1e9 conflicted.txt:2
100644:950b81b conflicted.txt:3
100644:df967b9 deleted.txt:1
100644:950b81b deleted.txt:3
100644:b19a1e9 df:2
100644:950b81b df/child:3

"#]]
    );

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    {
        let git2_repo = ctx.git2_repo.get()?;
        let mut index = git2_repo.index()?;
        index.clear()?;
        index.write()?;
    }
    // The conflict is gone before the restore, so it can only come back from the snapshot.
    snapbox::assert_data_eq!(index_entries(&gix_repo)?, snapbox::str![""]);
    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    assert_eq!(
        index_entries(&gix_repo)?,
        unmerged_index,
        "restore round-trips every conflict stage, the missing ours stage of the local deletion included"
    );
    Ok(())
}

#[test]
fn snapshot_has_authoritative_meta_and_omits_legacy_target() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let mut with_push_remote = ctx.project_meta()?;
    with_push_remote.push_remote = Some("origin".to_owned());
    ctx.set_project_meta(with_push_remote)?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    let repo = repo.open_repo();
    // The authoritative metadata stores the target ref, the target commit and the push remote.
    snapbox::assert_data_eq!(
        snapshot_blob(&repo, snapshot_id, "project_meta.toml")?,
        snapbox::str![[r#"
targetRef = "refs/remotes/origin/main"
targetCommitId = "0dc37334a458df421bf67ea806103bf5004845dd"
pushRemote = "origin"

"#]]
    );
    // The legacy TOML has no `[default_target]` table anymore.
    snapbox::assert_data_eq!(
        snapshot_blob(&repo, snapshot_id, "virtual_branches.toml")?,
        snapbox::str![[r#"
[branches]

"#]]
    );
    Ok(())
}

#[test]
fn snapshot_with_only_a_target_commit_omits_target_ref() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let mut target_commit_only = ctx.project_meta()?;
    target_commit_only.target_ref = None;
    ctx.set_project_meta(target_commit_only)?;

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;

    // An absent target ref stays absent, while the target commit is kept.
    snapbox::assert_data_eq!(
        snapshot_blob(&repo.open_repo(), snapshot_id, "project_meta.toml")?,
        snapbox::str![[r#"
targetCommitId = "0dc37334a458df421bf67ea806103bf5004845dd"
pushRemote = "origin"

"#]]
    );
    Ok(())
}

#[test]
fn restore_falls_back_to_the_legacy_target_in_old_snapshots() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let original = ctx.project_meta()?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/main
target_commit_id: 0dc3733
push_remote: origin

"#]]
    );

    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let old_snapshot_id = snapshot_as_legacy(
        &repo.open_repo(),
        snapshot_id,
        original.target_commit_id.unwrap(),
    )?;

    let mut changed = original;
    changed.target_ref = None;
    ctx.set_project_meta(changed)?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: None
target_commit_id: 0dc3733
push_remote: origin

"#]]
    );

    ctx.restore_snapshot(
        old_snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    // The legacy `[default_target]` table restores the original target.
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/main
target_commit_id: 0dc3733
push_remote: origin

"#]]
    );
    Ok(())
}

#[test]
fn restore_reverts_a_target_commit_only_change() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let original = ctx.project_meta()?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/main
target_commit_id: 0dc3733
push_remote: origin

"#]]
    );

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
    let mut changed = original;
    changed.target_commit_id = Some(alternate_target);
    ctx.set_project_meta(changed)?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/main
target_commit_id: 1485ac2
push_remote: origin

"#]]
    );

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    // Only the target commit changed, and the restore reverts exactly that.
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/main
target_commit_id: 0dc3733
push_remote: origin

"#]]
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
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/main
target_commit_id: 0dc3733
push_remote: origin

"#]]
    );
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
    snapbox::assert_data_eq!(
        format!("{error:#}"),
        snapbox::str![
            "invalid targetCommitId in project_meta.toml: A hash sized 16 hexadecimal characters is invalid"
        ]
    );
    // Validation fails before anything is written, so the metadata is untouched.
    snapbox::assert_data_eq!(
        project_meta_summary(ctx)?,
        snapbox::str![[r#"
target_ref: refs/remotes/origin/main
target_commit_id: 0dc3733
push_remote: origin

"#]]
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
    snapbox::assert_data_eq!(
        oplog_operations(ctx, None)?,
        snapbox::str!["[OnDemandSnapshot]"]
    );

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

    // The corrupt head is dropped rather than chained to, so there is still only one snapshot.
    snapbox::assert_data_eq!(
        oplog_operations(ctx, None)?,
        snapbox::str!["[OnDemandSnapshot]"]
    );
    assert_eq!(
        ctx.oplog_head()?,
        Some(replacement),
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

    // Restoring a snapshot that contains an empty branch succeeds and records the restore.
    snapbox::assert_data_eq!(
        oplog_operations(ctx, None)?,
        snapbox::str!["[RestoreFromSnapshotViaUndo, OnDemandSnapshot]"]
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
    // `A` and the workspace are rewound; only the tag still reaches the `second` commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&gix_repo)?,
        snapbox::str![[r#"
*   fc03769 (tag: test-workspace-two) GitButler Workspace Commit
|\  
| * 4bed59b add second
* | 21d3ffd (HEAD -> gitbutler/workspace, tag: test-workspace-one) GitButler Workspace Commit
|/  
* e9b402f (A) add first
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );

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

    // The missing commit is recreated from the snapshot and `A` moves back to it.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&gix_repo)?,
        snapbox::str![[r#"
*   fc03769 (HEAD -> gitbutler/workspace, tag: test-workspace-two) GitButler Workspace Commit
|\  
| * 4bed59b (A) add second
* | 21d3ffd (tag: test-workspace-one) GitButler Workspace Commit
|/  
* e9b402f add first
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
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
    // The workspace ref was moved back one commit, so `second` is staged relative to HEAD but gone from the worktree.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&gix_repo)?,
        snapbox::str![[r#"
*   fc03769 (tag: test-workspace-two) GitButler Workspace Commit
|\  
| * 4bed59b (A) add second
* | 21d3ffd (HEAD -> gitbutler/workspace, tag: test-workspace-one) GitButler Workspace Commit
|/  
* e9b402f add first
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        repo.git_status(),
        snapbox::str![[r#"
AD second

"#]]
    );

    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    // The workspace ref is back at the snapshotted commit and the worktree matches it again.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&gix_repo)?,
        snapbox::str![[r#"
*   fc03769 (HEAD -> gitbutler/workspace, tag: test-workspace-two) GitButler Workspace Commit
|\  
| * 4bed59b (A) add second
* | 21d3ffd (tag: test-workspace-one) GitButler Workspace Commit
|/  
* e9b402f add first
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(repo.git_status(), snapbox::str![[""]]);
    snapbox::assert_data_eq!(
        oplog_operations(ctx, None)?,
        snapbox::str!["[RestoreFromSnapshotViaUndo, OnDemandSnapshot]"]
    );
    Ok(())
}

#[test]
fn restore_round_trips_workspace_and_ad_hoc_checkouts() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    let repo = repo.open_repo();
    let workspace_ref: &gix::refs::FullNameRef = but_core::WORKSPACE_REF_NAME.try_into()?;
    let ad_hoc_ref = gix::refs::FullName::try_from("refs/heads/ad-hoc")?;
    let ad_hoc_commit = ctx.project_meta()?.target_commit_id_or_err()?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* ff4136d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );

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
    // Leaving the workspace: HEAD is on the ad-hoc branch and the workspace ref is gone.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0dc3733 (HEAD -> ad-hoc, origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );

    let ad_hoc_snapshot = ctx.restore_snapshot(
        workspace_snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    // Undoing the transition checks out the managed workspace and recreates its ref.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* ff4136d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target, ad-hoc) add M

"#]]
    );

    ctx.restore_snapshot(
        ad_hoc_snapshot,
        RestoreKind::RestoreFromSnapshotViaRedo,
        guard.write_permission(),
    )?;
    // Redoing the transition returns to the ad-hoc branch and removes the workspace ref again.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0dc3733 (HEAD -> ad-hoc, origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
    Ok(())
}

#[test]
fn snapshot_history_orders_and_paginates() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::default();
    let mut guard = ctx.exclusive_worktree_access();
    ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::CreateBranch),
        guard.write_permission(),
    )?;
    fs::write(repo.projects_root().join("one"), "one")?;
    let second = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::CreateCommit),
        guard.write_permission(),
    )?;
    fs::write(repo.projects_root().join("two"), "two")?;
    ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::GenericBranchUpdate),
        guard.write_permission(),
    )?;

    // Newest first.
    snapbox::assert_data_eq!(
        oplog_operations(ctx, None)?,
        snapbox::str!["[GenericBranchUpdate, CreateCommit, CreateBranch]"]
    );
    // Paginating after the second snapshot returns only the older entries.
    snapbox::assert_data_eq!(
        oplog_operations(ctx, Some(second))?,
        snapbox::str!["[CreateBranch]"]
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

fn project_meta_summary(ctx: &Context) -> anyhow::Result<String> {
    let meta = ctx.project_meta()?;
    Ok(format!(
        "target_ref: {}\ntarget_commit_id: {}\npush_remote: {}\n",
        meta.target_ref
            .as_ref()
            .map_or_else(|| "None".to_owned(), ToString::to_string),
        meta.target_commit_id
            .map_or_else(|| "None".to_owned(), |id| id.to_hex_with_len(7).to_string()),
        meta.push_remote.as_deref().unwrap_or("None"),
    ))
}

fn index_entries(repo: &gix::Repository) -> anyhow::Result<String> {
    let index = repo.open_index()?;
    Ok(visualize_index(&index))
}

fn branch_order(ctx: &Context) -> anyhow::Result<String> {
    let entries = ctx
        .db
        .get_cache()?
        .branch_order()
        .get_snapshot()?
        .entries
        .into_iter()
        .map(|entry| match entry.parent_ref_name {
            Some(parent) => format!("{} -> {parent}", entry.branch_ref_name),
            None => entry.branch_ref_name,
        })
        .collect::<Vec<_>>();
    Ok(format!("{entries:#?}"))
}

fn oplog_operations(ctx: &Context, after: Option<gix::ObjectId>) -> anyhow::Result<String> {
    let operations = ctx
        .snapshots_iter(after, Vec::new(), None)?
        .map(|snapshot| {
            Ok(snapshot?
                .details
                .map_or(OperationKind::Unknown, |details| details.operation))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(format!("{operations:?}"))
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
