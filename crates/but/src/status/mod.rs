use assignment::FileAssignment;
use bstr::BString;
use but_core::ui::{TreeChange, TreeStatus};
use but_hunk_assignment::HunkAssignment;
use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use std::collections::BTreeMap;
use std::path::Path;
pub(crate) mod assignment;

use crate::id::CliId;

pub(crate) fn worktree(repo_path: &Path, _json: bool) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_rules::process_rules(ctx).ok(); // TODO: this is doing double work (dependencies can be reused)

    let stack_id_to_branch = crate::log::stacks(ctx)?
        .iter()
        .filter_map(|s| {
            s.heads.first().and_then(|head| {
                let id = s.id?;
                let x = head.name.to_string();
                Some((id, x))
            })
        })
        .collect::<BTreeMap<but_workspace::StackId, String>>();

    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(project.path)?.changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;

    let mut by_file: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();
    for assignment in &assignments {
        by_file
            .entry(assignment.path_bytes.clone())
            .or_default()
            .push(assignment.clone());
    }
    let mut assignments_by_file: BTreeMap<BString, FileAssignment> = BTreeMap::new();
    for (path, assignments) in &by_file {
        assignments_by_file.insert(
            path.clone(),
            FileAssignment::from_assignments(path, assignments),
        );
    }
    if stack_id_to_branch.is_empty() {
        println!("No branches found. ¯\\_(ツ)_/¯");
        return Ok(());
    }

    let unassigned = assignment::filter_by_stack_id(assignments_by_file.values(), &None);
    print_group(None, unassigned, &changes)?;

    for (stack_id, branch) in &stack_id_to_branch {
        let filtered =
            assignment::filter_by_stack_id(assignments_by_file.values(), &Some(*stack_id));
        print_group(Some(branch.as_str()), filtered, &changes)?;
    }
    Ok(())
}

pub fn print_group(
    group: Option<&str>,
    assignments: Vec<FileAssignment>,
    changes: &[TreeChange],
) -> anyhow::Result<()> {
    let id = if let Some(group) = group {
        CliId::branch(group)
    } else {
        CliId::unassigned()
    }
    .to_string()
    .underline()
    .blue();
    let group = &group
        .map(|s| format!("[{}]", s))
        .unwrap_or("<UNASSIGNED>".to_string());
    println!("{}    {}", id, group.green().bold());
    for fa in assignments {
        let state = status_from_changes(changes, fa.path.clone());
        let path = match state {
            Some(state) => match state {
                TreeStatus::Addition { .. } => fa.path.to_string().green(),
                TreeStatus::Deletion { .. } => fa.path.to_string().red(),
                TreeStatus::Modification { .. } => fa.path.to_string().yellow(),
                TreeStatus::Rename { .. } => fa.path.to_string().purple(),
            },
            None => fa.path.to_string().normal(),
        };

        let id = CliId::file_from_assignment(&fa.assignments[0])
            .to_string()
            .underline()
            .blue();

        let mut locks = fa
            .assignments
            .iter()
            .flat_map(|a| a.hunk_locks.iter())
            .flatten()
            .map(|l| l.commit_id.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|commit_id| {
                format!(
                    "{}{}",
                    commit_id[..2].blue().underline(),
                    commit_id[2..7].blue()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        if !locks.is_empty() {
            locks = format!("🔒 {}", locks);
        }
        println!("{} ({}) {} {}", id, fa.assignments.len(), path, locks);
    }
    println!();
    Ok(())
}

pub(crate) fn all_files(ctx: &mut CommandContext) -> anyhow::Result<Vec<CliId>> {
    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(ctx.project().path.clone())?.changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;
    let out = assignments
        .iter()
        .map(CliId::file_from_assignment)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(out)
}

pub(crate) fn all_branches(ctx: &CommandContext) -> anyhow::Result<Vec<CliId>> {
    let stacks = crate::log::stacks(ctx)?;
    let mut branches = Vec::new();
    for stack in stacks {
        for head in stack.heads {
            branches.push(CliId::branch(&head.name.to_string()));
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
