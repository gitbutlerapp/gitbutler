use but_graph::VirtualBranchesTomlMetadata;
use but_hunk_assignment::{HunkAssignment, assign, assignments_to_requests};
use but_hunk_dependency::ui::HunkDependencies;
use but_workspace::{StackId, StacksFilter, ui::StackEntry};
use gitbutler_command_context::CommandContext;
use itertools::Itertools;
use std::str::FromStr;

use crate::{Filter, StackTarget};

pub fn process_workspace_rules(
    ctx: &mut CommandContext,
    assignments: &[HunkAssignment],
    dependencies: &Option<HunkDependencies>,
) -> anyhow::Result<usize> {
    let mut updates = 0;
    if assignments.is_empty() {
        // Dont create stacks if there are no changes to assign anywhere
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
            )
        })
        .collect_vec();

    if rules.is_empty() {
        return Ok(updates);
    }

    let repo = ctx.gix_repo_for_merging_non_persisting()?;
    let stacks_in_ws = if ctx.app_settings().feature_flags.ws3 {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)
    } else {
        but_workspace::stacks(ctx, &ctx.project().gb_dir(), &repo, StacksFilter::default())
    }?;

    for rule in rules {
        match rule.action {
            super::Action::Explicit(super::Operation::Assign { target }) => {
                if let Some(stack_id) = get_or_create_stack_id(ctx, target, &stacks_in_ws) {
                    let assignments = matching(assignments, rule.filters.clone())
                        .into_iter()
                        .filter(|e| e.stack_id != Some(stack_id))
                        .map(|mut e| {
                            e.stack_id = Some(stack_id);
                            e
                        })
                        .collect_vec();
                    updates +=
                        handle_assign(ctx, assignments, dependencies.as_ref()).unwrap_or_default();
                }
            }
            _ => continue,
        };
    }
    Ok(updates)
}

fn get_or_create_stack_id(
    ctx: &CommandContext,
    target: StackTarget,
    stacks_in_ws: &[StackEntry],
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
                    Option::Some(stack_id)
                } else {
                    Option::None
                }
            } else {
                Option::None
            }
        }
        StackTarget::Leftmost => {
            if sorted_stack_ids.is_empty() {
                create_stack(ctx).ok()
            } else {
                sorted_stack_ids.first().cloned()
            }
        }
        StackTarget::Rightmost => {
            if sorted_stack_ids.is_empty() {
                create_stack(ctx).ok()
            } else {
                sorted_stack_ids.last().cloned()
            }
        }
    }
}

fn create_stack(ctx: &CommandContext) -> anyhow::Result<StackId> {
    let template = gitbutler_stack::canned_branch_name(ctx.repo())?;
    let vb_state = &gitbutler_stack::VirtualBranchesHandle::new(ctx.project().gb_dir());
    let branch_name =
        gitbutler_stack::Stack::next_available_name(&ctx.gix_repo()?, vb_state, template, false)?;
    let create_req = gitbutler_branch::BranchCreateRequest {
        name: Some(branch_name),
        ownership: None,
        order: None,
        selected_for_changes: None,
    };

    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();

    let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &create_req, perm)?;
    Ok(stack.id)
}

fn handle_assign(
    ctx: &mut CommandContext,
    assignments: Vec<HunkAssignment>,
    deps: Option<&HunkDependencies>,
) -> anyhow::Result<usize> {
    let len = assignments.len();
    if assign(ctx, assignments_to_requests(assignments), deps).is_ok() {
        Ok(len)
    } else {
        Ok(0)
    }
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
