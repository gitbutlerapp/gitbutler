use crate::WorkspaceState;
use but_api_macros::but_api;
use but_core::{DiffSpec, DryRun, sync::RepoExclusive};
use but_graph::Workspace;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use tracing::instrument;

use super::types::CommitCreateResult;

/// Helper to find which branch contains a given commit by checking the workspace graph
fn find_branch_for_commit(ws: &Workspace, commit_id: gix::ObjectId) -> Option<gix::refs::FullName> {
    tracing::debug!("find_branch_for_commit: Searching for commit {} in {} stacks", commit_id, ws.stacks.len());

    // Iterate through all stacks and their segments
    for (stack_idx, stack) in ws.stacks.iter().enumerate() {
        tracing::debug!("find_branch_for_commit: Stack {}: {} segments", stack_idx, stack.segments.len());

        for (seg_idx, segment) in stack.segments.iter().enumerate() {
            let ref_name = segment.ref_info.as_ref().map(|i| i.ref_name.as_ref().to_string()).unwrap_or_else(|| "no-ref".to_string());
            tracing::debug!("find_branch_for_commit:   Segment {}: ref={}, commits={}", seg_idx, ref_name, segment.commits.len());

            // Check if any commit in this segment matches our target commit
            for commit in &segment.commits {
                if commit.id == commit_id {
                    // Found it! Return the branch reference for this segment
                    let result = segment.ref_info.as_ref().map(|info| info.ref_name.clone());
                    tracing::info!("find_branch_for_commit: FOUND! commit {} in segment with ref={:?}", commit_id, result);
                    return result;
                }
            }
        }
    }

    tracing::warn!("find_branch_for_commit: Commit {} not found in any branch", commit_id);
    None
}

/// Creates a commit from `changes` with `message`, inserted on `side` of
/// `relative_to`.
///
/// This acquires exclusive worktree access from `ctx` before creating the
/// commit. For lower-level implementation details, see
/// [`but_workspace::commit::commit_create()`]. When `dry_run` is enabled, the
/// returned workspace previews the inserted commit without materializing the
/// rebase.
#[but_api(try_from = crate::commit::json::CommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_create_only(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
    dry_run: DryRun,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        message,
        dry_run,
        context_lines,
        guard.write_permission(),
    )
}

/// Creates and inserts a commit relative to either a commit or a reference.
///
/// When `dry_run` is enabled, the returned workspace previews the inserted
/// commit without materializing the rebase.
#[expect(clippy::too_many_arguments)]
pub(crate) fn commit_create_only_impl(
    ctx: &mut but_ctx::Context,
    relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
    dry_run: DryRun,
    context_lines: u32,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let but_workspace::commit::CommitCreateOutcome {
        rebase,
        commit_selector,
        rejected_specs,
    } = but_workspace::commit::commit_create(
        editor,
        changes,
        relative_to,
        side,
        &message,
        context_lines,
    )?;

    let new_commit = commit_selector
        .map(|commit_selector| rebase.lookup_pick(commit_selector))
        .transpose()?;
    let workspace = WorkspaceState::from_successful_rebase(rebase, &repo, dry_run)?;

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
        workspace,
    })
}

/// Insert a new commit built from `changes` and record an oplog snapshot on
/// success.
///
/// `relative_to` and `side` choose where the commit is inserted. `message` is
/// the entire commit message text, not just the title. On success, this commits
/// a best-effort `CreateCommit` oplog snapshot using the same lock. When
/// `dry_run` is enabled, the returned workspace previews the inserted commit
/// and no oplog entry is persisted. For lower-level implementation details, see
/// [`but_workspace::commit::commit_create()`].
#[but_api(napi, try_from = crate::commit::json::CommitCreateResult)]
#[instrument(skip_all, fields(relative_to, side, message), err(Debug))]
pub fn commit_create(
    ctx: &mut but_ctx::Context,
    #[but_api(crate::commit::json::RelativeTo)] relative_to: RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    // First, determine which branch we're committing to by examining the workspace
    let branch_for_hook: Option<gix::refs::FullName> = {
        let mut meta = ctx.meta()?;
        let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm.read_permission())?;

        let result = match &relative_to {
            RelativeTo::Reference(r) => {
                tracing::info!("commit_create: RelativeTo::Reference({})", r);
                Some(r.clone())
            }
            RelativeTo::Commit(commit_id) => {
                tracing::info!("commit_create: RelativeTo::Commit({}), searching for branch...", commit_id);
                let found = find_branch_for_commit(&ws, *commit_id);
                if let Some(ref branch) = found {
                    tracing::info!("commit_create: Found branch: {}", branch);
                } else {
                    tracing::warn!("commit_create: No branch found for commit {}", commit_id);
                }
                found
            }
        };

        tracing::info!("commit_create: Final branch_for_hook: {:?}", result);
        result
    };

    // Run commit-msg hook with the determined branch context
    let final_message = {
        let branch_ref_opt = branch_for_hook.as_ref().map(|r| r.as_ref());

        if let Some(branch_ref) = branch_ref_opt {
            tracing::debug!("Running commit-msg hook with branch reference: {}", branch_ref);
        } else {
            tracing::debug!("Running commit-msg hook with workspace target branch (no branch found)");
        }

        match gitbutler_repo::hooks::commit_msg_with_branch(ctx, message.clone(), branch_ref_opt) {
            Ok(gitbutler_repo::hooks::MessageHookResult::Message(data)) => data.message,
            Ok(gitbutler_repo::hooks::MessageHookResult::Failure(data)) => {
                anyhow::bail!("commit-msg hook failed: {}", data.error);
            }
            Ok(_) => message,
            Err(e) => {
                tracing::warn!("commit-msg hook error: {}", e);
                message
            }
        }
    };

    let context_lines = ctx.settings.context_lines;
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CreateCommit),
        perm.read_permission(),
        dry_run,
    );

    let res = commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        final_message,
        dry_run,
        context_lines,
        perm,
    );
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
