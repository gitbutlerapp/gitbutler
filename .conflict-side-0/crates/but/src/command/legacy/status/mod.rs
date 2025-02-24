use std::collections::BTreeMap;

use crate::CLI_DATE;
use assignment::FileAssignment;
use bstr::{BString, ByteSlice};
use but_api::diff::ComputeLineStats;
use but_core::{TreeStatus, ui};
use but_ctx::{Context, LegacyProject};
use but_hunk_assignment::HunkAssignment;
use but_oxidize::{ObjectIdExt, OidExt, TimeExt};
use but_workspace::ui::StackDetails;
use colored::{ColoredString, Colorize};
use gix::date::time::CustomFormat;
use serde::Serialize;

const DATE_ONLY: CustomFormat = CustomFormat::new("%Y-%m-%d");

pub(crate) mod assignment;
pub(crate) mod json;

use crate::{IdMap, utils::OutputChannel};

type StackDetail = (Option<StackDetails>, Vec<FileAssignment>);
type StackEntry = (Option<gitbutler_stack::StackId>, StackDetail);

#[derive(Serialize)]
struct CommonMergeBase {
    target_name: String,
    common_merge_base: String,
    message: String,
    commit_date: String,
    commit_id: gix::ObjectId,
    created_at: i128,
    author_name: String,
    author_email: String,
}

#[derive(Serialize, Clone)]
struct UpstreamState {
    target_name: String,
    behind_count: usize,
    latest_commit: String,
    message: String,
    commit_date: String,
    last_fetched_ms: Option<u128>,
    commit_id: gix::ObjectId,
    created_at: i128,
    author_name: String,
    author_email: String,
}

pub(crate) async fn worktree(
    project: &LegacyProject,
    out: &mut OutputChannel,
    show_files: bool,
    verbose: bool,
    review: bool,
) -> anyhow::Result<()> {
    let ctx = &mut Context::new_from_legacy_project(project.clone())?;
    but_rules::process_rules(ctx).ok(); // TODO: this is doing double work (hunk-dependencies can be reused)

    let guard = ctx.shared_worktree_access();
    let meta = ctx.meta(guard.read_permission())?;

    // TODO: use this for status information instead.
    let head_info = but_workspace::head_info(
        &*ctx.repo.get()?,
        &meta,
        but_workspace::ref_info::Options {
            expensive_commit_info: true,
            ..Default::default()
        },
    )?;
    let mut id_map = IdMap::new_for_branches_and_commits(&head_info.stacks)?;
    id_map.add_file_info_from_context(ctx)?;

    let review_map = if review {
        crate::command::legacy::forge::review::get_review_map(project).await?
    } else {
        std::collections::HashMap::new()
    };

    let stacks = but_api::legacy::workspace::stacks(project.id, None)?;
    let worktree_changes = but_api::legacy::diff::changes_in_worktree(project.id)?;

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
            FileAssignment::from_assignments(&id_map, path, assignments),
        );
    }
    let mut stack_details: Vec<StackEntry> = vec![];

    let unassigned = assignment::filter_by_stack_id(assignments_by_file.values(), &None);
    stack_details.push((None, (None, unassigned)));

    // For JSON output, we'll need the original StackDetails to avoid redundant conversions
    let mut original_stack_details: Vec<(Option<gitbutler_stack::StackId>, Option<StackDetails>)> =
        vec![(None, None)];

    for stack in stacks {
        let details = but_api::legacy::workspace::stack_details(project.id, stack.id)?;
        let assignments = assignment::filter_by_stack_id(assignments_by_file.values(), &stack.id);
        original_stack_details.push((stack.id, Some(details.clone())));
        stack_details.push((stack.id, (Some(details), assignments)));
    }

    // Calculate common_merge_base data
    let stack = gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());
    let target = stack.get_default_target()?;
    let target_name = format!("{}/{}", target.branch.remote(), target.branch.branch());
    let repo = ctx.repo.get()?;
    let base_commit = repo.find_commit(target.sha.to_gix())?;
    let base_commit_decoded = base_commit.decode()?;
    let message = base_commit_decoded
        .message
        .to_string()
        .replace('\n', " ")
        .chars()
        .take(50)
        .collect::<String>();
    let formatted_date = base_commit_decoded
        .committer()?
        .time()?
        .format_or_unix(DATE_ONLY);
    let author = base_commit_decoded.author()?;
    let common_merge_base_data = CommonMergeBase {
        target_name: target_name.clone(),
        common_merge_base: target.sha.to_string()[..7].to_string(),
        message: message.clone(),
        commit_date: formatted_date,
        commit_id: target.sha.to_gix(),
        created_at: base_commit_decoded.committer()?.time()?.seconds as i128 * 1000,
        author_name: author.name.to_string(),
        author_email: author.email.to_string(),
    };

    // Get cached upstream state information (without fetching)
    let (upstream_state, last_fetched_ms) =
        but_api::legacy::virtual_branches::get_base_branch_data(project.id)
            .ok()
            .flatten()
            .map(|base_branch| {
                let last_fetched = base_branch.last_fetched_ms;
                let state = if base_branch.behind > 0 {
                    // Get the latest commit on the upstream branch (current_sha is the tip of the remote branch)
                    let commit_id = base_branch.current_sha;
                    repo.find_commit(commit_id.to_gix())
                        .ok()
                        .and_then(|commit_obj| {
                            let commit = commit_obj.decode().ok()?;
                            let commit_message = commit
                                .message
                                .to_string()
                                .replace('\n', " ")
                                .chars()
                                .take(50)
                                .collect::<String>();

                            let formatted_date = commit
                                .committer()
                                .ok()?
                                .time()
                                .ok()?
                                .format_or_unix(DATE_ONLY);

                            let author = commit.author().ok()?;

                            Some(UpstreamState {
                                target_name: base_branch.branch_name.clone(),
                                behind_count: base_branch.behind,
                                latest_commit: commit_id.to_string()[..7].to_string(),
                                message: commit_message,
                                commit_date: formatted_date,
                                last_fetched_ms: last_fetched,
                                commit_id: commit_id.to_gix(),
                                created_at: commit.committer().ok()?.time().ok()?.seconds as i128
                                    * 1000,
                                author_name: author.name.to_string(),
                                author_email: author.email.to_string(),
                            })
                        })
                } else {
                    None
                };
                (state, last_fetched)
            })
            .unwrap_or((None, None));

    if let Some(out) = out.for_json() {
        let workspace_status = json::build_workspace_status_json(
            &original_stack_details,
            &stack_details,
            &worktree_changes.worktree_changes.changes,
            &common_merge_base_data,
            &upstream_state,
            last_fetched_ms,
            &review_map,
            show_files,
            review,
            project.id,
            &repo,
            &mut id_map,
        )?;
        out.write_value(workspace_status)?;
        return Ok(());
    }

    let Some(out) = out.for_human() else {
        return Ok(());
    };

    drop(base_commit_decoded);
    drop(base_commit);
    drop(repo);
    let stack_details_len = stack_details.len();
    for (i, (stack_id, (details, assignments))) in stack_details.into_iter().enumerate() {
        let mut stack_mark = stack_id.and_then(|stack_id| {
            if crate::command::legacy::mark::stack_marked(ctx, stack_id).unwrap_or_default() {
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
            ctx,
            i == stack_details_len - 1,
            i == 0,
            &review_map,
            out,
            &mut id_map,
        )?;
    }
    // Format the last fetched time as relative time
    let last_checked_text = last_fetched_ms
        .map(|ms| {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let elapsed_ms = now_ms.saturating_sub(ms);
            let elapsed_secs = elapsed_ms / 1000;

            let relative_time = if elapsed_secs < 60 {
                format!("{} seconds ago", elapsed_secs)
            } else if elapsed_secs < 3600 {
                let minutes = elapsed_secs / 60;
                format!(
                    "{} {} ago",
                    minutes,
                    if minutes == 1 { "minute" } else { "minutes" }
                )
            } else if elapsed_secs < 86400 {
                let hours = elapsed_secs / 3600;
                format!(
                    "{} {} ago",
                    hours,
                    if hours == 1 { "hour" } else { "hours" }
                )
            } else {
                let days = elapsed_secs / 86400;
                format!("{} {} ago", days, if days == 1 { "day" } else { "days" })
            };

            format!(" (checked {})", relative_time)
        })
        .unwrap_or_default();

    // Display upstream state if there are new commits
    if let Some(upstream) = &upstream_state {
        let dot = "●".yellow();

        writeln!(
            out,
            "┊{dot} {} (upstream) ⏫ {} new commits {} {}{}",
            upstream.latest_commit.dimmed(),
            upstream.behind_count,
            upstream.commit_date.dimmed(),
            upstream.message,
            last_checked_text.dimmed()
        )?;
    }

    writeln!(
        out,
        "{} {} (common base) [{}] {} {}{}",
        if upstream_state.is_some() {
            "├╯"
        } else {
            "┴"
        },
        common_merge_base_data.common_merge_base.dimmed(),
        common_merge_base_data.target_name.green().bold(),
        common_merge_base_data.commit_date.dimmed(),
        common_merge_base_data.message,
        if upstream_state.is_none() {
            last_checked_text.dimmed().to_string()
        } else {
            String::new()
        }
    )?;
    Ok(())
}

fn print_assignments(
    assignments: &Vec<FileAssignment>,
    changes: &[ui::TreeChange],
    dotted: bool,
    out: &mut dyn std::fmt::Write,
) -> std::fmt::Result {
    for fa in assignments {
        let state = status_from_changes(changes, fa.path.clone());
        let path = match &state {
            Some(state) => path_with_color_ui(state, fa.path.to_string()),
            None => fa.path.to_string().normal(),
        };

        let status = state.as_ref().map(status_letter_ui).unwrap_or_default();

        let id = fa.assignments[0].cli_id.underline().blue();

        let mut locks = fa
            .assignments
            .iter()
            .flat_map(|a| a.inner.hunk_locks.iter())
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
            writeln!(out, "┊   {id} {status} {path} {locks}")?;
        } else {
            writeln!(out, "┊│   {id} {status} {path} {locks}")?;
        }
    }

    Ok(())
}

#[expect(clippy::too_many_arguments)]
pub fn print_group(
    project: &gitbutler_project::Project,
    group: Option<StackDetails>,
    assignments: Vec<FileAssignment>,
    changes: &[ui::TreeChange],
    show_files: bool,
    verbose: bool,
    show_url: bool,
    stack_mark: &mut Option<ColoredString>,
    ctx: &mut Context,
    _last: bool,
    first: bool,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    out: &mut dyn std::fmt::Write,
    id_map: &mut IdMap,
) -> anyhow::Result<()> {
    let repo = project.open_isolated_repo()?;
    if let Some(group) = &group {
        let mut first = true;
        for branch in &group.branch_details {
            let id = id_map
                .resolve_branch(branch.name.as_ref())
                .to_short_string()
                .underline()
                .blue();
            let notch = if first { "╭" } else { "├" };
            if !first {
                writeln!(out, "┊│")?;
            }

            let no_commits = if branch.commits.is_empty() {
                "(no commits)".to_string()
            } else {
                "".to_string()
            }
            .dimmed()
            .italic();

            let reviews = crate::command::legacy::forge::review::get_review_numbers(
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
                out,
                "┊{notch}┄{id} [{branch}{workspace}]{reviews} {no_commits} {stack_mark}",
                stack_mark = stack_mark.clone().unwrap_or_default(),
                branch = branch.name.to_string().green().bold(),
            )?;
            *stack_mark = None; // Only show the stack mark for the first branch
            if first {
                print_assignments(&assignments, changes, false, out)?;
            }
            first = false;
            for commit in &branch.upstream_commits {
                let dot = "●".yellow();
                print_commit(
                    ctx,
                    commit.id,
                    created_at_of_commit(ctx, commit.id)?,
                    commit.message.to_string(),
                    commit.author.name.clone(),
                    dot,
                    false,
                    show_files,
                    verbose,
                    false,
                    show_url,
                    None,
                    id_map,
                    out,
                )?;
            }
            for cli_commit in &branch.commits {
                let commit = &cli_commit;
                let marked =
                    crate::command::legacy::mark::commit_marked(ctx, commit.id.to_string())
                        .unwrap_or_default();
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
                    ctx,
                    commit.id,
                    created_at_of_commit(ctx, commit.id)?,
                    commit.message.to_string(),
                    commit.author.name.clone(),
                    dot,
                    marked,
                    show_files,
                    verbose,
                    commit.has_conflicts,
                    show_url,
                    commit.gerrit_review_url.clone(),
                    id_map,
                    out,
                )?;
            }
        }
    } else {
        id_map.add_file_info_from_context(ctx)?;
        let id = id_map.unassigned().to_short_string().underline().blue();
        writeln!(
            out,
            "╭┄{} [{}] {}",
            id,
            "Unassigned Changes".to_string().green().bold(),
            stack_mark.clone().unwrap_or_default()
        )?;
        print_assignments(&assignments, changes, true, out)?;
    }
    if !first {
        writeln!(out, "├╯")?;
    }
    writeln!(out, "┊")?;
    Ok(())
}

// TODO: we have the commit information, but the caller uses a degenerated structure that loses TZ information.
//       Use the original data (which would also fix frontend display).
fn created_at_of_commit(
    ctx: &Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<gix::date::Time> {
    Ok(ctx
        .git2_repo
        .get()?
        .find_commit(commit_id.to_git2())?
        .time()
        .to_gix())
}

fn status_letter(status: &TreeStatus) -> char {
    match status {
        TreeStatus::Addition { .. } => 'A',
        TreeStatus::Deletion { .. } => 'D',
        TreeStatus::Modification { .. } => 'M',
        TreeStatus::Rename { .. } => 'R',
    }
}

fn status_letter_ui(status: &ui::TreeStatus) -> char {
    match status {
        ui::TreeStatus::Addition { .. } => 'A',
        ui::TreeStatus::Deletion { .. } => 'D',
        ui::TreeStatus::Modification { .. } => 'M',
        ui::TreeStatus::Rename { .. } => 'R',
    }
}

fn path_with_color_ui(status: &ui::TreeStatus, path: String) -> ColoredString {
    match status {
        ui::TreeStatus::Addition { .. } => path.green(),
        ui::TreeStatus::Deletion { .. } => path.red(),
        ui::TreeStatus::Modification { .. } => path.yellow(),
        ui::TreeStatus::Rename { .. } => path.purple(),
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

fn status_from_changes(changes: &[ui::TreeChange], path: BString) -> Option<ui::TreeStatus> {
    changes.iter().find_map(|change| {
        if change.path_bytes == path {
            Some(change.status.clone())
        } else {
            None
        }
    })
}

#[expect(clippy::too_many_arguments)]
fn print_commit(
    ctx: &Context,
    commit_id: gix::ObjectId,
    created_at: gix::date::Time,
    message: String,
    author_name: String,
    dot: ColoredString,
    marked: bool,
    show_files: bool,
    verbose: bool,
    has_conflicts: bool,
    show_url: bool,
    review_url: Option<String>,
    id_map: &mut IdMap,
    out: &mut dyn std::fmt::Write,
) -> anyhow::Result<()> {
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

    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    let no_changes = if show_files && commit_details.diff_with_first_parent.is_empty() {
        "(no changes)".dimmed().italic()
    } else {
        "".to_string().normal()
    };

    if verbose {
        // Verbose format: author and timestamp on first line, message on second line
        let formatted_time = created_at.format_or_unix(CLI_DATE);
        writeln!(
            out,
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
        )?;
        writeln!(out, "┊│     {message}")?;
    } else {
        // Original format: everything on one line
        let review_url = if show_url {
            review_url.map(|r| format!("◖{}◗", r.underline().blue()))
        } else {
            review_url.map(|_| format!("◖{}◗", "r".normal()))
        }
        .unwrap_or_default();
        writeln!(
            out,
            "┊{dot}   {}{} {} {} {} {} {}",
            &commit_id.to_string()[..2].blue().underline(),
            &commit_id.to_string()[2..7].dimmed(),
            message,
            no_changes,
            conflicted_str,
            review_url,
            mark.unwrap_or_default()
        )?;
    }
    if show_files {
        for change in &commit_details.diff_with_first_parent {
            let cid = id_map
                .resolve_file_changed_in_commit_or_unassigned(commit_id, change.path.as_ref())
                .to_short_string()
                .blue()
                .underline();
            let path = path_with_color(&change.status, change.path.to_string());
            let status_letter = status_letter(&change.status);
            writeln!(out, "┊│     {cid} {status_letter} {path}")?;
        }
    }
    Ok(())
}
