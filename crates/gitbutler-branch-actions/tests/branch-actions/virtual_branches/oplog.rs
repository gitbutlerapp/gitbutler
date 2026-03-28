use std::{io::Write, path::Path};

use bstr::ByteSlice as _;
use but_core::{GitConfigSettings, RepositoryExt as _};
use but_testsupport::legacy::stack_details;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_oplog::OplogExt;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
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
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
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
    let _empty = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &Default::default(),
        guard.write_permission(),
    )?;
    drop(guard);

    let snapshots = ctx.list_snapshots(10, None, Vec::new(), None)?;
    assert_eq!(
        snapshots.len(),
        7,
        "3 vbranches + 3 commits + one empty branch"
    );

    let previous_files_count = wd_file_count(&worktree_dir)?;
    assert_eq!(previous_files_count, 3, "one file per round");
    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(snapshots[0].commit_id, guard.write_permission())
        .expect("restoration succeeds");

    assert_eq!(
        ctx.list_snapshots(10, None, Vec::new(), None)?.len(),
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
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )?;
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
    let _empty_branch_id = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )?;
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

    let snapshots = ctx.list_snapshots(10, None, Vec::new(), None)?;

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
        ctx.restore_snapshot(snapshots[1].clone().commit_id, guard.write_permission())?;
    }

    // restores the conflict files
    let file_lines = std::fs::read_to_string(&conflicts_path)?;
    assert_eq!(file_lines, "conflict A");
    let file_lines = std::fs::read_to_string(&base_merge_parent_path)?;
    assert_eq!(file_lines, "parent A");

    {
        let mut guard = ctx.exclusive_worktree_access();
        ctx.restore_snapshot(snapshots[2].clone().commit_id, guard.write_permission())?;
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
        ctx.restore_snapshot(snapshots[1].commit_id, guard.write_permission())?;
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
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )?;
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
    let snapshots = ctx.list_snapshots(10, None, Vec::new(), None)?;
    assert_eq!(
        snapshots.len(),
        3,
        "one vbranch, two commits, one snapshot each"
    );

    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(snapshots[0].commit_id, guard.write_permission())
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
    let all_snapshots = ctx.list_snapshots(10, None, Vec::new(), None)?;
    assert_eq!(
        all_snapshots.len(),
        4,
        "the restore is tracked as separate snapshot"
    );

    assert_eq!(
        ctx.list_snapshots(0, None, Vec::new(), None)?.len(),
        0,
        "it respects even non-sensical limits"
    );

    let snapshots = ctx.list_snapshots(1, None, Vec::new(), None)?;
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        ctx.list_snapshots(1, None, Vec::new(), None)?,
        snapshots,
        "traversal from oplog head is the same as if it wasn't specified, and the given head is returned first"
    );
    assert_eq!(
        ctx.list_snapshots(10, Some(all_snapshots[2].commit_id), Vec::new(), None)?,
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
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
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
    let _empty_branch = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest {
            name: Some("empty-branch".into()),
            ..Default::default()
        },
        guard.write_permission(),
    )?;
    drop(guard);

    let snapshots = ctx.list_snapshots(10, None, Vec::new(), None)?;
    // CreateBranch (empty), CreateCommit, CreateBranch (has-commits)
    assert_eq!(snapshots.len(), 3);

    // Restore to the snapshot taken *before* the commit was created.
    // This forces the restore code to walk the snapshot tree that contains
    // the empty branch entry (no `commits` subtree).
    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(snapshots[1].commit_id, guard.write_permission())
        .expect("restore must succeed even with an empty branch in the workspace");
    drop(guard);

    // Verify the restore was recorded.
    let snapshots_after = ctx.list_snapshots(10, None, Vec::new(), None)?;
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

    let snapshots = ctx.list_snapshots(10, None, Vec::new(), None).unwrap();
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

    let snapshots = ctx.list_snapshots(10, None, Vec::new(), None).unwrap();
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
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        guard.write_permission(),
    )?;
    drop(guard);

    // create first commit to create the very first snapshot
    fs::write(repo.path().join("file.txt"), "content")?;
    let _commit_id = super::create_commit(ctx, stack_entry.id, "first commit")?;

    let snapshots = ctx.list_snapshots(10, None, Vec::new(), None)?;
    assert!(!snapshots.is_empty(), "Should have at least one snapshot");

    // Test snapshot_diff on all snapshots to make sure none fail (including the first one)
    for snapshot in &snapshots {
        let diff_result = ctx.snapshot_diff(snapshot.commit_id);
        assert!(
            diff_result.is_ok(),
            "snapshot_diff should work for snapshot {}, got error: {:?}",
            snapshot.commit_id,
            diff_result.err()
        );
    }

    Ok(())
}
