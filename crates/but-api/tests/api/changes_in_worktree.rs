use anyhow::Result;
use but_api::commit::json::ChangesSource;
use but_api::diff::changes_in_worktree_with_perm;
use but_hunk_assignment::WorktreeChanges;
use gix::bstr::ByteSlice;
use snapbox::ToDebug;

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

fn paths(changes: &WorktreeChanges) -> Vec<std::borrow::Cow<'_, str>> {
    changes
        .worktree_changes
        .changes
        .iter()
        .map(|change| change.path.to_str_lossy())
        .collect()
}

#[test]
fn worktree_source_reads_the_linked_checkout() -> Result<()> {
    let (ctx, tmp, linked) = ctx_with_active_worktree()?;
    write_file(tmp.path(), "file.txt", "main worktree change\n")?;
    write_file(&linked.path().join("wt"), "file.txt", "linked change\n")?;

    let guard = ctx.shared_worktree_access();
    let changes = changes_in_worktree_with_perm(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        false,
        guard.read_permission(),
    )?;

    // Both checkouts modified `file.txt`, so only the patch tells the sources apart.
    snapbox::assert_data_eq!(
        paths(&changes).to_debug(),
        snapbox::str![[r#"
[
    "file.txt",
]

"#]]
    );
    let repo = ctx.repo.get()?;
    let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, "wt".into())?;
    let change = but_core::TreeChange::from(changes.worktree_changes.changes[0].clone());
    // `feature`, the linked checkout's branch, still has the first commit's content.
    snapbox::assert_data_eq!(
        change.unified_patch(&wt_repo, 3)?.to_debug(),
        snapbox::str![[r#"
Some(
    Patch {
        hunks: [
            DiffHunk("@@ -1,1 +1,1 @@
            -one
            +linked change
            "),
        ],
        is_result_of_binary_to_text_conversion: false,
        lines_added: 1,
        lines_removed: 1,
    },
)

"#]]
    );
    Ok(())
}

#[test]
fn worktree_source_ignores_the_computation_flag() -> Result<()> {
    let (ctx, _tmp, linked) = ctx_with_active_worktree()?;
    write_file(&linked.path().join("wt"), "file.txt", "linked change\n")?;

    let guard = ctx.shared_worktree_access();
    let changes = changes_in_worktree_with_perm(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        true,
        guard.read_permission(),
    )?;

    // Assignment and dependencies are workspace concepts; a linked worktree is not in
    // the workspace, so asking for them yields nothing and persists nothing.
    snapbox::assert_data_eq!(
        (
            paths(&changes),
            &changes.assignments,
            &changes.assignments_error,
            &changes.dependencies,
            &changes.dependencies_error,
        )
            .to_debug(),
        snapbox::str![[r#"
(
    [
        "file.txt",
    ],
    [],
    None,
    None,
    None,
)

"#]]
    );
    Ok(())
}

#[test]
fn worktree_source_requires_the_feature_flag() -> Result<()> {
    let (mut ctx, _tmp, _linked) = ctx_with_active_worktree()?;
    ctx.settings.feature_flags.worktree_manipulation = false;

    let guard = ctx.shared_worktree_access();
    let err = changes_in_worktree_with_perm(
        &ctx,
        ChangesSource::Worktree("wt".into()),
        false,
        guard.read_permission(),
    )
    .unwrap_err();
    snapbox::assert_data_eq!(
        err.to_string(),
        snapbox::str!["worktree manipulation is not enabled (featureFlags.worktreeManipulation)"]
    );
    Ok(())
}

#[test]
fn head_source_reads_the_main_worktree() -> Result<()> {
    let (ctx, tmp, linked) = ctx_with_active_worktree()?;
    write_file(tmp.path(), "main-only.txt", "main worktree change\n")?;
    write_file(&linked.path().join("wt"), "linked-only.txt", "linked\n")?;

    let guard = ctx.shared_worktree_access();
    let changes =
        changes_in_worktree_with_perm(&ctx, ChangesSource::Head, false, guard.read_permission())?;

    // An active linked worktree must not leak into the main worktree's changes.
    snapbox::assert_data_eq!(
        paths(&changes).to_debug(),
        snapbox::str![[r#"
[
    "main-only.txt",
]

"#]]
    );
    // A changed file that exists on disk gets a modification time.
    snapbox::assert_data_eq!(
        changes
            .worktree_changes
            .modification_times
            .keys()
            .collect::<Vec<_>>()
            .to_debug(),
        snapbox::str![[r#"
[
    "main-only.txt",
]

"#]]
    );
    Ok(())
}
