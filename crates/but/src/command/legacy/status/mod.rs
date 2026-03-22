use std::collections::BTreeMap;

use assignment::FileAssignment;
use bstr::{BStr, BString, ByteSlice};
use but_api::diff::ComputeLineStats;
use but_core::{RepositoryExt, TreeStatus, ui};
use but_ctx::Context;
use but_forge::ForgeReview;
use but_workspace::{ref_info::LocalCommitRelation, ui::PushStatus};
use gitbutler_branch_actions::upstream_integration::BranchStatus as UpstreamBranchStatus;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_stack::StackId;
use gix::date::time::CustomFormat;
use ratatui::{
    style::{Modifier, Style},
    text::Span,
};
use serde::Serialize;

use crate::{
    CLI_DATE, CliId, IdMap,
    command::legacy::{
        forge::review,
        status::output::{BranchLineContent, CommitLineContent, StatusOutput, StatusOutputLine},
    },
    id::{SegmentWithId, ShortId, StackWithId, TreeChangeWithId},
    tui::text::truncate_text,
    utils::{
        OutputChannel, WriteWithUtils, shorten_hex_object_id, shorten_object_id, split_short_id,
        time::format_relative_time_verbose,
    },
};

pub(crate) mod assignment;
pub(crate) mod json;

mod output;
mod render_oneshot;
mod tui;

const DATE_ONLY: CustomFormat = CustomFormat::new("%Y-%m-%d");

#[derive(Debug, Copy, Clone)]
pub struct StatusFlags {
    pub show_files: FilesStatusFlag,
    pub verbose: bool,
    pub refresh_prs: bool,
    pub show_upstream: bool,
    pub hint: bool,
}

impl StatusFlags {
    pub fn all_false() -> Self {
        Self {
            show_files: FilesStatusFlag::None,
            verbose: false,
            refresh_prs: false,
            show_upstream: false,
            hint: false,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum FilesStatusFlag {
    /// Don't show files for any commits.
    None,
    /// Show files changed for all commits.
    All,
    /// Only show files for this specific commit.
    Commit(gix::ObjectId),
}

impl FilesStatusFlag {
    pub fn show_files_for(self, commit: gix::ObjectId) -> bool {
        match self {
            FilesStatusFlag::None => false,
            FilesStatusFlag::All => true,
            FilesStatusFlag::Commit(object_id) => commit == object_id,
        }
    }

    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum StatusRenderMode {
    Oneshot,
    Tui { debug: bool },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CommitClassification {
    Upstream,
    LocalOnly,
    Pushed,
    Modified,
    Integrated,
}

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

struct StatusContext<'a> {
    flags: StatusFlags,
    stack_details: Vec<StackEntry>,
    worktree_changes: Vec<ui::TreeChange>,
    common_merge_base_data: CommonMergeBase,
    upstream_state: Option<UpstreamState>,
    last_fetched_ms: Option<u128>,
    review_map: std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    ci_map: BTreeMap<String, Vec<but_forge::CiCheck>>,
    branch_merge_statuses: BTreeMap<String, UpstreamBranchStatus>,
    has_branches: bool,
    is_paged: bool,
    should_truncate_for_terminal: bool,
    id_map: IdMap,
    base_branch: Option<gitbutler_branch_actions::BaseBranch>,
    mode: &'a gitbutler_operating_modes::OperatingMode,
}

fn show_edit_mode_status(ctx: &mut Context, out: &mut OutputChannel<'_>) -> anyhow::Result<()> {
    // Delegate to the resolve status logic to show actual conflict details
    crate::command::legacy::resolve::show_resolve_status(ctx, out)
}

pub(crate) async fn worktree(
    ctx: &mut Context,
    out: &mut OutputChannel<'_>,
    flags: StatusFlags,
    render_mode: StatusRenderMode,
) -> anyhow::Result<()> {
    // Check if we're in edit mode first, before doing any expensive operations
    let mode = but_api::legacy::modes::operating_mode(ctx)?.operating_mode;
    if let gitbutler_operating_modes::OperatingMode::Edit(_metadata) = mode {
        // In edit mode, show the conflict resolution status
        return show_edit_mode_status(ctx, out);
    }

    let status_ctx = build_status_context(ctx, out, &mode, flags, render_mode).await?;

    {
        // Re-acquire repo for use after the async call
        let repo = ctx.repo.get()?;

        if let Some(out) = out.for_json() {
            let workspace_status = json::build_workspace_status_json(&status_ctx, &repo)?;
            out.write_value(workspace_status)?;
            return Ok(());
        }
    }

    match render_mode {
        StatusRenderMode::Oneshot => {
            let Some(human_out) = out.for_human() else {
                return Ok(());
            };

            let mut output = StatusOutput::Immediate { out: human_out };
            build_status_output(ctx, &status_ctx, &mut output)?;
        }
        StatusRenderMode::Tui { debug } => {
            if out.for_human().is_none() {
                return Ok(());
            }

            let mut lines = Vec::new();
            let mut output = StatusOutput::Buffer { lines: &mut lines };
            build_status_output(ctx, &status_ctx, &mut output)?;
            let final_lines = tui::render_tui(ctx, out, &mode, flags, lines, debug).await?;

            if let Some(human_out) = out.for_human() {
                for line in final_lines {
                    render_oneshot::render_oneshot(line, human_out)?;
                }
            }
        }
    }

    Ok(())
}

async fn build_status_context<'a>(
    ctx: &mut Context,
    out: &mut OutputChannel<'_>,
    mode: &'a OperatingMode,
    flags: StatusFlags,
    render_mode: StatusRenderMode,
) -> anyhow::Result<StatusContext<'a>> {
    // Process rules with exclusive access to create repo and workspace
    let head_info = {
        let mut guard = ctx.exclusive_worktree_access();
        but_rules::process_rules(ctx, guard.write_permission()).ok(); // TODO: this is doing double work (hunk-dependencies can be reused)

        // TODO: use this for JSON status information (regular status information
        //       already uses this)
        let meta = ctx.meta()?;
        let mut cache = ctx.cache.get_cache_mut()?;
        but_workspace::head_info(
            &*ctx.repo.get()?,
            &meta,
            but_workspace::ref_info::Options {
                expensive_commit_info: true,
                ..Default::default()
            },
            &mut cache,
        )?
    };

    let cache_config = if flags.refresh_prs {
        but_forge::CacheConfig::NoCache
    } else {
        but_forge::CacheConfig::CacheOnly
    };
    let review_map = review::get_review_map(ctx, Some(cache_config.clone()))?;

    let worktree_changes = but_api::diff::changes_in_worktree(ctx)?;

    let id_map = IdMap::new(head_info.stacks, worktree_changes.assignments.clone())?;

    let stacks = id_map.stacks();
    // Store the count of stacks for hint logic later
    let has_branches = !stacks.is_empty();

    let assignments_by_file: BTreeMap<BString, FileAssignment> =
        FileAssignment::get_assignments_by_file(&id_map);
    let mut stack_details: Vec<StackEntry> = Vec::new();

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
        let base_commit = repo.find_commit(target.sha)?;
        let base_commit_decoded = base_commit.decode()?;
        let full_message = base_commit_decoded.message.to_string();
        let formatted_date = base_commit_decoded
            .committer()?
            .time()?
            .format_or_unix(DATE_ONLY);
        let author = base_commit_decoded.author()?;
        let common_merge_base_data = CommonMergeBase {
            target_name: target_name.clone(),
            common_merge_base: shorten_object_id(&repo, target.sha),
            message: full_message,
            commit_date: formatted_date,
            commit_id: target.sha,
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
                        repo.find_commit(commit_id).ok().and_then(|commit_obj| {
                            let commit = commit_obj.decode().ok()?;
                            let message = out.truncate_if_unpaged(
                                &commit.message.to_string().replace('\n', " "),
                                30,
                            );

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
                                latest_commit: shorten_object_id(&repo, commit_id),
                                message,
                                commit_date: formatted_date,
                                last_fetched_ms: last_fetched,
                                commit_id,
                                created_at: commit.committer().ok()?.time().ok()?.seconds as i128
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
    let branch_merge_statuses: BTreeMap<String, UpstreamBranchStatus> = if flags.show_upstream {
        compute_branch_merge_statuses(ctx).await?
    } else {
        BTreeMap::new()
    };

    let is_paged = out.is_paged();
    let should_truncate_for_terminal = truncation_policy(render_mode, is_paged);

    Ok(StatusContext {
        stack_details,
        worktree_changes: worktree_changes.worktree_changes.changes,
        common_merge_base_data,
        upstream_state,
        last_fetched_ms,
        review_map,
        ci_map,
        branch_merge_statuses,
        flags,
        has_branches,
        is_paged,
        should_truncate_for_terminal,
        id_map,
        base_branch,
        mode,
    })
}

/// Decide if status text should be pre-truncated for terminal output.
fn truncation_policy(render_mode: StatusRenderMode, is_paged: bool) -> bool {
    matches!(render_mode, StatusRenderMode::Oneshot) && !is_paged
}

fn build_status_output(
    ctx: &mut Context,
    status_ctx: &StatusContext<'_>,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    print_update_notice(ctx, status_ctx, output)?;
    print_worktree_status(ctx, status_ctx, output)?;
    print_upstream_state(ctx, status_ctx, output)?;
    print_common_merge_base_summary(status_ctx, output)?;
    let not_on_workspace = matches!(
        status_ctx.mode,
        gitbutler_operating_modes::OperatingMode::OutsideWorkspace(_)
    );
    print_outside_workspace_warning(not_on_workspace, output)?;
    print_hint(status_ctx, not_on_workspace, output)?;
    Ok(())
}

/// Print update information for human output when a newer `but` version is available.
fn print_update_notice(
    ctx: &mut Context,
    status_ctx: &StatusContext<'_>,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    let cache = ctx.app_cache.get_cache()?;
    if let Ok(Some(update)) = but_update::available_update(&cache) {
        output.update_notice(
            update
                .display_cli(
                    status_ctx.flags.verbose,
                    status_ctx.should_truncate_for_terminal,
                )
                .into_iter()
                .collect(),
        )?;
        output.connector(Vec::from([Span::raw("")]))?;
    }

    Ok(())
}

/// Print a warning when operating outside the GitButler workspace.
fn print_outside_workspace_warning(
    not_on_workspace: bool,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    if not_on_workspace {
        output.warning(Vec::from([Span::raw(
            "⚠️    You are in plain Git mode, directly on a branch. Some commands may be unavailable.    ⚠️",
        )]))?;
        output.warning(Vec::from([Span::raw(
            "⚠️    More info: https://github.com/gitbutlerapp/gitbutler/issues/11866                     ⚠️",
        )]))?;
    }

    Ok(())
}

/// Print a contextual hint at the end of status output when hints are enabled.
fn print_hint(
    status_ctx: &StatusContext<'_>,
    not_on_workspace: bool,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    if !status_ctx.flags.hint {
        return Ok(());
    }

    output.connector(Vec::from([Span::raw("")]))?;

    // Determine what hint to show based on workspace state
    let has_uncommitted_files = !status_ctx.worktree_changes.is_empty();

    let hint_text = if not_on_workspace {
        "Hint: run `but setup` to switch back to GitButler managed mode."
    } else if !status_ctx.has_branches {
        "Hint: run `but branch new` to create a new branch to work on"
    } else if has_uncommitted_files {
        "Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch"
    } else {
        "Hint: run `but help` for all commands"
    };

    output.hint(Vec::from([Span::styled(hint_text, Style::default().dim())]))?;

    Ok(())
}

/// Display upstream state information when upstream has commits ahead of the workspace base.
fn print_upstream_state(
    ctx: &mut Context,
    status_ctx: &StatusContext<'_>,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    let Some(upstream) = &status_ctx.upstream_state else {
        return Ok(());
    };

    // Format the last fetched time as relative time, unless NO_BG_TASKS is set.
    let last_checked_text = if std::env::var("NO_BG_TASKS").is_ok() {
        String::new()
    } else {
        status_ctx
            .last_fetched_ms
            .map(|ms| {
                let relative_time = format_relative_time_verbose(std::time::SystemTime::now(), ms);
                format!("(checked {relative_time})")
            })
            .unwrap_or_default()
    };

    let dot = Span::styled("●", Style::default().yellow());

    if status_ctx.flags.show_upstream {
        // When showing detailed commits, only show count in summary
        let mut upstream_summary = Vec::from([Span::raw(format!(
            "(upstream) ⏫ {} new commits",
            upstream.behind_count
        ))]);
        if !last_checked_text.is_empty() {
            upstream_summary.push(Span::raw(" "));
            upstream_summary.push(Span::styled(
                last_checked_text.clone(),
                Style::default().dim(),
            ));
        }
        output.upstream_changes(Vec::from([Span::raw("┊╭┄")]), upstream_summary)?;

        // Display detailed list of upstream commits
        if let Some(base_branch) = &status_ctx.base_branch
            && !base_branch.upstream_commits.is_empty()
        {
            let repo = ctx.repo.get()?;
            let commits = base_branch.upstream_commits.iter().take(8);
            for commit in commits {
                let commit_short = shorten_hex_object_id(&repo, &commit.id);
                let message = commit.description.to_string().replace('\n', " ");
                let message = message.trim_end().to_string();
                let truncated_msg =
                    truncate_when_needed(&message, 72, status_ctx.should_truncate_for_terminal);
                output.upstream_changes(
                    Vec::from([Span::raw("┊")]),
                    Vec::from([
                        dot.clone(),
                        Span::raw(" "),
                        Span::styled(commit_short, Style::default().yellow()),
                        Span::raw(" "),
                        Span::styled(truncated_msg, Style::default().dim()),
                    ]),
                )?;
            }
            let hidden_commits = base_branch.behind.saturating_sub(8);
            if hidden_commits > 0 {
                output.upstream_changes(
                    Vec::from([Span::raw("┊    ")]),
                    Vec::from([Span::styled(
                        format!("and {hidden_commits} more…"),
                        Style::default().dim(),
                    )]),
                )?;
            }
        }
        output.connector(Vec::from([Span::raw("┊┊")]))?;
    } else {
        // Without --upstream, show the summary with latest commit info
        let mut upstream_summary = Vec::from([
            Span::styled(upstream.latest_commit.clone(), Style::default().dim()),
            Span::raw(format!(
                " (upstream) ⏫ {} new commits",
                upstream.behind_count
            )),
        ]);
        if !last_checked_text.is_empty() {
            upstream_summary.push(Span::raw(" "));
            upstream_summary.push(Span::styled(last_checked_text, Style::default().dim()));
        }
        output.upstream_changes(
            Vec::from([Span::raw("┊"), dot, Span::raw(" ")]),
            upstream_summary,
        )?;
    }

    Ok(())
}

/// Print the common merge-base summary line at the bottom of the status tree.
fn print_common_merge_base_summary(
    status_ctx: &StatusContext<'_>,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    let first_line = status_ctx
        .common_merge_base_data
        .message
        .lines()
        .next()
        .unwrap_or("");
    let connector = if status_ctx.upstream_state.is_some() {
        "├╯"
    } else {
        "┴"
    };
    let first_line = truncate_when_needed(first_line, 40, status_ctx.should_truncate_for_terminal);
    output.merge_base(
        Vec::from([Span::raw(connector), Span::raw(" ")]),
        Vec::from([
            Span::styled(
                status_ctx.common_merge_base_data.common_merge_base.clone(),
                Style::default().dim(),
            ),
            Span::raw(" ["),
            Span::styled(
                status_ctx.common_merge_base_data.target_name.clone(),
                Style::default().green().bold(),
            ),
            Span::raw("] "),
            Span::styled(
                status_ctx.common_merge_base_data.commit_date.clone(),
                Style::default().dim(),
            ),
            Span::raw(" "),
            Span::raw(first_line.to_string()),
        ]),
    )?;
    Ok(())
}

/// Print per-stack status sections for human-readable output.
fn print_worktree_status(
    ctx: &mut Context,
    status_ctx: &StatusContext<'_>,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    for (i, (stack_id, (stack_with_id, assignments))) in status_ctx.stack_details.iter().enumerate()
    {
        let mut stack_mark = stack_id.and_then(|stack_id| {
            if crate::command::legacy::mark::stack_marked(ctx, stack_id).unwrap_or_default() {
                Some(Span::styled("◀ Marked ▶", Style::default().red().bold()))
            } else {
                None
            }
        });

        // assignments to the stack
        if let Some(stack_with_id) = stack_with_id {
            let branch_name = stack_with_id
                .segments
                .first()
                .map_or(Some(BStr::new(b"")), SegmentWithId::branch_name);
            let repo = ctx.repo.get()?;
            print_assignments(
                &repo,
                status_ctx,
                *stack_id,
                branch_name,
                assignments,
                false,
                output,
            )?;
        }

        print_group(
            ctx,
            status_ctx,
            stack_with_id,
            assignments,
            &mut stack_mark,
            i == 0,
            output,
        )?;
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
    repo: &gix::Repository,
    status_ctx: &StatusContext<'_>,
    stack: Option<StackId>,
    branch_name: Option<&BStr>,
    assignments: &[FileAssignment],
    unstaged: bool,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    // if there are no assignments and we're in the unstaged section, print "(no changes)" and return
    if assignments.is_empty() && unstaged {
        output.no_assignments_unstaged(
            Vec::from([Span::raw("┊     ")]),
            Vec::from([Span::styled("no changes", Style::default().dim().italic())]),
        )?;
        return Ok(());
    }

    let id = stack
        .and_then(|s| status_ctx.id_map.resolve_stack(s))
        .map(|s| Span::styled(s.to_short_string(), Style::default().bold().blue()))
        .unwrap_or_default();

    if !unstaged && !assignments.is_empty() {
        let staged_changes_cli_id = stack
            .and_then(|stack_id| status_ctx.id_map.resolve_stack(stack_id).cloned())
            .ok_or_else(|| anyhow::anyhow!("Could not resolve stack CLI id for staged changes"))?;

        output.staged_changes(
            Vec::from([Span::raw("┊  ╭┄")]),
            Vec::from([
                id,
                Span::raw(" ["),
                Span::styled(
                    branch_name
                        .as_ref()
                        .map(|name| format!("staged to {name}"))
                        .unwrap_or_else(|| "staged to ".to_string()),
                    Style::default().cyan().bold(),
                ),
                Span::raw("]"),
            ]),
            staged_changes_cli_id,
        )?;
    }

    let max_id_width = assignments
        .iter()
        .map(|fa| fa.assignments[0].cli_id.len())
        .max()
        .unwrap_or(0);

    for fa in assignments {
        let state = status_from_changes(&status_ctx.worktree_changes, fa.path.clone());
        let path = match &state {
            Some(state) => path_with_color_ui(state, fa.path.to_string()),
            None => Span::raw(fa.path.to_string()),
        };

        let status = state.as_ref().map(status_letter_ui).unwrap_or_default();

        let first_assignment = &fa.assignments[0];
        let cli_id = &first_assignment.cli_id;
        let pad = max_id_width.saturating_sub(cli_id.len());
        let id_padding = " ".repeat(pad);

        let file_cli_id = lookup_cli_id_for_short_id(
            &status_ctx.id_map,
            repo,
            cli_id,
            |id| matches!(id, CliId::Uncommitted(uncommitted) if uncommitted.is_entire_file),
            "uncommitted file",
        )?;

        let lock_items: Vec<(String, String)> = fa
            .assignments
            .iter()
            .flat_map(|a| a.inner.hunk_locks.iter())
            .flatten()
            .map(|l| l.commit_id.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|commit_id| {
                let short_id = shorten_hex_object_id(repo, &commit_id);
                let (lead, rest) = split_short_id(&short_id, 2);
                (lead.to_string(), rest.to_string())
            })
            .collect();

        let mut lock_spans: Vec<Span<'static>> = Vec::new();
        if !lock_items.is_empty() {
            lock_spans.push(Span::raw("🔒 "));
            for (i, (lead, rest)) in lock_items.into_iter().enumerate() {
                if i > 0 {
                    lock_spans.push(Span::raw(", "));
                }
                lock_spans.push(Span::styled(lead, Style::default().blue().bold()));
                lock_spans.push(Span::styled(rest, Style::default().blue()));
            }
        }

        let mut file_line = Vec::from([
            Span::raw(id_padding.clone()),
            Span::styled(cli_id.to_string(), Style::default().bold().blue()),
            Span::raw(" "),
            Span::raw(status.to_string()),
            Span::raw(" "),
            path,
        ]);
        if !lock_spans.is_empty() {
            file_line.push(Span::raw(" "));
            file_line.extend(lock_spans);
        }

        if unstaged {
            output.unstaged_file(Vec::from([Span::raw("┊   ")]), file_line, file_cli_id)?;
        } else {
            output.staged_file(Vec::from([Span::raw("┊  │ ")]), file_line, file_cli_id)?;
        }
    }

    if !unstaged && !assignments.is_empty() {
        output.connector(Vec::from([Span::raw("┊  │")]))?;
    }

    Ok(())
}

fn print_group(
    ctx: &mut Context,
    status_ctx: &StatusContext<'_>,
    stack_with_id: &Option<StackWithId>,
    assignments: &[FileAssignment],
    stack_mark: &mut Option<Span<'static>>,
    first: bool,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    let repo = ctx
        .legacy_project
        .open_isolated_repo()?
        .for_commit_shortening();
    if let Some(stack_with_id) = stack_with_id {
        let mut first = true;
        for segment in &stack_with_id.segments {
            let notch = if first { "╭" } else { "├" };
            if !first {
                output.connector(Vec::from([Span::raw("┊│")]))?;
            }

            let no_commits = if segment.workspace_commits.is_empty() {
                "(no commits)"
            } else {
                ""
            };

            let review_spans: Vec<Span<'static>> = segment
                .branch_name()
                .and_then(|branch_name| {
                    review::from_branch_details(
                        &status_ctx.review_map,
                        branch_name,
                        segment.pr_number(),
                    )
                })
                .map(|r| {
                    [Span::raw(" ")]
                        .into_iter()
                        .chain(r.display_cli(
                            status_ctx.flags.verbose,
                            status_ctx.should_truncate_for_terminal,
                        ))
                        .chain([Span::raw(" ")])
                        .collect()
                })
                .unwrap_or_default();

            let ci_spans: Vec<Span<'static>> = segment
                .branch_name()
                .and_then(|branch_name| status_ctx.ci_map.get(&branch_name.to_string()))
                .map(CiChecks::from)
                .map(|c| {
                    c.display_cli(
                        status_ctx.flags.verbose,
                        status_ctx.should_truncate_for_terminal,
                    )
                    .into_iter()
                    .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let merge_status = segment
                .branch_name()
                .and_then(|branch_name| {
                    status_ctx
                        .branch_merge_statuses
                        .get(&branch_name.to_string())
                })
                .map(|status| match status {
                    UpstreamBranchStatus::SaflyUpdatable => {
                        Span::styled(" [✓ upstream merges cleanly]", Style::default().blue())
                    }
                    UpstreamBranchStatus::Integrated => {
                        Span::styled(" [⬆ integrated upstream]", Style::default().magenta())
                    }
                    UpstreamBranchStatus::Conflicted { .. } => {
                        Span::styled(" [⚠ upstream conflicts]", Style::default().red())
                    }
                    UpstreamBranchStatus::Empty => Span::styled(" ○ empty", Style::default().dim()),
                })
                .unwrap_or(Span::raw(""));

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

            let branch = segment.branch_name().unwrap_or(BStr::new("")).to_string();
            let branch_cli_id = lookup_cli_id_for_short_id(
                &status_ctx.id_map,
                &repo,
                &segment.short_id,
                |id| matches!(id, CliId::Branch { .. }),
                "branch",
            )?;
            let mut branch_suffix = Vec::new();
            branch_suffix.extend(ci_spans);
            if !merge_status.content.is_empty() {
                branch_suffix.push(merge_status);
            }
            branch_suffix.extend(review_spans);
            if !no_commits.is_empty() {
                branch_suffix.push(Span::raw(" "));
                branch_suffix.push(Span::styled(no_commits, Style::default().dim().italic()));
            }
            if let Some(stack_mark) = stack_mark.as_ref().cloned() {
                branch_suffix.push(Span::raw(" "));
                branch_suffix.push(stack_mark);
            }

            output.branch(
                Vec::from([Span::raw(format!("┊{notch}┄"))]),
                BranchLineContent {
                    id: Vec::from([Span::styled(
                        segment.short_id.clone(),
                        Style::default().blue().bold(),
                    )]),
                    decoration_start: Vec::from([Span::raw(" [")]),
                    branch_name: Vec::from([
                        Span::styled(branch, Style::default().green().bold()),
                        Span::raw(workspace),
                    ]),
                    decoration_end: Vec::from([Span::raw("]")]),
                    suffix: branch_suffix,
                },
                branch_cli_id,
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
                output.connector(Vec::from([Span::raw("┊┊")]))?;
                output.upstream_changes(
                    Vec::from([Span::raw("┊╭┄┄")]),
                    Vec::from([Span::styled(
                        format!("(upstream: on {})", BStr::new(tracking_branch)),
                        Style::default().yellow(),
                    )]),
                )?;
            }
            for commit in &segment.remote_commits {
                let details =
                    but_api::diff::commit_details(ctx, commit.commit_id(), ComputeLineStats::No)?;
                print_commit(
                    &repo,
                    status_ctx,
                    stack_with_id.id,
                    commit.short_id.clone(),
                    &commit.inner,
                    CommitChanges::Remote(&details.diff_with_first_parent),
                    CommitClassification::Upstream,
                    false,
                    None,
                    output,
                )?;
            }
            if !segment.remote_commits.is_empty() {
                output.connector(Vec::from([Span::raw("┊-")]))?;
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

                print_commit(
                    &repo,
                    status_ctx,
                    stack_with_id.id,
                    commit.short_id.clone(),
                    &commit.inner.inner,
                    CommitChanges::Workspace(&commit.tree_changes_using_repo(&repo)?),
                    classification,
                    marked,
                    // TODO: populate the Gerrit review URL. It
                    // seems to be populated in handle_gerrit in
                    // crates/but-api/src/legacy/workspace.rs
                    None,
                    output,
                )?;
            }
        }
    } else {
        let cli_id = status_ctx.id_map.unassigned();
        let mut line = Vec::from([
            Span::styled(
                cli_id.to_short_string().to_string(),
                Style::default().bold().blue(),
            ),
            Span::raw(" ["),
            Span::styled("unstaged changes", Style::default().bold().cyan()),
            Span::raw("]"),
        ]);
        if let Some(stack_mark) = stack_mark {
            line.push(Span::raw(" "));
            line.push(stack_mark.clone());
        }
        output.unstaged_changes(Vec::from([Span::raw("╭┄")]), line, cli_id.clone())?;
        print_assignments(&repo, status_ctx, None, None, assignments, true, output)?;
    }
    if !first {
        output.connector(Vec::from([Span::raw("├╯")]))?;
    }
    output.connector(Vec::from([Span::raw("┊")]))?;
    Ok(())
}

fn lookup_cli_id_for_short_id(
    id_map: &IdMap,
    repo: &gix::Repository,
    short_id: &str,
    predicate: impl Fn(&CliId) -> bool,
    kind: &str,
) -> anyhow::Result<CliId> {
    let mut matches = id_map.parse_using_repo(short_id, repo)?;
    matches.retain(|id| id.to_short_string() == short_id && predicate(id));

    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(anyhow::anyhow!(
            "Could not find {kind} CLI id '{short_id}' in IdMap"
        )),
        _ => Err(anyhow::anyhow!(
            "CLI id '{short_id}' is ambiguous for {kind} in IdMap"
        )),
    }
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

pub fn path_with_color_ui(status: &ui::TreeStatus, path: String) -> Span<'static> {
    match status {
        ui::TreeStatus::Addition { .. } => Span::styled(path, Style::default().green()),
        ui::TreeStatus::Deletion { .. } => Span::styled(path, Style::default().red()),
        ui::TreeStatus::Modification { .. } => Span::styled(path, Style::default().yellow()),
        ui::TreeStatus::Rename { .. } => Span::styled(path, Style::default().magenta()),
    }
}

fn path_with_color(status: &TreeStatus, path: String) -> Span<'static> {
    match status {
        TreeStatus::Addition { .. } => Span::styled(path, Style::default().green()),
        TreeStatus::Deletion { .. } => Span::styled(path, Style::default().red()),
        TreeStatus::Modification { .. } => Span::styled(path, Style::default().yellow()),
        TreeStatus::Rename { .. } => Span::styled(path, Style::default().magenta()),
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
    repo: &gix::Repository,
    status_ctx: &StatusContext<'_>,
    stack_id: Option<StackId>,
    short_id: ShortId,
    commit: &but_workspace::ref_info::Commit,
    commit_changes: CommitChanges,
    classification: CommitClassification,
    marked: bool,
    review_url: Option<String>,
    output: &mut StatusOutput<'_>,
) -> anyhow::Result<()> {
    let dot = match classification {
        CommitClassification::Upstream => Span::styled("●", Style::default().yellow()),
        CommitClassification::LocalOnly => Span::raw("●"),
        CommitClassification::Pushed => Span::styled("●", Style::default().green()),
        CommitClassification::Modified => Span::styled("◐", Style::default().green()),
        CommitClassification::Integrated => Span::styled("●", Style::default().magenta()),
    };

    let upstream_commit = matches!(commit_changes, CommitChanges::Remote(_));

    let (details_line, _) = display_cli_commit_details(
        repo,
        short_id.clone(),
        commit,
        match commit_changes {
            CommitChanges::Workspace(tree_changes) => !tree_changes.is_empty(),
            CommitChanges::Remote(tree_changes) => !tree_changes.is_empty(),
        },
        status_ctx.flags.verbose,
        status_ctx.is_paged,
    );
    let commit_cli_id = lookup_cli_id_for_short_id(
        &status_ctx.id_map,
        repo,
        &short_id,
        |id| matches!(id, CliId::Commit { commit_id, .. } if *commit_id == commit.id),
        "commit",
    )?;

    let details_line = if upstream_commit {
        dim_commit_line_content(details_line)
    } else {
        details_line
    };

    if status_ctx.flags.verbose {
        output.commit(
            Vec::from([Span::raw("┊"), dot, Span::raw(" ")]),
            CommitLineContent {
                sha: details_line.sha,
                author: details_line.author,
                message: details_line.message,
                suffix: details_line
                    .suffix
                    .into_iter()
                    .chain(review_url.as_ref().into_iter().flat_map(|review_url| {
                        [
                            Span::raw(" "),
                            Span::raw("◖"),
                            Span::styled(
                                review_url.to_owned(),
                                Style::default().underlined().blue(),
                            ),
                            Span::raw("◗"),
                        ]
                    }))
                    .chain(
                        marked
                            .then(|| {
                                [
                                    Span::raw(" "),
                                    Span::styled("◀ Marked ▶", Style::default().red().bold()),
                                ]
                            })
                            .into_iter()
                            .flatten(),
                    )
                    .collect(),
            },
            commit_cli_id.clone(),
            stack_id,
            classification,
        )?;
        let (message, is_empty_message) = commit_message_display_cli(
            &commit.message,
            status_ctx.flags.verbose,
            status_ctx.is_paged,
            |truncated| {
                if upstream_commit {
                    Span::styled(truncated, Style::default().dim())
                } else {
                    Span::raw(truncated)
                }
            },
        );
        let line = Vec::from([message]);
        if is_empty_message {
            output.empty_commit_message(Vec::from([Span::raw("┊│     ")]), line)?;
        } else {
            output.commit_message(Vec::from([Span::raw("┊│     ")]), line)?;
        }
    } else {
        output.commit(
            Vec::from([Span::raw("┊"), dot, Span::raw("   ")]),
            CommitLineContent {
                sha: details_line.sha,
                author: details_line.author,
                message: details_line.message,
                suffix: details_line
                    .suffix
                    .into_iter()
                    .chain(review_url.as_ref().into_iter().flat_map(|review_url| {
                        [
                            Span::raw(" "),
                            Span::raw("◖"),
                            Span::styled(
                                review_url.to_owned(),
                                Style::default().underlined().blue(),
                            ),
                            Span::raw("◗"),
                        ]
                    }))
                    .chain(
                        marked
                            .then(|| {
                                [
                                    Span::raw(" "),
                                    Span::styled("◀ Marked ▶", Style::default().red().bold()),
                                ]
                            })
                            .into_iter()
                            .flatten(),
                    )
                    .collect(),
            },
            commit_cli_id.clone(),
            stack_id,
            classification,
        )?;
    }
    if status_ctx.flags.show_files.show_files_for(commit.id) {
        match commit_changes {
            CommitChanges::Workspace(tree_changes) => {
                for TreeChangeWithId { short_id, inner } in tree_changes {
                    let file_cli_id = CliId::CommittedFile {
                        commit_id: commit.id,
                        path: inner.path.clone(),
                        id: short_id.to_owned(),
                    };

                    output.file(
                        Vec::from([Span::raw("┊│     ")]),
                        [
                            Span::styled(short_id.to_owned(), Style::default().blue().bold()),
                            Span::raw(" "),
                        ]
                        .into_iter()
                        .chain(inner.display_cli(false, status_ctx.should_truncate_for_terminal))
                        .collect(),
                        file_cli_id,
                    )?;
                }
            }
            CommitChanges::Remote(tree_changes) => {
                for change in tree_changes {
                    output.file(
                        Vec::from([Span::raw("┊│     ")]),
                        change
                            .display_cli(false, status_ctx.should_truncate_for_terminal)
                            .into_iter()
                            .collect(),
                        commit_cli_id.clone(),
                    )?;
                }
            }
        }
    }
    Ok(())
}

trait CliDisplay {
    fn display_cli(
        &self,
        verbose: bool,
        should_truncate_for_terminal: bool,
    ) -> impl IntoIterator<Item = Span<'static>>;
}

impl CliDisplay for but_core::TreeChange {
    fn display_cli(
        &self,
        _verbose: bool,
        _should_truncate_for_terminal: bool,
    ) -> impl IntoIterator<Item = Span<'static>> {
        let path = path_with_color(&self.status, self.path.to_string());
        let status_letter = status_letter(&self.status);
        [Span::raw(format!("{status_letter} ")), path]
    }
}

fn display_cli_commit_details(
    repo: &gix::Repository,
    short_id: ShortId,
    commit: &but_workspace::ref_info::Commit,
    has_changes: bool,
    verbose: bool,
    is_paged: bool,
) -> (CommitLineContent, bool) {
    let commit_id_short = shorten_object_id(repo, commit.id);
    let end_id = if short_id.len() >= commit_id_short.len() {
        Span::raw("")
    } else {
        Span::styled(
            commit_id_short
                .get(short_id.len()..commit_id_short.len())
                .unwrap_or("")
                .to_string(),
            Style::default().dim(),
        )
    };
    let start_id = Span::styled(short_id.to_string(), Style::default().blue().bold());

    let no_changes = if has_changes {
        None
    } else {
        Some(Span::styled(
            "(no changes)",
            Style::default().dim().italic(),
        ))
    };

    let conflicted = if commit.has_conflicts {
        Some(Span::styled("{conflicted}", Style::default().red()))
    } else {
        None
    };

    if verbose {
        // No message when verbose since it goes to the next line
        let created_at = commit.author.time;
        let formatted_time = created_at.format_or_unix(CLI_DATE);
        (
            CommitLineContent {
                sha: Vec::from_iter([start_id, end_id]),
                author: Vec::from_iter([Span::raw(" "), Span::raw(commit.author.name.to_string())]),
                message: Vec::new(),
                suffix: Vec::from_iter(
                    [
                        Span::raw(" "),
                        Span::styled(formatted_time, Style::default().dim()),
                    ]
                    .into_iter()
                    .chain(maybe_with_leading_space(no_changes, conflicted)),
                ),
            },
            false,
        )
    } else {
        let (message, is_empty_message) =
            commit_message_display_cli(&commit.message, verbose, is_paged, Span::raw);
        let message = Span::styled(format!("{}", message.content), message.style);
        (
            CommitLineContent {
                sha: Vec::from([start_id, end_id]),
                author: Vec::new(),
                message: Vec::from_iter([Span::raw(" "), message]),
                suffix: maybe_with_leading_space(no_changes, conflicted),
            },
            is_empty_message,
        )
    }
}

fn maybe_with_leading_space(
    a: Option<Span<'static>>,
    b: Option<Span<'static>>,
) -> Vec<Span<'static>> {
    fn with_leading_space(span: Span<'static>) -> Span<'static> {
        Span::styled(format!(" {}", span.content), span.style)
    }

    match (a, b) {
        (None, None) => Vec::new(),
        (None, Some(b)) => Vec::from([with_leading_space(b)]),
        (Some(a), None) => Vec::from([with_leading_space(a)]),
        (Some(a), Some(b)) => Vec::from([with_leading_space(a), with_leading_space(b)]),
    }
}

fn dim_commit_line_content(content: CommitLineContent) -> CommitLineContent {
    let sha = dim_spans(content.sha);
    let mut author = dim_spans(content.author);
    let message = dim_spans(content.message);
    let mut suffix = dim_spans(content.suffix);

    if message.is_empty()
        && let (Some(last_author), Some(first_suffix)) = (author.last_mut(), suffix.first())
        && last_author.style == first_suffix.style
    {
        let first_suffix = suffix.remove(0);
        last_author.content.to_mut().push_str(&first_suffix.content);
    }

    CommitLineContent {
        sha,
        author,
        message,
        suffix,
    }
}

fn dim_spans(spans: Vec<Span<'static>>) -> Vec<Span<'static>> {
    let mut output: Vec<Span<'static>> = Vec::new();

    for span in spans {
        let style = span.style.add_modifier(Modifier::DIM);
        let content = span.content.into_owned();

        if let Some(last) = output.last_mut()
            && last.style == style
        {
            last.content.to_mut().push_str(&content);
        } else {
            output.push(Span::styled(content, style));
        }
    }

    output
}

/// Return the plain (uncolored, unstyled) message text.
/// First line only for inline mode, all lines joined for `verbose`.
/// `is_paged` is used to avoid truncating text when output is being piped to a pager.
/// `transform` can be used to further change the truncated text.
/// If `message` is empty, returns a styled message to indicate this.
fn commit_message_display_cli(
    message: &BString,
    verbose: bool,
    is_paged: bool,
    transform: impl FnOnce(String) -> Span<'static>,
) -> (Span<'static>, bool) {
    let message = message.to_string();
    let text = if verbose {
        message.replace('\n', " ")
    } else {
        message.lines().next().unwrap_or("").to_string()
    };

    if text.is_empty() {
        (
            Span::styled("(no commit message)", Style::default().dim().italic()),
            true,
        )
    } else if is_paged {
        (Span::raw(text), false)
    } else {
        (transform(text), false)
    }
}

/// Truncate `text` to `max_width` when terminal rendering policy requires it.
fn truncate_when_needed(
    text: &str,
    max_width: usize,
    should_truncate_for_terminal: bool,
) -> String {
    if should_truncate_for_terminal {
        truncate_text(text, max_width).into_owned()
    } else {
        text.to_owned()
    }
}

impl CliDisplay for ForgeReview {
    fn display_cli(
        &self,
        verbose: bool,
        should_truncate_for_terminal: bool,
    ) -> impl IntoIterator<Item = Span<'static>> {
        if verbose {
            Vec::from([
                Span::raw("#"),
                Span::styled(self.number.to_string(), Style::default().bold()),
                Span::raw(": "),
                Span::styled(
                    self.html_url.to_string(),
                    Style::default().underlined().blue(),
                ),
            ])
        } else {
            let trimmed: String = self
                .title
                .trim_end_matches(|c: char| !c.is_ascii() && !c.is_alphanumeric())
                .to_string();
            let title = truncate_when_needed(&trimmed, 50, should_truncate_for_terminal);
            Vec::from([
                Span::raw("#"),
                Span::styled(self.number.to_string(), Style::default().bold()),
                Span::raw(": "),
                Span::raw(title),
            ])
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
    fn display_cli(
        &self,
        _verbose: bool,
        _should_truncate_for_terminal: bool,
    ) -> impl IntoIterator<Item = Span<'static>> {
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
            [Span::raw(" CI: ❌")]
        } else if in_progress > 0 {
            [Span::raw(" CI: ⏳")]
        } else if success > 0 {
            [Span::raw(" CI: ✅")]
        } else {
            [Span::raw("")]
        }
    }
}

impl CliDisplay for but_update::AvailableUpdate {
    fn display_cli(
        &self,
        verbose: bool,
        _should_truncate_for_terminal: bool,
    ) -> impl IntoIterator<Item = Span<'static>> {
        let upgrade_hint = {
            #[cfg(feature = "packaged-but-distribution")]
            {
                "upgrade with your package manager"
            }
            #[cfg(not(feature = "packaged-but-distribution"))]
            {
                "upgrade with `but update install`"
            }
        };

        let mut spans = Vec::from([
            Span::raw("Update available: "),
            Span::styled(self.current_version.to_string(), Style::default().dim()),
            Span::raw(" → "),
            Span::styled(
                self.available_version.to_string(),
                Style::default().green().bold(),
            ),
        ]);

        if verbose {
            if let Some(url) = &self.url {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    url.to_string(),
                    Style::default().underlined().blue(),
                ));
            }
        } else {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("({upgrade_hint} or `but update suppress` to dismiss)"),
                Style::default().dim(),
            ));
        }

        spans
    }
}

async fn compute_branch_merge_statuses(
    ctx: &Context,
) -> anyhow::Result<BTreeMap<String, UpstreamBranchStatus>> {
    use gitbutler_branch_actions::upstream_integration::StackStatuses;

    // Get upstream integration statuses using the public API
    let statuses =
        but_api::legacy::virtual_branches::upstream_integration_statuses(ctx.to_sync(), None)
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

#[cfg(test)]
mod tests {
    use super::{StatusRenderMode, truncate_when_needed, truncation_policy};

    #[test]
    fn truncate_when_needed_truncates_text_when_policy_requests_it() {
        assert_eq!(truncate_when_needed("hello world", 5, true), "hell…");
    }

    #[test]
    fn truncate_when_needed_keeps_text_when_policy_disables_it() {
        assert_eq!(truncate_when_needed("hello world", 5, false), "hello world");
    }

    #[test]
    fn truncate_when_needed_keeps_short_text_when_policy_requests_truncation() {
        assert_eq!(truncate_when_needed("hello", 5, true), "hello");
    }

    #[test]
    fn truncation_policy_enables_truncation_for_oneshot_unpaged() {
        assert!(truncation_policy(StatusRenderMode::Oneshot, false));
    }

    #[test]
    fn truncation_policy_disables_truncation_for_oneshot_paged() {
        assert!(!truncation_policy(StatusRenderMode::Oneshot, true));
    }

    #[test]
    fn truncation_policy_disables_truncation_for_tui() {
        assert!(!truncation_policy(
            StatusRenderMode::Tui { debug: false },
            false,
        ));
        assert!(!truncation_policy(
            StatusRenderMode::Tui { debug: false },
            true,
        ));
    }
}
