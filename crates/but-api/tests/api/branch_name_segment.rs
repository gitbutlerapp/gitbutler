use but_api::commit::json::ChangesSource;
use but_testsupport::{CommandExt as _, git_at_dir};
use gitbutler_oplog::{OplogExt, RestoreKind};

use crate::support::{persist_default_target, repo_with_feature_branch};

#[test]
fn branch_name_segment_attaches_the_detached_linked_worktree() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    assert!(
        but_api::worktrees::worktrees_list(&mut ctx)?
            .active
            .is_empty(),
        "adoption runs before the worktree exists, so it starts out active"
    );
    let worktree_dir = tempfile::tempdir()?;
    let worktree_path = worktree_dir.path().join("wt");
    git_at_dir(tmp.path())
        .args([
            "worktree",
            "add",
            "--detach",
            worktree_path.to_str().expect("utf-8 path"),
            "feature",
        ])
        .run();
    git_at_dir(&worktree_path)
        .args(["commit", "--allow-empty", "-m", "W"])
        .run();
    let top = gix::open(&worktree_path)?.head_id()?.detach();
    let main_head = ctx.repo.get()?.head_name()?;

    let result = but_api::branch::branch_name_segment(
        &mut ctx,
        ChangesSource::Worktree("wt".into()),
        top,
        None,
    )?;

    let linked = gix::open(&worktree_path)?;
    assert_eq!(
        linked.head_name()?,
        Some(result.new_ref),
        "the linked worktree is attached to the canned branch"
    );
    assert_eq!(linked.head_id()?, top, "naming moves no commits");
    assert_eq!(
        ctx.repo.get()?.head_name()?,
        main_head,
        "the main worktree is left alone"
    );

    let snapshot = ctx.oplog_head()?.expect("naming records an undo point");
    let mut guard = ctx.exclusive_worktree_access();
    ctx.restore_snapshot(
        snapshot,
        RestoreKind::RestoreFromSnapshotViaUndo,
        guard.write_permission(),
    )?;
    Ok(())
}
