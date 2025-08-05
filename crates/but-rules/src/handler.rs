use but_graph::VirtualBranchesTomlMetadata;
use but_hunk_assignment::{HunkAssignment, WorktreeChanges, assign, assignments_to_requests};
use but_hunk_dependency::ui::HunkDependencies;
use but_workspace::{StackId, StacksFilter};
use gitbutler_command_context::CommandContext;
use itertools::Itertools;
use std::str::FromStr;

use crate::{Filter, StackTarget};

pub fn on_filesystem_change(
    ctx: &mut CommandContext,
    worktree_changes: &WorktreeChanges,
) -> anyhow::Result<usize> {
    let mut updates = 0;
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
        but_workspace::stacks_v3(&repo, &meta, StacksFilter::InWorkspace)
    } else {
        but_workspace::stacks(ctx, &ctx.project().gb_dir(), &repo, StacksFilter::default())
    }?;

    for rule in rules {
        match rule.action {
            super::Action::Explicit(super::Operation::Assign { target }) => {
                match target {
                    StackTarget::StackId(stack_id) => {
                        if let Ok(stack_id) = StackId::from_str(&stack_id) {
                            if !stacks_in_ws
                                .iter()
                                .any(|e| e.id.is_some_and(|id| id == stack_id))
                            {
                                continue;
                            }
                            let assignments =
                                matching(worktree_changes.clone(), rule.filters.clone())
                                    .into_iter()
                                    .filter(|e| e.stack_id != Some(stack_id))
                                    .map(|mut e| {
                                        e.stack_id = Some(stack_id);
                                        e
                                    })
                                    .collect_vec();
                            updates += handle_assign(
                                ctx,
                                assignments,
                                worktree_changes.dependencies.as_ref(),
                            )
                            .unwrap_or_default();
                        }
                    }
                    StackTarget::Leftmost => continue,  // TODO
                    StackTarget::Rightmost => continue, // TODO
                }
            }
            _ => continue,
        };
    }
    Ok(updates)
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

fn matching(worktree_changes: WorktreeChanges, filters: Vec<Filter>) -> Vec<HunkAssignment> {
    if filters.is_empty() {
        return worktree_changes.assignments;
    }
    let mut assignments = Vec::new();
    for filter in filters {
        match filter {
            Filter::PathMatchesRegex(regex) => {
                for change in worktree_changes.assignments.iter() {
                    if regex.is_match(&change.path) {
                        assignments.push(change.clone());
                    }
                }
            }
            Filter::ContentMatchesRegex(_) => continue,
            Filter::FileChangeType(_) => continue,
            Filter::SemanticType(_) => continue,
        }
    }
    assignments
}
