use std::collections::BTreeMap;

use assignment::FileAssignment;
use bstr::{BStr, BString, ByteSlice};
use but_api::diff::ComputeLineStats;
use but_core::{TreeStatus, diff::CommitDetails, ui};
use but_ctx::Context;
use but_forge::ForgeReview;
use but_oxidize::OidExt;
use but_workspace::ref_info::LocalCommitRelation;
use but_workspace::ui::PushStatus;
use colored::{ColoredString, Colorize};
use gitbutler_branch_actions::upstream_integration::BranchStatus as UpstreamBranchStatus;
use gitbutler_stack::StackId;
use gix::date::time::CustomFormat;
use serde::Serialize;

use crate::id::{SegmentWithId, StackWithId};
use crate::{CLI_DATE, utils::time::format_relative_time_verbose};

const DATE_ONLY: CustomFormat = CustomFormat::new("%Y-%m-%d");

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

use crate::command::legacy::forge::review;
use crate::{IdMap, utils::OutputChannel};

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
    if let Some(out) = out.for_human()
        && let Ok(Some(update)) = but_update::available_update()
    {
        writeln!(out, "{}", update.display_cli(verbose))?;
        writeln!(out)?;
    }

    but_rules::process_rules(ctx).ok(); // TODO: this is doing double work (hunk-dependencies can be reused)

    let guard = ctx.shared_worktree_access();
    let meta = ctx.meta(guard.read_permission())?;

    // TODO: use this for JSON status information (regular status information
    // already uses this)
    let head_info = but_workspace::head_info(
        &*ctx.repo.get()?,
        &meta,
        but_workspace::ref_info::Options {
            expensive_commit_info: true,
            ..Default::default()
        },
    )?;

    let cache_config = if refresh_prs {
        but_forge::CacheConfig::NoCache
    } else {
        but_forge::CacheConfig::CacheOnly
    };
    let review_map = crate::command::legacy::forge::review::get_review_map(
        &ctx.legacy_project,
        Some(cache_config.clone()),
    )?;

    let worktree_changes = but_api::legacy::diff::changes_in_worktree(ctx)?;

    let mut id_map = IdMap::new(head_info.stacks, worktree_changes.assignments.clone())?;
    id_map.add_committed_file_info_from_context(ctx)?;

    let stacks = &id_map.stacks;
    // Store the count of stacks for hint logic later
    let has_branches = !stacks.is_empty();

    let assignments_by_file: BTreeMap<BString, FileAssignment> =
        FileAssignment::get_assignments_by_file(&id_map);
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
        let (upstream_state, last_fetched_ms, base_branch) =
            but_api::legacy::virtual_branches::get_base_branch_data(ctx.legacy_project.id)
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
                                    .take(30)
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
                                    created_at: commit.committer().ok()?.time().ok()?.seconds
                                        as i128
                                        * 1000,
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
        (
            common_merge_base_data,
            upstream_state,
            last_fetched_ms,
            base_branch,
        )
    };

    // Compute upstream integration statuses if --upstream flag is set
    // We need to drop locks before computing merge statuses
    // because upstream_integration_statuses requires exclusive access
    let branch_merge_statuses: BTreeMap<String, UpstreamBranchStatus> = if show_upstream {
        drop(guard);
        drop(meta);
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
            ctx.legacy_project.id,
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
    // Format the last fetched time as relative time
    let last_checked_text = last_fetched_ms
        .map(|ms| {
            let relative_time = format_relative_time_verbose(ms);
            format!("(checked {})", relative_time)
        })
        .unwrap_or_default();

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
                    writeln!(
                        out,
                        "┊{dot} {} {}",
                        commit.id[..7].yellow(),
                        commit
                            .description
                            .to_string()
                            .replace('\n', " ")
                            .chars()
                            .take(72)
                            .collect::<String>()
                            .dimmed()
                    )?;
                }
                let hidden_commits = base_branch.behind.saturating_sub(8);
                if hidden_commits > 0 {
                    writeln!(
                        out,
                        "┊    {}",
                        format!("and {hidden_commits} more…").dimmed()
                    )?;
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

    let not_on_workspace = matches!(
        mode,
        gitbutler_operating_modes::OperatingMode::OutsideWorkspace(_)
    );

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
            let message = format!(
                "Hint: run `but {}` to switch back to GitButler managed mode.",
                crate::args::Subcommands::SwitchBack.as_ref()
            );
            writeln!(out, "{}", message.dimmed())?;
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
        .map(|s| s.to_short_string().underline().blue())
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
            let id = segment.short_id.underline().blue();
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
                .and_then(|branch_name| {
                    review::from_branch_details(review_map, branch_name, segment.pr_number())
                })
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
                let details =
                    but_api::diff::commit_details(ctx, commit.commit_id(), ComputeLineStats::No)?;
                print_commit(
                    commit.short_id.clone(),
                    details,
                    CommitClassification::Upstream,
                    false,
                    show_files,
                    verbose,
                    None,
                    id_map,
                    out,
                    true,
                )?;
            }
            if !segment.remote_commits.is_empty() {
                writeln!(out, "┊-")?;
            }
            for commit in segment.workspace_commits.iter() {
                let marked = crate::command::legacy::mark::commit_marked(
                    ctx,
                    commit.commit_id().to_string(),
                )
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

                let details =
                    but_api::diff::commit_details(ctx, commit.commit_id(), ComputeLineStats::No)?;
                print_commit(
                    commit.short_id.clone(),
                    details,
                    classification,
                    marked,
                    show_files,
                    verbose,
                    // TODO: populate the Gerrit review URL. It
                    // seems to be populated in handle_gerrit in
                    // crates/but-api/src/legacy/workspace.rs
                    None,
                    id_map,
                    out,
                    false,
                )?;
            }
        }
    } else {
        let id = id_map.unassigned().to_short_string().underline().blue();
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

#[expect(clippy::too_many_arguments)]
fn print_commit(
    short_id: String,
    commit_details: CommitDetails,
    classification: CommitClassification,
    marked: bool,
    show_files: bool,
    verbose: bool,
    review_url: Option<String>,
    id_map: &IdMap,
    out: &mut dyn std::fmt::Write,
    upstream_commit: bool,
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

    let details_string = display_cli_commit_details(short_id, &commit_details, verbose);
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
        let message = CommitMessage(commit_details.commit.inner.message).display_cli(verbose);
        let message = if upstream_commit {
            message.dimmed().to_string()
        } else {
            message
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
        for change in &commit_details.diff_with_first_parent {
            let cid = id_map
                .resolve_file_changed_in_commit_or_unassigned(
                    commit_details.commit.id,
                    change.path.as_ref(),
                )
                .to_short_string()
                .blue()
                .underline();
            writeln!(out, "┊│     {cid} {}", change.display_cli(false))?;
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
    commit_details: &CommitDetails,
    verbose: bool,
) -> String {
    let end_id = if short_id.len() >= 7 {
        "".to_string()
    } else {
        let commit_id = commit_details.commit.id.to_string();
        commit_id[short_id.len()..7].dimmed().to_string()
    };
    let start_id = short_id.blue().underline();

    let conflicted_str = if commit_details.conflict_entries.is_some() {
        " {conflicted}".red()
    } else {
        "".normal()
    };

    let no_changes = if commit_details.diff_with_first_parent.is_empty() {
        " (no changes)".dimmed().italic()
    } else {
        "".to_string().normal()
    };

    if verbose {
        // No message when verbose since it goes to the next line
        let created_at = commit_details.commit.inner.committer.time;
        let formatted_time = created_at.format_or_unix(CLI_DATE);
        format!(
            "{}{} {} {}{}{}",
            start_id,
            end_id,
            commit_details.commit.inner.author.name,
            formatted_time.dimmed(),
            no_changes,
            conflicted_str,
        )
    } else {
        let message =
            CommitMessage(commit_details.commit.inner.message.clone()).display_cli(verbose);
        format!(
            "{}{} {}{}{}",
            start_id, end_id, message, no_changes, conflicted_str,
        )
    }
}

struct CommitMessage(pub BString);

impl CliDisplay for CommitMessage {
    fn display_cli(&self, verbose: bool) -> String {
        let message = self.0.to_string();
        let text = if verbose {
            message.replace('\n', " ")
        } else {
            message.lines().next().unwrap_or("").to_string()
        };

        let truncated: String = text.chars().take(50).collect();

        if truncated.is_empty() {
            "(no commit message)".dimmed().italic().to_string()
        } else {
            truncated.normal().to_string()
        }
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
            format!(
                "#{}: {}",
                self.number.to_string().bold(),
                self.title
                    .chars()
                    .take(50)
                    .collect::<String>()
                    .trim_end_matches(|c: char| !c.is_ascii() && !c.is_alphanumeric())
            )
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
                format!(
                    "Update available: {} ({})",
                    version_info,
                    url.underline().blue()
                )
            } else {
                format!("Update available: {}", version_info)
            }
        } else {
            format!(
                "Update available: {} {}",
                version_info,
                "(run `but status -v` for link or `but update suppress` to dismiss)".dimmed()
            )
        }
    }
}

async fn compute_branch_merge_statuses(
    ctx: &Context,
) -> anyhow::Result<BTreeMap<String, UpstreamBranchStatus>> {
    use gitbutler_branch_actions::upstream_integration::StackStatuses;

    // Get upstream integration statuses using the public API
    let statuses = but_api::legacy::virtual_branches::upstream_integration_statuses(
        ctx.legacy_project.id,
        None,
    )
    .await?;

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
