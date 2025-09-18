use colored::Colorize;
use gitbutler_branch_actions::reorder::commits_order;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_stack::VirtualBranchesHandle;
use gix::ObjectId;

use super::{assign::branch_name_to_stack_id, undo::stack_id_by_commit_id};

pub(crate) fn to_branch(
    ctx: &mut CommandContext,
    oid: &ObjectId,
    branch_name: &str,
) -> anyhow::Result<()> {
    let target_stack_id = branch_name_to_stack_id(ctx, Some(branch_name))?.ok_or(
        anyhow::anyhow!("Could not find stack for branch {}", branch_name),
    )?;
    let source_stack_id = stack_id_by_commit_id(ctx, oid)?;
    if source_stack_id == target_stack_id {
        let vb_state = &VirtualBranchesHandle::new(ctx.project().gb_dir());
        let stack = vb_state.get_stack_in_workspace(source_stack_id)?;
        let mut stack_order = commits_order(ctx, &stack)?;
        let git2_oid = oid.to_git2();
        stack_order.series.iter_mut().for_each(|series| {
            series.commit_ids.retain(|commit_id| commit_id != &git2_oid);
        });
        if let Some(series) = stack_order
            .series
            .iter_mut()
            .find(|s| s.name == branch_name)
        {
            series.commit_ids.insert(0, git2_oid);
        }
        gitbutler_branch_actions::reorder_stack(ctx, source_stack_id, stack_order)?;
    } else if let Some(illegal_move) =
        gitbutler_branch_actions::move_commit(ctx, target_stack_id, oid.to_git2(), source_stack_id)?
    {
        match illegal_move {
            gitbutler_branch_actions::MoveCommitIllegalAction::DependsOnCommits(deps) => {
                println!(
                    "Cannot move commit {} because it depends on commits: {}",
                    oid,
                    deps.join(", ")
                );
            }
            gitbutler_branch_actions::MoveCommitIllegalAction::HasDependentChanges(deps) => {
                println!(
                    "Cannot move commit {} because it has dependent changes: {}",
                    oid,
                    deps.join(", ")
                );
            }
            gitbutler_branch_actions::MoveCommitIllegalAction::HasDependentUncommittedChanges => {
                println!("Cannot move commit {oid} because it has dependent uncommitted changes");
            }
        }

        return Ok(());
    }
    println!(
        "Moved {} → {}",
        oid.to_string()[..7].blue(),
        format!("[{branch_name}]").green()
    );
    Ok(())
}
