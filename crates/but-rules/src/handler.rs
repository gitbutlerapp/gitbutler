use std::str::FromStr;

use anyhow::ensure;
use but_core::{ChangeId, DiffSpec, RefMetadata, ref_metadata::StackId, sync::RepoExclusive};
use but_db::{DbHandle, HunkAssignmentsHandleMut};
use but_hunk_assignment::HunkAssignment;
use but_rebase::graph_rebase::Editor;
use itertools::Itertools;

use crate::{Filter, StackTarget, WorkspaceRule};

/// Apply matching workspace `rules` to the current worktree `assignments`.
///
/// `rules` provides the enabled filesystem-change rules to evaluate. `assignments`
/// is the current hunk-assignment view used for matching filters and deciding
/// whether an update is necessary. `repo` is used for diff and commit lookups.
/// `ws` is the mutable workspace graph that may be updated when a rule creates a
/// new stack or amends a commit. `db` provides mutable access to persisted hunk
/// assignments. `meta` is updated by workspace graph editing operations. `perm`
/// proves the caller holds exclusive access because rule processing may create a
/// stack and update the workspace graph.
/// `context_lines` controls diff context when assigning or amending hunks.
#[expect(clippy::too_many_arguments)]
pub fn process_workspace_rules(
    rules: Vec<WorkspaceRule>,
    assignments: &[HunkAssignment],
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    db: &mut DbHandle,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    context_lines: u32,
) -> anyhow::Result<usize> {
    let mut updates = 0;
    if assignments.is_empty() {
        // Don't create stacks if there are no changes to assign anywhere
        return Ok(updates);
    }
    let rules = rules
        .into_iter()
        .filter(|r| r.enabled)
        .filter(|r| matches!(r.trigger, super::Trigger::FileSytemChange))
        .filter(|r| {
            matches!(
                &r.action,
                super::Action::Explicit(super::Operation::Assign { .. })
            ) || matches!(
                &r.action,
                super::Action::Explicit(super::Operation::Amend { .. })
            )
        })
        .collect_vec();

    if rules.is_empty() {
        return Ok(updates);
    }

    let stack_ids: Vec<_> = ws.stacks.iter().filter_map(|s| s.id).collect();
    let mut new_ws = None;

    for rule in rules {
        match rule.action {
            super::Action::Explicit(super::Operation::Assign { target }) => {
                if let Some((stack_id, maybe_new_ws)) =
                    get_or_create_stack_id(repo, ws, meta, perm, target, &stack_ids)
                {
                    if let Some(ws) = maybe_new_ws {
                        ensure!(
                            new_ws.is_none(),
                            "BUG: new stacks are only created once if there are no stacks"
                        );
                        new_ws = Some(ws);
                    }
                    let assignments = matching(assignments, rule.filters.clone())
                        .into_iter()
                        .filter(|e| e.stack_id != Some(stack_id))
                        .map(|mut e| {
                            e.stack_id = Some(stack_id);
                            e.branch_ref_bytes = None;
                            e
                        })
                        .collect_vec();
                    updates += handle_assign(
                        db.hunk_assignments_mut()?,
                        repo,
                        new_ws.as_ref().unwrap_or(&*ws),
                        assignments,
                        context_lines,
                    )
                    .unwrap_or_default();
                }
            }
            super::Action::Explicit(super::Operation::Amend { change_id }) => {
                let assignments = matching(assignments, rule.filters.clone());
                let ws = if let Some(new_ws) = new_ws.as_mut() {
                    new_ws
                } else {
                    &mut *ws
                };
                handle_amend(repo, ws, meta, assignments, &change_id, context_lines)
                    .unwrap_or_default();
            }
            _ => continue,
        };
    }

    if let Some(new_ws) = new_ws {
        *ws = new_ws;
    }

    Ok(updates)
}

/// Amend the commit identified by `change_id` with the provided `assignments`.
///
/// `repo` is used to inspect commits and materialize the resulting rebase. `ws`
/// is the mutable workspace graph edited by the amend operation. `meta` stores
/// graph metadata updates produced by the editor. `assignments` are flattened
/// into diff specs to apply to the matched commit. `change_id` selects the
/// destination commit by its Gerrit Change-Id header. `context_lines` controls
/// diff context for the amend operation.
fn handle_amend(
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    meta: &mut impl but_core::RefMetadata,
    assignments: Vec<HunkAssignment>,
    change_id: &ChangeId,
    context_lines: u32,
) -> anyhow::Result<()> {
    let changes: Vec<DiffSpec> =
        but_workspace::flatten_diff_specs(assignments.into_iter().map(DiffSpec::from));
    let mut commit_id: Option<gix::ObjectId> = None;
    'outer: for commit in ws.commits() {
        let commit_change_id = commit.attach(repo)?.headers().and_then(|hdr| hdr.change_id);
        if commit_change_id.is_some_and(|cid| cid == *change_id) {
            commit_id = Some(commit.id);
            break 'outer;
        }
    }

    let commit_id = commit_id.ok_or_else(|| {
        anyhow::anyhow!("No commit with Change-Id {change_id} found in the current workspace")
    })?;

    let editor = Editor::create(ws, meta, repo)?;
    let outcome = but_workspace::commit::commit_amend(editor, commit_id, changes, context_lines)?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(
            ?outcome.rejected_specs,
            "Failed to commit at least one hunk"
        );
    }
    outcome.rebase.materialize()?;
    Ok(())
}

/// Resolve a rule `target` into a stack ID, creating a stack if necessary.
///
/// `repo` is used when a new branch name must be generated. `ws` is inspected
/// to find existing stacks or used as the base for a newly-created workspace
/// graph. `meta` receives metadata updates if a new stack is created. `target`
/// describes the requested stack selection. `perm` proves that stack creation is
/// allowed if the target requires a new stack. `stack_ids_in_ws` is the
/// precomputed list of stack IDs currently present in the workspace.
fn get_or_create_stack_id(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    meta: &mut impl but_core::RefMetadata,
    perm: &mut RepoExclusive,
    target: StackTarget,
    stack_ids_in_ws: &[StackId],
) -> Option<(StackId, Option<but_graph::Workspace>)> {
    match target {
        StackTarget::StackId(stack_id) => {
            if let Ok(stack_id) = StackId::from_str(&stack_id) {
                if stack_ids_in_ws.iter().any(|e| e == &stack_id) {
                    Some((stack_id, None))
                } else {
                    None
                }
            } else {
                None
            }
        }
        StackTarget::Leftmost => {
            if stack_ids_in_ws.is_empty() {
                create_stack(repo, ws, meta, perm)
                    .ok()
                    .map(|(id, ws)| (id, Some(ws)))
            } else {
                stack_ids_in_ws.first().copied().map(|id| (id, None))
            }
        }
        StackTarget::Rightmost => {
            if stack_ids_in_ws.is_empty() {
                create_stack(repo, ws, meta, perm)
                    .ok()
                    .map(|(id, ws)| (id, Some(ws)))
            } else {
                stack_ids_in_ws.last().copied().map(|id| (id, None))
            }
        }
    }
}

/// Create a new stack in `ws` and return its generated ID and updated workspace.
///
/// `repo` is used to generate a unique canned branch name. `ws` is the workspace
/// graph used as the base for the new reference. `meta` receives metadata updates
/// from reference creation. `_perm` proves the caller holds exclusive access for
/// the workspace change.
fn create_stack(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    meta: &mut impl but_core::RefMetadata,
    _perm: &mut RepoExclusive,
) -> anyhow::Result<(StackId, but_graph::Workspace)> {
    use anyhow::Context;
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
    let (stack, _) = new_ws
        .find_segment_and_stack_by_refname(branch_name.as_ref())
        .context("BUG: need to find stack that was just created")?;
    stack
        .id
        .context("BUG: newly created stacks always have an ID")
        .map(|id| (id, new_ws.into_owned()))
}

/// Persist assignment updates and return how many were attempted.
///
/// `db` is the mutable hunk-assignment table handle to update. `repo` and
/// `workspace` provide the repository/workspace context required by assignment
/// validation. `assignments` are converted into assignment requests before
/// writing. `context_lines` controls diff context during assignment.
fn handle_assign(
    db: HunkAssignmentsHandleMut,
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    assignments: Vec<HunkAssignment>,
    context_lines: u32,
) -> anyhow::Result<usize> {
    let len = assignments.len();
    but_hunk_assignment::assign(
        db,
        repo,
        workspace,
        but_hunk_assignment::assignments_to_requests(assignments),
        context_lines,
    )
    .map(|()| len)
    .or_else(|_| Ok(0))
}

/// Return worktree assignments matching any of the provided `filters`.
///
/// `wt_assignments` is the source list to filter. `filters` contains the rule
/// filter predicates to evaluate; an empty list matches all assignments.
fn matching(wt_assignments: &[HunkAssignment], filters: Vec<Filter>) -> Vec<HunkAssignment> {
    if filters.is_empty() {
        return wt_assignments.to_vec();
    }
    let mut assignments = Vec::new();
    for filter in filters {
        match filter {
            Filter::PathMatchesRegex(regex) => {
                for change in wt_assignments.iter() {
                    if regex.is_match(&change.path) {
                        assignments.push(change.clone());
                    }
                }
            }
            Filter::ContentMatchesRegex(regex) => {
                for change in wt_assignments.iter() {
                    if let Some(diff) = change.diff.clone() {
                        let diff = diff.to_string();
                        let matching_lines: Vec<&str> =
                            diff.lines().filter(|line| line.starts_with('+')).collect();
                        if matching_lines.iter().any(|line| regex.is_match(line)) {
                            assignments.push(change.clone());
                        }
                    }
                }
            }
            Filter::FileChangeType(_) => continue,
            Filter::SemanticType(_) => continue,
            Filter::ClaudeCodeSessionId(_) => continue,
        }
    }
    assignments
}
