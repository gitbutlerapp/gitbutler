use anyhow::Result;
use but_api::commit::json::ChangesSource;

use crate::support::{checkout_branch_in_linked_worktree, repo_with_feature_branch, write_file};

/// A context whose project has `wt` as an active linked worktree with `feature` checked out.
///
/// Adoption archives every worktree that exists when the flag is first read, so the
/// worktree has to be explicitly unarchived to be reachable as a changes source.
fn ctx_with_active_worktree() -> Result<(but_ctx::Context, tempfile::TempDir, tempfile::TempDir)> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let linked = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    ctx.settings.feature_flags.worktree_manipulation = true;
    but_api::worktrees::worktree_set_archived(&mut ctx, "wt".into(), false)?;
    Ok((ctx, tmp, linked))
}

#[test]
fn worktree_source_reads_the_linked_checkout() -> Result<()> {
    let (ctx, tmp, linked) = ctx_with_active_worktree()?;
    write_file(tmp.path(), "file.txt", "main worktree change\n")?;
    write_file(&linked.path().join("wt"), "file.txt", "linked change\n")?;

    let guard = ctx.shared_worktree_access();
    let changes = but_api::diff::changes_in_worktree_with_perm(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        false,
        guard.read_permission(),
    )?;

    // Both checkouts modified `file.txt`, so only the source distinguishes them - a
    // source that read the main worktree would produce an identical path list.
    let paths: Vec<_> = changes
        .worktree_changes
        .changes
        .iter()
        .map(|change| change.path.clone())
        .collect();
    assert_eq!(paths, ["file.txt"]);

    let repo = ctx.repo.get()?;
    let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, "wt".into())?;
    let change = but_core::TreeChange::from(changes.worktree_changes.changes[0].clone());
    let patch = change.unified_patch(&wt_repo, 3)?;
    assert!(
        format!("{patch:?}").contains("linked change"),
        "the diff must come from the linked checkout, not the main one: {patch:?}"
    );
    Ok(())
}

#[test]
fn worktree_source_ignores_the_computation_flag() -> Result<()> {
    let (ctx, _tmp, linked) = ctx_with_active_worktree()?;
    write_file(&linked.path().join("wt"), "file.txt", "linked change\n")?;

    let guard = ctx.shared_worktree_access();
    let changes = but_api::diff::changes_in_worktree_with_perm(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        true,
        guard.read_permission(),
    )?;

    assert_eq!(changes.worktree_changes.changes.len(), 1);
    // Assignment and dependencies are workspace concepts; a linked worktree is not in
    // the workspace, so asking for them yields nothing and persists nothing.
    assert!(changes.assignments.is_empty());
    assert!(changes.assignments_error.is_none());
    assert!(changes.dependencies.is_none());
    assert!(changes.dependencies_error.is_none());
    Ok(())
}

#[test]
fn worktree_source_requires_the_feature_flag() -> Result<()> {
    let (mut ctx, _tmp, _linked) = ctx_with_active_worktree()?;
    ctx.settings.feature_flags.worktree_manipulation = false;

    let guard = ctx.shared_worktree_access();
    let err = but_api::diff::changes_in_worktree_with_perm(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        false,
        guard.read_permission(),
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("worktree manipulation is not enabled"),
        "{err}"
    );
    Ok(())
}

#[test]
fn head_source_reads_the_main_worktree() -> Result<()> {
    let (ctx, tmp, linked) = ctx_with_active_worktree()?;
    write_file(tmp.path(), "main-only.txt", "main worktree change\n")?;
    write_file(&linked.path().join("wt"), "linked-only.txt", "linked\n")?;

    let guard = ctx.shared_worktree_access();
    let changes = but_api::diff::changes_in_worktree_with_perm(
        &ctx,
        ChangesSource::Head,
        false,
        guard.read_permission(),
    )?;

    let paths: Vec<_> = changes
        .worktree_changes
        .changes
        .iter()
        .map(|change| change.path.clone())
        .collect();
    assert_eq!(
        paths,
        ["main-only.txt"],
        "an active linked worktree must not leak into the main worktree's changes"
    );
    Ok(())
}
