use assignment::FileAssignment;
use bstr::{BString, ByteSlice};
use but_core::ui::{TreeChange, TreeStatus};
use but_hunk_assignment::HunkAssignment;
use but_settings::AppSettings;
use but_workspace::ui::StackDetails;
use colored::{ColoredString, Colorize};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use std::collections::BTreeMap;
use std::path::Path;
pub(crate) mod assignment;

use crate::id::CliId;

pub(crate) fn worktree(repo_path: &Path, _json: bool, show_files: bool) -> anyhow::Result<()> {
    let project = Project::find_by_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_rules::process_rules(ctx).ok(); // TODO: this is doing double work (dependencies can be reused)

    let stacks = but_api::workspace::stacks(project.id, None)?;
    let worktree_changes = but_api::diff::changes_in_worktree(project.id)?;

    let mut by_file: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();
    for assignment in worktree_changes.assignments {
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
    let mut stack_details = vec![];

    let unassigned = assignment::filter_by_stack_id(assignments_by_file.values(), &None);
    stack_details.push((None, (None, unassigned)));
    for stack in stacks {
        let details = but_api::workspace::stack_details(project.id, stack.id)?;
        let assignments = assignment::filter_by_stack_id(assignments_by_file.values(), &stack.id);
        stack_details.push((stack.id, (Some(details), assignments)));
    }

    for (stack_id, (details, assignments)) in stack_details {
        let mut stack_mark = stack_id.and_then(|stack_id| {
            if crate::mark::stack_marked(ctx, stack_id).unwrap_or_default() {
                Some("◀ Marked ▶".red().bold())
            } else {
                None
            }
        });

        print_group(
            &project,
            details,
            assignments,
            &worktree_changes.worktree_changes.changes,
            show_files,
            &mut stack_mark,
            ctx,
        )?;
    }
    Ok(())
}

fn print_assignments(assignments: &Vec<FileAssignment>, changes: &[TreeChange]) {
    for fa in assignments {
        let state = status_from_changes(changes, fa.path.clone());
        let path = match &state {
            Some(state) => path_with_color(state, fa.path.to_string()),
            None => fa.path.to_string().normal(),
        };

        let status = state.as_ref().map(status_letter).unwrap_or_default();

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
            locks = format!("🔒 {locks}");
        }
        println!("│ {id}  {path} {status} {locks}");
    }
}

pub fn print_group(
    project: &Project,
    group: Option<StackDetails>,
    assignments: Vec<FileAssignment>,
    changes: &[TreeChange],
    show_files: bool,
    stack_mark: &mut Option<ColoredString>,
    ctx: &mut CommandContext,
) -> anyhow::Result<()> {
    if let Some(group) = &group {
        let mut first = true;
        for branch in &group.branch_details {
            let id = CliId::branch(branch.name.to_str()?)
                .to_string()
                .underline()
                .blue();
            let notch = if first { "╭" } else { "├" };
            if !first {
                println!("│");
            }
            println!(
                "{} {}  [{}] {}",
                notch,
                id,
                branch.name.to_string().green().bold(),
                stack_mark.clone().unwrap_or_default()
            );
            *stack_mark = None; // Only show the stack mark for the first branch
            if first {
                print_assignments(&assignments, changes);
            }
            first = false;
            for commit in &branch.commits {
                let marked =
                    crate::mark::commit_marked(ctx, commit.id.to_string()).unwrap_or_default();
                let mark = if marked {
                    Some("◀ Marked ▶".red().bold())
                } else {
                    None
                };
                let conflicted_str = if commit.has_conflicts {
                    "{conflicted}".red()
                } else {
                    "".normal()
                };
                println!(
                    "● {}{} {} {} {}",
                    &commit.id.to_string()[..2].blue().underline(),
                    &commit.id.to_string()[2..7].blue(),
                    conflicted_str,
                    commit
                        .message
                        .to_string()
                        .replace('\n', " ")
                        .chars()
                        .take(50)
                        .collect::<String>(),
                    mark.unwrap_or_default()
                );
                if show_files {
                    let commit_details =
                        but_api::diff::commit_details(project.id, commit.id.into())?;
                    for change in &commit_details.changes.changes {
                        let cid = CliId::committed_file(&change.path.to_string(), commit.id)
                            .to_string()
                            .blue()
                            .underline();
                        let path = path_with_color(&change.status, change.path.to_string());
                        let status_letter = status_letter(&change.status);
                        println!("│ {cid}  {path} {status_letter}");
                    }
                    if commit_details.changes.changes.is_empty() {
                        println!("│     {}", "(no changes)".dimmed().italic());
                    }
                }
            }
            if branch.commits.is_empty() {
                println!("│     {}", "(no commits)".dimmed().italic());
            }
        }
    } else {
        let id = CliId::branch("UNASSIGNED").to_string().underline().blue();
        println!(
            "╭ {}  [{}] {}",
            id,
            "UNASSIGNED".to_string().green().bold(),
            stack_mark.clone().unwrap_or_default()
        );
        print_assignments(&assignments, changes);
    }
    println!("┊");
    Ok(())
}

fn status_letter(status: &TreeStatus) -> char {
    match status {
        TreeStatus::Addition { .. } => 'A',
        TreeStatus::Deletion { .. } => 'D',
        TreeStatus::Modification { .. } => 'M',
        TreeStatus::Rename { .. } => 'R',
    }
}

fn path_with_color(status: &TreeStatus, path: String) -> ColoredString {
    match status {
        TreeStatus::Addition { .. } => path.green(),
        TreeStatus::Deletion { .. } => path.red(),
        TreeStatus::Modification { .. } => path.yellow(),
        TreeStatus::Rename { .. } => path.purple(),
    }
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

pub(crate) fn all_committed_files(ctx: &mut CommandContext) -> anyhow::Result<Vec<CliId>> {
    let mut committed_files = Vec::new();
    let stacks = but_api::workspace::stacks(ctx.project().id, None)?;
    for stack in stacks {
        let details = but_api::workspace::stack_details(ctx.project().id, stack.id)?;
        for branch in details.branch_details {
            for commit in branch.commits {
                let commit_details =
                    but_api::diff::commit_details(ctx.project().id, commit.id.into())?;
                for change in &commit_details.changes.changes {
                    let cid = CliId::committed_file(&change.path.to_string(), commit.id);
                    committed_files.push(cid);
                }
            }
        }
    }
    Ok(committed_files)
}
