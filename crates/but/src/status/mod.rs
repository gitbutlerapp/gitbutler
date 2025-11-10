use std::collections::BTreeMap;

use crate::CLI_DATE;
use assignment::FileAssignment;
use bstr::{BString, ByteSlice};
use but_core::ui::{TreeChange, TreeStatus};
use but_hunk_assignment::HunkAssignment;
use but_project::Project;
use but_workspace::ui::StackDetails;
use colored::{ColoredString, Colorize};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::{ObjectIdExt, OidExt, TimeExt};
use serde::Serialize;
use std::io::Write;

pub(crate) mod assignment;

use crate::id::CliId;

type StackDetail = (Option<StackDetails>, Vec<FileAssignment>);
type StackEntry = (Option<gitbutler_stack::StackId>, StackDetail);

#[derive(Serialize)]
struct CommonMergeBase {
    target_name: String,
    common_merge_base: String,
    message: String,
    commit_date: String,
}

#[derive(Serialize)]
struct WorktreeStatus {
    stacks: Vec<StackEntry>,
    common_merge_base: CommonMergeBase,
}

pub(crate) async fn worktree(
    project: &Project,
    json: bool,
    show_files: bool,
    verbose: bool,
    review: bool,
) -> anyhow::Result<()> {
    let mut ctx = project.legacy_ctx()?;
    let mut stdout = std::io::stdout();
    but_rules::process_rules(&mut ctx).ok(); // TODO: this is doing double work (dependencies can be reused)

    let guard = project.shared_worktree_access();
    let meta = project.meta(guard.read_permission())?;

    // TODO: use this for status inforamtion instead.
    let _head_info = but_workspace::head_info(
        &project.repo,
        &meta,
        but_workspace::ref_info::Options {
            expensive_commit_info: true,
            ..Default::default()
        },
    )?;

    let project = &project.legacy;
    let review_map = if review {
        crate::forge::review::get_review_map(project).await?
    } else {
        std::collections::HashMap::new()
    };

    let stacks = but_api::workspace::stacks(project.id, None)?;
    let worktree_changes = but_api::diff::changes_in_worktree(project.id)?;

    let mut by_file: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();
    for assignment in worktree_changes.assignments {
        by_file
            .entry(assignment.path_bytes.clone())
            .or_default()
            .push(assignment);
    }
    let mut assignments_by_file: BTreeMap<BString, FileAssignment> = BTreeMap::new();
    for (path, assignments) in &by_file {
        assignments_by_file.insert(
            path.clone(),
            FileAssignment::from_assignments(path, assignments),
        );
    }
    let mut stack_details: Vec<StackEntry> = vec![];

    let unassigned = assignment::filter_by_stack_id(assignments_by_file.values(), &None);
    stack_details.push((None, (None, unassigned)));
    for stack in stacks {
        let details = but_api::workspace::stack_details(project.id, stack.id)?;
        let assignments = assignment::filter_by_stack_id(assignments_by_file.values(), &stack.id);
        stack_details.push((stack.id, (Some(details), assignments)));
    }

    // Calculate common_merge_base data
    let stack = gitbutler_stack::VirtualBranchesHandle::new(ctx.project().gb_dir());
    let target = stack.get_default_target()?;
    let target_name = format!("{}/{}", target.branch.remote(), target.branch.branch());
    let repo = ctx.gix_repo()?;
    let base_commit = repo.find_commit(target.sha.to_gix())?;
    let base_commit = base_commit.decode()?;
    let message = base_commit
        .message
        .to_string()
        .replace('\n', " ")
        .chars()
        .take(50)
        .collect::<String>();
    let formatted_date = base_commit.committer().time()?.format_or_unix(CLI_DATE);
    let common_merge_base_data = CommonMergeBase {
        target_name: target_name.clone(),
        common_merge_base: target.sha.to_string()[..7].to_string(),
        message: message.clone(),
        commit_date: formatted_date,
    };

    if json {
        let worktree_status = WorktreeStatus {
            stacks: stack_details,
            common_merge_base: common_merge_base_data,
        };
        let json_output = serde_json::to_string_pretty(&worktree_status)?;
        writeln!(stdout, "{json_output}")?;
        return Ok(());
    }

    let stack_details_len = stack_details.len();
    for (i, (stack_id, (details, assignments))) in stack_details.into_iter().enumerate() {
        let mut stack_mark = stack_id.and_then(|stack_id| {
            if crate::mark::stack_marked(&mut ctx, stack_id).unwrap_or_default() {
                Some("◀ Marked ▶".red().bold())
            } else {
                None
            }
        });

        print_group(
            project,
            details,
            assignments,
            &worktree_changes.worktree_changes.changes,
            show_files,
            verbose,
            review,
            &mut stack_mark,
            &mut ctx,
            i == stack_details_len - 1,
            i == 0,
            &review_map,
        )?;
    }
    let dot = "●".purple();
    writeln!(
        stdout,
        "{dot} {} (common base) [{}] {} {}",
        common_merge_base_data.common_merge_base.dimmed(),
        common_merge_base_data.target_name.green().bold(),
        common_merge_base_data.commit_date.dimmed(),
        common_merge_base_data.message
    )
    .ok();
    Ok(())
}

fn print_assignments(
    assignments: &Vec<FileAssignment>,
    changes: &[TreeChange],
    dotted: bool,
) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
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
        if dotted {
            writeln!(stdout, "┊   {id} {status} {path} {locks}")?;
        } else {
            writeln!(stdout, "┊│   {id} {status} {path} {locks}")?;
        }
    }

    Ok(())
}

#[expect(clippy::too_many_arguments)]
pub fn print_group(
    project: &gitbutler_project::Project,
    group: Option<StackDetails>,
    assignments: Vec<FileAssignment>,
    changes: &[TreeChange],
    show_files: bool,
    verbose: bool,
    show_url: bool,
    stack_mark: &mut Option<ColoredString>,
    ctx: &mut CommandContext,
    _last: bool,
    first: bool,
    review_map: &std::collections::HashMap<String, Vec<gitbutler_forge::review::ForgeReview>>,
) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout();
    let repo = project.open_isolated()?;
    if let Some(group) = &group {
        let mut first = true;
        for branch in &group.branch_details {
            let id = CliId::branch(branch.name.to_str()?)
                .to_string()
                .underline()
                .blue();
            let notch = if first { "╭" } else { "├" };
            if !first {
                writeln!(stdout, "┊│").ok();
            }

            let no_commits = if branch.commits.is_empty() {
                "(no commits)".to_string()
            } else {
                "".to_string()
            }
            .dimmed()
            .italic();

            let reviews = crate::forge::review::get_review_numbers(
                &branch.name.to_string(),
                &branch.pr_number,
                review_map,
            );

            let workspace = branch
                .linked_worktree_id
                .as_ref()
                .and_then(|id| {
                    let ws = repo.worktree_proxy_by_id(id.as_bstr())?;
                    let base = ws.base().ok()?;
                    let git_dir = gix::path::realpath(repo.git_dir()).ok();
                    let base = git_dir
                        .and_then(|git_dir| base.strip_prefix(git_dir).ok())
                        .unwrap_or_else(|| &base);
                    format!(" 📁 {base}", base = base.display()).into()
                })
                .unwrap_or_default();
            writeln!(
                stdout,
                "┊{notch}┄{id} [{branch}{workspace}]{reviews} {no_commits} {stack_mark}",
                stack_mark = stack_mark.clone().unwrap_or_default(),
                branch = branch.name.to_string().green().bold(),
            )
            .ok();
            *stack_mark = None; // Only show the stack mark for the first branch
            if first {
                print_assignments(&assignments, changes, false).ok();
            }
            first = false;
            for commit in &branch.upstream_commits {
                let dot = "●".yellow();
                print_commit(
                    commit.id,
                    created_at_of_commit(ctx, commit.id)?,
                    commit.message.to_string(),
                    commit.author.name.clone(),
                    dot,
                    project.id,
                    false,
                    show_files,
                    verbose,
                    false,
                    show_url,
                    None,
                )?;
            }
            for commit in &branch.commits {
                let marked =
                    crate::mark::commit_marked(ctx, commit.id.to_string()).unwrap_or_default();
                let dot = match commit.state {
                    but_workspace::ui::CommitState::LocalOnly => "●".normal(),
                    but_workspace::ui::CommitState::LocalAndRemote(object_id) => {
                        if object_id == commit.id {
                            "●".green()
                        } else {
                            "◐".green()
                        }
                    }
                    but_workspace::ui::CommitState::Integrated => "●".purple(),
                };
                print_commit(
                    commit.id,
                    created_at_of_commit(ctx, commit.id)?,
                    commit.message.to_string(),
                    commit.author.name.clone(),
                    dot,
                    project.id,
                    marked,
                    show_files,
                    verbose,
                    commit.has_conflicts,
                    show_url,
                    commit.gerrit_review_url.clone(),
                )?;
            }
        }
    } else {
        let id = CliId::Unassigned.to_string().underline().blue();
        writeln!(
            stdout,
            "╭┄{} [{}] {}",
            id,
            "Unassigned Changes".to_string().green().bold(),
            stack_mark.clone().unwrap_or_default()
        )
        .ok();
        print_assignments(&assignments, changes, true).ok();
    }
    if !first {
        writeln!(stdout, "├╯").ok();
    }
    writeln!(stdout, "┊").ok();
    Ok(())
}

// TODO: we have the commit information, but the caller uses a degenerated structure that loses TZ information.
//       Use the original data (which would also fix frontend display).
fn created_at_of_commit(
    ctx: &CommandContext,
    commit_id: gix::ObjectId,
) -> anyhow::Result<gix::date::Time> {
    Ok(ctx.repo().find_commit(commit_id.to_git2())?.time().to_gix())
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
        but_core::diff::ui::worktree_changes_by_worktree_dir(ctx.project().worktree_dir()?.into())?
            .changes;
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

#[expect(clippy::too_many_arguments)]
fn print_commit(
    commit_id: gix::ObjectId,
    created_at: gix::date::Time,
    message: String,
    author_name: String,
    dot: ColoredString,
    project_id: gitbutler_project::ProjectId,
    marked: bool,
    show_files: bool,
    verbose: bool,
    has_conflicts: bool,
    show_url: bool,
    review_url: Option<String>,
) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout();
    let mark = if marked {
        Some("◀ Marked ▶".red().bold())
    } else {
        None
    };
    let conflicted_str = if has_conflicts {
        "{conflicted}".red()
    } else {
        "".normal()
    };

    let mut message = if verbose {
        message
            .replace('\n', " ")
            .chars()
            .take(50)
            .collect::<String>()
    } else {
        // For non-verbose mode, only use the first line (title)
        message
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(50)
            .collect::<String>()
    }
    .normal();
    if message.is_empty() {
        message = "(no commit message)".to_string().dimmed().italic();
    }

    let commit_details = but_api::diff::commit_details(project_id, commit_id.into())?;
    let no_changes = if show_files && commit_details.changes.changes.is_empty() {
        "(no changes)".dimmed().italic()
    } else {
        "".to_string().normal()
    };

    if verbose {
        // Verbose format: author and timestamp on first line, message on second line
        let formatted_time = created_at.format_or_unix(CLI_DATE);
        writeln!(
            stdout,
            "┊{dot}   {}{} {} {} {} {} {} {}",
            &commit_id.to_string()[..2].blue().underline(),
            &commit_id.to_string()[2..7].dimmed(),
            author_name,
            formatted_time.dimmed(),
            no_changes,
            conflicted_str,
            review_url
                .map(|r| format!("◖{}◗", r.underline().blue()))
                .unwrap_or_default(),
            mark.unwrap_or_default()
        )
        .ok();
        writeln!(stdout, "┊│     {message}").ok();
    } else {
        // Original format: everything on one line
        let review_url = if show_url {
            review_url.map(|r| format!("◖{}◗", r.underline().blue()))
        } else {
            review_url.map(|_| format!("◖{}◗", "r".normal()))
        }
        .unwrap_or_default();
        writeln!(
            stdout,
            "┊{dot}   {}{} {} {} {} {} {}",
            &commit_id.to_string()[..2].blue().underline(),
            &commit_id.to_string()[2..7].dimmed(),
            message,
            no_changes,
            conflicted_str,
            review_url,
            mark.unwrap_or_default()
        )
        .ok();
    }
    if show_files {
        for change in &commit_details.changes.changes {
            let cid = CliId::committed_file(&change.path.to_string(), commit_id)
                .to_string()
                .blue()
                .underline();
            let path = path_with_color(&change.status, change.path.to_string());
            let status_letter = status_letter(&change.status);
            writeln!(stdout, "┊│     {cid} {status_letter} {path}").ok();
        }
    }
    Ok(())
}
