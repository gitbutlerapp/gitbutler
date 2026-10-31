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
    assert!(
        changes.assignments.is_empty(),
        "assignments are computed only on request"
    );

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
fn worktree_source_has_unassigned_hunks_and_no_dependencies() -> Result<()> {
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
    let [assignment] = changes.assignments.as_slice() else {
        panic!("one hunk, one assignment: {:?}", changes.assignments);
    };
    assert_eq!(assignment.path, "file.txt");
    assert!(
        assignment.hunk_header.is_some() && assignment.stack_id.is_none(),
        "a linked worktree isn't in the workspace, so its hunks are all unassigned"
    );
    assert!(changes.assignments_error.is_none());
    assert!(changes.dependencies.is_none());
    assert!(changes.dependencies_error.is_none());
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
    assert!(
        changes
            .worktree_changes
            .modification_times
            .contains_key("main-only.txt"),
        "a changed file that exists on disk gets a modification time"
    );
    Ok(())
}
