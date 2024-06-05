use super::*;
use itertools::Itertools;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

#[tokio::test]
async fn workdir_vbranch_restore() -> anyhow::Result<()> {
    let test = Test::default();
    let Test {
        repository,
        project_id,
        controller,
        project,
        ..
    } = &test;

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let worktree_dir = repository.path();
    for round in 0..3 {
        let line_count = round * 20;
        fs::write(
            worktree_dir.join(format!("file{round}.txt")),
            &make_lines(line_count),
        )?;
        let branch_id = controller
            .create_virtual_branch(
                *project_id,
                &branch::BranchCreateRequest {
                    name: Some(round.to_string()),
                    ..Default::default()
                },
            )
            .await?;
        controller
            .create_commit(
                *project_id,
                branch_id,
                &format!("commit {round}"),
                None,
                false, /* run hook */
            )
            .await?;
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
    let _empty = controller
        .create_virtual_branch(*project_id, &Default::default())
        .await?;

    let snapshots = project.list_snapshots(10, None)?;
    assert_eq!(
        snapshots.len(),
        7,
        "3 vbranches + 3 commits + one empty branch"
    );

    let previous_files_count = wd_file_count(&worktree_dir)?;
    assert_eq!(previous_files_count, 3, "one file per round");
    project
        .restore_snapshot(snapshots[0].commit_id)
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

#[tokio::test]
async fn basic_oplog() -> anyhow::Result<()> {
    let Test {
        repository,
        project_id,
        controller,
        project,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse()?)
        .await?;

    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await?;

    // create commit
    fs::write(repository.path().join("file.txt"), "content")?;
    let _commit1_id = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
        .await?;

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
    let commit2_id = controller
        .create_commit(*project_id, branch_id, "commit two", None, false)
        .await?;

    // Create conflict state
    let conflicts_path = repository.path().join(".git").join("conflicts");
    std::fs::write(&conflicts_path, "conflict A")?;
    let base_merge_parent_path = repository.path().join(".git").join("base_merge_parent");
    std::fs::write(&base_merge_parent_path, "parent A")?;

    // create state with conflict state
    let _empty_branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await?;

    std::fs::remove_file(&base_merge_parent_path)?;
    std::fs::remove_file(&conflicts_path)?;

    fs::write(repository.path().join("file4.txt"), "content4")?;
    let _commit3_id = controller
        .create_commit(*project_id, branch_id, "commit three", None, false)
        .await?;

    let branch = controller
        .list_virtual_branches(*project_id)
        .await?
        .0
        .into_iter()
        .find(|b| b.id == branch_id)
        .unwrap();

    let branches = controller.list_virtual_branches(*project_id).await?;
    assert_eq!(branches.0.len(), 2);

    assert_eq!(branch.commits.len(), 3);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[1].files.len(), 3);

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

    project.restore_snapshot(snapshots[1].clone().commit_id)?;

    // restores the conflict files
    let file_lines = std::fs::read_to_string(&conflicts_path)?;
    assert_eq!(file_lines, "conflict A");
    let file_lines = std::fs::read_to_string(&base_merge_parent_path)?;
    assert_eq!(file_lines, "parent A");

    assert_eq!(snapshots[1].lines_added, 2);
    assert_eq!(snapshots[1].lines_removed, 0);

    project.restore_snapshot(snapshots[2].clone().commit_id)?;

    // the restore removed our new branch
    let branches = controller.list_virtual_branches(*project_id).await?;
    assert_eq!(branches.0.len(), 1);

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

    project.restore_snapshot(snapshots[1].clone().commit_id)?;

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

#[tokio::test]
async fn restores_gitbutler_integration() -> anyhow::Result<()> {
    let Test {
        repository,
        project_id,
        controller,
        project,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse()?)
        .await?;

    assert_eq!(project.virtual_branches().list_branches()?.len(), 0);
    let branch_id = controller
        .create_virtual_branch(*project_id, &branch::BranchCreateRequest::default())
        .await?;
    assert_eq!(project.virtual_branches().list_branches()?.len(), 1);

    // create commit
    fs::write(repository.path().join("file.txt"), "content")?;
    let _commit1_id = controller
        .create_commit(*project_id, branch_id, "commit one", None, false)
        .await?;

    let repo = git2::Repository::open(&project.path)?;

    // check the integration commit
    let head = repo.head().expect("never unborn");
    let commit = &head.peel_to_commit()?;
    let commit1_id = commit.id();
    let message = commit.summary().unwrap();
    assert_eq!(message, "GitButler Integration Commit");

    // create second commit
    fs::write(repository.path().join("file.txt"), "changed content")?;
    let _commit2_id = controller
        .create_commit(*project_id, branch_id, "commit two", None, false)
        .await?;

    // check the integration commit changed
    let head = repo.head().expect("never unborn");
    let commit = &head.peel_to_commit()?;
    let commit2_id = commit.id();
    let message = commit.summary().unwrap();
    assert_eq!(message, "GitButler Integration Commit");
    assert_ne!(commit1_id, commit2_id);

    // restore the first
    let snapshots = project.list_snapshots(10, None)?;
    assert_eq!(
        snapshots.len(),
        3,
        "one vbranch, two commits, one snapshot each"
    );
    project
        .restore_snapshot(snapshots[0].commit_id)
        .expect("can restore the most recent snapshot, to undo commit 2, resetting to commit 1");

    let head = repo.head().expect("never unborn");
    let current_commit = &head.peel_to_commit()?;
    let id_of_restored_commit = current_commit.id();
    assert_eq!(
        commit1_id, id_of_restored_commit,
        "head now points to the first commit, it's not commit 2 anymore"
    );

    let vbranches = project.virtual_branches().list_branches()?;
    assert_eq!(
        vbranches.len(),
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
#[tokio::test]
async fn head_corrupt_is_recreated_automatically() {
    let Test {
        repository,
        project_id,
        controller,
        project,
        ..
    } = &Test::default();

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();
    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
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

    controller
        .set_base_branch(*project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .expect("the snapshot doesn't fail despite the corrupt head");

    let snapshots = project.list_snapshots(10, None).unwrap();
    assert_eq!(
        snapshots.len(),
        1,
        "it should have just reset the oplog head, so only 1, not 2"
    );
}
