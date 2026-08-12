#![expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]

use anyhow::{Context as _, Result};
use but_testsupport::CommandExt as _;
use but_testsupport::visualize_tree;
use gitbutler_stack::VirtualBranchesHandle;
use gix::prelude::ObjectIdExt;
use snapbox::IntoData;
use tempfile::TempDir;

use but_ctx::Context;

use crate::driverless;

fn command_ctx(name: &str) -> Result<(Context, TempDir)> {
    driverless::writable_context("workspace-commit.sh", name)
}

/// When two applied stacks have trees that conflict on the same file,
/// `remerged_workspace_tree_v2` (called by `update_workspace_commit`) detects the
/// gix merge conflict and marks the later stack as `in_workspace = false`.
/// With the fix in `remerged_workspace_commit_v2`, that evicted stack's head must
/// be excluded from the workspace commit's parent list.
///
/// Without the fix, the workspace commit tree would not contain the evicted stack's
/// changes but its head would still be a parent — causing phantom uncommitted changes
/// when diffing the workspace commit against its parents.
#[test]
fn conflicting_stacks_evicted_from_workspace_commit_parents() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("conflicting-stacks")?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks_before = vb_state.list_stacks_in_workspace()?;
    assert_eq!(
        stacks_before.len(),
        2,
        "precondition: 2 stacks in workspace"
    );

    // Rebuild the workspace commit through the legacy path.
    // remerged_workspace_tree_v2 iterates both stacks and merges each tree:
    //   - The first stack merges cleanly onto the target tree
    //   - The second stack conflicts (same file, different content) → in_workspace = false
    // remerged_workspace_commit_v2 (with our fix) then excludes the evicted stack
    // from the workspace commit's parent list.
    gitbutler_branch_actions::update_workspace_commit(&ctx, false)?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

    // Exactly one of the two conflicting stacks should have been evicted.
    let stacks_after = vb_state.list_stacks_in_workspace()?;
    assert_eq!(
        stacks_after.len(),
        1,
        "Only the non-conflicting stack should remain in workspace"
    );
    let surviving_stack = &stacks_after[0];

    // The workspace commit must have exactly 1 parent: the surviving stack's head.
    let repo = ctx.repo.get()?;
    let ws_ref = repo.find_reference("refs/heads/gitbutler/workspace")?;
    let ws_commit = ws_ref.into_fully_peeled_id()?.object()?.try_into_commit()?;
    let parent_ids: Vec<_> = ws_commit.parent_ids().collect();

    assert_eq!(
        parent_ids.len(),
        1,
        "Workspace commit should have only the surviving stack as parent"
    );

    let surviving_head = surviving_stack.head_oid(&ctx)?;
    assert_eq!(
        parent_ids[0].detach(),
        surviving_head,
        "The only parent should be the surviving stack's head"
    );

    Ok(())
}

/// When two applied stacks modify nearby, non-overlapping sections of the same
/// file with unchanged context between them, `merge_workspace` must produce a
/// clean merge.
///
/// Stack A owns lines 1–5 and 11–15; Stack B owns lines 7–9. Lines 6 and 10
/// separate the hunks.
#[test]
fn merge_workspace_succeeds_with_separated_hunks_from_both_sides() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("adjacent-stacks")?;

    // Build the workspace commit so both stacks are properly registered.
    gitbutler_branch_actions::update_workspace_commit(&ctx, false)?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = vb_state.list_stacks_in_workspace()?;
    assert_eq!(stacks.len(), 2, "both stacks should be in workspace");

    // Build a WorkspaceState from both stacks and call merge_workspace directly.
    // This is the exact function that was fixed from git2 to gix.
    let guard = ctx.shared_worktree_access();
    let workspace =
        gitbutler_workspace::branch_trees::WorkspaceState::create(&ctx, guard.read_permission())?;
    let gix_repo = ctx.clone_repo_for_merging()?;
    gitbutler_workspace::branch_trees::merge_workspace(&gix_repo, &workspace)?;

    Ok(())
}

/// Regression test for a merge-base mismatch in `merge_workspace`.
///
/// The graph is:
///
/// ```text
/// * C: {x, y, c}
/// |
/// * B: {x, b, c} (target)
/// |
/// |  * D: {a, b, z}
/// |/
/// * A: {a, b, c}
/// ```
///
/// Merging C and D against their real merge base A applies `A -> C` plus
/// `A -> D`, producing `{x, y, z}`. Using the target B as the merge base would
/// also apply the inverse of B's change and incorrectly produce `{a, y, z}`.
#[test]
fn merge_workspace_with_diverged_stacks() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("diverged-stacks")?;

    let repo = ctx.repo.get()?;
    let target_oid = repo.rev_parse_single("target-b")?.detach();
    let head_oids: Vec<gix::ObjectId> = ["stack_c", "stack_d"]
        .iter()
        .map(|name| repo.rev_parse_single(*name).map(|id| id.detach()))
        .collect::<Result<_, _>>()?;

    let workspace =
        gitbutler_workspace::branch_trees::WorkspaceState::create_from_heads_and_target(
            &repo, &head_oids, target_oid,
        )?;

    let gix_repo = ctx.clone_repo_for_merging()?;
    let merged_tree_id = gitbutler_workspace::branch_trees::merge_workspace(&gix_repo, &workspace)
        .expect("workspace should merge cleanly with per-stack merge bases");

    // merged tree should contain x, y, and z when C and D are merged using A as their merge base
    snapbox::assert_data_eq!(
        visualize_tree(merged_tree_id.attach(&gix_repo)).to_string(),
        snapbox::str![[r#"
8999a87
├── x:100644:587be6b "x\n"
├── y:100644:975fbec "y\n"
└── z:100644:b680253 "z\n"

"#]]
        .raw()
    );

    Ok(())
}

/// Regression test for the same merge-base mismatch in
/// `remerged_workspace_tree_v2`, which updates the workspace commit.
#[test]
fn update_workspace_commit_with_diverged_stacks_preserves_target_content() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("diverged-stacks")?;

    gitbutler_branch_actions::update_workspace_commit(&ctx, false)?;

    let repo = ctx.repo.get()?;
    let ws_ref = repo.find_reference("refs/heads/gitbutler/workspace")?;
    let ws_tree_id = ws_ref
        .into_fully_peeled_id()?
        .object()?
        .try_into_commit()?
        .tree_id()?;

    // workspace commit tree should contain x, y, and z when C and D are merged using A as their merge base
    snapbox::assert_data_eq!(
        visualize_tree(ws_tree_id).to_string(),
        snapbox::str![[r#"
8999a87
├── x:100644:587be6b "x\n"
├── y:100644:975fbec "y\n"
└── z:100644:b680253 "z\n"

"#]]
        .raw()
    );

    Ok(())
}

/// `skip-worktree` tells Git to leave a tracked file's worktree copy alone. Sparse checkouts set
/// it, and it is also set by hand to keep local edits to a checked-in file out of the way.
/// Rebuilding `.git/index` from the workspace tree must not quietly drop it.
///
/// `shared.txt` is untouched by either stack, so this also shows the flag going missing on a
/// path the workspace commit does not change at all.
#[test]
fn workspace_commit_preserves_skip_worktree() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("adjacent-stacks")?;
    let worktree_dir = ctx
        .repo
        .get()?
        .workdir()
        .expect("fixture repo has a worktree")
        .to_owned();

    but_testsupport::git_at_dir(&worktree_dir)
        .args(["update-index", "--skip-worktree", "shared.txt"])
        .run();
    assert!(
        index_flag_is_set(&ctx, "shared.txt", gix::index::entry::Flags::SKIP_WORKTREE)?,
        "precondition: the flag is set before the workspace commit is rebuilt"
    );

    gitbutler_branch_actions::update_workspace_commit(&ctx, false)?;

    assert!(
        index_flag_is_set(&ctx, "shared.txt", gix::index::entry::Flags::SKIP_WORKTREE)?,
        "rebuilding the index from the workspace tree keeps per-file flags"
    );
    Ok(())
}

fn index_flag_is_set(ctx: &Context, path: &str, flag: gix::index::entry::Flags) -> Result<bool> {
    let repo = ctx.repo.get()?;
    let index = repo.index()?;
    let entry = index
        .entry_by_path(path.into())
        .with_context(|| format!("{path} should be tracked"))?;
    Ok(entry.flags.contains(flag))
}

/// `assume-unchanged` is the other per-file flag Git keeps only in the index, and the issue asks
/// for it alongside `skip-worktree`.
///
/// This one uses `file`, which both stacks modify, because it is only lost when the rebuild
/// actually replaces the entry - libgit2 keeps an entry whose blob and mode are unchanged, flags
/// and all. `skip-worktree` above needs no such setup, being dropped either way.
#[test]
fn workspace_commit_preserves_assume_unchanged() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("adjacent-stacks")?;
    let worktree_dir = ctx
        .repo
        .get()?
        .workdir()
        .expect("fixture repo has a worktree")
        .to_owned();

    but_testsupport::git_at_dir(&worktree_dir)
        .args(["update-index", "--assume-unchanged", "file"])
        .run();
    assert!(
        index_flag_is_set(&ctx, "file", gix::index::entry::Flags::ASSUME_VALID)?,
        "precondition: the flag is set before the workspace commit is rebuilt"
    );

    gitbutler_branch_actions::update_workspace_commit(&ctx, false)?;

    assert!(
        index_flag_is_set(&ctx, "file", gix::index::entry::Flags::ASSUME_VALID)?,
        "rebuilding the index from the workspace tree keeps per-file flags"
    );
    Ok(())
}

/// The `checkout_new_worktree` leg checks the working tree out before the index is rebuilt, and
/// that checkout writes the index itself. The flags have to be taken before it runs, or they are
/// already gone by the time the rebuild sees them.
#[test]
fn workspace_commit_preserves_flags_when_checking_out() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("adjacent-stacks")?;
    let worktree_dir = ctx
        .repo
        .get()?
        .workdir()
        .expect("fixture repo has a worktree")
        .to_owned();

    but_testsupport::git_at_dir(&worktree_dir)
        .args(["update-index", "--skip-worktree", "shared.txt"])
        .run();
    but_testsupport::git_at_dir(&worktree_dir)
        .args(["update-index", "--assume-unchanged", "file"])
        .run();

    gitbutler_branch_actions::update_workspace_commit(&ctx, true)?;

    assert!(
        index_flag_is_set(&ctx, "shared.txt", gix::index::entry::Flags::SKIP_WORKTREE)?,
        "skip-worktree survives the checkout leg"
    );
    assert!(
        index_flag_is_set(&ctx, "file", gix::index::entry::Flags::ASSUME_VALID)?,
        "assume-unchanged survives the checkout leg"
    );
    Ok(())
}
