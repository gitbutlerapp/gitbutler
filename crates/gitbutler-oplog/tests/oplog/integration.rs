use std::{fs, path::Path};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt as _;

use anyhow::Context as _;
use but_core::{GitConfigSettings, RepositoryExt as _};
use but_ctx::Context;
use but_testsupport::Sandbox;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gitbutler_oplog::{OplogExt, RestoreKind};
use gix::bstr::ByteSlice as _;

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

    assert!(
        snapshot_metadata(&repo.open_repo(), snapshot_id)?
            .metadata
            .branch_stack_order(gix::refs::FullName::try_from("refs/heads/A")?.as_ref())
            .is_some(),
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
        snapshot_metadata(&repo.open_repo(), snapshot_id)?
            .metadata
            .branch_stack_order(gix::refs::FullName::try_from("refs/heads/A")?.as_ref())
            .is_some(),
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
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .replace_branch_order(&but_db::BranchOrderSnapshot {
            entries: Vec::new(),
        })?;
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
fn restore_removes_only_historically_inherited_branch_order() -> anyhow::Result<()> {
    use but_core::ref_metadata::WorkspaceStackBranch;
    for (case, order) in [
        ("inherited", &["A", "B"][..]),
        ("extended above", &["external", "A", "B"][..]),
        ("extended below", &["A", "B", "external"][..]),
        ("reversed", &["B", "A"][..]),
        ("unrelated", &["external", "other"][..]),
        ("without file", &["A", "B"][..]),
        ("modern", &["A", "B"][..]),
    ] {
        let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
        let git_repo = repo.open_repo();
        let tip = git_repo.rev_parse_single("A")?.detach();
        let base = git_repo.rev_parse_single("A~1")?.detach();
        git_repo.reference(
            "refs/heads/B",
            base,
            gix::refs::transaction::PreviousValue::Any,
            "test",
        )?;
        let workspace_ref: &gix::refs::FullNameRef = but_core::WORKSPACE_REF_NAME.try_into()?;
        let mut workspace = ctx
            .db
            .get_cache()?
            .meta()?
            .workspace(workspace_ref)
            .unwrap()
            .clone();
        workspace.stacks[0].branches.push(WorkspaceStackBranch {
            ref_name: "refs/heads/B".try_into()?,
            archived: false,
        });
        ctx.db
            .get_cache_mut()?
            .meta_mut()?
            .set_workspace(workspace_ref, &workspace)?;
        let mut guard = ctx.exclusive_worktree_access();
        let mut snapshot_id = ctx.create_snapshot(
            SnapshotDetails::new(OperationKind::OnDemandSnapshot),
            guard.write_permission(),
        )?;
        let names = order
            .iter()
            .map(|name| gix::refs::FullName::try_from(format!("refs/heads/{name}")))
            .collect::<Result<Vec<_>, _>>()?;
        ctx.db
            .get_cache_mut()?
            .branch_order_mut()?
            .set_order(&names)?;
        let archived_order = ctx.db.get_cache()?.branch_order().get_snapshot()?;
        if case == "modern" {
            snapshot_id = ctx.create_snapshot(
                SnapshotDetails::new(OperationKind::OnDemandSnapshot),
                guard.write_permission(),
            )?;
        } else {
            let snapshot = git_repo.find_commit(snapshot_id)?;
            let mut tree = snapshot.tree()?.edit()?;
            tree.remove("ref_metadata.json")?;
            let id = workspace.stacks[0].id;
            let historical = format!(
                r#"
[branches.{id}]
order = 0
[[branches.{id}.heads]]
name = "B"
head = {{ CommitId = "{base}" }}
[[branches.{id}.heads]]
name = "A"
head = {{ CommitId = "{tip}" }}
"#
            );
            tree.upsert(
                "virtual_branches.toml",
                gix::object::tree::EntryKind::Blob,
                git_repo.write_blob(historical.as_bytes())?,
            )?;
            if case != "without file" {
                tree.upsert(
                    "branch_order.toml",
                    gix::object::tree::EntryKind::Blob,
                    git_repo.write_blob(toml::to_string(&archived_order)?)?,
                )?;
            }
            snapshot_id = git_repo
                .write_object(gix::objs::Commit {
                    tree: tree.write()?.detach(),
                    ..snapshot.decode()?.to_owned()?
                })?
                .detach();
        }
        if case != "without file" {
            set_branch_order(ctx, &["refs/heads/current"])?;
        }
        ctx.restore_snapshot(
            snapshot_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )?;
        let expected_order = if case == "inherited" {
            but_db::BranchOrderSnapshot {
                entries: Vec::new(),
            }
        } else {
            archived_order
        };
        assert_eq!(
            ctx.db.get_cache()?.branch_order().get_snapshot()?,
            expected_order,
            "only complete historical workspace-derived chains are discarded: {case}"
        );

        workspace.stacks[0].branches.reverse();
        ctx.db
            .get_cache_mut()?
            .meta_mut()?
            .set_workspace(workspace_ref, &workspace)?;
        let metadata = ctx.db.get_cache()?.meta()?;
        let actual = metadata
            .branch_stack_order("refs/heads/A".try_into()?)
            .unwrap()
            .iter()
            .map(|name| name.shorten().to_str().map(ToOwned::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        let expected = if case == "inherited" || !order.contains(&"A") {
            &["B", "A"][..]
        } else {
            order
        };
        assert_eq!(
            actual, expected,
            "workspace edits use derived order unless an explicit override remains: {case}"
        );
    }
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
fn restore_historical_branch_order_from_string_entries() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let historical_id = snapshot_with_branch_order(
        &repo.open_repo(),
        snapshot_id,
        br#"
[[entries]]
branch_ref_name = "refs/heads/A"
parent_ref_name = "refs/heads/B"
[[entries]]
branch_ref_name = "refs/heads/B"
"#,
    )?;
    ctx.restore_snapshot(
        historical_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    let metadata = ctx.db.get_cache()?.meta()?;
    let expected: [gix::refs::FullName; 2] =
        ["refs/heads/A".try_into()?, "refs/heads/B".try_into()?];
    assert_eq!(
        metadata.branch_stack_order(expected[0].as_ref()),
        Some(expected.as_slice()),
        "historical TOML string entries restore as typed reference ordering"
    );
    Ok(())
}

#[test]
fn invalid_historical_branch_order_names_fail_before_restore_mutates_state() -> anyhow::Result<()> {
    for contents in [
        "[[entries]]\nbranch_ref_name = \"refs/heads/invalid..name\"\n",
        "[[entries]]\nbranch_ref_name = \"refs/heads/A\"\nparent_ref_name = \"refs/heads/invalid..parent\"\n",
    ] {
        assert_invalid_historical_branch_order(contents)?;
    }
    Ok(())
}

#[test]
fn cyclic_historical_branch_order_fails_before_restore_mutates_state() -> anyhow::Result<()> {
    assert_invalid_historical_branch_order(
        r#"
[[entries]]
branch_ref_name = "refs/heads/A"
parent_ref_name = "refs/heads/B"
[[entries]]
branch_ref_name = "refs/heads/B"
parent_ref_name = "refs/heads/A"
"#,
    )
}

fn assert_invalid_historical_branch_order(contents: &str) -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let git_repo = repo.open_repo();
    let invalid_id = snapshot_with_branch_order(&git_repo, snapshot_id, contents.as_bytes())?;
    let changed_path = repo.projects_root().join("first");
    fs::write(&changed_path, "changed after snapshot")?;
    let metadata = ctx.db.get_cache()?.meta()?;
    let workspace_head = git_repo.head_id()?.detach();
    let branch_head = git_repo.rev_parse_single("A")?.detach();
    let oplog_head = ctx.oplog_head()?;
    let error = ctx
        .restore_snapshot(
            invalid_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )
        .expect_err("invalid historical branch orders must fail validation");
    assert!(
        error.to_string().contains("branch_order.toml"),
        "restore identifies the invalid historical ordering payload: {error:#}"
    );
    assert_eq!(
        fs::read_to_string(changed_path)?,
        "changed after snapshot",
        "ordering validation precedes worktree changes"
    );
    assert_eq!(
        ctx.db.get_cache()?.meta()?,
        metadata,
        "ordering validation precedes database changes"
    );
    assert_eq!(
        git_repo.head_id()?.detach(),
        workspace_head,
        "ordering validation precedes workspace ref changes"
    );
    assert_eq!(
        git_repo.rev_parse_single("A")?.detach(),
        branch_head,
        "ordering validation precedes branch ref changes"
    );
    assert_eq!(
        ctx.oplog_head()?,
        oplog_head,
        "ordering validation precedes oplog changes"
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
    assert_eq!(
        ctx.project_meta()?.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/other".to_string()),
        "the target change is visible before restoring the snapshot"
    );

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        ctx.project_meta()?.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/main".to_string()),
        "undoing a base-branch switch reverts the target everywhere, not just in the TOML"
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
    let (base_blob, ours_blob, theirs_blob) = {
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

    {
        let git2_repo = ctx.git2_repo.get()?;
        let mut index = git2_repo.index()?;
        index.clear()?;
        index.write()?;
        assert!(
            !index.has_conflicts(),
            "the conflict is cleared before restore"
        );
    }
    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    let index = ctx.git2_repo.get()?.index()?;
    for (path, stage, expected) in [
        ("conflicted.txt", 1, base_blob),
        ("conflicted.txt", 2, ours_blob),
        ("conflicted.txt", 3, theirs_blob),
        ("deleted.txt", 1, base_blob),
        ("deleted.txt", 3, theirs_blob),
        ("df", 2, ours_blob),
        ("df/child", 3, theirs_blob),
    ] {
        assert_eq!(
            index.get_path(Path::new(path), stage).map(|entry| entry.id),
            Some(expected),
            "restore preserves stage {stage} for {path}"
        );
    }
    assert!(
        index.get_path(Path::new("deleted.txt"), 2).is_none(),
        "restore preserves the missing ours stage for a local deletion"
    );
    #[cfg(unix)]
    {
        let path = Path::new(std::ffi::OsStr::from_bytes(b"invalid-\xff.txt"));
        for (stage, expected) in [(1, base_blob), (2, ours_blob), (3, theirs_blob)] {
            assert_eq!(
                index.get_path(path, stage).map(|entry| entry.id),
                Some(expected),
                "restore preserves non-UTF-8 conflict paths at stage {stage}"
            );
        }
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct ArchivedMetadata {
    metadata: but_db::Metadata,
    references: Vec<ArchivedReference>,
}

#[derive(serde::Deserialize)]
struct ArchivedReference {
    ref_name: gix::refs::FullName,
    #[serde(with = "but_serde::object_id_opt")]
    target: Option<gix::ObjectId>,
}

fn snapshot_metadata(
    repo: &gix::Repository,
    snapshot: gix::ObjectId,
) -> anyhow::Result<ArchivedMetadata> {
    Ok(serde_json::from_str(&snapshot_blob(
        repo,
        snapshot,
        "ref_metadata.json",
    )?)?)
}

#[test]
fn snapshot_and_restore_preserve_per_ref_metadata() -> anyhow::Result<()> {
    use but_core::ref_metadata::{Branch, Workspace};
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let branch: gix::refs::FullName = "refs/heads/main".try_into()?;
    let secondary: gix::refs::FullName = "refs/heads/gitbutler/workspaces/secondary".try_into()?;
    let git_repo = repo.open_repo();
    let head = git_repo.head_id()?.detach();
    let mut secondary_commit = git_repo.find_commit(head)?.decode()?.to_owned()?;
    secondary_commit.message = "secondary workspace".into();
    let secondary_commit = git_repo.write_object(secondary_commit)?.detach();
    git_repo.reference(
        secondary.as_ref(),
        secondary_commit,
        gix::refs::transaction::PreviousValue::Any,
        "test",
    )?;
    let mut metadata = Branch::default();
    metadata.review.review_id = Some("saved review".into());
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .set_branch(branch.as_ref(), &metadata)?;
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .set_workspace(secondary.as_ref(), &Workspace::default())?;
    let before = ctx.db.get_cache()?.meta()?;
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    assert_eq!(
        snapshot_metadata(&repo.open_repo(), snapshot_id)?.metadata,
        before,
        "snapshots retain per-ref metadata, including empty workspaces and branches outside stacks"
    );
    metadata.review.review_id = None;
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .set_branch(branch.as_ref(), &metadata)?;
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .remove(secondary.as_ref())?;
    git_repo.reference(
        secondary.as_ref(),
        head,
        gix::refs::transaction::PreviousValue::Any,
        "test",
    )?;
    let hex = secondary_commit.to_string();
    let loose_object = git_repo
        .git_dir()
        .join("objects")
        .join(&hex[..2])
        .join(&hex[2..]);
    assert!(
        loose_object.is_file(),
        "the secondary workspace commit is loose"
    );
    fs::remove_file(loose_object)?;
    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        ctx.db.get_cache()?.meta()?,
        before,
        "undo restores the complete per-ref metadata snapshot"
    );
    assert_eq!(
        git_repo.rev_parse_single(secondary.as_ref())?.detach(),
        secondary_commit,
        "undo restores additional workspace references"
    );
    assert!(
        git_repo.find_commit(secondary_commit).is_ok(),
        "undo reconstructs additional workspace commits"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn snapshot_roundtrips_non_utf8_ref_metadata() -> anyhow::Result<()> {
    use but_core::ref_metadata::{Branch, Workspace};
    let Test { repo, ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    let branch = gix::refs::FullName::try_from(b"refs/heads/branch-\xff".as_bstr())?;
    let workspace = gix::refs::FullName::try_from(b"refs/heads/workspace-\xfe".as_bstr())?;
    let mut branch_value = Branch::default();
    branch_value.review.review_id = Some("byte-preserving review".into());
    let mut workspace_value = Workspace::default();
    workspace_value.ref_info.created_at = Some(gix::date::Time::new(123, 3600));
    let branch_order = vec![branch.clone(), "refs/heads/base".try_into()?];
    let metadata = but_db::Metadata::from_parts(
        vec![
            (workspace, workspace_value),
            ("refs/heads/secondary".try_into()?, Workspace::default()),
        ],
        vec![(branch, branch_value)],
        vec![branch_order.clone()],
    );
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .replace_snapshot(&metadata)?;
    let expected = ctx.db.get_cache()?.meta()?;
    assert_eq!(
        expected.branch_orders().collect::<Vec<_>>(),
        vec![branch_order.as_slice()],
        "ad-hoc ordering persists arbitrary reference bytes"
    );
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    assert_eq!(
        snapshot_metadata(&repo.open_repo(), snapshot_id)?.metadata,
        expected,
        "JSON preserves arbitrary reference bytes across multiple workspace rows"
    );
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .replace_snapshot(&but_db::Metadata::default())?;
    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        ctx.db.get_cache()?.meta()?,
        expected,
        "restore retains names, timestamps and reviews without UTF-8 conversion"
    );
    Ok(())
}

#[test]
fn historical_snapshot_restores_workspace_and_branch_metadata() -> anyhow::Result<()> {
    use but_core::ref_metadata::{StackId, WorkspaceCommitRelation};
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let repo = repo.open_repo();
    let tip = repo.rev_parse_single("A")?.detach();
    let base = repo.rev_parse_single("A~1")?.detach();
    let applied = StackId::from_number_for_testing(7);
    let outside = StackId::from_number_for_testing(9);
    repo.reference(
        "refs/heads/C",
        tip,
        gix::refs::transaction::PreviousValue::Any,
        "test",
    )?;
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let snapshot = repo.find_commit(snapshot_id)?;
    let mut tree = snapshot.tree()?.edit()?;
    tree.remove("ref_metadata.json")?;
    let archived = format!(
        r#"
[branches.{applied}]
order = 0

[[branches.{applied}.heads]]
name = "C"
head = {{ ChangeId = "legacy-change" }}

[[branches.{applied}.heads]]
name = "B"
target = {{ CommitId = "{base}" }}
archived = true
pr_number = 17
review_id = "historical review"

[[branches.{applied}.heads]]
name = "A"
head = {{ CommitId = "{tip}" }}

[branches.{outside}]
order = 1
in_workspace = false

[[branches.{outside}.heads]]
name = "D"
head = {{ CommitId = "{tip}" }}
"#
    );
    tree.upsert(
        "virtual_branches.toml",
        gix::object::tree::EntryKind::Blob,
        repo.write_blob(archived.as_bytes())?,
    )?;
    let historical_id = repo
        .write_object(gix::objs::Commit {
            tree: tree.write()?.detach(),
            ..snapshot.decode()?.to_owned()?
        })?
        .detach();
    repo.reference(
        "refs/heads/A",
        base,
        gix::refs::transaction::PreviousValue::Any,
        "test",
    )?;
    ctx.restore_snapshot(
        historical_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;

    let metadata = ctx.db.get_cache()?.meta()?;
    let workspace = metadata
        .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)
        .unwrap();
    assert_eq!(
        workspace.ref_info.created_at,
        Some(gix::date::Time::new(1675176957, 0)),
        "old snapshots retain the deterministic managed-workspace marker"
    );
    assert_eq!(
        workspace
            .stacks
            .iter()
            .map(|stack| stack.id)
            .collect::<Vec<_>>(),
        vec![applied, outside],
        "historical stack order and identities survive restoration"
    );
    assert_eq!(
        workspace.stacks[0]
            .branches
            .iter()
            .map(|branch| branch.ref_name.as_bstr())
            .collect::<Vec<_>>(),
        vec![
            b"refs/heads/A".as_bstr(),
            b"refs/heads/B".as_bstr(),
            b"refs/heads/C".as_bstr()
        ],
        "historical base-to-tip heads become tip-to-base branches"
    );
    assert!(
        workspace.stacks[0].branches[1].archived,
        "archived branch flags are preserved"
    );
    assert_eq!(
        workspace.stacks[1].workspacecommit_relation,
        WorkspaceCommitRelation::Outside,
        "unapplied stacks remain outside the workspace"
    );
    let branch = metadata.branch("refs/heads/B".try_into()?).unwrap();
    assert_eq!(
        branch.review.pull_request,
        Some(17),
        "historical PR numbers become per-ref reviews"
    );
    assert_eq!(
        branch.review.review_id.as_deref(),
        Some("historical review"),
        "historical review IDs are retained"
    );
    assert_eq!(
        repo.rev_parse_single("A")?.detach(),
        tip,
        "the head field restores the saved target"
    );
    assert_eq!(
        repo.rev_parse_single("B")?.detach(),
        base,
        "the historical target alias restores the saved target"
    );
    assert_eq!(
        repo.rev_parse_single("C")?.detach(),
        tip,
        "unknown historical ChangeIds do not delete refs"
    );
    Ok(())
}

#[test]
fn invalid_ref_metadata_fails_before_restore_mutates_state() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let git_repo = repo.open_repo();
    let snapshot = git_repo.find_commit(snapshot_id)?;
    let archived: serde_json::Value =
        serde_json::from_str(&snapshot_blob(&git_repo, snapshot_id, "ref_metadata.json")?)?;
    let mut duplicate_branch = archived.clone();
    let branch = archived["metadata"]["workspaces"][0][1]["stacks"][0]["branches"][0].clone();
    duplicate_branch["metadata"]["workspaces"][0][1]["stacks"][0]["branches"]
        .as_array_mut()
        .unwrap()
        .push(branch);
    let mut duplicate_reference = archived.clone();
    duplicate_reference["references"]
        .as_array_mut()
        .unwrap()
        .push(archived["references"][0].clone());
    let mut invalid_reference = archived;
    invalid_reference["references"][0]["ref_name"] = serde_json::json!("refs/heads/invalid..name");
    let changed_path = repo.projects_root().join("first");
    fs::write(&changed_path, "changed after snapshot")?;
    let metadata = ctx.db.get_cache()?.meta()?;
    let head = git_repo.head_id()?.detach();
    let oplog_head = ctx.oplog_head()?;
    for archived in [duplicate_branch, duplicate_reference, invalid_reference] {
        let mut tree = snapshot.tree()?.edit()?;
        tree.upsert(
            "ref_metadata.json",
            gix::object::tree::EntryKind::Blob,
            git_repo.write_blob(serde_json::to_vec(&archived)?)?,
        )?;
        let invalid_id = git_repo
            .write_object(gix::objs::Commit {
                tree: tree.write()?.detach(),
                ..snapshot.decode()?.to_owned()?
            })?
            .detach();
        let error = ctx
            .restore_snapshot(
                invalid_id,
                RestoreKind::RestoreFromSnapshotViaUndo,
                guard.write_permission(),
            )
            .expect_err("invalid metadata names and duplicate references must fail validation");
        assert!(
            error.to_string().contains("ref_metadata.json"),
            "restore identifies invalid metadata: {error:#}"
        );
        assert_eq!(
            fs::read_to_string(&changed_path)?,
            "changed after snapshot",
            "validation precedes worktree changes"
        );
        assert_eq!(
            ctx.db.get_cache()?.meta()?,
            metadata,
            "validation precedes database changes"
        );
        assert_eq!(
            git_repo.head_id()?.detach(),
            head,
            "validation precedes ref changes"
        );
        assert_eq!(
            ctx.oplog_head()?,
            oplog_head,
            "validation precedes oplog changes"
        );
    }
    Ok(())
}

#[test]
fn snapshot_resolves_refs_and_preserves_shared_segments_without_mutating_metadata()
-> anyhow::Result<()> {
    use but_core::ref_metadata::{
        Branch, StackId, Workspace, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    };
    let Test { repo, ctx } = &mut Test::from_scenario("multi-lane-with-shared-segment", &[]);
    let names = [
        vec!["A", "shared"],
        vec!["B", "shared"],
        vec!["D", "C", "shared"],
    ];
    let mut workspace = Workspace::default();
    for (position, names) in names.iter().enumerate() {
        workspace.stacks.push(WorkspaceStack {
            id: StackId::from_number_for_testing(position as u128 + 10),
            branches: names
                .iter()
                .map(|name| {
                    Ok(WorkspaceStackBranch {
                        ref_name: format!("refs/heads/{name}").try_into()?,
                        archived: false,
                    })
                })
                .collect::<anyhow::Result<_>>()?,
            workspacecommit_relation: WorkspaceCommitRelation::Merged,
        });
    }
    workspace.stacks[0].workspacecommit_relation =
        WorkspaceCommitRelation::MergeFrom { commit_id: None };
    workspace.stacks[1].workspacecommit_relation = WorkspaceCommitRelation::MergeFrom {
        commit_id: Some(repo.open_repo().rev_parse_single("shared")?.detach()),
    };
    let stale = WorkspaceStack {
        id: StackId::from_number_for_testing(42),
        branches: vec![WorkspaceStackBranch {
            ref_name: "refs/heads/shared".try_into()?,
            archived: true,
        }],
        workspacecommit_relation: WorkspaceCommitRelation::Outside,
    };
    workspace.stacks.push(stale.clone());
    let workspace_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .set_workspace(workspace_ref.as_ref(), &workspace)?;
    let mut shared = Branch::default();
    shared.review.pull_request = Some(42);
    ctx.db
        .get_cache_mut()?
        .meta_mut()?
        .set_branch("refs/heads/shared".try_into()?, &shared)?;
    let before = ctx.db.get_cache()?.meta()?;
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let repo = repo.open_repo();
    let archived = snapshot_metadata(&repo, snapshot_id)?;
    let archived_workspace = archived.metadata.workspace(workspace_ref.as_ref()).unwrap();
    for expected in &workspace.stacks {
        assert_eq!(
            archived_workspace
                .stacks
                .iter()
                .find(|stack| stack.id == expected.id),
            Some(expected),
            "shared segments and unapplied stacks retain their workspace metadata"
        );
    }
    assert_eq!(
        archived.metadata.branch("refs/heads/shared".try_into()?),
        Some(&shared),
        "shared segments have one authoritative per-ref review record"
    );
    for reference in archived.references {
        assert_eq!(
            reference.target,
            Some(repo.rev_parse_single(reference.ref_name.as_ref())?.detach()),
            "snapshot ref targets come from Git"
        );
    }
    assert_eq!(
        ctx.db.get_cache()?.meta()?,
        before,
        "snapshot preparation does not change live metadata"
    );
    Ok(())
}

#[test]
fn snapshot_keeps_heads_checked_out_in_linked_worktrees() -> anyhow::Result<()> {
    let Test { repo, ctx } = &mut Test::from_scenario("one-stack-two-commits", &["A"]);
    let tmp = tempfile::tempdir()?;
    let worktree = tmp.path().join("linked");
    repo.invoke_git(&format!("worktree add '{}' A", worktree.display()));
    ctx.settings.feature_flags.worktree_manipulation = true;
    ctx.db.get_cache_mut()?.worktree_meta_mut().mark_adopted()?;
    let guard = ctx.shared_worktree_access();
    let ws = ctx.workspace_from_head_uncached(guard.read_permission())?;
    assert!(
        !ws.graph.worktree_tips.is_empty(),
        "the fixture includes the active linked worktree"
    );
    assert!(
        ws.stacks
            .iter()
            .all(|stack| stack.segments.iter().all(|segment| segment
                .ref_name()
                .is_none_or(|name| name.as_bstr() != b"refs/heads/A"))),
        "the linked branch is omitted from the main-worktree projection"
    );
    drop(guard);
    let mut guard = ctx.exclusive_worktree_access();
    let snapshot_id = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::OnDemandSnapshot),
        guard.write_permission(),
    )?;
    let archived = snapshot_metadata(&repo.open_repo(), snapshot_id)?;
    let stack = &archived
        .metadata
        .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)
        .unwrap()
        .stacks[0];
    assert!(
        stack.is_in_workspace(),
        "a linked checkout does not unapply its recorded stack"
    );
    assert_eq!(
        stack.branches[0].ref_name.as_bstr(),
        b"refs/heads/A",
        "the archive retains the linked branch"
    );
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
    let metadata = snapshot_metadata(&repo, snapshot_id)?;

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
        metadata.metadata.workspaces().count() == 0,
        "a target-only project has no synthetic workspace metadata"
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
    configure_default_target(ctx)?;
    let original = ctx.project_meta()?;

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
    let before_metadata = ctx.db.get_cache()?.meta()?;

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
        ctx.db.get_cache()?.meta()?,
        before_metadata,
        "a failed restore does not rewrite database metadata"
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
            .map(|name| gix::refs::FullName::try_from(*name))
            .collect::<Result<Vec<_>, _>>()?,
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
    tree.remove("ref_metadata.json")?;
    tree.upsert(
        "virtual_branches.toml",
        gix::object::tree::EntryKind::Blob,
        repo.write_blob(b"[branches]\n")?,
    )?;
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
    tree.remove("ref_metadata.json")?;
    tree.upsert(
        "virtual_branches.toml",
        gix::object::tree::EntryKind::Blob,
        repo.write_blob(b"[branches]\n")?,
    )?;
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
    tree.remove("ref_metadata.json")?;
    let virtual_branches = format!(
        "[branches]\n\n[default_target]\nbranchName = \"main\"\nremoteName = \"origin\"\nremoteUrl = \"\"\nsha = \"{target_id}\"\npushRemoteName = \"origin\"\n"
    );
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
