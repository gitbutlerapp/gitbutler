#![expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]

use std::{io::Write, path::Path};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt as _;

use anyhow::Context as _;
use bstr::ByteSlice as _;
use but_core::{GitConfigSettings, RepositoryExt as _};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::BranchManagerExt;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gitbutler_oplog::{OplogExt, RestoreKind};
use gitbutler_stack::VirtualBranchesHandle;
use itertools::Itertools;

use super::*;

#[test]
fn workdir_vbranch_restore() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let worktree_dir = repo.path();
    for round in 0..3 {
        let line_count = round * 20;
        fs::write(
            worktree_dir.join(format!("file{round}.txt")),
            make_lines(line_count),
        )?;
        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = ctx.branch_manager().create_virtual_branch(
            &BranchCreateRequest {
                name: Some(round.to_string()),
                ..Default::default()
            },
            guard.write_permission(),
        )?;
        drop(guard);
        super::create_commit(ctx, stack_entry.id, &format!("commit {round}"))?;
        assert_eq!(
            wd_file_count(&worktree_dir)?,
            round + 1,
            "each round creates a new file, and it persists"
        );
    }
    let mut guard = ctx.exclusive_worktree_access();
    let _empty = ctx
        .branch_manager()
        .create_virtual_branch(&Default::default(), guard.write_permission())?;
    drop(guard);

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(
        snapshots.len(),
        7,
        "3 vbranches + 3 commits + one empty branch"
    );

    let previous_files_count = wd_file_count(&worktree_dir)?;
    assert_eq!(previous_files_count, 3, "one file per round");
    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(
        snapshots[0].commit_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )
    .expect("restoration succeeds");

    assert_eq!(
        ctx.snapshots_iter(None, Vec::new(), None)?
            .take(10)
            .collect::<anyhow::Result<Vec<_>>>()?
            .len(),
        8,
        "all the previous + 1 restore commit"
    );

    let current_files = wd_file_count(&worktree_dir)?;
    assert_eq!(
        current_files, previous_files_count,
        "we only removed an empty vbranch, no worktree change"
    );
    Ok(())
}

#[test]
fn restore_snapshot_reverts_the_target() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    // A second remote branch to switch to.
    {
        let gix_repo = repo.open();
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

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/other".parse()?,
        guard.write_permission(),
    )?;
    assert_eq!(
        ctx.project_meta()?.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/other".to_string())
    );

    ctx.restore_snapshot(
        snapshot_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    assert_eq!(
        ctx.project_meta()?.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/master".to_string()),
        "undoing a base-branch switch reverts the target everywhere, not just in the TOML"
    );
    Ok(())
}

#[test]
fn snapshot_creation_works_with_unmerged_index() -> anyhow::Result<()> {
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
        repo.path().join("conflicted.txt"),
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

fn conflict_index_entry(path: impl AsRef<[u8]>, stage: u16, blob: git2::Oid) -> git2::IndexEntry {
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

    let repo = repo.open();
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
    let repo = repo.open();
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
        &repo.open(),
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

    assert_eq!(ctx.project_meta()?, original);
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
    let repo = repo.open();
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
    assert_eq!(ctx.project_meta()?, original);
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
        &repo.open(),
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
    assert_eq!(ctx.project_meta()?, before_meta);
    assert_eq!(ctx.oplog_head()?, before_oplog_head);
    assert_eq!(fs::read(live_path)?, before_toml);
    Ok(())
}

fn wd_file_count(worktree_dir: &&Path) -> anyhow::Result<usize> {
    Ok(glob::glob(&worktree_dir.join("file*").to_string_lossy())?.count())
}

fn make_lines(count: usize) -> Vec<u8> {
    (0..count).map(|n| n.to_string()).join("\n").into()
}

fn configure_default_target(ctx: &mut Context) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse()?,
        guard.write_permission(),
    )?;
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
        "\n[default_target]\nbranchName = \"master\"\nremoteName = \"origin\"\nremoteUrl = \"\"\nsha = \"{target_id}\"\n"
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

fn commit_summary(ctx: &Context, id: gix::ObjectId) -> anyhow::Result<String> {
    Ok(ctx
        .repo
        .get()?
        .find_commit(id)?
        .message()?
        .summary()
        .to_str_lossy()
        .into_owned())
}

#[test]
fn basic_oplog() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse()?,
        guard.write_permission(),
    )?;
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = ctx
        .branch_manager()
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())?;
    drop(guard);

    // create commit
    fs::write(repo.path().join("file.txt"), "content")?;
    let _commit1_id = super::create_commit(ctx, stack_entry.id, "commit one")?;

    // dont store large files
    let file_path = repo.path().join("large.txt");
    // write 33MB of random data in the file
    let mut file = std::fs::File::create(file_path)?;
    for _ in 0..33 * 1024 {
        let data = [0u8; 1024];
        file.write_all(&data)?;
    }

    // create commit with large file
    fs::write(repo.path().join("file2.txt"), "content2")?;
    fs::write(repo.path().join("file3.txt"), "content3")?;
    let commit2_id = super::create_commit(ctx, stack_entry.id, "commit two")?;

    // Create conflict state
    let conflicts_path = repo.path().join(".git").join("conflicts");
    std::fs::write(&conflicts_path, "conflict A")?;
    let base_merge_parent_path = repo.path().join(".git").join("base_merge_parent");
    std::fs::write(&base_merge_parent_path, "parent A")?;

    // create state with conflict state
    let mut guard = ctx.exclusive_worktree_access();
    let _empty_branch_id = ctx
        .branch_manager()
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())?;
    drop(guard);

    std::fs::remove_file(&base_merge_parent_path)?;
    std::fs::remove_file(&conflicts_path)?;

    fs::write(repo.path().join("file4.txt"), "content4")?;
    let _commit3_id = super::create_commit(ctx, stack_entry.id, "commit three")?;

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|d| d.0 == stack_entry.id)
        .unwrap();

    assert_eq!(stack_details(ctx).len(), 2);

    assert_eq!(b.branch_details[0].clone().commits.len(), 3);
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[0].id)?.len(),
        1
    );
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].clone().commits[1].id)?.len(),
        3
    );

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;

    let ops = snapshots
        .iter()
        .map(|c| &c.details.as_ref().unwrap().title)
        .collect::<Vec<_>>();

    assert_eq!(
        ops,
        vec![
            "CreateCommit",
            "CreateBranch",
            "CreateCommit",
            "CreateCommit",
            "CreateBranch",
        ]
    );

    {
        let mut guard = ctx.exclusive_worktree_access();
        ctx.restore_snapshot(
            snapshots[1].clone().commit_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )?;
    }

    // restores the conflict files
    let file_lines = std::fs::read_to_string(&conflicts_path)?;
    assert_eq!(file_lines, "conflict A");
    let file_lines = std::fs::read_to_string(&base_merge_parent_path)?;
    assert_eq!(file_lines, "parent A");

    {
        let mut guard = ctx.exclusive_worktree_access();
        ctx.restore_snapshot(
            snapshots[2].clone().commit_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )?;
    }

    // the restore removed our new branch
    assert_eq!(stack_details(ctx).len(), 1);

    // assert that the conflicts file was removed
    assert!(!&conflicts_path.try_exists()?);

    // remove commit2_oid from odb
    let commit_str = &commit2_id.to_string();
    // find file in odb
    let file_path = repo
        .path()
        .join(".git")
        .join("objects")
        .join(&commit_str[..2]);
    let file_path = file_path.join(&commit_str[2..]);
    assert!(file_path.exists());
    // remove file
    std::fs::remove_file(file_path)?;

    // try to look up that object
    let commit_missing = !ctx.repo.get()?.has_object(commit2_id);
    assert!(commit_missing);

    {
        let mut guard = ctx.exclusive_worktree_access();
        // The ctx stores the `git2` repo
        ctx.restore_snapshot(
            snapshots[1].commit_id,
            RestoreKind::RestoreFromSnapshotViaUndo,
            guard.write_permission(),
        )?;
    }

    // test missing commits are recreated
    let commit_restored = ctx.repo.get()?.has_object(commit2_id);
    assert!(commit_restored);

    let file_path = repo.path().join("large.txt");
    assert!(file_path.exists());

    let file_path = repo.path().join("file.txt");
    let file_lines = std::fs::read_to_string(file_path)?;
    assert_eq!(file_lines, "content");

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
fn workspace_commits_ignore_commit_signing_configuration() -> anyhow::Result<()> {
    let Test { ctx, .. } = &mut Test::default();
    configure_default_target(ctx)?;
    enable_failing_commit_signing(ctx)?;

    let workspace_commit_id = gitbutler_branch_actions::update_workspace_commit(ctx, false)?;
    let repo = ctx.repo.get()?;
    assert!(
        !has_signature(&repo, workspace_commit_id)?,
        "GitButler workspace commits must stay unsigned even when user commit signing is enabled"
    );
    Ok(())
}

#[test]
fn restores_gitbutler_workspace() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse()?,
        guard.write_permission(),
    )?;
    drop(guard);

    assert_eq!(
        VirtualBranchesHandle::new(ctx.project_data_dir())
            .list_stacks_in_workspace()?
            .len(),
        0
    );
    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = ctx
        .branch_manager()
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())?;
    drop(guard);
    assert_eq!(
        VirtualBranchesHandle::new(ctx.project_data_dir())
            .list_stacks_in_workspace()?
            .len(),
        1
    );

    // create commit
    fs::write(repo.path().join("file.txt"), "content")?;
    let _commit1_id = super::create_commit(ctx, stack_entry.id, "commit one")?;

    // check the workspace commit
    let commit1_id = ctx.repo.get()?.head_id()?.detach();
    assert_eq!(
        commit_summary(ctx, commit1_id)?,
        GITBUTLER_WORKSPACE_COMMIT_TITLE
    );

    // create second commit
    fs::write(repo.path().join("file.txt"), "changed content")?;
    let _commit2_id = super::create_commit(ctx, stack_entry.id, "commit two")?;

    // check the workspace commit changed
    {
        let commit2_id = ctx.repo.get()?.head_id()?.detach();
        let message = commit_summary(ctx, commit2_id)?;
        assert_eq!(message, GITBUTLER_WORKSPACE_COMMIT_TITLE);
        assert_ne!(commit1_id, commit2_id);
    }

    // restore the first
    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(
        snapshots.len(),
        3,
        "one vbranch, two commits, one snapshot each"
    );

    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(
        snapshots[0].commit_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )
    .expect("can restore the most recent snapshot, to undo commit 2, resetting to commit 1");
    drop(guard);

    assert_eq!(
        commit1_id,
        ctx.repo.get()?.head_id()?.detach(),
        "head now points to the first commit, it's not commit 2 anymore"
    );

    let stacks = VirtualBranchesHandle::new(ctx.project_data_dir()).list_stacks_in_workspace()?;
    assert_eq!(
        stacks.len(),
        1,
        "vbranches aren't affected by this (only the head commit)"
    );
    let all_snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(
        all_snapshots.len(),
        4,
        "the restore is tracked as separate snapshot"
    );

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(1)
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        ctx.snapshots_iter(None, Vec::new(), None)?
            .take(1)
            .collect::<anyhow::Result<Vec<_>>>()?,
        snapshots,
        "traversal from the oplog head is the same as when no cursor is specified"
    );
    assert_eq!(
        ctx.snapshots_iter(Some(all_snapshots[2].commit_id), Vec::new(), None)?
            .take(10)
            .collect::<anyhow::Result<Vec<_>>>()?,
        &all_snapshots[3..],
    );

    Ok(())
}

/// Restoring a snapshot must not fail when the workspace contains a branch
/// with zero commits (head == target). Such branches have no `commits`
/// subtree in the snapshot tree, and the restore code must skip them
/// instead of erroring out.
#[test]
fn restore_snapshot_with_empty_branch_in_workspace() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    configure_default_target(ctx)?;

    // Create a branch *with* a commit so the snapshot has something to reconstitute.
    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = ctx.branch_manager().create_virtual_branch(
        &BranchCreateRequest {
            name: Some("has-commits".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )?;
    drop(guard);

    fs::write(repo.path().join("file.txt"), "hello")?;
    let _commit_id = super::create_commit(ctx, stack_entry.id, "first commit")?;

    // Now create a second branch that stays empty (zero commits).
    let mut guard = ctx.exclusive_worktree_access();
    let _empty_branch = ctx.branch_manager().create_virtual_branch(
        &BranchCreateRequest {
            name: Some("empty-branch".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )?;
    drop(guard);

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;
    // CreateBranch (empty), CreateCommit, CreateBranch (has-commits)
    assert_eq!(snapshots.len(), 3);

    // Restore to the snapshot taken *before* the commit was created.
    // This forces the restore code to walk the snapshot tree that contains
    // the empty branch entry (no `commits` subtree).
    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(
        snapshots[1].commit_id,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )
    .expect("restore must succeed even with an empty branch in the workspace");
    drop(guard);

    // Verify the restore was recorded.
    let snapshots_after = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(
        snapshots_after.len(),
        snapshots.len() + 1,
        "the restore itself creates a new snapshot entry"
    );

    Ok(())
}

// test operations-log.toml head is not a commit
#[test]
fn head_corrupt_is_recreated_automatically() {
    let Test { ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);
    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)
        .unwrap()
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        snapshots.len(),
        1,
        "No snapshots can be created before a base branch is set, hence only 1 snapshot despite two calls"
    );

    // overwrite oplog head with a non-commit sha
    let oplog_path = ctx.project_data_dir().join("operations-log.toml");
    fs::write(
        oplog_path,
        "head_sha = \"758d54f587227fba3da3b61fbb54a99c17903d59\"",
    )
    .unwrap();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .expect("the snapshot doesn't fail despite the corrupt head");

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)
        .unwrap()
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        snapshots.len(),
        1,
        "it should have just reset the oplog head, so only 1, not 2"
    );
}

#[test]
fn first_snapshot_diff_works() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &mut Test::default();

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse()?,
        guard.write_permission(),
    )?;
    drop(guard);

    let mut guard = ctx.exclusive_worktree_access();
    let stack_entry = ctx
        .branch_manager()
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())?;
    drop(guard);

    // create first commit to create the very first snapshot
    fs::write(repo.path().join("file.txt"), "content")?;
    let _commit_id = super::create_commit(ctx, stack_entry.id, "first commit")?;

    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .take(10)
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert!(!snapshots.is_empty(), "Should have at least one snapshot");

    // Test snapshot_diff on all snapshots to make sure none fail (including the first one)
    for snapshot in &snapshots {
        let diff_result = ctx.snapshot_diff(snapshot.commit_id, None);
        assert!(
            diff_result.is_ok(),
            "snapshot_diff should work for snapshot {}, got error: {:?}",
            snapshot.commit_id,
            diff_result.err()
        );
    }

    Ok(())
}
