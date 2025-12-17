use anyhow::Result;
use but_ctx::Context;
use but_rebase::Rebase;
use gitbutler_stack::{StackId, VirtualBranchesHandle};

use crate::legacy::{
    MoveChangesResult,
    stack_ext::StackExt,
    tree_manipulation::{
        remove_changes_from_commit_in_stack::keep_only_file_changes_in_commit,
        utils::replace_pick_with_multiple_commits,
    },
};

/// Splits a commit into multiple commits based on the specified file changes.
///
/// This function creates new commits for each specified piece of the original commit.
/// The new commits will contain only the specified files, effectively splitting the original commit.
/// In steps:
/// 1. Create new commits for each specified piece of the original commit.
/// 2. Replace the original commit in the stack with the new commits.
/// 3. Update the stack to reflect the new commits.
pub fn split_commit(
    ctx: &mut Context,
    stack_id: StackId,
    source_commit_id: gix::ObjectId,
    pieces: &[CommitFiles],
    context_lines: u32,
) -> Result<CommmitSplitOutcome> {
    let repository = ctx.repo.get()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

    let source_stack = vb_state.get_stack_in_workspace(stack_id)?;

    let mut steps = source_stack.as_rebase_steps(ctx, &repository)?;
    let commit_pieces = new_commits(ctx, source_commit_id, pieces, context_lines)?;
    replace_pick_with_multiple_commits(&mut steps, source_commit_id, &commit_pieces)?;
    let base = source_stack.merge_base(ctx)?;
    let mut rebase = Rebase::new(&repository, base, None)?;
    rebase.steps(steps)?;
    rebase.rebase_noops(false);
    let result = rebase.rebase()?;

    let commit_mapping = result
        .commit_mapping
        .iter()
        .filter_map(
            |(_, old, new)| {
                if old == new { None } else { Some((*old, *new)) }
            },
        )
        .collect();

    let mut source_stack = source_stack;
    source_stack.set_heads_from_rebase_output(ctx, result.references)?;

    let new_commits = commit_pieces.iter().map(|(id, _)| *id).collect::<Vec<_>>();

    Ok(CommmitSplitOutcome {
        new_commits,
        move_changes_result: MoveChangesResult {
            replaced_commits: commit_mapping,
        },
    })
}

fn new_commits(
    ctx: &Context,
    source_commit_id: gix::ObjectId,
    pieces: &[CommitFiles],
    context_lines: u32,
) -> Result<Vec<(gix::ObjectId, Option<String>)>> {
    let mut new_commits = Vec::new();
    for piece in pieces {
        if let Some(rewritten_commit) = keep_only_file_changes_in_commit(
            ctx,
            source_commit_id,
            &piece.files,
            context_lines,
            false,
        )? {
            new_commits.push((rewritten_commit, Some(piece.message.clone())));
        }
    }
    Ok(new_commits)
}

/// Represents the files to be included in a new commit when splitting an existing commit.
pub struct CommitFiles {
    /// The message for the new commit.
    pub message: String,
    /// A subset of the files in the commit that should be included in the new commit.
    pub files: Vec<String>,
}

/// Represents the outcome of splitting a commit, including the newly created commits
/// and the result of moving changes between them.
///
/// # Fields
/// - `new_commits`: A vector containing the object IDs of the new commits that were created as a result of the split.
/// - `move_changes_result`: The result of the operation that moved changes between commits during the split process.
pub struct CommmitSplitOutcome {
    /// The new commits created
    pub new_commits: Vec<gix::ObjectId>,
    /// The moved changes outcome
    pub move_changes_result: MoveChangesResult,
}
