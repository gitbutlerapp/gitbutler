use std::collections::BTreeMap;

use assignment::FileAssignment;
use bstr::{BStr, BString, ByteSlice};
use but_api::diff::ComputeLineStats;
use but_core::{TreeStatus, ui};
use but_ctx::Context;
use but_forge::ForgeReview;
use but_oxidize::OidExt;
use but_workspace::{ref_info::LocalCommitRelation, ui::PushStatus};
use colored::{ColoredString, Colorize};
use gitbutler_branch_actions::upstream_integration::BranchStatus as UpstreamBranchStatus;
use gitbutler_stack::StackId;
use gix::date::time::CustomFormat;
use serde::Serialize;
use terminal_size::Width;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    CLI_DATE,
    id::{SegmentWithId, StackWithId, TreeChangeWithId},
    utils::time::format_relative_time_verbose,
};

const DATE_ONLY: CustomFormat = CustomFormat::new("%Y-%m-%d");

/// Returns the current terminal width, defaulting to 80 columns.
fn terminal_width() -> usize {
    terminal_size::terminal_size().map_or(80, |(Width(w), _)| w as usize)
}

// Truncate `text` to fit within `max_width` display columns
// Uses [`unicode_width`] so that CJK / emoji characters (which occupy
// two terminal columns each) are measured correctly. When truncation
// occurs an `…` character (1 column wide) is appended and the total
// result is guaranteed to be ≤ `max_width` columns
fn truncate_text(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut width = 0;
    let mut result = String::new();

    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max_width {
            // Text will be truncated – reserve 1 column for '…'
            // Walk back if needed so the ellipsis still fits
            while width >= max_width {
                if let Some(last) = result.pop() {
                    width -= last.width().unwrap_or(0);
                } else {
                    break;
                }
            }
            result.push('…');
            return result;
        }
        result.push(ch);
        width += ch_width;
    }

    // No truncation needed.
    result
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CommitClassification {
    Upstream,
    LocalOnly,
    Pushed,
    Modified,
    Integrated,
}

pub(crate) mod assignment;
pub(crate) mod json;

use crate::{IdMap, command::legacy::forge::review, utils::OutputChannel};

type StackDetail = (Option<StackWithId>, Vec<FileAssignment>);
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

fn show_edit_mode_status(ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    // Delegate to the resolve status logic to show actual conflict details
    crate::command::legacy::resolve::show_resolve_status(ctx, out)
}

pub(crate) async fn worktree(
    ctx: &mut Context,
    out: &mut OutputChannel,
    show_files: bool,
    verbose: bool,
    refresh_prs: bool,
    show_upstream: bool,
    hint: bool,
) -> anyhow::Result<()> {
    // Check if we're in edit mode first, before doing any expensive operations
    let mode = gitbutler_operating_modes::operating_mode(ctx);
    if let gitbutler_operating_modes::OperatingMode::Edit(_metadata) = mode {
        // In edit mode, show the conflict resolution status
        return show_edit_mode_status(ctx, out);
    }

    // Check for available updates and display if present
    if let Some(out) = out.for_human() {
        let cache = ctx.app_cache.get_cache()?;
        if let Ok(Some(update)) = but_update::available_update(&cache) {
            writeln!(out, "{}", update.display_cli(verbose))?;
            writeln!(out)?;
        }
    }

    // Process rules with exclusive access to create repo and workspace
    let head_info = {
        let mut guard = ctx.exclusive_worktree_access();
        but_rules::process_rules(ctx, guard.write_permission()).ok(); // TODO: this is doing double work (hunk-dependencies can be reused)

        // TODO: use this for JSON status information (regular status information
        //       already uses this)
        let meta = ctx.meta()?;
        but_workspace::head_info(
            &*ctx.repo.get()?,
            &meta,
            but_workspace::ref_info::Options {
                expensive_commit_info: true,
                ..Default::default()
            },
        )?
    };

    let cache_config = if refresh_prs {
        but_forge::CacheConfig::NoCache
    } else {
        but_forge::CacheConfig::CacheOnly
    };
    let review_map = review::get_review_map(ctx, Some(cache_config.clone()))?;

    let worktree_changes = but_api::legacy::diff::changes_in_worktree(ctx)?;

    let id_map = IdMap::new(head_info.stacks, worktree_changes.assignments.clone())?;

    let stacks = id_map.stacks();
    // Store the count of stacks for hint logic later
    let has_branches = !stacks.is_empty();

    let assignments_by_file: BTreeMap<BString, FileAssignment> = FileAssignment::get_assignments_by_file(&id_map);
    let mut stack_details: Vec<StackEntry> = vec![];

    let unassigned = assignment::filter_by_stack_id(assignments_by_file.values(), &None);
    stack_details.push((None, (None, unassigned)));

    for stack in stacks {
        let assignments = assignment::filter_by_stack_id(assignments_by_file.values(), &stack.id);
        stack_details.push((stack.id, (Some(stack.clone()), assignments)));
    }
    let ci_map = ci_map(ctx, &cache_config, &stack_details)?;

    // Calculate common_merge_base data and upstream state in a scope
    // to ensure repo reference is dropped before any async operations
    let (common_merge_base_data, upstream_state, last_fetched_ms, base_branch) = {
        let stack = gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());
        let target = stack.get_default_target()?;
        let target_name = format!("{}/{}", target.branch.remote(), target.branch.branch());
        let repo = ctx.repo.get()?;
        let base_commit = repo.find_commit(target.sha.to_gix())?;
        let base_commit_decoded = base_commit.decode()?;
        let full_message = base_commit_decoded.message.to_string();
        let formatted_date = base_commit_decoded.committer()?.time()?.format_or_unix(DATE_ONLY);
        let author = base_commit_decoded.author()?;
        let common_merge_base_data = CommonMergeBase {
            target_name: target_name.clone(),
            common_merge_base: target.sha.to_string()[..7].to_string(),
            message: full_message,
            commit_date: formatted_date,
            commit_id: target.sha.to_gix(),
            created_at: base_commit_decoded.committer()?.time()?.seconds as i128 * 1000,
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
        };

        // Get cached upstream state information (without fetching)
        let (upstream_state, last_fetched_ms, base_branch) =
            but_api::legacy::virtual_branches::get_base_branch_data(ctx)
                .ok()
                .flatten()
                .map(|base_branch| {
                    let last_fetched = base_branch.last_fetched_ms;
                    let state = if base_branch.behind > 0 {
                        // Get the latest commit on the upstream branch (current_sha is the tip of the remote branch)
                        let commit_id = base_branch.current_sha;
                        repo.find_commit(commit_id.to_gix()).ok().and_then(|commit_obj| {
                            let commit = commit_obj.decode().ok()?;
                            let commit_message = {
                                let raw = commit.message.to_string().replace('\n', " ");
                                truncate_text(&raw, 30)
                            };

                            let formatted_date = commit.committer().ok()?.time().ok()?.format_or_unix(DATE_ONLY);

                            let author = commit.author().ok()?;

                            Some(UpstreamState {
                                target_name: base_branch.branch_name.clone(),
                                behind_count: base_branch.behind,
                                latest_commit: commit_id.to_string()[..7].to_string(),
                                message: commit_message,
                                commit_date: formatted_date,
                                last_fetched_ms: last_fetched,
                                commit_id: commit_id.to_gix(),
                                created_at: commit.committer().ok()?.time().ok()?.seconds as i128 * 1000,
                                author_name: author.name.to_string(),
                                author_email: author.email.to_string(),
                            })
                        })
                    } else {
                        None
                    };
                    (state, last_fetched, Some(base_branch))
                })
                .unwrap_or((None, None, None));

        // repo, base_commit, and base_commit_decoded are automatically dropped here at end of scope
        (common_merge_base_data, upstream_state, last_fetched_ms, base_branch)
    };

    // Compute upstream integration statuses if --upstream flag is set
    // We need to drop locks before computing merge statuses
    // because upstream_integration_statuses requires exclusive access
    let branch_merge_statuses: BTreeMap<String, UpstreamBranchStatus> = if show_upstream {
        compute_branch_merge_statuses(ctx).await?
    } else {
        BTreeMap::new()
    };

    // Re-acquire repo for use after the async call
    let repo = ctx.repo.get()?;

    if let Some(out) = out.for_json() {
        let workspace_status = json::build_workspace_status_json(
            &stack_details,
            &worktree_changes.worktree_changes.changes,
            &common_merge_base_data,
            &upstream_state,
            last_fetched_ms,
            &review_map,
            &ci_map,
            &branch_merge_statuses,
            show_files,
            &repo,
            &id_map,
            base_branch.as_ref(),
            show_upstream,
        )?;
        out.write_value(workspace_status)?;
        return Ok(());
    }

    let Some(out) = out.for_human() else {
        return Ok(());
    };

    // Drop repo to release the borrow on ctx before the loop
    drop(repo);

    for (i, (stack_id, (stack_with_id, assignments))) in stack_details.into_iter().enumerate() {
        let mut stack_mark = stack_id.and_then(|stack_id| {
            if crate::command::legacy::mark::stack_marked(ctx, stack_id).unwrap_or_default() {
                Some("◀ Marked ▶".red().bold())
            } else {
                None
            }
        });

        // assignments to the stack
        if let Some(stack_with_id) = &stack_with_id {
            let branch_name = stack_with_id
                .segments
                .first()
                .map_or(Some(BStr::new(b"")), SegmentWithId::branch_name);
            print_assignments(
                stack_id,
                &id_map,
                branch_name,
                &assignments,
                &worktree_changes.worktree_changes.changes,
                false,
                out,
            )?;
        }

        print_group(
            &stack_with_id,
            assignments,
            &worktree_changes.worktree_changes.changes,
            show_files,
            verbose,
            &mut stack_mark,
            ctx,
            i == 0,
            &review_map,
            &ci_map,
            &branch_merge_statuses,
            out,
            &id_map,
        )?;
    }

    // Format the last fetched time as relative time, unless NO_BG_TASKS is set
    let last_checked_text = if std::env::var("NO_BG_TASKS").is_ok() {
        String::new()
    } else {
        last_fetched_ms
            .map(|ms| {
                let relative_time = format_relative_time_verbose(std::time::SystemTime::now(), ms);
                format!("(checked {relative_time})")
            })
            .unwrap_or_default()
    };

    // Display upstream state if there are new commits
    if let Some(upstream) = &upstream_state {
        let dot = "●".yellow();

        if show_upstream {
            // When showing detailed commits, only show count in summary
            writeln!(
                out,
                "┊╭┄(upstream) ⏫ {} new commits{}",
                upstream.behind_count,
                last_checked_text.dimmed()
            )?;

            // Display detailed list of upstream commits
            if let Some(ref base_branch) = base_branch
                && !base_branch.upstream_commits.is_empty()
            {
                let commits = base_branch.upstream_commits.iter().take(8);
                for commit in commits {
                    // Measure prefix width using plain text (no ANSI codes)
                    let prefix_width = format!("┊● {} ", &commit.id[..7]).width();
                    let max_msg_width = terminal_width().saturating_sub(prefix_width);
                    writeln!(
                        out,
                        "┊{dot} {} {}",
                        commit.id[..7].yellow(),
                        {
                            let raw = commit.description.to_string().replace('\n', " ");
                            truncate_text(&raw, max_msg_width).dimmed()
                        }
                    )?;
                }
                let hidden_commits = base_branch.behind.saturating_sub(8);
                if hidden_commits > 0 {
                    writeln!(out, "┊    {}", format!("and {hidden_commits} more…").dimmed())?;
                }
            }
            writeln!(out, "┊┊")?;
        } else {
            // Without --upstream, show the summary with latest commit info
            writeln!(
                out,
                "┊{dot} {} (upstream) ⏫ {} new commits {}",
                upstream.latest_commit.dimmed(),
                upstream.behind_count,
                last_checked_text.dimmed()
            )?;
        }
    }

    let first_line = common_merge_base_data.message.lines().next().unwrap_or("");
    let connector = if upstream_state.is_some() { "├╯" } else { "┴" };
    // Build the prefix so we can measure its exact display width
    let prefix = format!(
        "{} {} [{}] {} ",
        connector,
        common_merge_base_data.common_merge_base,
        common_merge_base_data.target_name,
        common_merge_base_data.commit_date,
    );
    let max_width = terminal_width().saturating_sub(prefix.width());
    let display_message = truncate_text(first_line, max_width);

    writeln!(
        out,
        "{} {} [{}] {} {}",
        connector,
        common_merge_base_data.common_merge_base.dimmed(),
        common_merge_base_data.target_name.green().bold(),
        common_merge_base_data.commit_date.dimmed(),
        display_message,
    )?;

    let not_on_workspace = matches!(mode, gitbutler_operating_modes::OperatingMode::OutsideWorkspace(_));

    if not_on_workspace {
        writeln!(
            out,
            r#"
⚠️    You are in plain Git mode, directly on a branch. Some commands may be unavailable.    ⚠️
⚠️    More info: https://github.com/gitbutlerapp/gitbutler/issues/11866                     ⚠️
"#,
        )?;
    }

    if hint {
        writeln!(out)?;

        // Determine what hint to show based on workspace state
        let has_uncommitted_files = !worktree_changes.worktree_changes.changes.is_empty();

        // Check whether we're inside the workspace
        if not_on_workspace {
            writeln!(
                out,
                "{}",
                "Hint: run `but setup` to switch back to GitButler managed mode.".dimmed()
            )?;
        } else if !has_branches {
            writeln!(
                out,
                "{}",
                "Hint: run `but branch new` to create a new branch to work on".dimmed()
            )?;
        } else if has_uncommitted_files {
            writeln!(
                out,
                "{}",
                "Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch"
                    .dimmed()
            )?;
        } else {
            writeln!(out, "{}", "Hint: run `but help` for all commands".dimmed())?;
        }
    }

    Ok(())
}

fn ci_map(
    ctx: &mut Context,
    cache_config: &but_forge::CacheConfig,
    stack_details: &[StackEntry],
) -> Result<BTreeMap<String, Vec<but_forge::CiCheck>>, anyhow::Error> {
    let mut ci_map = BTreeMap::new();
    for (_, (stack_with_id, _)) in stack_details {
        if let Some(stack_with_id) = stack_with_id {
            for segment in &stack_with_id.segments {
                if segment.pr_number().is_some()
                    && !matches!(segment.inner.push_status, PushStatus::Integrated)
                    && let Some(branch_name) = segment.branch_name()
                    && let Ok(checks) = but_api::legacy::forge::list_ci_checks_and_update_cache(
                        ctx,
                        branch_name.to_string(),
                        Some(cache_config.clone()),
                    )
                {
                    ci_map.insert(branch_name.to_string(), checks);
                }
            }
        }
    }
    Ok(ci_map)
}

fn print_assignments(
    stack: Option<StackId>,
    id_map: &IdMap,
    branch_name: Option<&BStr>,
    assignments: &Vec<FileAssignment>,
    changes: &[ui::TreeChange],
    unstaged: bool,
    out: &mut dyn std::fmt::Write,
) -> std::fmt::Result {
    // if there are no assignments and we're in the unstaged section, print "(no changes)" and return
    if assignments.is_empty() && unstaged {
        writeln!(out, "┊     {}", "no changes".dimmed().italic())?;
        return Ok(());
    }

    let id = stack
        .and_then(|s| id_map.resolve_stack(s))
        .map(|s| s.to_short_string().bold().blue())
        .unwrap_or_default();

    if !unstaged && !assignments.is_empty() {
        writeln!(
            out,
            "┊  ╭┄{id} [{}]",
            branch_name
                .as_ref()
                .map(|name| format!("staged to {name}"))
                .unwrap_or_else(|| "staged to ".to_string())
                .cyan()
                .bold(),
        )?;
    }

    let max_id_width = assignments
        .iter()
        .map(|fa| fa.assignments[0].cli_id.len())
        .max()
        .unwrap_or(0);

    for fa in assignments {
        let state = status_from_changes(changes, fa.path.clone());
        let path = match &state {
            Some(state) => path_with_color_ui(state, fa.path.to_string()),
            None => fa.path.to_string().normal(),
        };

        let status = state.as_ref().map(status_letter_ui).unwrap_or_default();

        let cli_id = &fa.assignments[0].cli_id;
        let pad = max_id_width.saturating_sub(cli_id.len());
        let id = format!("{:pad$}{}", "", cli_id.bold().blue());

        let mut locks = fa
            .assignments
            .iter()
            .flat_map(|a| a.inner.hunk_locks.iter())
            .flatten()
            .map(|l| l.commit_id.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|commit_id| format!("{}{}", commit_id[..2].blue().bold(), commit_id[2..7].blue()))
            .collect::<Vec<_>>()
            .join(", ");

        if !locks.is_empty() {
            locks = format!("🔒 {locks}");
        }
        if unstaged {
            writeln!(out, "┊   {id} {status} {path} {locks}")?;
        } else {
            writeln!(out, "┊  {} {id} {status} {path} {locks}", "│".dimmed())?;
        }
    }

    if !unstaged && !assignments.is_empty() {
        writeln!(out, "┊  {}", "│".dimmed())?;
    }

    Ok(())
}

#[expect(clippy::too_many_arguments)]
pub fn print_group(
    stack_with_id: &Option<StackWithId>,
    assignments: Vec<FileAssignment>,
    changes: &[ui::TreeChange],
    show_files: bool,
    verbose: bool,
    stack_mark: &mut Option<ColoredString>,
    ctx: &mut Context,
    first: bool,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    ci_map: &BTreeMap<String, Vec<but_forge::CiCheck>>,
    branch_merge_statuses: &BTreeMap<String, UpstreamBranchStatus>,
    out: &mut dyn std::fmt::Write,
    id_map: &IdMap,
) -> anyhow::Result<()> {
    let repo = ctx.legacy_project.open_isolated_repo()?;
    if let Some(stack_with_id) = stack_with_id {
        let mut first = true;
        for segment in &stack_with_id.segments {
            let id = segment.short_id.bold().blue();
            let notch = if first { "╭" } else { "├" };
            if !first {
                writeln!(out, "┊│")?;
            }

            let no_commits = if segment.workspace_commits.is_empty() {
                "(no commits)".to_string()
            } else {
                "".to_string()
            }
            .dimmed()
            .italic();

            let review = segment
                .branch_name()
                .and_then(|branch_name| review::from_branch_details(review_map, branch_name, segment.pr_number()))
                .map(|r| format!(" {} ", r.display_cli(verbose)))
                .unwrap_or_default();

            let ci = segment
                .branch_name()
                .and_then(|branch_name| ci_map.get(&branch_name.to_string()))
                .map(CiChecks::from)
                .map(|c| c.display_cli(verbose))
                .unwrap_or_default();

            let merge_status = segment
                .branch_name()
                .and_then(|branch_name| branch_merge_statuses.get(&branch_name.to_string()))
                .map(|status| match status {
                    UpstreamBranchStatus::SaflyUpdatable => " [✓ upstream merges cleanly]".blue(),
                    UpstreamBranchStatus::Integrated => " [⬆ integrated upstream]".purple(),
                    UpstreamBranchStatus::Conflicted { .. } => " [⚠ upstream conflicts]".red(),
                    UpstreamBranchStatus::Empty => " ○ empty".dimmed(),
                })
                .map(|s| s.to_string())
                .unwrap_or_default();

            let workspace = segment
                .linked_worktree_id()
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
                "┊{notch}┄{id} [{branch}{workspace}]{ci}{merge_status}{review} {no_commits} {stack_mark}",
                stack_mark = stack_mark.clone().unwrap_or_default(),
                branch = segment
                    .branch_name()
                    .unwrap_or(BStr::new(""))
                    .to_string()
                    .green()
                    .bold(),
            )?;

            *stack_mark = None; // Only show the stack mark for the first branch
            first = false;

            if !segment.remote_commits.is_empty() {
                let tracking_branch = segment
                    .inner
                    .remote_tracking_ref_name
                    .as_ref()
                    .and_then(|rtb| rtb.as_bstr().strip_prefix(b"refs/remotes/"))
                    .unwrap_or(b"unknown");
                writeln!(out, "┊┊")?;
                writeln!(
                    out,
                    "┊╭┄┄{}",
                    format!("(upstream: on {})", BStr::new(tracking_branch)).yellow()
                )?;
            }
            for commit in &segment.remote_commits {
                let details = but_api::diff::commit_details(ctx, commit.commit_id(), ComputeLineStats::No)?;
                print_commit(
                    commit.short_id.clone(),
                    &commit.inner,
                    CommitChanges::Remote(&details.diff_with_first_parent),
                    CommitClassification::Upstream,
                    false,
                    show_files,
                    verbose,
                    None,
                    out,
                )?;
            }
            if !segment.remote_commits.is_empty() {
                writeln!(out, "┊-")?;
            }
            for commit in segment.workspace_commits.iter() {
                let marked = crate::command::legacy::mark::commit_marked(ctx, commit.commit_id().to_string())
                    .unwrap_or_default();
                let classification = match commit.relation() {
                    LocalCommitRelation::LocalOnly => CommitClassification::LocalOnly,
                    LocalCommitRelation::LocalAndRemote(object_id) => {
                        if object_id == commit.commit_id() {
                            CommitClassification::Pushed
                        } else {
                            CommitClassification::Modified
                        }
                    }
                    LocalCommitRelation::Integrated(_) => CommitClassification::Integrated,
                };

                print_commit(
                    commit.short_id.clone(),
                    &commit.inner.inner,
                    CommitChanges::Workspace(&commit.tree_changes_using_repo(&repo)?),
                    classification,
                    marked,
                    show_files,
                    verbose,
                    // TODO: populate the Gerrit review URL. It
                    // seems to be populated in handle_gerrit in
                    // crates/but-api/src/legacy/workspace.rs
                    None,
                    out,
                )?;
            }
        }
    } else {
        let id = id_map.unassigned().to_short_string().bold().blue();
        writeln!(
            out,
            "╭┄{} [{}] {}",
            id,
            "unstaged changes".to_string().cyan().bold(),
            stack_mark.clone().unwrap_or_default()
        )?;
        print_assignments(None, id_map, None, &assignments, changes, true, out)?;
    }
    if !first {
        writeln!(out, "├╯")?;
    }
    writeln!(out, "┊")?;
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

pub fn status_letter_ui(status: &ui::TreeStatus) -> char {
    match status {
        ui::TreeStatus::Addition { .. } => 'A',
        ui::TreeStatus::Deletion { .. } => 'D',
        ui::TreeStatus::Modification { .. } => 'M',
        ui::TreeStatus::Rename { .. } => 'R',
    }
}

pub fn path_with_color_ui(status: &ui::TreeStatus, path: String) -> ColoredString {
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

enum CommitChanges<'a> {
    Workspace(&'a [TreeChangeWithId]),
    Remote(&'a [but_core::TreeChange]),
}

#[expect(clippy::too_many_arguments)]
fn print_commit(
    short_id: String,
    commit: &but_workspace::ref_info::Commit,
    commit_changes: CommitChanges,
    classification: CommitClassification,
    marked: bool,
    show_files: bool,
    verbose: bool,
    review_url: Option<String>,
    out: &mut dyn std::fmt::Write,
) -> anyhow::Result<()> {
    let mark = if marked {
        Some("◀ Marked ▶".red().bold())
    } else {
        None
    };

    let dot = match classification {
        CommitClassification::Upstream => "●".yellow(),
        CommitClassification::LocalOnly => "●".normal(),
        CommitClassification::Pushed => "●".green(),
        CommitClassification::Modified => "◐".green(),
        CommitClassification::Integrated => "●".purple(),
    };

    let upstream_commit = matches!(commit_changes, CommitChanges::Remote(_));

    // Build the plain-text equivalent of the trailing suffix that appears
    // after details_string: "┊{dot}   {details} {review_url} {mark}".
    // We intentionally omit ANSI styling — escape codes are invisible and
    // occupy zero display columns, so plain text gives the correct width.
    let mut trailing_suffix = String::new();
    if let Some(r) = &review_url {
        trailing_suffix.push(' ');
        trailing_suffix.push('◖');
        trailing_suffix.push_str(r);
        trailing_suffix.push('◗');
    }
    if marked {
        trailing_suffix.push(' ');
        trailing_suffix.push_str("◀ Marked ▶");
    }
    let trailing_width = trailing_suffix.width();

    let details_string = display_cli_commit_details(
        short_id,
        commit,
        match commit_changes {
            CommitChanges::Workspace(tree_changes) => !tree_changes.is_empty(),
            CommitChanges::Remote(tree_changes) => !tree_changes.is_empty(),
        },
        verbose,
        trailing_width,
    );
    let details_string = if upstream_commit {
        details_string.dimmed().to_string()
    } else {
        details_string
    };

    if verbose {
        // Verbose format: author and timestamp on first line, message on second line
        writeln!(
            out,
            "┊{dot} {} {} {}",
            details_string,
            review_url
                .map(|r| format!("◖{}◗", r.underline().blue()))
                .unwrap_or_default(),
            mark.unwrap_or_default()
        )?;
        let prefix = "┊│     ";
        let max_width = terminal_width().saturating_sub(prefix.width());
        let message = CommitMessage(commit.message.clone()).text(verbose);
        let message = match message {
            Some(text) => {
                let truncated = truncate_text(&text, max_width);
                if upstream_commit {
                    truncated.dimmed().to_string()
                } else {
                    truncated
                }
            }
            None => "(no commit message)".dimmed().italic().to_string(),
        };
        writeln!(out, "┊│     {message}")?;
    } else {
        // Original format: everything on one line
        let review_url = review_url
            .map(|r| format!("◖{}◗", r.underline().blue()))
            .unwrap_or_default();
        writeln!(
            out,
            "┊{dot}   {} {} {}",
            details_string,
            review_url,
            mark.unwrap_or_default()
        )?;
    }
    if show_files {
        match commit_changes {
            CommitChanges::Workspace(tree_changes) => {
                for TreeChangeWithId { short_id, inner } in tree_changes {
                    let cid = short_id.blue().bold();
                    writeln!(out, "┊│     {cid} {}", inner.display_cli(false))?;
                }
            }
            CommitChanges::Remote(tree_changes) => {
                for change in tree_changes {
                    writeln!(out, "┊│     {}", change.display_cli(false))?;
                }
            }
        }
    }
    Ok(())
}

trait CliDisplay {
    fn display_cli(&self, verbose: bool) -> String;
}

impl CliDisplay for but_core::TreeChange {
    fn display_cli(&self, _verbose: bool) -> String {
        let path = path_with_color(&self.status, self.path.to_string());
        let status_letter = status_letter(&self.status);
        format!("{status_letter} {path}")
    }
}

fn display_cli_commit_details(
    short_id: String,
    commit: &but_workspace::ref_info::Commit,
    has_changes: bool,
    verbose: bool,
    extra_suffix_width: usize,
) -> String {
    let end_id = if short_id.len() >= 7 {
        "".to_string()
    } else {
        let commit_id = commit.id.to_string();
        commit_id[short_id.len()..7].dimmed().to_string()
    };
    let start_id = short_id.blue().bold();

    let conflicted_str = if commit.has_conflicts {
        " {conflicted}".red()
    } else {
        "".normal()
    };

    let no_changes = if has_changes {
        "".to_string().normal()
    } else {
        " (no changes)".dimmed().italic()
    };

    if verbose {
        // No message when verbose since it goes to the next line
        let created_at = commit.author.time;
        let formatted_time = created_at.format_or_unix(CLI_DATE);
        format!(
            "{}{} {} {}{}{}",
            start_id,
            end_id,
            commit.author.name,
            formatted_time.dimmed(),
            no_changes,
            conflicted_str,
        )
    } else {
        // Measure the line prefix and suffixes so we know exactly how much
        // space is left for the commit message.
        let prefix = format!("┊●   {short_id_placeholder} ", short_id_placeholder = "x".repeat(7));
        let suffix_width = if has_changes { 0 } else { " (no changes)".width() }
            + if commit.has_conflicts { " {conflicted}".width() } else { 0 }
            + extra_suffix_width;
        let max_width = terminal_width().saturating_sub(prefix.width() + suffix_width);
        let message = CommitMessage(commit.message.clone()).text(verbose);
        let message = match message {
            Some(text) => truncate_text(&text, max_width),
            None => "(no commit message)".dimmed().italic().to_string(),
        };
        format!("{start_id}{end_id} {message}{no_changes}{conflicted_str}",)
    }
}

struct CommitMessage(pub BString);

impl CommitMessage {
    /// Return the plain (uncolored, unstyled) message text.
    /// First line only for inline mode, all lines joined for verbose.
    /// Returns `None` when the commit has no message.
    fn text(&self, verbose: bool) -> Option<String> {
        let message = self.0.to_string();
        let text = if verbose {
            message.replace('\n', " ")
        } else {
            message.lines().next().unwrap_or("").to_string()
        };

        if text.is_empty() { None } else { Some(text) }
    }
}

impl CliDisplay for ForgeReview {
    fn display_cli(&self, verbose: bool) -> String {
        if verbose {
            format!(
                "#{}: {}",
                self.number.to_string().bold(),
                self.html_url.underline().blue(),
            )
        } else {
            // In non-verbose mode, show PR number and title without truncation.
            // This method can't know the actual prefix width since ForgeReview
            // appears on branch header lines with dynamic preceding content.
            format!("#{}: {}", self.number.to_string().bold(), self.title)
        }
    }
}

#[derive(Clone, Debug)]
struct CiChecks(pub Vec<but_forge::CiCheck>);

impl From<&Vec<but_forge::CiCheck>> for CiChecks {
    fn from(checks: &Vec<but_forge::CiCheck>) -> Self {
        CiChecks(checks.clone())
    }
}

impl CliDisplay for CiChecks {
    fn display_cli(&self, _verbose: bool) -> String {
        let success = self
            .0
            .iter()
            .filter(|check| {
                matches!(
                    check.status,
                    but_forge::CiStatus::Complete {
                        conclusion: but_forge::CiConclusion::Success,
                        ..
                    }
                )
            })
            .count();
        let failed = self
            .0
            .iter()
            .filter(|check| {
                matches!(
                    check.status,
                    but_forge::CiStatus::Complete {
                        conclusion: but_forge::CiConclusion::Failure,
                        ..
                    }
                )
            })
            .count();
        let in_progress = self
            .0
            .iter()
            .filter(|check| {
                matches!(
                    check.status,
                    but_forge::CiStatus::InProgress | but_forge::CiStatus::Queued
                )
            })
            .count();

        if failed > 0 {
            " CI: ❌".to_string()
        } else if in_progress > 0 {
            " CI: ⏳".to_string()
        } else if success > 0 {
            " CI: ✅".to_string()
        } else {
            "".to_string()
        }
    }
}

impl CliDisplay for but_update::AvailableUpdate {
    fn display_cli(&self, verbose: bool) -> String {
        let version_info = format!(
            "{} → {}",
            self.current_version.dimmed(),
            self.available_version.green().bold()
        );

        if verbose {
            if let Some(url) = &self.url {
                format!("Update available: {} {}", version_info, url.underline().blue())
            } else {
                format!("Update available: {version_info}")
            }
        } else {
            format!(
                "Update available: {} {}",
                version_info,
                "(you can run `but update install` or `but update suppress` to dismiss)".dimmed()
            )
        }
    }
}

async fn compute_branch_merge_statuses(ctx: &Context) -> anyhow::Result<BTreeMap<String, UpstreamBranchStatus>> {
    use gitbutler_branch_actions::upstream_integration::StackStatuses;

    // Get upstream integration statuses using the public API
    let statuses = but_api::legacy::virtual_branches::upstream_integration_statuses(ctx.to_sync(), None).await?;

    let mut result = BTreeMap::new();

    if let StackStatuses::UpdatesRequired { statuses, .. } = statuses {
        for (_stack_id, stack_status) in statuses {
            for branch_status in stack_status.branch_statuses {
                result.insert(branch_status.name.clone(), branch_status.status);
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::truncate_text;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn short_text_is_not_truncated() {
        assert_eq!(truncate_text("hello", 10), "hello");
    }

    #[test]
    fn text_at_exact_limit_is_not_truncated() {
        assert_eq!(truncate_text("hello", 5), "hello");
    }

    #[test]
    fn text_exceeding_limit_is_truncated_with_ellipsis() {
        assert_eq!(truncate_text("hello world", 5), "hell…");
    }

    #[test]
    fn empty_text_stays_empty() {
        assert_eq!(truncate_text("", 10), "");
    }

    #[test]
    fn max_width_of_zero_gives_empty_string() {
        assert_eq!(truncate_text("hello", 0), "");
    }

    #[test]
    fn max_width_of_one_gives_ellipsis_only() {
        assert_eq!(truncate_text("hello", 1), "…");
    }

    #[test]
    fn unicode_single_width_characters_are_handled() {
        // ü is a single-width character (1 display column)
        assert_eq!(truncate_text("über-cool", 5), "über…");
    }

    #[test]
    fn cjk_double_width_characters_are_handled() {
        // Each CJK character occupies 2 display columns.
        // 你(2) + 好(2) = 4 cols, + …(1) = 5 cols total.
        assert_eq!(truncate_text("你好世界", 5), "你好…");
        assert_eq!(truncate_text("你好世界", 5).width(), 5);
    }

    #[test]
    fn cjk_truncation_does_not_exceed_max_width() {
        // With max_width 4, a second CJK char (2 cols) leaves no room
        // for the ellipsis alongside it, so only the first char + … fits.
        // 你(2) + …(1) = 3 cols ≤ 4
        let result = truncate_text("你好世界", 4);
        assert!(result.width() <= 4);
        assert_eq!(result, "你…");
    }

    #[test]
    fn truncation_preserves_exact_boundary() {
        let msg = "this is a overly long commit message to demonstrate truncation";
        let result = truncate_text(msg, 60);
        assert!(result.ends_with('…'));
        // For ASCII text, display width == char count
        assert_eq!(result.width(), 60);
    }

    #[test]
    fn emoji_characters_are_handled() {
        // Many emoji are wide characters (2 display columns each)
        let single = "🙂";
        let single_width = single.width();
        assert!(single_width >= 1);
        assert_eq!(truncate_text(single, single_width), single);
        // Repeated emoji should truncate without exceeding max_width
        let repeated = "🙂🙂🙂";
        let result = truncate_text(repeated, single_width * 2);
        assert!(result.width() <= single_width * 2);
    }

    #[test]
    fn zero_width_combining_characters_are_handled() {
        // "a" + COMBINING ACUTE ACCENT; display width should be 1
        let text = "a\u{0301}";
        assert_eq!(text.width(), 1);
        assert_eq!(truncate_text(text, 1), text);
    }
}
