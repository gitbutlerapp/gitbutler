use anyhow::{Result, anyhow};
use bstr::ByteSlice;
use but_api::json::HexHash;
use but_oxidize::{OidExt, TimeExt};
use but_settings::AppSettings;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use gitbutler_project::{Project, ProjectId};
use gix::date::time::CustomFormat;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::ListState,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    time::Duration,
};

use super::render::ui;
use crate::status::assignment::FileAssignment;
use but_forge::ForgeReview;
use but_hunk_assignment::HunkAssignment;
use but_workspace::ui::StackDetails;
use gitbutler_branch_actions::{
    integrate_upstream,
    upstream_integration::{
        BranchStatus as UpstreamBranchStatus, Resolution, ResolutionApproach, StackStatuses,
    },
    upstream_integration_statuses,
};
use gitbutler_command_context::CommandContext;

pub(super) const DATE_ONLY: CustomFormat = CustomFormat::new("%Y-%m-%d");

#[derive(PartialEq, Eq)]
pub(super) enum Panel {
    Upstream,
    Status,
    Oplog,
}

pub(super) struct LazyApp {
    pub(super) active_panel: Panel,
    pub(super) unassigned_files: Vec<FileAssignment>,
    pub(super) stacks: Vec<StackInfo>,
    pub(super) oplog_entries: Vec<OplogEntry>,
    pub(super) upstream_info: Option<UpstreamInfo>,
    pub(super) upstream_integration_status: Option<StackStatuses>,
    pub(super) status_state: ListState,
    pub(super) oplog_state: ListState,
    pub(super) command_log: Vec<String>,
    pub(super) main_view_content: Vec<Line<'static>>,
    pub(super) should_quit: bool,
    pub(super) show_help: bool,
    pub(super) help_scroll: u16,
    pub(super) command_log_visible: bool,
    pub(super) show_restore_modal: bool,
    pub(super) show_update_modal: bool,
    pub(super) project_id: ProjectId,
    pub(super) last_refresh: std::time::Instant,
    pub(super) last_fetch: std::time::Instant,
    // Panel areas for mouse click detection
    pub(super) upstream_area: Option<Rect>,
    pub(super) status_area: Option<Rect>,
    pub(super) oplog_area: Option<Rect>,
    pub(super) details_area: Option<Rect>,
    // Commit modal state
    pub(super) show_commit_modal: bool,
    pub(super) commit_subject: String,
    pub(super) commit_message: String,
    pub(super) commit_modal_focus: CommitModalFocus,
    pub(super) commit_branch_options: Vec<CommitBranchOption>,
    pub(super) commit_selected_branch_idx: usize,
    pub(super) commit_new_branch_name: String,
    pub(super) commit_only_mode: bool,
    pub(super) commit_files: Vec<CommitFileOption>,
    pub(super) commit_selected_file_idx: usize,
    pub(super) commit_selected_file_paths: HashSet<String>,
    pub(super) show_reword_modal: bool,
    pub(super) reword_subject: String,
    pub(super) reword_message: String,
    pub(super) reword_modal_focus: RewordModalFocus,
    pub(super) reword_target: Option<RewordTargetInfo>,
    pub(super) show_uncommit_modal: bool,
    pub(super) uncommit_target: Option<UncommitTargetInfo>,
    pub(super) show_diff_modal: bool,
    pub(super) diff_modal_files: Vec<CommitDiffFile>,
    pub(super) diff_modal_selected_file: usize,
    pub(super) diff_modal_scroll: u16,
    pub(super) show_branch_rename_modal: bool,
    pub(super) branch_rename_input: String,
    pub(super) branch_rename_target: Option<BranchRenameTarget>,
    pub(super) show_absorb_modal: bool,
    pub(super) absorb_summary: Option<AbsorbSummary>,
    pub(super) restore_target: Option<OplogEntry>,
    pub(super) show_squash_modal: bool,
    pub(super) squash_target: Option<SquashTargetInfo>,
    // Details pane state
    pub(super) details_selected: bool,
    pub(super) details_scroll: u16,
}

#[derive(Debug, Clone)]
pub(super) struct CommitBranchOption {
    pub(super) stack_id: Option<but_core::ref_metadata::StackId>,
    pub(super) branch_name: String,
    pub(super) is_new_branch: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CommitModalFocus {
    BranchSelect,
    Files,
    NewBranchName,
    Subject,
    Message,
}

#[allow(dead_code)]
pub(super) struct UpstreamInfo {
    pub(super) behind_count: usize,
    latest_commit: String,
    message: String,
    commit_date: String,
    pub(super) last_fetched_ms: Option<u128>,
    commits: Vec<UpstreamCommitInfo>,
}

#[allow(dead_code)]
pub(super) struct UpstreamCommitInfo {
    pub(super) id: String,
    pub(super) full_id: String,
    pub(super) message: String,
    pub(super) author: String,
    pub(super) created_at: String,
}

#[allow(dead_code)]
pub(super) struct StackInfo {
    pub(super) id: Option<gitbutler_stack::StackId>,
    pub(super) name: String,
    pub(super) branches: Vec<BranchInfo>,
}

#[derive(Clone)]
pub(super) struct BranchInfo {
    pub(super) name: String,
    pub(super) commits: Vec<CommitInfo>,
    pub(super) assignments: Vec<FileAssignment>,
}

#[derive(Clone)]
pub(super) struct CommitInfo {
    pub(super) id: String,
    pub(super) full_id: String,
    pub(super) message: String,
    pub(super) author: String,
    pub(super) author_email: String,
    pub(super) author_date: String,
    pub(super) committer: String,
    pub(super) committer_email: String,
    pub(super) committer_date: String,
    pub(super) state: but_workspace::ui::CommitState,
}

#[derive(Clone)]
pub(super) struct CommitFileOption {
    pub(super) file: FileAssignment,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RewordModalFocus {
    Subject,
    Message,
}

pub(super) struct RewordTargetInfo {
    pub(super) stack_id: gitbutler_stack::StackId,
    pub(super) branch_name: String,
    pub(super) commit_short_id: String,
    pub(super) commit_full_id: String,
    pub(super) original_message: String,
}

#[derive(Clone)]
pub(super) struct UncommitTargetInfo {
    pub(super) stack_id: gitbutler_stack::StackId,
    pub(super) branch_name: String,
    pub(super) commit_short_id: String,
    pub(super) commit_full_id: String,
    pub(super) commit_message: String,
}

#[derive(Clone, Default)]
pub(super) struct AbsorbSummary {
    pub(super) file_count: usize,
    pub(super) hunk_count: usize,
    pub(super) total_additions: usize,
    pub(super) total_removals: usize,
}

pub(super) struct SquashTargetInfo {
    pub(super) stack_id: gitbutler_stack::StackId,
    pub(super) branch_name: String,
    pub(super) source_short_id: String,
    pub(super) source_full_id: String,
    pub(super) source_message: String,
    pub(super) destination_short_id: String,
    pub(super) destination_full_id: String,
    pub(super) destination_message: String,
}

#[derive(Clone)]
pub(super) struct UnassignedFileStat {
    pub(super) path: String,
    pub(super) additions: usize,
    pub(super) removals: usize,
}

#[derive(Clone)]
pub(super) struct CommitDiffFile {
    pub(super) path: String,
    pub(super) status: but_core::ui::TreeStatus,
    pub(super) lines: Vec<CommitDiffLine>,
}

#[derive(Clone)]
pub(super) struct CommitDiffLine {
    pub(super) text: String,
    pub(super) kind: DiffLineKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DiffLineKind {
    Header,
    Added,
    Removed,
    Context,
    Info,
}

#[derive(Clone)]
pub(super) struct OplogEntry {
    pub(super) id: String,
    pub(super) full_id: String,
    pub(super) operation: String,
    pub(super) title: String,
    pub(super) time: String,
}

pub(super) struct BranchRenameTarget {
    pub(super) stack_id: gitbutler_stack::StackId,
    pub(super) current_name: String,
}

impl LazyApp {
    fn new(project: &Project) -> Result<Self> {
        let now = std::time::Instant::now();
        let mut app = Self {
            active_panel: Panel::Status,
            unassigned_files: Vec::new(),
            stacks: Vec::new(),
            oplog_entries: Vec::new(),
            upstream_info: None,
            upstream_integration_status: None,
            status_state: ListState::default(),
            oplog_state: ListState::default(),
            command_log: Vec::new(),
            main_view_content: Vec::new(),
            should_quit: false,
            show_help: false,
            help_scroll: 0,
            command_log_visible: true,
            show_restore_modal: false,
            show_update_modal: false,
            project_id: project.id,
            last_refresh: now,
            last_fetch: now,
            upstream_area: None,
            status_area: None,
            oplog_area: None,
            details_area: None,
            show_commit_modal: false,
            commit_subject: String::new(),
            commit_message: String::new(),
            commit_modal_focus: CommitModalFocus::BranchSelect,
            commit_branch_options: Vec::new(),
            commit_selected_branch_idx: 0,
            commit_new_branch_name: String::new(),
            commit_only_mode: false,
            commit_files: Vec::new(),
            commit_selected_file_idx: 0,
            commit_selected_file_paths: HashSet::new(),
            show_reword_modal: false,
            reword_subject: String::new(),
            reword_message: String::new(),
            reword_modal_focus: RewordModalFocus::Subject,
            reword_target: None,
            show_uncommit_modal: false,
            uncommit_target: None,
            show_diff_modal: false,
            diff_modal_files: Vec::new(),
            diff_modal_selected_file: 0,
            diff_modal_scroll: 0,
            show_branch_rename_modal: false,
            branch_rename_input: String::new(),
            branch_rename_target: None,
            show_absorb_modal: false,
            absorb_summary: None,
            restore_target: None,
            show_squash_modal: false,
            squash_target: None,
            details_selected: false,
            details_scroll: 0,
        };

        app.load_data_with_project(project)?;
        app.command_log
            .push("GitButler Lazy TUI started".to_string());

        // Select first item in status list
        let status_item_count = app.count_status_items();
        if status_item_count > 0 {
            app.status_state.select(Some(0));
        }
        if !app.oplog_entries.is_empty() {
            app.oplog_state.select(Some(0));
        }

        app.update_main_view();
        Ok(app)
    }

    pub(super) fn load_data_with_project(&mut self, project: &Project) -> Result<()> {
        self.load_data(project.id)?;

        let command_context = Self::open_command_context(project);

        // Load upstream state information separately since it needs the project
        self.command_log
            .push("but_api::legacy::virtual_branches::get_base_branch_data()".to_string());
        self.upstream_info = but_api::legacy::virtual_branches::get_base_branch_data(project.id)
            .ok()
            .flatten()
            .and_then(|base_branch| {
                if base_branch.behind > 0 {
                    let ctx = command_context.as_ref()?;
                    let repo = ctx.gix_repo().ok()?;
                    let commit_obj = repo.find_commit(base_branch.current_sha.to_gix()).ok()?;
                    let commit = commit_obj.decode().ok()?;
                    let commit_message = commit
                        .message
                        .to_string()
                        .replace('\n', " ")
                        .chars()
                        .take(50)
                        .collect::<String>();
                    let formatted_date = commit.committer().time().ok()?.format_or_unix(DATE_ONLY);

                    // Collect upstream commits from base_branch
                    let mut all_upstream_commits = Vec::new();
                    for uc in &base_branch.upstream_commits {
                        let created_at = {
                            let seconds = (uc.created_at / 1000) as i64;
                            let dt =
                                chrono::DateTime::from_timestamp(seconds, 0).unwrap_or_default();
                            dt.format("%Y-%m-%d %H:%M:%S").to_string()
                        };
                        all_upstream_commits.push(UpstreamCommitInfo {
                            id: uc.id.to_string()[..7].to_string(),
                            full_id: uc.id.to_string(),
                            message: uc.description.to_string(),
                            author: uc.author.name.clone(),
                            created_at,
                        });
                    }

                    Some(UpstreamInfo {
                        behind_count: base_branch.behind,
                        latest_commit: base_branch.current_sha.to_string()[..7].to_string(),
                        message: commit_message,
                        commit_date: formatted_date,
                        last_fetched_ms: base_branch.last_fetched_ms,
                        commits: all_upstream_commits,
                    })
                } else {
                    None
                }
            });

        if let Some(ctx) = command_context.as_ref() {
            self.refresh_upstream_statuses(ctx);
        } else {
            self.upstream_integration_status = None;
        }

        Ok(())
    }

    fn load_data(&mut self, project_id: ProjectId) -> Result<()> {
        // Clear existing data
        self.unassigned_files.clear();
        self.stacks.clear();
        self.oplog_entries.clear();

        // Load unassigned files and stacks
        self.command_log
            .push("but_api::legacy::workspace::stacks()".to_string());
        let stacks = but_api::legacy::workspace::stacks(project_id, None)?;

        self.command_log
            .push("but_api::legacy::diff::changes_in_worktree()".to_string());
        let worktree_changes = but_api::legacy::diff::changes_in_worktree(project_id)?;

        let mut by_file: BTreeMap<bstr::BString, Vec<HunkAssignment>> = BTreeMap::new();
        for assignment in worktree_changes.assignments {
            by_file
                .entry(assignment.path_bytes.clone())
                .or_default()
                .push(assignment);
        }

        let mut assignments_by_file: BTreeMap<bstr::BString, FileAssignment> = BTreeMap::new();
        for (path, assignments) in &by_file {
            assignments_by_file.insert(
                path.clone(),
                FileAssignment::from_assignments(path, assignments),
            );
        }

        // Get unassigned files
        self.unassigned_files =
            crate::status::assignment::filter_by_stack_id(assignments_by_file.values(), &None);

        // Load stacks and branches
        for stack in stacks {
            self.command_log.push(format!(
                "but_api::legacy::workspace::stack_details({:?})",
                stack.id
            ));
            let details = but_api::legacy::workspace::stack_details(project_id, stack.id)?;
            let assignments = crate::status::assignment::filter_by_stack_id(
                assignments_by_file.values(),
                &stack.id,
            );

            let stack_info = self.convert_stack_details(stack.id, details, assignments)?;
            self.stacks.push(stack_info);
        }

        // Load oplog entries
        self.command_log
            .push("but_api::legacy::oplog::list_snapshots()".to_string());
        let snapshots = but_api::legacy::oplog::list_snapshots(project_id, 50, None, None)?;
        for snapshot in snapshots {
            let operation = if let Some(details) = &snapshot.details {
                match details.operation {
                    gitbutler_oplog::entry::OperationKind::CreateCommit => "CREATE",
                    gitbutler_oplog::entry::OperationKind::CreateBranch => "BRANCH",
                    gitbutler_oplog::entry::OperationKind::AmendCommit => "AMEND",
                    gitbutler_oplog::entry::OperationKind::UndoCommit => "UNDO",
                    gitbutler_oplog::entry::OperationKind::SquashCommit => "SQUASH",
                    gitbutler_oplog::entry::OperationKind::UpdateCommitMessage => "REWORD",
                    gitbutler_oplog::entry::OperationKind::MoveCommit => "MOVE",
                    gitbutler_oplog::entry::OperationKind::RestoreFromSnapshot => "RESTORE",
                    gitbutler_oplog::entry::OperationKind::ApplyBranch => "APPLY",
                    gitbutler_oplog::entry::OperationKind::UnapplyBranch => "UNAPPLY",
                    _ => "OTHER",
                }
            } else {
                "UNKNOWN"
            };

            let time = snapshot.created_at.to_gix();
            let time_string = time
                .format(gix::date::time::format::ISO8601)
                .unwrap_or_else(|_| time.seconds.to_string());

            let commit_id = snapshot.commit_id.to_string();
            let short_id = commit_id[..7].to_string();
            self.oplog_entries.push(OplogEntry {
                id: short_id,
                full_id: commit_id,
                operation: operation.to_string(),
                title: snapshot
                    .details
                    .as_ref()
                    .map(|d| d.title.clone())
                    .unwrap_or_default(),
                time: time_string,
            });
        }

        Ok(())
    }

    fn convert_stack_details(
        &self,
        stack_id: Option<gitbutler_stack::StackId>,
        details: StackDetails,
        assignments: Vec<FileAssignment>,
    ) -> Result<StackInfo> {
        let mut branches = Vec::new();

        for (idx, branch) in details.branch_details.into_iter().enumerate() {
            let commits = branch
                .commits
                .iter()
                .map(|c| {
                    let message = c.message.to_str().unwrap_or("");

                    // Format dates
                    let author_date = {
                        let seconds = (c.created_at / 1000) as i64;
                        let dt = chrono::DateTime::from_timestamp(seconds, 0).unwrap_or_default();
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    };

                    CommitInfo {
                        id: c.id.to_string()[..7].to_string(),
                        full_id: c.id.to_string(),
                        message: message.to_string(),
                        author: c.author.name.clone(),
                        author_email: c.author.email.clone(),
                        author_date: author_date.clone(),
                        committer: c.author.name.clone(), // We don't have separate committer info
                        committer_email: c.author.email.clone(),
                        committer_date: author_date,
                        state: c.state.clone(),
                    }
                })
                .collect();

            // Assign all stack files to the first branch (top of stack)
            let branch_assignments = if idx == 0 {
                assignments.clone()
            } else {
                Vec::new()
            };

            branches.push(BranchInfo {
                name: branch.name.to_string(),
                commits,
                assignments: branch_assignments,
            });
        }

        Ok(StackInfo {
            id: stack_id,
            name: details.derived_name,
            branches,
        })
    }

    pub(super) fn count_status_items(&self) -> usize {
        let mut count = 0;
        if !self.unassigned_files.is_empty() {
            count += 1; // Header for unassigned files
            count += self.unassigned_files.len();
        }
        for stack in &self.stacks {
            for branch in &stack.branches {
                count += 1; // Branch header
                count += branch.assignments.len(); // Assigned files
                count += branch.commits.len(); // Commits
            }
            if !stack.branches.is_empty() {
                count += 1; // Blank line between stacks
            }
        }
        count
    }

    pub(super) fn refresh(&mut self) -> Result<()> {
        let project_id = self.project_id;

        // Store current selection indices
        let status_idx = self.status_state.selected();
        let oplog_idx = self.oplog_state.selected();

        // Reload data - need to load project to refresh upstream info
        let project = gitbutler_project::get(project_id)?;
        self.load_data_with_project(&project)?;

        // Restore selections if still valid
        if let Some(idx) = status_idx {
            let total_items = self.count_status_items();
            if idx < total_items {
                self.status_state.select(Some(idx));
            } else if total_items > 0 {
                self.status_state.select(Some(0));
            }
        }

        if let Some(idx) = oplog_idx {
            if idx < self.oplog_entries.len() {
                self.oplog_state.select(Some(idx));
            } else if !self.oplog_entries.is_empty() {
                self.oplog_state.select(Some(0));
            }
        }

        self.update_main_view();
        self.command_log.push("Refreshed data".to_string());
        self.last_refresh = std::time::Instant::now();
        Ok(())
    }

    pub(super) fn fetch_upstream(&mut self) -> Result<()> {
        self.command_log
            .push("but_api::legacy::virtual_branches::fetch_from_remotes()".to_string());

        match but_api::legacy::virtual_branches::fetch_from_remotes(
            self.project_id,
            Some("manual-fetch".to_string()),
        ) {
            Ok(base_branch) => {
                if base_branch.behind > 0 {
                    self.command_log.push(format!(
                        "Fetch completed: {} new commits",
                        base_branch.behind
                    ));
                } else {
                    self.command_log
                        .push("Fetch completed: up to date".to_string());
                }

                // Show fetch results in main view
                self.main_view_content.clear();
                self.main_view_content.push(Line::from(vec![Span::styled(
                    "Fetch Results",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));
                self.main_view_content.push(Line::from(""));

                self.main_view_content.push(Line::from(vec![
                    Span::styled("Remote: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!(
                        "{}/{}",
                        base_branch.remote_name, base_branch.branch_name
                    )),
                ]));
                self.main_view_content.push(Line::from(""));

                if base_branch.behind > 0 {
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(
                            format!("{} new commits available", base_branch.behind),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]));
                } else {
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled("Up to date", Style::default().fg(Color::Green)),
                    ]));
                }
                self.main_view_content.push(Line::from(""));

                if base_branch.conflicted {
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("⚠ ", Style::default().fg(Color::Red)),
                        Span::styled("Conflicted with upstream", Style::default().fg(Color::Red)),
                    ]));
                    self.main_view_content.push(Line::from(""));
                }

                if base_branch.diverged {
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("⚠ ", Style::default().fg(Color::Yellow)),
                        Span::styled("Diverged from upstream", Style::default().fg(Color::Yellow)),
                    ]));
                    self.main_view_content.push(Line::from(vec![
                        Span::raw("  Ahead: "),
                        Span::styled(
                            format!("{} commits", base_branch.diverged_ahead.len()),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]));
                    self.main_view_content.push(Line::from(vec![
                        Span::raw("  Behind: "),
                        Span::styled(
                            format!("{} commits", base_branch.diverged_behind.len()),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]));
                    self.main_view_content.push(Line::from(""));
                }

                // Reload data to update the upstream panel and commit list
                let project = gitbutler_project::get(self.project_id)?;
                self.load_data_with_project(&project)?;
                self.update_main_view();
                self.last_fetch = std::time::Instant::now();

                Ok(())
            }
            Err(e) => {
                self.command_log.push(format!("Fetch error: {}", e));

                // Show error in main view
                self.main_view_content.clear();
                self.main_view_content.push(Line::from(vec![Span::styled(
                    "Fetch Failed",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]));
                self.main_view_content.push(Line::from(""));
                self.main_view_content.push(Line::from(vec![
                    Span::styled("Error: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(e.to_string()),
                ]));

                Err(e)
            }
        }
    }

    pub(super) fn active_panel_name(&self) -> &str {
        match self.active_panel {
            Panel::Upstream => "Upstream",
            Panel::Status => "Status",
            Panel::Oplog => "Oplog",
        }
    }

    pub(super) fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Upstream => Panel::Status,
            Panel::Status => Panel::Oplog,
            Panel::Oplog => {
                if self.upstream_info.is_some() {
                    Panel::Upstream
                } else {
                    Panel::Status
                }
            }
        };
    }

    pub(super) fn prev_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Upstream => Panel::Oplog,
            Panel::Status => {
                if self.upstream_info.is_some() {
                    Panel::Upstream
                } else {
                    Panel::Oplog
                }
            }
            Panel::Oplog => Panel::Status,
        };
    }

    fn is_blank_line(&self, idx: usize) -> bool {
        let mut current_idx = 0;

        // Check if unassigned files section has a blank line at this position
        if !self.unassigned_files.is_empty() {
            current_idx += 1; // Header
            current_idx += self.unassigned_files.len();
            if current_idx == idx {
                return true; // Blank line after unassigned files
            }
            current_idx += 1; // Blank line
        }

        // Check stacks for blank lines
        for stack in &self.stacks {
            for branch in &stack.branches {
                current_idx += 1; // Branch header
                current_idx += branch.assignments.len(); // Assigned files
                current_idx += branch.commits.len(); // Commits
            }
            // Blank line between stacks
            if !stack.branches.is_empty() {
                if current_idx == idx {
                    return true; // This is a blank line
                }
                current_idx += 1;
            }
        }

        false
    }

    pub(super) fn select_next(&mut self) {
        match self.active_panel {
            Panel::Upstream => {
                // No selection for upstream panel
            }
            Panel::Status => {
                let total_items = self.count_status_items();
                if total_items > 0 {
                    let mut i = match self.status_state.selected() {
                        Some(i) => {
                            if i >= total_items - 1 {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };

                    // Skip blank lines when moving forward
                    let mut attempts = 0;
                    while self.is_blank_line(i) && attempts < total_items {
                        i = if i >= total_items - 1 { 0 } else { i + 1 };
                        attempts += 1;
                    }

                    self.status_state.select(Some(i));
                }
            }
            Panel::Oplog => {
                if !self.oplog_entries.is_empty() {
                    let i = match self.oplog_state.selected() {
                        Some(i) => {
                            if i >= self.oplog_entries.len() - 1 {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };
                    self.oplog_state.select(Some(i));
                }
            }
        }
    }

    pub(super) fn select_prev(&mut self) {
        match self.active_panel {
            Panel::Upstream => {
                // No selection for upstream panel
            }
            Panel::Status => {
                let total_items = self.count_status_items();
                if total_items > 0 {
                    let mut i = match self.status_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                total_items - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };

                    // Skip blank lines when moving backward
                    let mut attempts = 0;
                    while self.is_blank_line(i) && attempts < total_items {
                        i = if i == 0 { total_items - 1 } else { i - 1 };
                        attempts += 1;
                    }

                    self.status_state.select(Some(i));
                }
            }
            Panel::Oplog => {
                if !self.oplog_entries.is_empty() {
                    let i = match self.oplog_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                self.oplog_entries.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    self.oplog_state.select(Some(i));
                }
            }
        }
    }

    fn branch_ranges(&self) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let mut idx = 0;

        if !self.unassigned_files.is_empty() {
            idx += 1; // Header
            idx += self.unassigned_files.len();
            idx += 1; // Blank line after unassigned files
        }

        for stack in &self.stacks {
            for branch in &stack.branches {
                let start = idx;
                idx += 1; // Branch header
                idx += branch.assignments.len();
                idx += branch.commits.len();
                ranges.push((start, idx));
            }
            if !stack.branches.is_empty() {
                idx += 1; // Blank line between stacks
            }
        }

        ranges
    }

    pub(super) fn select_next_branch(&mut self) -> bool {
        self.select_branch(true)
    }

    pub(super) fn select_prev_branch(&mut self) -> bool {
        self.select_branch(false)
    }

    fn select_branch(&mut self, forward: bool) -> bool {
        let ranges = self.branch_ranges();
        if ranges.is_empty() {
            return false;
        }

        let selected_idx = self.status_state.selected().unwrap_or(ranges[0].0);
        let current_branch_idx = ranges
            .iter()
            .position(|(start, end)| selected_idx >= *start && selected_idx < *end)
            .unwrap_or_else(|| if forward { 0 } else { ranges.len() - 1 });

        let target_idx = if forward {
            (current_branch_idx + 1) % ranges.len()
        } else if current_branch_idx == 0 {
            ranges.len() - 1
        } else {
            current_branch_idx - 1
        };

        self.status_state.select(Some(ranges[target_idx].0));
        true
    }

    pub(super) fn get_selected_file(&self) -> Option<&FileAssignment> {
        let idx = self.status_state.selected()?;
        let mut current_idx = 0;

        // Check unassigned files
        if !self.unassigned_files.is_empty() {
            current_idx += 1; // Header
            for file in &self.unassigned_files {
                if current_idx == idx {
                    return Some(file);
                }
                current_idx += 1;
            }
            current_idx += 1; // Blank line
        }

        // Check stacks
        for stack in &self.stacks {
            for branch in &stack.branches {
                current_idx += 1; // Branch header

                // Check assigned files
                for file in &branch.assignments {
                    if current_idx == idx {
                        return Some(file);
                    }
                    current_idx += 1;
                }

                // Skip commits
                current_idx += branch.commits.len();
            }
            current_idx += 1; // Blank line
        }

        None
    }

    pub(super) fn is_unassigned_header_selected(&self) -> bool {
        if self.unassigned_files.is_empty() {
            return false;
        }
        matches!(self.status_state.selected(), Some(0))
    }

    pub(super) fn summarize_unassigned_files(&self) -> (AbsorbSummary, Vec<UnassignedFileStat>) {
        let mut summary = AbsorbSummary::default();
        let mut stats = Vec::new();

        for file in &self.unassigned_files {
            summary.file_count += 1;
            let mut file_additions = 0;
            let mut file_removals = 0;
            let mut file_hunks = 0;

            for assignment in &file.assignments {
                file_hunks += 1;
                if let Some(added) = &assignment.inner.line_nums_added {
                    let count = added.len();
                    summary.total_additions += count;
                    file_additions += count;
                }
                if let Some(removed) = &assignment.inner.line_nums_removed {
                    let count = removed.len();
                    summary.total_removals += count;
                    file_removals += count;
                }
            }

            summary.hunk_count += file_hunks;
            stats.push(UnassignedFileStat {
                path: file.path.to_string(),
                additions: file_additions,
                removals: file_removals,
            });
        }

        (summary, stats)
    }

    pub(super) fn get_selected_branch(&self) -> Option<&BranchInfo> {
        let idx = self.status_state.selected()?;
        let mut current_idx = 0;

        // Skip unassigned files
        if !self.unassigned_files.is_empty() {
            current_idx += 1; // Header
            current_idx += self.unassigned_files.len();
            current_idx += 1; // Blank line
        }

        // Check stacks
        for stack in &self.stacks {
            for branch in &stack.branches {
                if current_idx == idx {
                    return Some(branch);
                }
                current_idx += 1; // Branch header
                current_idx += branch.assignments.len(); // Skip assigned files
                current_idx += branch.commits.len(); // Skip commits
            }
            current_idx += 1; // Blank line
        }

        None
    }

    pub(super) fn get_selected_commit(&self) -> Option<&CommitInfo> {
        let idx = self.status_state.selected()?;
        let mut current_idx = 0;

        // Skip unassigned files
        if !self.unassigned_files.is_empty() {
            current_idx += 1; // Header
            current_idx += self.unassigned_files.len();
            current_idx += 1; // Blank line
        }

        // Check stacks
        for stack in &self.stacks {
            for branch in &stack.branches {
                current_idx += 1; // Branch header
                current_idx += branch.assignments.len(); // Skip assigned files

                // Check commits
                for commit in &branch.commits {
                    if current_idx == idx {
                        return Some(commit);
                    }
                    current_idx += 1;
                }
            }
            current_idx += 1; // Blank line
        }

        None
    }

    pub(super) fn get_selected_oplog_entry(&self) -> Option<&OplogEntry> {
        let idx = self.oplog_state.selected()?;
        self.oplog_entries.get(idx)
    }

    pub(super) fn get_selected_branch_context(
        &self,
    ) -> Option<(Option<gitbutler_stack::StackId>, &BranchInfo)> {
        if let Some(branch) = self.get_selected_branch() {
            if let Some(context) =
                self.find_branch_context(|candidate| std::ptr::eq(candidate, branch))
            {
                return Some(context);
            }
        }

        if let Some(commit) = self.get_selected_commit() {
            if let Some(context) = self.find_branch_context(|branch| {
                branch
                    .commits
                    .iter()
                    .any(|candidate| std::ptr::eq(candidate, commit))
            }) {
                return Some(context);
            }
        }

        if let Some(file) = self.get_selected_file() {
            if let Some(context) = self.find_branch_context(|branch| {
                branch
                    .assignments
                    .iter()
                    .any(|candidate| std::ptr::eq(candidate, file))
            }) {
                return Some(context);
            }
        }

        None
    }

    pub(super) fn find_branch_context(
        &self,
        mut predicate: impl FnMut(&BranchInfo) -> bool,
    ) -> Option<(Option<gitbutler_stack::StackId>, &BranchInfo)> {
        for stack in &self.stacks {
            for branch in &stack.branches {
                if predicate(branch) {
                    return Some((stack.id, branch));
                }
            }
        }
        None
    }

    pub(super) fn get_commit_file_changes(
        &self,
        commit_sha: &str,
    ) -> Result<(Vec<but_core::ui::TreeChange>, but_core::ui::TreeStats)> {
        let oid = gix::ObjectId::from_hex(commit_sha.as_bytes())?;
        let commit_id = HexHash::from(oid);
        let commit_details = but_api::legacy::diff::commit_details(self.project_id, commit_id)?;
        Ok((commit_details.changes.changes, commit_details.changes.stats))
    }

    fn status_letter(status: &but_core::ui::TreeStatus) -> char {
        match status {
            but_core::ui::TreeStatus::Addition { .. } => 'A',
            but_core::ui::TreeStatus::Deletion { .. } => 'D',
            but_core::ui::TreeStatus::Modification { .. } => 'M',
            but_core::ui::TreeStatus::Rename { .. } => 'R',
        }
    }

    fn status_colors(status: &but_core::ui::TreeStatus) -> (Color, Color) {
        // Returns (path_color, status_letter_color)
        match status {
            but_core::ui::TreeStatus::Addition { .. } => (Color::Green, Color::Green),
            but_core::ui::TreeStatus::Deletion { .. } => (Color::Red, Color::Red),
            but_core::ui::TreeStatus::Modification { .. } => (Color::Yellow, Color::Yellow),
            but_core::ui::TreeStatus::Rename { .. } => (Color::Magenta, Color::Magenta),
        }
    }

    fn describe_branch_status(status: &UpstreamBranchStatus) -> (&'static str, String, Color) {
        match status {
            UpstreamBranchStatus::SaflyUpdatable => {
                ("✓", "Will rebase cleanly".to_string(), Color::Green)
            }
            UpstreamBranchStatus::Integrated => {
                ("↺", "Already integrated".to_string(), Color::Blue)
            }
            UpstreamBranchStatus::Conflicted { rebasable } => {
                if *rebasable {
                    (
                        "⚠",
                        "Conflicts expected (rebasable)".to_string(),
                        Color::Yellow,
                    )
                } else {
                    (
                        "✖",
                        "Will conflict (manual merge required)".to_string(),
                        Color::Red,
                    )
                }
            }
            UpstreamBranchStatus::Empty => {
                ("•", "No changes to apply".to_string(), Color::DarkGray)
            }
        }
    }

    pub(super) fn update_main_view(&mut self) {
        self.main_view_content.clear();

        match self.active_panel {
            Panel::Status => {
                // Check if a branch is selected first
                if let Some(branch) = self.get_selected_branch().cloned() {
                    // Branch name header
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("Branch: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(
                            branch.name.clone(),
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    self.main_view_content.push(Line::from(""));

                    // Collect unique authors from commits
                    let mut authors: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    for commit in &branch.commits {
                        authors.insert(commit.author.clone());
                    }

                    // Display authors section
                    if !authors.is_empty() {
                        self.main_view_content.push(Line::from(vec![Span::styled(
                            "Authors:",
                            Style::default().add_modifier(Modifier::BOLD),
                        )]));
                        let mut authors_vec: Vec<_> = authors.into_iter().collect();
                        authors_vec.sort();
                        for author in authors_vec {
                            self.main_view_content.push(Line::from(vec![
                                Span::raw("  • "),
                                Span::styled(author, Style::default().fg(Color::Yellow)),
                            ]));
                        }
                        self.main_view_content.push(Line::from(""));
                    }

                    // Display commits section
                    if !branch.commits.is_empty() {
                        self.main_view_content.push(Line::from(vec![Span::styled(
                            "Commits:",
                            Style::default().add_modifier(Modifier::BOLD),
                        )]));
                        self.main_view_content.push(Line::from(""));

                        for commit in &branch.commits {
                            // Commit header with SHA
                            let (dot_symbol, dot_color) = match &commit.state {
                                but_workspace::ui::CommitState::LocalOnly => ("●", Color::White),
                                but_workspace::ui::CommitState::LocalAndRemote(object_id) => {
                                    if object_id.to_string() == commit.full_id {
                                        ("●", Color::Green)
                                    } else {
                                        ("◐", Color::Green)
                                    }
                                }
                                but_workspace::ui::CommitState::Integrated => ("●", Color::Magenta),
                            };

                            self.main_view_content.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(dot_symbol, Style::default().fg(dot_color)),
                                Span::raw(" "),
                                Span::styled(commit.id.clone(), Style::default().fg(Color::Green)),
                                Span::raw(" "),
                                Span::styled(
                                    commit.author.clone(),
                                    Style::default().fg(Color::Cyan),
                                ),
                                Span::raw(" "),
                                Span::styled(
                                    commit.author_date.clone(),
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]));

                            // Commit message (indented)
                            let message_first_line =
                                commit.message.lines().next().unwrap_or("").to_string();
                            self.main_view_content.push(Line::from(vec![
                                Span::raw("    "),
                                Span::raw(message_first_line),
                            ]));

                            // Get file changes for this commit
                            if let Ok((changes, _)) = self.get_commit_file_changes(&commit.full_id)
                            {
                                // Show files modified (indented further)
                                for change in changes.iter().take(5) {
                                    let status_char = Self::status_letter(&change.status);
                                    let (path_color, status_color) =
                                        Self::status_colors(&change.status);

                                    self.main_view_content.push(Line::from(vec![
                                        Span::raw("      "),
                                        Span::styled(
                                            format!("{} ", status_char),
                                            Style::default()
                                                .fg(status_color)
                                                .add_modifier(Modifier::BOLD),
                                        ),
                                        Span::styled(
                                            change.path.to_string(),
                                            Style::default().fg(path_color),
                                        ),
                                    ]));
                                }
                                if changes.len() > 5 {
                                    self.main_view_content.push(Line::from(vec![
                                        Span::raw("      "),
                                        Span::styled(
                                            format!("... {} more files", changes.len() - 5),
                                            Style::default().fg(Color::DarkGray),
                                        ),
                                    ]));
                                }
                            }

                            self.main_view_content.push(Line::from(""));
                        }
                    } else {
                        self.main_view_content.push(Line::from(vec![Span::styled(
                            "No commits in this branch",
                            Style::default().fg(Color::DarkGray),
                        )]));
                    }
                } else if let Some(commit) = self.get_selected_commit().cloned() {
                    // Commit SHA with bold label and green SHA
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("Commit: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(commit.full_id.clone(), Style::default().fg(Color::Green)),
                    ]));
                    self.main_view_content.push(Line::from(""));

                    // Author with bold label, yellow name, purple email
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("Author: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(commit.author.clone(), Style::default().fg(Color::Yellow)),
                        Span::raw(" <"),
                        Span::styled(
                            commit.author_email.clone(),
                            Style::default().fg(Color::Magenta),
                        ),
                        Span::raw(">"),
                    ]));

                    // Author date with bold label and blue date
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("Date: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(commit.author_date.clone(), Style::default().fg(Color::Blue)),
                    ]));
                    self.main_view_content.push(Line::from(""));

                    // Committer with bold label, yellow name, purple email
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("Committer: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(commit.committer.clone(), Style::default().fg(Color::Yellow)),
                        Span::raw(" <"),
                        Span::styled(
                            commit.committer_email.clone(),
                            Style::default().fg(Color::Magenta),
                        ),
                        Span::raw(">"),
                    ]));

                    // Committer date with bold label and blue date
                    self.main_view_content.push(Line::from(vec![
                        Span::styled("Date: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(
                            commit.committer_date.clone(),
                            Style::default().fg(Color::Blue),
                        ),
                    ]));
                    self.main_view_content.push(Line::from(""));

                    // Message with bold label
                    self.main_view_content.push(Line::from(vec![Span::styled(
                        "Message:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )]));
                    for line in commit.message.lines() {
                        self.main_view_content
                            .push(Line::from(format!("  {}", line)));
                    }
                    self.main_view_content.push(Line::from(""));

                    // Get commit details to show file changes
                    self.command_log.push(format!(
                        "but_api::legacy::diff::commit_details({:?})",
                        commit.full_id
                    ));

                    match self.get_commit_file_changes(&commit.full_id) {
                        Ok((changes, stats)) => {
                            // Statistics header
                            self.main_view_content.push(Line::from(vec![
                                Span::styled(
                                    "Changes: ",
                                    Style::default().add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    format!("{} files changed", stats.files_changed),
                                    Style::default().fg(Color::Cyan),
                                ),
                                Span::raw(", "),
                                Span::styled(
                                    format!("+{}", stats.lines_added),
                                    Style::default().fg(Color::Green),
                                ),
                                Span::raw(", "),
                                Span::styled(
                                    format!("-{}", stats.lines_removed),
                                    Style::default().fg(Color::Red),
                                ),
                            ]));
                            self.main_view_content.push(Line::from(""));

                            // File list with status
                            self.main_view_content.push(Line::from(vec![Span::styled(
                                "Files:",
                                Style::default().add_modifier(Modifier::BOLD),
                            )]));

                            for change in changes {
                                let status_char = Self::status_letter(&change.status);
                                let (path_color, status_color) =
                                    Self::status_colors(&change.status);

                                self.main_view_content.push(Line::from(vec![
                                    Span::raw("  "),
                                    Span::styled(
                                        format!("{} ", status_char),
                                        Style::default()
                                            .fg(status_color)
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                    Span::styled(
                                        change.path.to_string(),
                                        Style::default().fg(path_color),
                                    ),
                                ]));
                            }
                        }
                        Err(e) => {
                            self.main_view_content.push(Line::from(vec![
                                Span::styled(
                                    "Error loading file changes: ",
                                    Style::default().fg(Color::Red),
                                ),
                                Span::raw(e.to_string()),
                            ]));
                        }
                    }
                } else if self.is_unassigned_header_selected() {
                    let (summary, stats) = self.summarize_unassigned_files();

                    if summary.file_count == 0 {
                        self.main_view_content.push(Line::from(vec![Span::styled(
                            "No unassigned files",
                            Style::default().fg(Color::DarkGray),
                        )]));
                    } else {
                        self.main_view_content.push(Line::from(vec![Span::styled(
                            "Unassigned Files",
                            Style::default().add_modifier(Modifier::BOLD),
                        )]));
                        self.main_view_content.push(Line::from(vec![
                            Span::styled(
                                format!("{} files", summary.file_count),
                                Style::default().fg(Color::Cyan),
                            ),
                            Span::raw("  •  "),
                            Span::styled(
                                format!("{} hunks", summary.hunk_count),
                                Style::default().fg(Color::Yellow),
                            ),
                            Span::raw("  •  "),
                            Span::styled(
                                format!("+{}", summary.total_additions),
                                Style::default().fg(Color::Green),
                            ),
                            Span::raw("  "),
                            Span::styled(
                                format!("-{}", summary.total_removals),
                                Style::default().fg(Color::Red),
                            ),
                        ]));
                        self.main_view_content.push(Line::from(""));

                        for stat in stats {
                            self.main_view_content.push(Line::from(vec![
                                Span::styled(
                                    format!("+{}", stat.additions),
                                    Style::default().fg(Color::Green),
                                ),
                                Span::raw("  "),
                                Span::styled(
                                    format!("-{}", stat.removals),
                                    Style::default().fg(Color::Red),
                                ),
                                Span::raw("  "),
                                Span::styled(stat.path.clone(), Style::default().fg(Color::Yellow)),
                            ]));
                        }
                    }
                } else if let Some(file) = self.get_selected_file().cloned() {
                    // Show file diff with bold file path
                    self.main_view_content.push(Line::from(vec![
                        Span::raw("File: "),
                        Span::styled(
                            file.path.to_string(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    self.main_view_content.push(Line::from(""));

                    for assignment in &file.assignments {
                        let hunk_header = assignment
                            .inner
                            .hunk_header
                            .as_ref()
                            .map(|h| {
                                format!(
                                    "@@ -{},{} +{},{} @@",
                                    h.old_start, h.old_lines, h.new_start, h.new_lines
                                )
                            })
                            .unwrap_or_else(|| "(no hunk info)".to_string());

                        // Style hunk header in cyan
                        self.main_view_content.push(Line::from(vec![Span::styled(
                            hunk_header,
                            Style::default().fg(Color::Cyan),
                        )]));

                        // Show the diff lines with syntax highlighting
                        if let Some(diff) = &assignment.inner.diff {
                            for line in diff.lines() {
                                let line_str = String::from_utf8_lossy(line);

                                // Color diff lines based on their prefix
                                let styled_line = if line_str.starts_with('+') {
                                    Line::from(vec![Span::styled(
                                        line_str.to_string(),
                                        Style::default().fg(Color::Green),
                                    )])
                                } else if line_str.starts_with('-') {
                                    Line::from(vec![Span::styled(
                                        line_str.to_string(),
                                        Style::default().fg(Color::Red),
                                    )])
                                } else if line_str.starts_with("@@") {
                                    Line::from(vec![Span::styled(
                                        line_str.to_string(),
                                        Style::default().fg(Color::Cyan),
                                    )])
                                } else {
                                    Line::from(line_str.to_string())
                                };

                                self.main_view_content.push(styled_line);
                            }
                        }
                        self.main_view_content.push(Line::from(""));
                    }
                } else {
                    self.main_view_content
                        .push(Line::from("Select a file or commit to view details"));
                }
            }
            Panel::Upstream => {
                if let Some(upstream) = &self.upstream_info {
                    self.main_view_content.push(Line::from(vec![Span::styled(
                        "Upstream Commits",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )]));
                    self.main_view_content.push(Line::from(""));

                    for commit in &upstream.commits {
                        self.main_view_content.push(Line::from(vec![
                            Span::styled("●", Style::default().fg(Color::Yellow)),
                            Span::raw(" "),
                            Span::styled(commit.id.clone(), Style::default().fg(Color::Yellow)),
                            Span::raw(" "),
                            Span::styled(commit.author.clone(), Style::default().fg(Color::Cyan)),
                            Span::raw(" "),
                            Span::styled(
                                commit.created_at.clone(),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                        self.main_view_content.push(Line::from(vec![
                            Span::raw("  "),
                            Span::raw(commit.message.clone()),
                        ]));
                        self.main_view_content.push(Line::from(""));
                    }
                }

                if let Some(statuses) = &self.upstream_integration_status {
                    self.main_view_content.push(Line::from(vec![Span::styled(
                        "Local Branch Status",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )]));
                    self.main_view_content.push(Line::from(""));

                    match statuses {
                        StackStatuses::UpToDate => {
                            self.main_view_content.push(Line::from(vec![Span::styled(
                                "All applied branches are up to date",
                                Style::default().fg(Color::Green),
                            )]));
                        }
                        StackStatuses::UpdatesRequired {
                            worktree_conflicts,
                            statuses,
                        } => {
                            if !worktree_conflicts.is_empty() {
                                self.main_view_content.push(Line::from(vec![Span::styled(
                                    "Uncommitted worktree changes may conflict with updates",
                                    Style::default().fg(Color::Red),
                                )]));
                                self.main_view_content.push(Line::from(""));
                            }

                            if statuses.is_empty() {
                                self.main_view_content.push(Line::from(vec![Span::styled(
                                    "No active branches require updates",
                                    Style::default().fg(Color::DarkGray),
                                )]));
                            }

                            for (maybe_stack_id, stack_status) in statuses {
                                let stack_name = maybe_stack_id
                                    .and_then(|id| {
                                        self.stacks
                                            .iter()
                                            .find(|stack| stack.id == Some(id))
                                            .map(|stack| stack.name.clone())
                                    })
                                    .unwrap_or_else(|| "Workspace".to_string());

                                self.main_view_content.push(Line::from(vec![Span::styled(
                                    format!("Stack: {}", stack_name),
                                    Style::default().add_modifier(Modifier::BOLD),
                                )]));

                                for branch_status in &stack_status.branch_statuses {
                                    let (icon, label, color) =
                                        Self::describe_branch_status(&branch_status.status);
                                    self.main_view_content.push(Line::from(vec![
                                        Span::raw("  "),
                                        Span::styled(icon, Style::default().fg(color)),
                                        Span::raw(" "),
                                        Span::styled(
                                            branch_status.name.clone(),
                                            Style::default().fg(Color::White),
                                        ),
                                        Span::raw(": "),
                                        Span::styled(label, Style::default().fg(color)),
                                    ]));
                                }

                                self.main_view_content.push(Line::from(""));
                            }
                        }
                    }
                }
            }
            Panel::Oplog => {
                if let Some(idx) = self.oplog_state.selected() {
                    if let Some(entry) = self.oplog_entries.get(idx) {
                        self.main_view_content
                            .push(Line::from(format!("Oplog Entry: {}", entry.id)));
                        self.main_view_content.push(Line::from(""));
                        self.main_view_content
                            .push(Line::from(format!("Operation: {}", entry.operation)));
                        self.main_view_content
                            .push(Line::from(format!("Title: {}", entry.title)));
                        self.main_view_content
                            .push(Line::from(format!("Time: {}", entry.time)));
                        self.main_view_content.push(Line::from(""));
                        self.main_view_content.push(Line::from(vec![
                            Span::styled(
                                "Press 'r'",
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(
                                " to restore your workspace to this snapshot. This will overwrite",
                            ),
                        ]));
                        self.main_view_content.push(Line::from(
                            "current worktree changes, so make sure everything important is saved.",
                        ));
                    }
                }
            }
        }
    }

    pub(super) fn get_details_title(&self) -> String {
        match self.active_panel {
            Panel::Status => {
                if self.get_selected_branch().is_some() {
                    "Branch Details".to_string()
                } else if self.get_selected_commit().is_some() {
                    "Commit Details".to_string()
                } else if self.is_unassigned_header_selected() {
                    "Unassigned Files".to_string()
                } else if self.get_selected_file().is_some() {
                    "File Changes".to_string()
                } else {
                    "Details".to_string()
                }
            }
            Panel::Upstream => "Upstream Commits".to_string(),
            Panel::Oplog => "Oplog Entry".to_string(),
        }
    }

    fn open_command_context(project: &Project) -> Option<CommandContext> {
        let settings = AppSettings::load_from_default_path_creating().ok()?;
        CommandContext::open(project, settings).ok()
    }

    fn refresh_upstream_statuses(&mut self, ctx: &CommandContext) {
        let review_map: HashMap<String, ForgeReview> = HashMap::new();
        match upstream_integration_statuses(ctx, None, &review_map) {
            Ok(statuses) => {
                self.upstream_integration_status = Some(statuses);
            }
            Err(e) => {
                self.command_log
                    .push(format!("Failed to compute upstream status: {}", e));
                self.upstream_integration_status = None;
            }
        }
    }

    pub(super) fn perform_upstream_update(&mut self) -> Result<()> {
        let project = gitbutler_project::get(self.project_id)?;
        let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
        let review_map: HashMap<String, ForgeReview> = HashMap::new();
        let status = upstream_integration_statuses(&ctx, None, &review_map)?;

        match status {
            StackStatuses::UpToDate => {
                self.command_log
                    .push("Branches are already up to date".to_string());
            }
            StackStatuses::UpdatesRequired {
                worktree_conflicts,
                statuses,
            } => {
                if !worktree_conflicts.is_empty() {
                    self.command_log.push(
                        "Cannot update: uncommitted worktree changes would conflict".to_string(),
                    );
                    return Err(anyhow!("Worktree conflicts prevent update"));
                }

                let mut resolutions = Vec::new();
                for (maybe_stack_id, stack_status) in statuses {
                    let Some(stack_id) = maybe_stack_id else {
                        self.command_log
                            .push("Skipping stack without identifier during update".to_string());
                        continue;
                    };
                    let all_integrated = stack_status
                        .branch_statuses
                        .iter()
                        .all(|s| matches!(s.status, UpstreamBranchStatus::Integrated));
                    let approach = if all_integrated
                        && stack_status.tree_status != gitbutler_branch_actions::upstream_integration::TreeStatus::Conflicted
                    {
                        ResolutionApproach::Delete
                    } else {
                        ResolutionApproach::Rebase
                    };
                    resolutions.push(Resolution {
                        stack_id,
                        approach,
                        delete_integrated_branches: true,
                    });
                }

                if resolutions.is_empty() {
                    self.command_log
                        .push("No branches require updating".to_string());
                    return Ok(());
                }

                integrate_upstream(&ctx, &resolutions, None, &review_map)?;
                self.command_log
                    .push("Updated applied branches from upstream".to_string());

                self.load_data_with_project(&project)?;
            }
        }

        Ok(())
    }

    pub(super) fn open_upstream_update_modal(&mut self) {
        if !matches!(self.active_panel, Panel::Upstream) {
            self.command_log
                .push("Switch to the Upstream panel to update branches".to_string());
            return;
        }

        self.show_update_modal = true;
        self.command_log
            .push("Preparing to rebase applied branches onto upstream".to_string());
    }
}

pub fn run(project: &Project) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = LazyApp::new(project)?;

    // Run main loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut LazyApp,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if app.should_quit {
            break;
        }

        // Check if we need to auto-refresh (every 10 seconds)
        if app.last_refresh.elapsed() >= Duration::from_secs(10) {
            if let Ok(project) = gitbutler_project::get(app.project_id) {
                let _ = app.load_data_with_project(&project);
                app.update_main_view();
                app.last_refresh = std::time::Instant::now();
            }
        }

        // Check if we need to auto-fetch (every 5 minutes)
        if app.last_fetch.elapsed() >= Duration::from_secs(300) {
            app.command_log
                .push("but_api::legacy::virtual_branches::fetch_from_remotes() [auto]".to_string());
            match but_api::legacy::virtual_branches::fetch_from_remotes(
                app.project_id,
                Some("auto-refresh".to_string()),
            ) {
                Ok(base_branch) => {
                    if base_branch.behind > 0 {
                        app.command_log.push(format!(
                            "Auto-fetch completed: {} new commits",
                            base_branch.behind
                        ));
                    } else {
                        app.command_log
                            .push("Auto-fetch completed: up to date".to_string());
                    }
                    // Reload data after fetch to show new upstream commits
                    if let Ok(project) = gitbutler_project::get(app.project_id) {
                        let _ = app.load_data_with_project(&project);
                        app.update_main_view();
                    }
                }
                Err(e) => {
                    app.command_log.push(format!("Auto-fetch failed: {}", e));
                }
            }
            app.last_fetch = std::time::Instant::now();
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    app.handle_input(key.code, key.modifiers);
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                _ => {}
            }
        }
    }

    Ok(())
}
