use std::collections::{BTreeMap, HashMap};

use anyhow::Context as _;
use assignment::FileAssignment;
use bstr::{BStr, BString, ByteSlice};
use but_api::diff::ComputeLineStats;
use but_core::{RepositoryExt, TreeStatus, ref_metadata::StackId, ui};
use but_ctx::Context;
use but_forge::ForgeReview;
use but_graph::SegmentIndex;
use but_workspace::{
    ref_info::{Commit, LocalCommit, LocalCommitRelation, Segment},
    ui::PushStatus,
};
use gitbutler_branch_actions::upstream_integration::BranchStatus as UpstreamBranchStatus;
use gitbutler_operating_modes::OperatingMode;
use gix::date::time::CustomFormat;
use ratatui::{style::Modifier, text::Span};
use serde::Serialize;

use crate::{
    CLI_DATE, CliId, IdMap,
    command::legacy::{
        forge::review,
        status::output::{
            BranchLineContent, CommitLineContent, FileLineContent, StatusOutput, StatusOutputLine,
        },
        workspace_target,
    },
    id::{SegmentWithId, ShortId, StackWithId, TreeChangeWithId},
    tui::text::truncate_text,
    utils::{
        OutputChannel, WriteWithUtils, shorten_hex_object_id, shorten_object_id,
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

    pub fn for_tui() -> Self {
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

    #[expect(dead_code)]
    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum StatusRenderMode {
    Oneshot,
    Tui(TuiLaunchOptions),
}

#[derive(Debug, Default, Copy, Clone)]
pub struct TuiLaunchOptions {
    pub debug: bool,
    pub quit_after: Option<u64>,
    pub headless: bool,
    pub skip_status_after: bool,
    pub show_diff: bool,
    pub select_commit: Option<gix::ObjectId>,
    pub quit_after_rendering_full_diff: bool,
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
type StackEntry = (Option<StackId>, StackDetail);

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
    push_statuses_by_segment_id: HashMap<SegmentIndex, but_workspace::ui::PushStatus>,
    local_commits_by_id: HashMap<gix::ObjectId, LocalCommit>,
    remote_commits_by_id: HashMap<gix::ObjectId, Commit>,
    base_branch: Option<gitbutler_branch_actions::BaseBranch>,
    mode: &'a gitbutler_operating_modes::OperatingMode,
}

fn show_edit_mode_status(ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    // Delegate to the resolve status logic to show actual conflict details
    crate::command::legacy::resolve::show_resolve_status(ctx, out)
}

pub(crate) async fn worktree(
    ctx: &mut Context,
    out: &mut OutputChannel,
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
        StatusRenderMode::Tui(options) => {
            if out.for_human().is_none() {
                return Ok(());
            }

            let mut lines = Vec::new();
            let mut output = StatusOutput::Buffer { lines: &mut lines };
            build_status_output(ctx, &status_ctx, &mut output)?;
            let final_lines = tui::render_tui(ctx, out, &mode, flags, lines, options).await?;

            if !options.skip_status_after
                && let Some(human_out) = out.for_human()
            {
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
    out: &mut OutputChannel,
    mode: &'a OperatingMode,
    flags: StatusFlags,
    render_mode: StatusRenderMode,
) -> anyhow::Result<StatusContext<'a>> {
    // Process rules with exclusive access to create repo and workspace
    let (
        push_statuses_by_segment_id,
        local_commits_by_id,
        remote_commits_by_id,
        stacks,
        resolved_target,
    ) = {
        let context_lines = ctx.settings.context_lines;
        let mut meta = ctx.meta()?;
        let mut guard;
        {
            let (new_guard, repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut()?;
            guard = new_guard;
            if let Ok(rules) = but_rules::list_rules(&db) {
                but_rules::process_rules(
                    rules,
                    &repo,
                    &mut ws,
                    &mut db,
                    &mut meta,
                    guard.write_permission(),
                    context_lines,
                )
                .ok(); // TODO: this is doing double work (hunk-dependencies can be reused)
            }
        }

        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        let head_info = but_workspace::graph_to_ref_info(
            &ws,
            &repo,
            but_workspace::ref_info::Options {
                expensive_commit_info: true,
                ..Default::default()
            },
        )?;
        let mut push_statuses_by_segment_id = HashMap::<SegmentIndex, PushStatus>::new();
        let mut local_commits_by_id = HashMap::<gix::ObjectId, LocalCommit>::new();
        let mut remote_commits_by_id = HashMap::<gix::ObjectId, Commit>::new();
        for stack in head_info.stacks {
            for segment in stack.segments {
                let Segment {
                    commits,
                    commits_on_remote,
                    push_status,
                    ..
                } = segment;
                for local_commit in commits {
                    local_commits_by_id.insert(local_commit.id, local_commit);
                }
                for remote_commit in commits_on_remote {
                    remote_commits_by_id.insert(remote_commit.id, remote_commit);
                }
                push_statuses_by_segment_id.insert(segment.id, push_status);
            }
        }

        let resolved_target = workspace_target::ResolvedTarget::from_workspace(&ws)?;
        (
            push_statuses_by_segment_id,
            local_commits_by_id,
            remote_commits_by_id,
            ws.stacks.clone(),
            resolved_target,
        )
    };

    let cache_config = if flags.refresh_prs {
        but_forge::CacheConfig::NoCache
    } else {
        but_forge::CacheConfig::CacheOnly
    };
    let review_map = review::get_review_map(ctx, Some(cache_config.clone()))?;

    let worktree_changes = but_api::diff::changes_in_worktree(ctx)?;

    let id_map = IdMap::new(stacks, worktree_changes.assignments.clone())?;

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
    let ci_map = ci_map(
        ctx,
        &cache_config,
        &stack_details,
        &push_statuses_by_segment_id,
    )?;

    // Calculate common_merge_base data and upstream state in a scope
    // to ensure repo reference is dropped before any async operations
    let (common_merge_base_data, upstream_state, last_fetched_ms, base_branch) = {
        let base_branch = {
            let mut guard = ctx.exclusive_worktree_access();
            but_api::legacy::virtual_branches::get_base_branch_data(ctx, guard.write_permission())
                .ok()
                .flatten()
        };
        let status_target = resolved_target.for_status(base_branch.as_ref());
        let repo = ctx.repo.get()?;
        let target_commit_id = status_target.commit_id;
        let base_commit = repo.find_commit(target_commit_id)?;
        let base_commit_decoded = base_commit.decode()?;
        let full_message = base_commit_decoded.message.to_string();
        let formatted_date = base_commit_decoded
            .committer()?
            .time()?
            .format_or_unix(DATE_ONLY);
        let author = base_commit_decoded.author()?;
        let common_merge_base_data = CommonMergeBase {
            target_name: status_target.display_name,
            common_merge_base: shorten_object_id(&repo, target_commit_id),
            message: full_message,
            commit_date: formatted_date,
            commit_id: target_commit_id,
            created_at: base_commit_decoded.committer()?.time()?.seconds as i128 * 1000,
            author_name: author.name.to_string(),
            author_email: author.email.to_string(),
        };

        // Get cached upstream state information (without fetching)
        let (upstream_state, last_fetched_ms) = base_branch
            .as_ref()
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
                (state, last_fetched)
            })
            .unwrap_or((None, None));

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
        push_statuses_by_segment_id,
        local_commits_by_id,
        remote_commits_by_id,
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

    output.hint(Vec::from([Span::styled(
        hint_text,
        crate::theme::get().hint,
    )]))?;

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

    let t = crate::theme::get();
    let dot = Span::styled("●", t.success);

    if status_ctx.flags.show_upstream {
        // When showing detailed commits, only show count in summary
        let mut upstream_summary = Vec::from([Span::raw(format!(
            "(upstream) ⏫ {} {}",
            upstream.behind_count,
            if upstream.behind_count == 1 {
                "commit"
            } else {
                "commits"
            }
        ))]);
        if !last_checked_text.is_empty() {
            upstream_summary.push(Span::raw(" "));
            upstream_summary.push(Span::styled(last_checked_text.clone(), t.hint));
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
                        Span::styled(commit_short, t.commit_id),
                        Span::raw(" "),
                        Span::styled(truncated_msg, t.hint),
                    ]),
                )?;
            }
            let hidden_commits = base_branch.behind.saturating_sub(8);
            if hidden_commits > 0 {
                output.upstream_changes(
                    Vec::from([Span::raw("┊    ")]),
                    Vec::from([Span::styled(format!("and {hidden_commits} more…"), t.hint)]),
                )?;
            }
        }
        output.connector(Vec::from([Span::raw("┊┊")]))?;
    } else {
        // Without --upstream, show the summary with latest commit info
        let mut upstream_summary = Vec::from([
            Span::styled(upstream.latest_commit.clone(), t.hint),
            Span::raw(format!(
                " (upstream) ⏫ {} {}",
                upstream.behind_count,
                if upstream.behind_count == 1 {
                    "commit"
                } else {
                    "commits"
                }
            )),
        ]);
        if !last_checked_text.is_empty() {
            upstream_summary.push(Span::raw(" "));
            upstream_summary.push(Span::styled(last_checked_text, t.hint));
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
    let t = crate::theme::get();
    let first_line = truncate_when_needed(first_line, 40, status_ctx.should_truncate_for_terminal);
    output.merge_base(
        Vec::from([Span::raw(connector), Span::raw(" ")]),
        Vec::from([
            Span::styled(
                status_ctx.common_merge_base_data.common_merge_base.clone(),
                t.hint,
            ),
            Span::raw(" (common base) "),
            Span::styled(
                status_ctx.common_merge_base_data.commit_date.clone(),
                t.hint,
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
                Some(Span::styled("◀ Marked ▶", crate::theme::get().attention))
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
    ctx: &Context,
    cache_config: &but_forge::CacheConfig,
    stack_details: &[StackEntry],
    push_statuses_by_segment_id: &HashMap<SegmentIndex, PushStatus>,
) -> Result<BTreeMap<String, Vec<but_forge::CiCheck>>, anyhow::Error> {
    let mut ci_map = BTreeMap::new();
    for (_, (stack_with_id, _)) in stack_details {
        if let Some(stack_with_id) = stack_with_id {
            for segment in &stack_with_id.segments {
                let push_status = push_statuses_by_segment_id.get(&segment.inner.id);
                if push_status.is_none() {
                    eprintln!("warning: head_info does not contain segment that graph has");
                }
                if segment.pr_number().is_some()
                    && !matches!(push_status, Some(PushStatus::Integrated))
                    && let Some(branch_name) = segment.branch_name()
                    && let Ok(checks) = but_api::legacy::forge::list_ci_checks(
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
    let t = crate::theme::get();
    let id = stack
        .and_then(|s| status_ctx.id_map.resolve_stack(s))
        .map(|s| Span::styled(s.to_short_string(), t.cli_id))
        .unwrap_or_default();

    if let Some(stack) = stack
        && (!unstaged && !assignments.is_empty())
    {
        let staged_changes_cli_id = status_ctx
            .id_map
            .resolve_stack(stack)
            .cloned()
            .with_context(|| {
                format!("Could not resolve stack CLI id for staged changes. stack_id={stack:?}")
            })?;

        output.staged_changes(
            Vec::from([Span::raw("┊  ╭┄")]),
            [
                id,
                Span::raw(" ["),
                Span::styled(
                    branch_name
                        .as_ref()
                        .map(|name| format!("staged to {name}"))
                        .unwrap_or_else(|| "staged to ".to_string()),
                    t.info,
                ),
                Span::raw("]"),
            ]
            .into_iter()
            .chain(
                assignments
                    .is_empty()
                    .then(|| [Span::raw(" "), Span::styled("(no changes)", t.hint)])
                    .into_iter()
                    .flatten(),
            )
            .collect(),
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

        let file_line = FileLineContent {
            id: Vec::from([
                Span::raw(id_padding.clone()),
                Span::styled(cli_id.to_string(), t.cli_id),
                Span::raw(" "),
            ]),
            status: Vec::from([Span::raw(status.to_string()), Span::raw(" ")]),
            path: Vec::from([path]),
        };

        if unstaged {
            output.unassigned_file(Vec::from([Span::raw("┊   ")]), file_line, file_cli_id)?;
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
    let t = crate::theme::get();
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
                    UpstreamBranchStatus::SafelyUpdatable => {
                        Span::styled(" [✓ upstream merges cleanly]", t.success)
                    }
                    UpstreamBranchStatus::Integrated => {
                        Span::styled(" [⬆ integrated upstream]", t.remote_branch)
                    }
                    UpstreamBranchStatus::Conflicted { .. } => {
                        Span::styled(" [⚠ upstream conflicts]", t.error)
                    }
                    UpstreamBranchStatus::Empty => Span::styled(" ○ empty", t.hint),
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
                branch_suffix.push(Span::styled(no_commits, t.hint));
            }
            if let Some(stack_mark) = stack_mark.as_ref().cloned() {
                branch_suffix.push(Span::raw(" "));
                branch_suffix.push(stack_mark);
            }

            output.branch(
                Vec::from([Span::raw(format!("┊{notch}┄"))]),
                BranchLineContent {
                    id: Vec::from([Span::styled(segment.short_id.clone(), t.cli_id)]),
                    decoration_start: Vec::from([Span::raw(" [")]),
                    branch_name: Vec::from([
                        Span::styled(branch, t.local_branch),
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
                        t.attention,
                    )]),
                )?;
            }
            let mut remote_commit_printed = false;
            for commit in &segment.remote_commits {
                let Some(inner) = status_ctx.remote_commits_by_id.get(&commit.commit_id()) else {
                    // This was filtered out because there is a corresponding
                    // local commit, so don't show it.
                    continue;
                };
                let details =
                    but_api::diff::commit_details(ctx, commit.commit_id(), ComputeLineStats::No)?;
                print_commit(
                    &repo,
                    status_ctx,
                    stack_with_id.id,
                    commit.short_id.clone(),
                    inner,
                    CommitChanges::Remote(&details.diff_with_first_parent),
                    CommitClassification::Upstream,
                    false,
                    None,
                    output,
                )?;
                remote_commit_printed = true;
            }
            if remote_commit_printed {
                output.connector(Vec::from([Span::raw("┊-")]))?;
            }
            for commit in segment.workspace_commits.iter() {
                let inner = status_ctx
                    .local_commits_by_id
                    .get(&commit.commit_id())
                    .context("BUG: head_info does not contain local commit that graph has")?;
                let marked = crate::command::legacy::mark::commit_marked(
                    ctx,
                    commit.commit_id().to_string(),
                )
                .unwrap_or_default();
                let classification = match inner.relation {
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
                    &inner.inner,
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
            Span::styled(cli_id.to_short_string().to_string(), t.cli_id),
            Span::raw(" ["),
            Span::styled("unassigned changes", t.info),
            Span::raw("]"),
        ]);
        if assignments.is_empty() {
            line.extend([Span::raw(" "), Span::styled("(no changes)", t.hint)]);
        }
        if let Some(stack_mark) = stack_mark {
            line.extend([Span::raw(" "), stack_mark.clone()]);
        }
        output.unstaged_changes(Vec::from([Span::raw("╭┄")]), line, cli_id.clone())?;
        if !assignments.is_empty() {
            print_assignments(&repo, status_ctx, None, None, assignments, true, output)?;
        }
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
    let t = crate::theme::get();
    match status {
        ui::TreeStatus::Addition { .. } => Span::styled(path, t.addition),
        ui::TreeStatus::Deletion { .. } => Span::styled(path, t.deletion),
        ui::TreeStatus::Modification { .. } => Span::styled(path, t.modification),
        ui::TreeStatus::Rename { .. } => Span::styled(path, t.renaming),
    }
}

fn path_with_color(status: &TreeStatus, path: String) -> Span<'static> {
    let t = crate::theme::get();
    match status {
        TreeStatus::Addition { .. } => Span::styled(path, t.addition),
        TreeStatus::Deletion { .. } => Span::styled(path, t.deletion),
        TreeStatus::Modification { .. } => Span::styled(path, t.modification),
        TreeStatus::Rename { .. } => Span::styled(path, t.renaming),
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
    let t = crate::theme::get();
    let dot = match classification {
        CommitClassification::Upstream => Span::styled("●", t.attention),
        CommitClassification::LocalOnly => Span::raw("●"),
        CommitClassification::Pushed => Span::styled("●", t.success),
        CommitClassification::Modified => Span::styled("◐", t.success),
        CommitClassification::Integrated => Span::styled("●", t.remote_branch),
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
                            Span::styled(review_url.to_owned(), t.link),
                            Span::raw("◗"),
                        ]
                    }))
                    .chain(
                        marked
                            .then(|| [Span::raw(" "), Span::styled("◀ Marked ▶", t.attention)])
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
                    Span::styled(truncated, t.hint)
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
                            Span::styled(review_url.to_owned(), t.link),
                            Span::raw("◗"),
                        ]
                    }))
                    .chain(
                        marked
                            .then(|| [Span::raw(" "), Span::styled("◀ Marked ▶", t.attention)])
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

                    let (status, path) = tree_change_display_cli(inner);
                    output.file(
                        Vec::from([Span::raw("┊│     ")]),
                        FileLineContent {
                            id: Vec::from([
                                Span::styled(short_id.to_owned(), t.cli_id),
                                Span::raw(" "),
                            ]),
                            status: Vec::from([status]),
                            path: Vec::from([path]),
                        },
                        file_cli_id,
                    )?;
                }
            }
            CommitChanges::Remote(tree_changes) => {
                for change in tree_changes {
                    let (status, path) = tree_change_display_cli(change);
                    output.file(
                        Vec::from([Span::raw("┊│     ")]),
                        FileLineContent {
                            id: Vec::new(),
                            status: Vec::from([status]),
                            path: Vec::from([path]),
                        },
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

fn tree_change_display_cli(change: &but_core::TreeChange) -> (Span<'static>, Span<'static>) {
    let path = path_with_color(&change.status, change.path.to_string());
    let status_letter = status_letter(&change.status);
    (Span::raw(format!("{status_letter} ")), path)
}

impl CliDisplay for but_core::TreeChange {
    fn display_cli(
        &self,
        _verbose: bool,
        _should_truncate_for_terminal: bool,
    ) -> impl IntoIterator<Item = Span<'static>> {
        let (status, path) = tree_change_display_cli(self);
        [status, path]
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
    let t = crate::theme::get();
    let commit_id_short = shorten_object_id(repo, commit.id);
    let end_id = if short_id.len() >= commit_id_short.len() {
        Span::raw("")
    } else {
        Span::styled(
            commit_id_short
                .get(short_id.len()..commit_id_short.len())
                .unwrap_or("")
                .to_string(),
            t.hint,
        )
    };
    let start_id = Span::styled(short_id.to_string(), t.cli_id);

    let no_changes = if has_changes {
        None
    } else {
        Some(Span::styled("(no changes)", t.hint))
    };

    let conflicted = if commit.has_conflicts {
        Some(Span::styled("{conflicted}", t.error))
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
                    [Span::raw(" "), Span::styled(formatted_time, t.hint)]
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
            Span::styled("(no commit message)", crate::theme::get().hint),
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
        let t = crate::theme::get();
        if verbose {
            Vec::from([
                Span::raw("#"),
                Span::styled(self.number.to_string(), t.important),
                Span::raw(": "),
                Span::styled(self.html_url.to_string(), t.link),
            ])
        } else {
            let trimmed: String = self
                .title
                .trim_end_matches(|c: char| !c.is_ascii() && !c.is_alphanumeric())
                .to_string();
            let title = truncate_when_needed(&trimmed, 50, should_truncate_for_terminal);
            Vec::from([
                Span::raw("#"),
                Span::styled(self.number.to_string(), t.important),
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
        let t = crate::theme::get();
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
            Span::styled(self.current_version.to_string(), t.hint),
            Span::raw(" → "),
            Span::styled(self.available_version.to_string(), t.attention),
        ]);

        if verbose {
            if let Some(url) = &self.url {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(url.to_string(), t.link));
            }
        } else {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("({upgrade_hint} or `but update suppress` to dismiss)"),
                t.hint,
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
    use crate::command::legacy::status::TuiLaunchOptions;

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
            StatusRenderMode::Tui(TuiLaunchOptions {
                debug: false,
                ..Default::default()
            }),
            false,
        ));
        assert!(!truncation_policy(
            StatusRenderMode::Tui(TuiLaunchOptions {
                debug: false,
                ..Default::default()
            }),
            true,
        ));
    }
}
