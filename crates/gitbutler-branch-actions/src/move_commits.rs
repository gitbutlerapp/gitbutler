use anyhow::{Context as _, Result, anyhow, bail};
use but_ctx::{Context, access::RepoExclusive};
use but_rebase::RebaseStep;
use but_workspace::legacy::stack_ext::StackExt;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_stack::StackId;
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommitted_changes};
use serde::Serialize;

use crate::VirtualBranchesExt;

/// move a commit from one stack to another
///
/// commit will end up at the top of the destination stack
pub(crate) fn move_commit(
    ctx: &mut Context,
    target_stack_id: StackId,
    subject_commit_oid: gix::ObjectId,
    perm: &mut RepoExclusive,
    source_stack_id: StackId,
) -> Result<Option<MoveCommitIllegalAction>> {
    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let vb_state = ctx.virtual_branches();
    let applied_stacks = vb_state
        .list_stacks_in_workspace()
        .context("failed to read virtual branches")?;

    if !applied_stacks.iter().any(|b| b.id == target_stack_id) {
        bail!("Destination branch not found");
    }

    let source_stack = vb_state
        .try_stack(source_stack_id)?
        .ok_or(anyhow!("Source stack not found"))?;

    let destination_stack = vb_state
        .try_stack(target_stack_id)?
        .ok_or(anyhow!("Destination branch not found"))?;

    if source_stack_id == target_stack_id {
        // Intra-stack move: no cross-stack conflicts are possible, so just
        // snapshot and apply sequentially.
        let _ = ctx.create_snapshot(SnapshotDetails::new(OperationKind::MoveCommit), perm);
        let mut source_stack = source_stack;
        take_commit_from_source_stack(ctx, &mut source_stack, subject_commit_oid)?;
        move_commit_to_destination_stack(ctx, destination_stack, subject_commit_oid)?;
    } else {
        // Cross-stack move: compute both rebases (ODB-only writes), check for
        // conflicts, then snapshot, then apply state changes.
        let source_merge_base = source_stack.merge_base(ctx)?;
        let dest_merge_base = destination_stack.merge_base(ctx)?;

        let (source_output, dest_output) = {
            let repo = ctx.repo.get()?;
            repo.find_commit(subject_commit_oid).with_context(|| {
                format!("commit {subject_commit_oid} to be moved could not be found")
            })?;

            // Source: rebase remaining commits without the moved one.
            let source_output = rebase_without_commit(
                ctx,
                &repo,
                &source_stack,
                subject_commit_oid,
                source_merge_base,
            )?;

            // Dest: rebase dest stack with the moved commit inserted at the top.
            let dest_output = rebase_with_commit_at_top(
                ctx,
                &repo,
                &destination_stack,
                subject_commit_oid,
                dest_merge_base,
            )?;

            // Conflict check — bail before any state is written.
            bail_on_new_conflicts(
                &repo,
                &source_output,
                "This move would cause a conflict in the source stack: \
                 other commits depend on the changes being moved.",
            )?;
            bail_on_new_conflicts(
                &repo,
                &dest_output,
                "This move would cause a conflict in the destination stack: \
                 the commit does not apply cleanly at the target location.",
            )?;

            (source_output, dest_output)
        };

        // Snapshot after the conflict check, but before any state writes.
        let _ = ctx.create_snapshot(SnapshotDetails::new(OperationKind::MoveCommit), perm);

        // Apply source changes.
        {
            let repo = ctx.repo.get()?;
            let mut vb_state = ctx.virtual_branches();
            let mut source_stack = vb_state
                .try_stack(source_stack_id)?
                .ok_or(anyhow!("Source stack not found"))?;
            source_stack.set_heads_from_rebase_output(ctx, source_output.references)?;
            source_stack.set_stack_head(&mut vb_state, &repo, source_output.top_commit)?;
        }

        // Apply dest changes.
        {
            let repo = ctx.repo.get()?;
            let mut vb_state = ctx.virtual_branches();
            let mut destination_stack = vb_state
                .try_stack(target_stack_id)?
                .ok_or(anyhow!("Destination branch not found"))?;
            destination_stack.set_heads_from_rebase_output(ctx, dest_output.references)?;
            destination_stack.set_stack_head(&mut vb_state, &repo, dest_output.top_commit)?;
        }
    }

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let _ = update_uncommitted_changes(ctx, old_workspace, new_workspace, perm);
    crate::integration::update_workspace_commit_with_vb_state(&ctx.virtual_branches(), ctx, false)
        .context("failed to update gitbutler workspace")?;

    Ok(None)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum MoveCommitIllegalAction {
    /// The commit being moved has dependencies on some of its parent commits.
    DependsOnCommits(Vec<String>),
    /// The commit being moves has dependent child commits.
    HasDependentChanges(Vec<String>),
    /// The commit being moved has dependent uncommitted changes. (This should not matter in the v3 worlds)
    HasDependentUncommittedChanges,
}

/// Rebase `stack` with `subject_commit_oid` removed, writing only to the ODB.
fn rebase_without_commit(
    ctx: &Context,
    repo: &gix::Repository,
    stack: &gitbutler_stack::Stack,
    subject_commit_oid: gix::ObjectId,
    merge_base: gix::ObjectId,
) -> Result<but_rebase::RebaseOutput> {
    let steps: Vec<RebaseStep> = stack
        .as_rebase_steps(ctx)?
        .into_iter()
        .filter(|s| match s {
            RebaseStep::Pick { commit_id, .. } => *commit_id != subject_commit_oid,
            _ => true,
        })
        .collect();
    let mut rebase = but_rebase::Rebase::new(repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    rebase.rebase(&*ctx.cache.get_cache()?)
}

/// Rebase `stack` with `subject_commit_oid` inserted at the top, writing only to the ODB.
///
/// TODO: In the future we can make the API provide additional info for exactly
/// where to place the commit on the destination stack.
fn rebase_with_commit_at_top(
    ctx: &Context,
    repo: &gix::Repository,
    stack: &gitbutler_stack::Stack,
    subject_commit_oid: gix::ObjectId,
    merge_base: gix::ObjectId,
) -> Result<but_rebase::RebaseOutput> {
    let mut steps = stack.as_rebase_steps(ctx)?;
    if steps.is_empty() {
        anyhow::bail!("destination stack has no branches to insert into");
    }
    steps.insert(
        steps.len() - 1,
        RebaseStep::Pick {
            commit_id: subject_commit_oid,
            new_message: None,
        },
    );
    let mut rebase = but_rebase::Rebase::new(repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    rebase.rebase(&*ctx.cache.get_cache()?)
}

/// Remove the commit from the source stack.
///
/// Will fail if the commit is not in the source stack or if has dependent changes.
fn take_commit_from_source_stack(
    ctx: &Context,
    source_stack: &mut gitbutler_stack::Stack,
    subject_commit_id: gix::ObjectId,
) -> Result<Option<MoveCommitIllegalAction>, anyhow::Error> {
    let merge_base = source_stack.merge_base(ctx)?;
    let repo = ctx.repo.get()?;
    let output = rebase_without_commit(ctx, &repo, source_stack, subject_commit_id, merge_base)?;
    source_stack.set_heads_from_rebase_output(ctx, output.references)?;
    let mut vb_state = ctx.virtual_branches();
    source_stack.set_stack_head(&mut vb_state, &repo, output.top_commit)?;
    Ok(None)
}

/// Move the commit to the destination stack.
fn move_commit_to_destination_stack(
    ctx: &Context,
    mut destination_stack: gitbutler_stack::Stack,
    commit_id: gix::ObjectId,
) -> Result<(), anyhow::Error> {
    let repo = ctx.repo.get()?;
    let merge_base = destination_stack.merge_base(ctx)?;
    let output = rebase_with_commit_at_top(ctx, &repo, &destination_stack, commit_id, merge_base)?;
    destination_stack.set_heads_from_rebase_output(ctx, output.references)?;
    destination_stack.set_stack_head(&mut ctx.virtual_branches(), &repo, output.top_commit)?;
    Ok(())
}

/// Check a rebase output for commits that became conflicted as a result of
/// the rebase (excluding commits that were already conflicted beforehand).
///
/// This is used to validate rebase outputs computed during the move operations
/// before any state is written to disk.
pub(crate) fn bail_on_new_conflicts(
    repo: &gix::Repository,
    output: &but_rebase::RebaseOutput,
    error_message: &str,
) -> Result<()> {
    use gix::prelude::ObjectIdExt as _;
    for (_, old, new) in &output.commit_mapping {
        let was_conflicted = but_core::Commit::from_id(old.attach(repo))
            .with_context(|| format!("failed to read original commit {old}"))?
            .is_conflicted();
        if was_conflicted {
            continue;
        }
        if but_core::Commit::from_id(new.attach(repo))
            .with_context(|| format!("failed to read rebased commit {new}"))?
            .is_conflicted()
        {
            bail!("{error_message}");
        }
    }
    Ok(())
}
