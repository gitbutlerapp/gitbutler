use crate::{Filter, StackTarget};
use but_core::sync::WorktreeWritePermission;
use but_core::{ChangeId, DiffSpec, ref_metadata::StackId};
use but_ctx::Context;
use but_db::HunkAssignmentsHandleMut;
use but_hunk_assignment::HunkAssignment;
use but_hunk_dependency::ui::HunkDependencies;
use but_meta::VirtualBranchesTomlMetadata;
use but_workspace::legacy::{StacksFilter, commit_engine, ui::StackEntry};
use itertools::Itertools;
use std::path::Path;
use std::str::FromStr;

pub fn process_workspace_rules(
    ctx: &mut Context,
    assignments: &[HunkAssignment],
    dependencies: &Option<HunkDependencies>,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<usize> {
    let mut updates = 0;
    if assignments.is_empty() {
        // Don't create stacks if there are no changes to assign anywhere
        return Ok(updates);
    }
    let rules = super::list_rules(ctx)?
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

    let repo = &*ctx.repo.get()?;
    let ws = &ctx.workspace_for_editing_with_perm(perm)?;

    let stacks_in_ws = {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project_data_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::legacy::stacks_v3(repo, &meta, StacksFilter::InWorkspace, None)
    }?;

    for rule in rules {
        match rule.action {
            super::Action::Explicit(super::Operation::Assign { target }) => {
                if let Some(stack_id) = get_or_create_stack_id(ctx, target, &stacks_in_ws, perm) {
                    let assignments = matching(assignments, rule.filters.clone())
                        .into_iter()
                        .filter(|e| e.stack_id != Some(stack_id))
                        .map(|mut e| {
                            e.stack_id = Some(stack_id);
                            e
                        })
                        .collect_vec();
                    updates += handle_assign(
                        ctx.db.get_mut()?.hunk_assignments_mut()?,
                        repo,
                        ws,
                        assignments,
                        dependencies.as_ref(),
                        ctx.settings.context_lines,
                    )
                    .unwrap_or_default();
                }
            }
            super::Action::Explicit(super::Operation::Amend { change_id }) => {
                let assignments = matching(assignments, rule.filters.clone());
                handle_amend(
                    &ctx.project_data_dir(),
                    repo,
                    ws,
                    assignments,
                    &change_id,
                    perm,
                    ctx.settings.context_lines,
                )
                .unwrap_or_default();
            }
            _ => continue,
        };
    }
    Ok(updates)
}

fn handle_amend(
    project_data_dir: &Path,
    repo: &gix::Repository,
    ws: &but_graph::projection::Workspace,
    assignments: Vec<HunkAssignment>,
    change_id: &ChangeId,
    perm: &mut WorktreeWritePermission,
    context_lines: u32,
) -> anyhow::Result<()> {
    let changes: Vec<DiffSpec> = assignments.into_iter().map(|a| a.into()).collect();
    let mut commit_id: Option<gix::ObjectId> = None;
    'outer: for commit in ws.commits() {
        let commit_change_id = commit.attach(repo)?.headers().and_then(|hdr| hdr.change_id);
        if commit_change_id.is_some_and(|cid| cid == *change_id) {
            commit_id = Some(commit.id);
            break 'outer;
        }
    }

    let commit_id = commit_id.ok_or_else(|| {
        anyhow::anyhow!(
            "No commit with Change-Id {} found in the current workspace",
            change_id
        )
    })?;

    commit_engine::create_commit_and_update_refs_with_project(
        repo,
        project_data_dir,
        None,
        but_workspace::commit_engine::Destination::AmendCommit {
            commit_id,
            // TODO: Expose this in the UI for 'edit message' functionality.
            new_message: None,
        },
        changes,
        context_lines,
        perm,
    )?;
    Ok(())
}

fn get_or_create_stack_id(
    ctx: &Context,
    target: StackTarget,
    stacks_in_ws: &[StackEntry],
    perm: &mut WorktreeWritePermission,
) -> Option<StackId> {
    let sorted_stack_ids = stacks_in_ws
        .iter()
        .sorted_by(|a, b| Ord::cmp(&a.order.unwrap_or_default(), &b.order.unwrap_or_default()))
        .filter_map(|s| s.id)
        .collect_vec();
    match target {
        StackTarget::StackId(stack_id) => {
            if let Ok(stack_id) = StackId::from_str(&stack_id) {
                if sorted_stack_ids.iter().any(|e| e == &stack_id) {
                    Some(stack_id)
                } else {
                    None
                }
            } else {
                None
            }
        }
        StackTarget::Leftmost => {
            if sorted_stack_ids.is_empty() {
                create_stack(ctx, perm).ok()
            } else {
                sorted_stack_ids.first().cloned()
            }
        }
        StackTarget::Rightmost => {
            if sorted_stack_ids.is_empty() {
                create_stack(ctx, perm).ok()
            } else {
                sorted_stack_ids.last().cloned()
            }
        }
    }
}

fn create_stack(ctx: &Context, perm: &mut WorktreeWritePermission) -> anyhow::Result<StackId> {
    use anyhow::Context;
    let repo = &*ctx.repo.get()?;
    let branch_name = but_core::branch::unique_canned_refname(repo)?;
    let ws = ctx.workspace_for_editing_with_perm(perm)?;
    let mut meta = ctx.meta(perm.read_permission())?;
    let new_ws = but_workspace::branch::create_reference(
        branch_name.as_ref(),
        None,
        repo,
        &ws,
        &mut meta,
        |_| StackId::generate(),
        None,
    )?;
    let (stack, _) = new_ws
        .find_segment_and_stack_by_refname(branch_name.as_ref())
        .context("BUG: need to find stack that was just created")?;
    // TODO: when caching is available, return new_ws and update the mut ctx
    stack
        .id
        .context("BUG: newly created stacks always have an ID")
}

fn handle_assign(
    db: HunkAssignmentsHandleMut,
    repo: &gix::Repository,
    workspace: &but_graph::projection::Workspace,
    assignments: Vec<HunkAssignment>,
    deps: Option<&HunkDependencies>,
    context_lines: u32,
) -> anyhow::Result<usize> {
    let len = assignments.len();
    but_hunk_assignment::assign(
        db,
        repo,
        workspace,
        but_hunk_assignment::assignments_to_requests(assignments),
        deps,
        context_lines,
    )
    .map(|_| len)
    .or_else(|_| Ok(0))
}

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
