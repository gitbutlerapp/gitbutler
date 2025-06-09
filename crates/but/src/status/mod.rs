use bstr::BString;
use but_core::ui::{TreeChange, TreeStatus};
use but_hunk_assignment::HunkAssignment;
use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use std::collections::BTreeMap;
use std::path::Path;

use crate::id::CliId;

pub(crate) fn worktree(repo_path: &Path, _json: bool) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::default())?;

    let stack_id_to_branch = crate::log::stacks(ctx)?
        .iter()
        .filter_map(|s| {
            s.heads.first().map(|head| {
                let x = head.name.to_string();
                (s.id, x)
            })
        })
        .collect::<BTreeMap<but_workspace::StackId, String>>();

    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(project.path)?.changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()))?;

    // Group the assignments by branch
    let mut groups = std::collections::HashMap::new();
    for assignment in assignments {
        let stack_id = assignment.stack_id;
        groups
            .entry(stack_id)
            .or_insert_with(Vec::new)
            .push(assignment);
    }

    if groups.is_empty() {
        println!("No uncommitted changes. ¯\\_(ツ)_/¯");
    } else {
        let unassigned_str = "<UNASSIGNED>".to_string();
        // Iterate over the groups, but always start with the unassigned group
        if let Some(unassigned) = groups.remove(&None) {
            print_group(&unassigned_str, unassigned, &changes)?;
        }
        // Iterate over the remaining groups
        for (stack_id, assignments) in groups {
            let branch_name = if let Some(stack_id) = stack_id {
                stack_id_to_branch.get(&stack_id).unwrap_or(&unassigned_str)
            } else {
                &unassigned_str
            };
            print_group(&format!("[{}]", branch_name), assignments, &changes)?;
        }
    }
    Ok(())
}

pub fn print_group(
    group: &str,
    assignments: Vec<HunkAssignment>,
    changes: &[TreeChange],
) -> anyhow::Result<()> {
    let id = CliId::branch(group).to_string().underline().blue();
    println!("{}    {}", id, group.green().bold());
    let mut unique_with_count = BTreeMap::new();
    for assignment in assignments {
        let (count, _) = unique_with_count
            .entry(assignment.path.clone())
            .or_insert((0, assignment));
        *count += 1;
    }
    for (path, (count, a)) in unique_with_count {
        let state = status_from_changes(changes, path.clone().into());
        let path = match state {
            Some(state) => match state {
                TreeStatus::Addition { .. } => path.green(),
                TreeStatus::Deletion { .. } => path.red(),
                TreeStatus::Modification { .. } => path.yellow(),
                TreeStatus::Rename { .. } => path.purple(),
            },
            None => path.normal(),
        };
        let id = CliId::file_from_assignment(&a)
            .to_string()
            .underline()
            .blue();
        println!("{} ({}) {}", id, count, path);
    }
    println!();
    Ok(())
}

pub(crate) fn file_from_hash(ctx: &mut CommandContext, hash: &str) -> anyhow::Result<Vec<CliId>> {
    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(ctx.project().path.clone())?.changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()))?;

    let mut matches = Vec::new();
    for assignment in assignments {
        let id = CliId::file_from_assignment(&assignment);
        if id.matches(hash) {
            matches.push(id);
        }
    }
    let out = matches
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(out)
}

pub(crate) fn branch_from_hash(ctx: &CommandContext, hash: &str) -> anyhow::Result<Vec<CliId>> {
    let stacks = crate::log::stacks(ctx)?;
    let mut branches = Vec::new();
    for stack in stacks {
        for head in stack.heads {
            let id = CliId::branch(&head.name.to_string());
            if id.matches(hash) {
                branches.push(id);
            }
        }
    }
    Ok(branches)
}

fn status_from_changes(changes: &[TreeChange], path: BString) -> Option<TreeStatus> {
    changes.iter().find_map(|change| {
        if change.path_bytes == path {
            Some(change.status.clone())
        } else {
            None
        }
    })
}
