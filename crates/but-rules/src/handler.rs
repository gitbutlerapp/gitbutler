use std::str::FromStr;

use but_core::{DiffSpec, ref_metadata::StackId};
use but_ctx::Context;
use but_hunk_assignment::{HunkAssignment, assign, assignments_to_requests};
use but_hunk_dependency::ui::HunkDependencies;
use but_meta::VirtualBranchesTomlMetadata;
use but_workspace::legacy::{StacksFilter, commit_engine, ui::StackEntry};
use itertools::Itertools;

use crate::{Filter, StackTarget};

pub fn process_workspace_rules(
    ctx: &mut Context,
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
            ) || matches!(
                &r.action,
                super::Action::Explicit(super::Operation::Amend { .. })
            )
        })
        .collect_vec();

    if rules.is_empty() {
        return Ok(updates);
    }

    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let stacks_in_ws = {
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project_data_dir().join("virtual_branches.toml"),
        )?;
        but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)
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
            super::Action::Explicit(super::Operation::Amend { change_id }) => {
                let assignments = matching(assignments, rule.filters.clone());
                handle_amend(ctx, assignments, change_id).unwrap_or_default();
            }
            _ => continue,
        };
    }
    Ok(updates)
}

fn handle_amend(
    ctx: &mut Context,
    assignments: Vec<HunkAssignment>,
    change_id: String,
) -> anyhow::Result<()> {
    let changes: Vec<DiffSpec> = assignments.into_iter().map(|a| a.into()).collect();
    let mut guard = ctx.exclusive_worktree_access();
    let repo = ctx.clone_repo_for_merging()?;

    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project_data_dir().join("virtual_branches.toml"),
    )?;
    let ref_info_options = but_workspace::ref_info::Options {
        expensive_commit_info: true,
        traversal: but_graph::init::Options::limited(),
    };
    let info = but_workspace::head_info(&repo, &meta, ref_info_options)?;
    let mut commit_id: Option<gix::ObjectId> = None;
    'outer: for stack in info.stacks {
        for segment in stack.segments {
            for commit in segment.commits {
                if Some(change_id.clone()) == commit.change_id.map(|c| c.to_string()) {
                    commit_id = Some(commit.id);
                    break 'outer;
                }
            }
        }
    }

    let commit_id = commit_id.ok_or_else(|| {
        anyhow::anyhow!(
            "No commit with Change-Id {} found in the current workspace",
            change_id
        )
    })?;

    commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &ctx.project_data_dir(),
        None,
        but_workspace::commit_engine::Destination::AmendCommit {
            commit_id,
            // TODO: Expose this in the UI for 'edit message' functionality.
            new_message: None,
        },
        changes,
        ctx.settings().context_lines,
        guard.write_permission(),
    )?;
    Ok(())
}

fn get_or_create_stack_id(
    ctx: &Context,
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

fn create_stack(ctx: &Context) -> anyhow::Result<StackId> {
    let template = gitbutler_stack::canned_branch_name(&*ctx.git2_repo.get()?)?;
    let vb_state = &gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());
    let branch_name =
        gitbutler_stack::Stack::next_available_name(&*ctx.repo.get()?, vb_state, template, false)?;
    let create_req = gitbutler_branch::BranchCreateRequest {
        name: Some(branch_name),
        ownership: None,
        order: None,
        selected_for_changes: None,
    };

    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();

    let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &create_req, perm)?;
    Ok(stack.id)
}

fn handle_assign(
    ctx: &mut Context,
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
