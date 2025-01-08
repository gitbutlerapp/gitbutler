use std::{io::Write, path::Path, time::Duration};

use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::list_commit_files;
use gitbutler_oplog::OplogExt;
use gitbutler_stack::VirtualBranchesHandle;
use itertools::Itertools;

use super::*;

#[test]
fn workdir_vbranch_restore() -> anyhow::Result<()> {
    let test = Test::default();
    let Test {
        repository,

        project,
        ..
    } = &test;

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let worktree_dir = repository.path();
    for round in 0..3 {
        let line_count = round * 20;
        fs::write(
            worktree_dir.join(format!("file{round}.txt")),
            make_lines(line_count),
        )?;
        let branch_id = gitbutler_branch_actions::create_virtual_branch(
            project,
            &BranchCreateRequest {
                name: Some(round.to_string()),
                ..Default::default()
            },
        )?;
        gitbutler_branch_actions::create_commit(
            project,
            branch_id,
            &format!("commit {round}"),
            None,
        )?;
        assert_eq!(
            wd_file_count(&worktree_dir)?,
            round + 1,
            "each round creates a new file, and it persists"
        );
        assert_eq!(
            project.should_auto_snapshot(Duration::ZERO)?,
            line_count > 20
        );
    }
    let _empty = gitbutler_branch_actions::create_virtual_branch(project, &Default::default())?;

    let snapshots = project.list_snapshots(10, None)?;
    assert_eq!(
        snapshots.len(),
        7,
        "3 vbranches + 3 commits + one empty branch"
    );

    let previous_files_count = wd_file_count(&worktree_dir)?;
    assert_eq!(previous_files_count, 3, "one file per round");
    let mut guard = project.exclusive_worktree_access();
    project
        .restore_snapshot(snapshots[0].commit_id, guard.write_permission())
        .expect("restoration succeeds");

    assert_eq!(
        project.list_snapshots(10, None)?.len(),
        8,
        "all the previous + 1 restore commit"
    );

    let current_files = wd_file_count(&worktree_dir)?;
    assert_eq!(
        current_files, previous_files_count,
        "we only removed an empty vbranch, no worktree change"
    );
    assert!(
        !project.should_auto_snapshot(Duration::ZERO)?,
        "not enough lines changed"
    );
    Ok(())
}

fn wd_file_count(worktree_dir: &&Path) -> anyhow::Result<usize> {
    Ok(glob::glob(&worktree_dir.join("file*").to_string_lossy())?.count())
}

fn make_lines(count: usize) -> Vec<u8> {
    (0..count).map(|n| n.to_string()).join("\n").into()
}

#[test]
fn basic_oplog() -> anyhow::Result<()> {
    let Test {
        repository,

        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(project, &"refs/remotes/origin/master".parse()?)?;

    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())?;

    // create commit
    fs::write(repository.path().join("file.txt"), "content")?;
    let _commit1_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None)?;

    // dont store large files
    let file_path = repository.path().join("large.txt");
    // write 33MB of random data in the file
    let mut file = std::fs::File::create(file_path)?;
    for _ in 0..33 * 1024 {
        let data = [0u8; 1024];
        file.write_all(&data)?;
    }

    // create commit with large file
    fs::write(repository.path().join("file2.txt"), "content2")?;
    fs::write(repository.path().join("file3.txt"), "content3")?;
    let commit2_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None)?;

    // Create conflict state
    let conflicts_path = repository.path().join(".git").join("conflicts");
    std::fs::write(&conflicts_path, "conflict A")?;
    let base_merge_parent_path = repository.path().join(".git").join("base_merge_parent");
    std::fs::write(&base_merge_parent_path, "parent A")?;

    // create state with conflict state
    let _empty_branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())?;

    std::fs::remove_file(&base_merge_parent_path)?;
    std::fs::remove_file(&conflicts_path)?;

    fs::write(repository.path().join("file4.txt"), "content4")?;
    let _commit3_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit three", None)?;

    let branch = gitbutler_branch_actions::list_virtual_branches(project)?
        .branches
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    let branches = gitbutler_branch_actions::list_virtual_branches(project)?;
    assert_eq!(branches.branches.len(), 2);

    assert_eq!(branch.series[0].clone()?.patches.len(), 3);
    assert_eq!(
        list_commit_files(project, branch.series[0].clone()?.patches[0].id)?.len(),
        1
    );
    assert_eq!(
        list_commit_files(project, branch.series[0].clone()?.patches[1].id)?.len(),
        3
    );

    let snapshots = project.list_snapshots(10, None)?;

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
        let mut guard = project.exclusive_worktree_access();
        project.restore_snapshot(snapshots[1].clone().commit_id, guard.write_permission())?;
    }

    // restores the conflict files
    let file_lines = std::fs::read_to_string(&conflicts_path)?;
    assert_eq!(file_lines, "conflict A");
    let file_lines = std::fs::read_to_string(&base_merge_parent_path)?;
    assert_eq!(file_lines, "parent A");

    assert_eq!(snapshots[1].lines_added, 2);
    assert_eq!(snapshots[1].lines_removed, 0);

    {
        let mut guard = project.exclusive_worktree_access();
        project.restore_snapshot(snapshots[2].clone().commit_id, guard.write_permission())?;
    }

    // the restore removed our new branch
    let branches = gitbutler_branch_actions::list_virtual_branches(project)?;
    assert_eq!(branches.branches.len(), 1);

    // assert that the conflicts file was removed
    assert!(!&conflicts_path.try_exists()?);

    // remove commit2_oid from odb
    let commit_str = &commit2_id.to_string();
    // find file in odb
    let file_path = repository
        .path()
        .join(".git")
        .join("objects")
        .join(&commit_str[..2]);
    let file_path = file_path.join(&commit_str[2..]);
    assert!(file_path.exists());
    // remove file
    std::fs::remove_file(file_path)?;

    // try to look up that object
    let repo = git2::Repository::open(&project.path)?;
    let commit = repo.find_commit(commit2_id);
    assert!(commit.is_err());

    {
        let mut guard = project.exclusive_worktree_access();
        project.restore_snapshot(snapshots[1].clone().commit_id, guard.write_permission())?;
    }

    // test missing commits are recreated
    let commit = repo.find_commit(commit2_id);
    assert!(commit.is_ok());

    let file_path = repository.path().join("large.txt");
    assert!(file_path.exists());

    let file_path = repository.path().join("file.txt");
    let file_lines = std::fs::read_to_string(file_path)?;
    assert_eq!(file_lines, "content");

    assert!(
        !project.should_auto_snapshot(Duration::ZERO)?,
        "not enough lines changed"
    );
    Ok(())
}

#[test]
fn restores_gitbutler_workspace() -> anyhow::Result<()> {
    let Test {
        repository,

        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(project, &"refs/remotes/origin/master".parse()?)?;

    assert_eq!(
        VirtualBranchesHandle::new(project.gb_dir())
            .list_stacks_in_workspace()?
            .len(),
        0
    );
    let branch_id =
        gitbutler_branch_actions::create_virtual_branch(project, &BranchCreateRequest::default())?;
    assert_eq!(
        VirtualBranchesHandle::new(project.gb_dir())
            .list_stacks_in_workspace()?
            .len(),
        1
    );

    // create commit
    fs::write(repository.path().join("file.txt"), "content")?;
    let _commit1_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit one", None)?;

    let repo = git2::Repository::open(&project.path)?;

    // check the workspace commit
    let head = repo.head().expect("never unborn");
    let commit = &head.peel_to_commit()?;
    let commit1_id = commit.id();
    let message = commit.summary().unwrap();
    assert_eq!(message, GITBUTLER_WORKSPACE_COMMIT_TITLE);

    // create second commit
    fs::write(repository.path().join("file.txt"), "changed content")?;
    let _commit2_id =
        gitbutler_branch_actions::create_commit(project, branch_id, "commit two", None)?;

    // check the workspace commit changed
    let head = repo.head().expect("never unborn");
    let commit = &head.peel_to_commit()?;
    let commit2_id = commit.id();
    let message = commit.summary().unwrap();
    assert_eq!(message, GITBUTLER_WORKSPACE_COMMIT_TITLE);
    assert_ne!(commit1_id, commit2_id);

    // restore the first
    let snapshots = project.list_snapshots(10, None)?;
    assert_eq!(
        snapshots.len(),
        3,
        "one vbranch, two commits, one snapshot each"
    );

    let mut guard = project.exclusive_worktree_access();
    project
        .restore_snapshot(snapshots[0].commit_id, guard.write_permission())
        .expect("can restore the most recent snapshot, to undo commit 2, resetting to commit 1");

    let head = repo.head().expect("never unborn");
    let current_commit = &head.peel_to_commit()?;
    let id_of_restored_commit = current_commit.id();
    assert_eq!(
        commit1_id, id_of_restored_commit,
        "head now points to the first commit, it's not commit 2 anymore"
    );

    let stacks = VirtualBranchesHandle::new(project.gb_dir()).list_stacks_in_workspace()?;
    assert_eq!(
        stacks.len(),
        1,
        "vbranches aren't affected by this (only the head commit)"
    );
    let all_snapshots = project.list_snapshots(10, None)?;
    assert_eq!(
        all_snapshots.len(),
        4,
        "the restore is tracked as separate snapshot"
    );

    assert_eq!(
        project.list_snapshots(0, None)?.len(),
        0,
        "it respects even non-sensical limits"
    );

    let snapshots = project.list_snapshots(1, None)?;
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        project.list_snapshots(1, Some(snapshots[0].commit_id))?,
        snapshots,
        "traversal from oplog head is the same as if it wasn't specified, and the given head is returned first"
    );
    assert_eq!(
        project.list_snapshots(10, Some(all_snapshots[2].commit_id))?,
        &all_snapshots[2..],
    );

    let first_snapshot = all_snapshots.last().unwrap();
    assert_eq!(
        (
            first_snapshot.lines_added,
            first_snapshot.lines_removed,
            first_snapshot.files_changed.len()
        ),
        (0, 0, 0),
        "The first snapshot is intentionally not listing everything as changed"
    );
    Ok(())
}

// test operations-log.toml head is not a commit
#[test]
fn head_corrupt_is_recreated_automatically() {
    let Test {
        repository,

        project,
        ..
    } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();
    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .unwrap();

    let snapshots = project.list_snapshots(10, None).unwrap();
    assert_eq!(
        snapshots.len(),
        1,
        "No snapshots can be created before a base branch is set, hence only 1 snapshot despite two calls"
    );

    // overwrite oplog head with a non-commit sha
    let oplog_path = repository.path().join(".git/gitbutler/operations-log.toml");
    fs::write(
        oplog_path,
        "head_sha = \"758d54f587227fba3da3b61fbb54a99c17903d59\"",
    )
    .unwrap();

    gitbutler_branch_actions::set_base_branch(
        project,
        &"refs/remotes/origin/master".parse().unwrap(),
    )
    .expect("the snapshot doesn't fail despite the corrupt head");

    let snapshots = project.list_snapshots(10, None).unwrap();
    assert_eq!(
        snapshots.len(),
        1,
        "it should have just reset the oplog head, so only 1, not 2"
    );
}
