use std::collections::HashMap;

use anyhow::{Context as _, anyhow};
use but_core::{DiffSpec, RefMetadata, ref_metadata::StackId, sync::RepoExclusive};
use but_db::DbHandle;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeToRef},
};

use crate::Outcome;

struct StackForAction {
    id: StackId,
    branch_name: String,
}

/// Create commits for currently uncommitted changes and report the updated stacks.
///
/// The simple handler:
/// - loads hunk assignments for the current worktree changes;
/// - returns without changes if there is nothing to commit;
/// - creates a new workspace stack when no stack exists yet;
/// - groups assigned changes by their target stack and unassigned changes by the default stack;
/// - skips unassigned changes when `exclusive_stack` is set;
/// - creates one commit per stack with pending changes via flattened diff-specs;
/// - materializes successful rebases and reports the branches that received new commits.
///
/// `change_summary` forms the commit message, optionally prefixed by `external_prompt`.
/// `exclusive_stack` limits commits to one stack and leaves unassigned changes untouched when set.
/// `perm` proves that the caller holds exclusive worktree access for the commit and reference
/// mutations. `repo`, `ws`, `db`, and `meta` are supplied by the caller, so this function does not
/// acquire repository guards. `context_lines` controls hunk assignment fallback and commit creation
/// context.
#[expect(clippy::too_many_arguments)]
pub(crate) fn handle_changes(
    change_summary: &str,
    external_prompt: Option<String>,
    exclusive_stack: Option<StackId>,
    perm: &mut RepoExclusive,
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    db: &mut DbHandle,
    meta: &mut impl RefMetadata,
    context_lines: u32,
) -> anyhow::Result<Outcome> {
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        repo,
        ws,
        None::<Vec<but_core::TreeChange>>,
        context_lines,
    )
    .map_err(|err| serde_error::Error::new(&*err))?;
    if assignments.is_empty() {
        return Ok(Outcome {
            updated_branches: vec![],
        });
    }

    // Get the current stacks in the workspace, creating one if none exists.
    let stacks = stacks_creating_if_none(repo, ws, meta, perm)?;

    // Put the assignments into buckets by stack ID.
    let mut stack_assignments: HashMap<StackId, Vec<DiffSpec>> =
        stacks.iter().map(|s| (s.id, vec![])).collect();
    let default_stack_id = stacks
        .first()
        .map(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("No stacks found in the workspace"))?;
    for assignment in assignments {
        if let Some(stack_id) = assignment.stack_id {
            stack_assignments
                .entry(stack_id)
                .or_default()
                .push(assignment.into());
        } else if exclusive_stack.is_none() {
            // If there is an exclusive stack. We don't want to do anything with
            // the unassigned changes.
            stack_assignments
                .entry(default_stack_id)
                .or_default()
                .push(assignment.into());
        }
    }
    // Go over the stack_assignments and flatten the diff specs for each stack.
    for (_, specs) in stack_assignments.iter_mut() {
        *specs = but_workspace::flatten_diff_specs(specs.clone());
    }

    let mut updated_branches = vec![];

    let commit_message = if let Some(prompt) = external_prompt {
        format!("{prompt}\n\n{change_summary}")
    } else {
        change_summary.to_string()
    };

    for (stack_id, diff_specs) in stack_assignments {
        if diff_specs.is_empty() {
            continue;
        }
        if let Some(exclusive_stack) = exclusive_stack
            && exclusive_stack != stack_id
        {
            continue; // Skip stacks that are not the exclusive stack.
        }

        let stack_branch_name = stacks
            .iter()
            .find(|s| s.id == stack_id)
            .map(|s| s.branch_name.clone())
            .ok_or(anyhow!("Could not find associated reference name"))?;
        let full_ref_name: gix::refs::FullName =
            format!("refs/heads/{stack_branch_name}").try_into()?;

        let editor = Editor::create(ws, meta, repo)?;
        let outcome = but_workspace::commit::commit_create(
            editor,
            diff_specs,
            RelativeToRef::Reference(full_ref_name.as_ref()),
            InsertSide::Below,
            &commit_message,
            context_lines,
        )?;

        if !outcome.rejected_specs.is_empty() {
            tracing::warn!(
                ?outcome.rejected_specs,
                "Failed to commit at least one hunk"
            );
        }

        if let Some(new_commit) = outcome
            .commit_selector
            .map(|selector| outcome.rebase.lookup_pick(selector))
            .transpose()?
        {
            outcome.rebase.materialize()?;
            updated_branches.push(crate::UpdatedBranch {
                stack_id,
                branch_name: stack_branch_name,
                new_commits: vec![new_commit.to_string()],
            });
        }
    }

    Ok(Outcome { updated_branches })
}

/// Return the applied stacks that can receive action commits, creating one if none exists.
///
/// `repo` is used to generate the canned branch name and create the underlying reference. `ws` is
/// updated when the first stack has to be created. `meta` records the stack metadata written by the
/// reference creation operation. `_perm` proves that the caller holds exclusive worktree access for
/// the reference creation path.
fn stacks_creating_if_none(
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    meta: &mut impl RefMetadata,
    _perm: &mut RepoExclusive,
) -> anyhow::Result<Vec<StackForAction>> {
    let stacks = stack_info(ws);
    if !stacks.is_empty() {
        return Ok(stacks);
    }

    let branch_name = but_core::branch::unique_canned_refname(repo)?;
    let new_ws = but_workspace::branch::create_reference(
        branch_name.as_ref(),
        None,
        repo,
        ws,
        meta,
        |_| StackId::generate(),
        None,
    )?;
    *ws = new_ws.into_owned();
    let stack = ws
        .stacks
        .iter()
        .find(|stack| stack.ref_name() == Some(branch_name.as_ref()))
        .context("BUG: need to find stack that was just created")?;
    let id = stack
        .id
        .context("BUG: newly created stacks always have an ID")?;
    Ok(vec![StackForAction {
        id,
        branch_name: branch_name.shorten().to_string(),
    }])
}

/// Extract stack IDs and shortened branch names from `ws` for action commit targeting.
///
/// Stacks without an ID or reference name are skipped because the action needs both values to map
/// assignments to a branch and report the resulting update.
fn stack_info(ws: &but_graph::Workspace) -> Vec<StackForAction> {
    ws.stacks
        .iter()
        .filter_map(|stack| {
            let id = stack.id?;
            let branch_name = stack.ref_name()?.shorten().to_string();
            Some(StackForAction { id, branch_name })
        })
        .collect()
}
