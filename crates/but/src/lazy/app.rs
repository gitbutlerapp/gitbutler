use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use bstr::ByteSlice;
use but_ctx::Context;
use but_oxidize::OidExt;
use crossterm::event::{self, Event};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;

use crate::command::legacy::status::assignment::{self, CLIHunkAssignment, FileAssignment};

use super::render::ui;

// ---------------------------------------------------------------------------
// Panels
// ---------------------------------------------------------------------------

#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum Panel {
    Upstream,
    Status,
    Oplog,
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

pub(super) struct StackInfo {
    pub id: Option<gitbutler_stack::StackId>,
    pub branches: Vec<BranchInfo>,
}

#[derive(Clone)]
pub(super) struct BranchInfo {
    pub name: String,
    pub commits: Vec<CommitInfo>,
    pub files: Vec<FileAssignment>,
}

#[derive(Clone)]
pub(super) struct CommitInfo {
    pub id: String,
    pub full_id: String,
    pub message: String,
    pub author: String,
    pub created_at: String,
    pub state: but_workspace::ui::CommitState,
}

pub(super) struct UpstreamInfo {
    pub behind_count: usize,
    pub latest_commit: String,
    pub message: String,
    pub commit_date: String,
    pub last_fetched_ms: Option<u128>,
    pub commits: Vec<UpstreamCommit>,
}

pub(super) struct UpstreamCommit {
    pub id: String,
    pub message: String,
    pub author: String,
    pub created_at: String,
}

#[derive(Clone)]
pub(super) struct OplogEntry {
    pub id: String,
    pub full_id: String,
    pub operation: String,
    pub title: String,
    pub time: String,
}

// ---------------------------------------------------------------------------
// Commit modal types
// ---------------------------------------------------------------------------

#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum CommitModalFocus {
    BranchSelect,
    Files,
    Subject,
    Message,
    CommitButton,
}

pub(super) struct CommitBranchOption {
    pub stack_id: Option<but_core::ref_metadata::StackId>,
    pub branch_name: String,
    pub is_new_branch: bool,
}

pub(super) struct CommitFileOption {
    pub path: String,
    pub selected: bool,
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

pub(super) struct App {
    // Data
    pub unassigned_files: Vec<FileAssignment>,
    pub stacks: Vec<StackInfo>,
    pub oplog_entries: Vec<OplogEntry>,
    pub upstream_info: Option<UpstreamInfo>,

    // Navigation
    pub active_panel: Panel,
    pub status_state: ListState,
    pub oplog_state: ListState,
    pub details_scroll: u16,
    pub details_selected: bool,

    // Panel areas (for mouse click detection)
    pub upstream_area: Option<Rect>,
    pub status_area: Option<Rect>,
    pub oplog_area: Option<Rect>,
    pub details_area: Option<Rect>,

    // UI state
    pub should_quit: bool,
    pub show_help: bool,
    pub help_scroll: u16,
    pub command_log: Vec<String>,
    pub command_log_visible: bool,

    // Commit modal
    pub show_commit_modal: bool,
    pub commit_focus: CommitModalFocus,
    pub commit_branch_options: Vec<CommitBranchOption>,
    pub commit_selected_branch: usize,
    pub commit_files: Vec<CommitFileOption>,
    pub commit_file_cursor: usize,
    pub commit_subject: String,
    pub commit_message: String,
    pub commit_staged_only: bool,

    // Timers
    pub last_refresh: Instant,
    pub last_fetch: Instant,
}

impl App {
    pub fn new(ctx: &mut Context) -> Result<Self> {
        let now = Instant::now();
        let mut app = Self {
            unassigned_files: Vec::new(),
            stacks: Vec::new(),
            oplog_entries: Vec::new(),
            upstream_info: None,
            active_panel: Panel::Status,
            status_state: ListState::default(),
            oplog_state: ListState::default(),
            details_scroll: 0,
            details_selected: false,
            upstream_area: None,
            status_area: None,
            oplog_area: None,
            details_area: None,
            should_quit: false,
            show_help: false,
            help_scroll: 0,
            command_log: Vec::new(),
            command_log_visible: true,
            show_commit_modal: false,
            commit_focus: CommitModalFocus::BranchSelect,
            commit_branch_options: Vec::new(),
            commit_selected_branch: 0,
            commit_files: Vec::new(),
            commit_file_cursor: 0,
            commit_subject: String::new(),
            commit_message: String::new(),
            commit_staged_only: false,
            last_refresh: now,
            last_fetch: now,
        };

        app.load_data(ctx)?;
        app.command_log.push("TUI started".to_string());

        if app.count_status_items() > 0 {
            app.status_state.select(Some(0));
        }
        if !app.oplog_entries.is_empty() {
            app.oplog_state.select(Some(0));
        }

        Ok(app)
    }

    // ------------------------------------------------------------------
    // Data loading
    // ------------------------------------------------------------------

    pub fn load_data(&mut self, ctx: &mut Context) -> Result<()> {
        self.unassigned_files.clear();
        self.stacks.clear();
        self.oplog_entries.clear();

        // Load stacks
        self.command_log.push("workspace::stacks()".to_string());
        let stacks = but_api::legacy::workspace::stacks(ctx, None)?;

        // Load worktree changes for file assignments
        self.command_log
            .push("diff::changes_in_worktree()".to_string());
        let worktree_changes = but_api::legacy::diff::changes_in_worktree(ctx)?;

        // Build file assignments grouped by path
        let mut by_file: BTreeMap<bstr::BString, Vec<but_hunk_assignment::HunkAssignment>> =
            BTreeMap::new();
        for hunk_assignment in worktree_changes.assignments {
            by_file
                .entry(hunk_assignment.path_bytes.clone())
                .or_default()
                .push(hunk_assignment);
        }

        let mut assignments_by_file: BTreeMap<bstr::BString, FileAssignment> = BTreeMap::new();
        for (path, hunks) in &by_file {
            let file = FileAssignment {
                path: path.clone(),
                assignments: hunks
                    .iter()
                    .map(|h| CLIHunkAssignment {
                        inner: h.clone(),
                        cli_id: String::new(), // no CLI IDs needed for TUI
                    })
                    .collect(),
            };
            assignments_by_file.insert(path.clone(), file);
        }

        // Get unassigned files
        self.unassigned_files =
            assignment::filter_by_stack_id(assignments_by_file.values(), &None);

        // Load each stack's details
        for stack in stacks {
            self.command_log
                .push(format!("workspace::stack_details({:?})", stack.id));
            let details = but_api::legacy::workspace::stack_details(ctx, stack.id)?;
            let stack_files =
                assignment::filter_by_stack_id(assignments_by_file.values(), &stack.id);

            let mut branches = Vec::new();
            for (idx, branch) in details.branch_details.into_iter().enumerate() {
                let commits = branch
                    .commits
                    .iter()
                    .map(|c| {
                        let seconds = (c.created_at / 1000) as i64;
                        let dt =
                            chrono::DateTime::from_timestamp(seconds, 0).unwrap_or_default();
                        CommitInfo {
                            id: c.id.to_string()[..7].to_string(),
                            full_id: c.id.to_string(),
                            message: c.message.to_str().unwrap_or("").to_string(),
                            author: c.author.name.clone(),
                            created_at: dt.format("%Y-%m-%d %H:%M").to_string(),
                            state: c.state.clone(),
                        }
                    })
                    .collect();

                // First branch gets the stack files
                let files = if idx == 0 {
                    stack_files.clone()
                } else {
                    Vec::new()
                };

                branches.push(BranchInfo {
                    name: branch.name.to_string(),
                    commits,
                    files,
                });
            }

            self.stacks.push(StackInfo {
                id: stack.id,
                branches,
            });
        }

        // Load oplog
        self.command_log.push("oplog::list_snapshots()".to_string());
        let snapshots =
            but_api::legacy::oplog::list_snapshots(ctx, 50, None, None, None)?;
        for snapshot in snapshots {
            let operation = snapshot
                .details
                .as_ref()
                .map(|d| {
                    use gitbutler_oplog::entry::OperationKind::*;
                    match d.operation {
                        CreateCommit => "COMMIT",
                        CreateBranch => "BRANCH",
                        AmendCommit => "AMEND",
                        UndoCommit => "UNDO",
                        SquashCommit => "SQUASH",
                        UpdateCommitMessage => "REWORD",
                        MoveCommit => "MOVE",
                        RestoreFromSnapshot => "RESTORE",
                        ApplyBranch => "APPLY",
                        UnapplyBranch => "UNAPPLY",
                        _ => "OTHER",
                    }
                })
                .unwrap_or("UNKNOWN");

            let seconds = snapshot.created_at.seconds();
            let dt = chrono::DateTime::from_timestamp(seconds, 0).unwrap_or_default();
            let time_string = dt.format("%Y-%m-%d %H:%M:%S").to_string();

            let commit_id = snapshot.commit_id.to_string();
            self.oplog_entries.push(OplogEntry {
                id: commit_id[..7.min(commit_id.len())].to_string(),
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

        // Load upstream info
        self.load_upstream(ctx);

        Ok(())
    }

    fn load_upstream(&mut self, ctx: &mut Context) {
        use gix::date::time::CustomFormat;
        const DATE_ONLY: CustomFormat = CustomFormat::new("%Y-%m-%d");

        self.upstream_info = but_api::legacy::virtual_branches::get_base_branch_data(ctx)
            .ok()
            .flatten()
            .and_then(|base_branch| {
                if base_branch.behind == 0 {
                    return None;
                }

                let repo = ctx.repo.get().ok()?;
                let commit_obj = repo
                    .find_commit(base_branch.current_sha.to_gix())
                    .ok()?;
                let commit = commit_obj.decode().ok()?;
                let msg = commit
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
                    .format(DATE_ONLY)
                    .unwrap_or_default();

                let commits = base_branch
                    .upstream_commits
                    .iter()
                    .map(|uc| {
                        let seconds = (uc.created_at / 1000) as i64;
                        let dt =
                            chrono::DateTime::from_timestamp(seconds, 0).unwrap_or_default();
                        UpstreamCommit {
                            id: uc.id.to_string()[..7].to_string(),
                            message: uc.description.to_string(),
                            author: uc.author.name.clone(),
                            created_at: dt.format("%Y-%m-%d %H:%M").to_string(),
                        }
                    })
                    .collect();

                Some(UpstreamInfo {
                    behind_count: base_branch.behind,
                    latest_commit: base_branch.current_sha.to_string()[..7].to_string(),
                    message: msg,
                    commit_date: formatted_date,
                    last_fetched_ms: base_branch.last_fetched_ms,
                    commits,
                })
            });
    }

    // ------------------------------------------------------------------
    // Navigation helpers
    // ------------------------------------------------------------------

    pub fn count_status_items(&self) -> usize {
        let mut count = 0;
        if !self.unassigned_files.is_empty() {
            count += 1 + self.unassigned_files.len(); // header + files
            count += 1; // separator (┊ line)
        }
        for (si, stack) in self.stacks.iter().enumerate() {
            let has_staged = stack
                .branches
                .first()
                .map_or(false, |b| !b.files.is_empty());
            if has_staged {
                count += 1; // staged header
                count += stack.branches[0].files.len(); // staged files
            }
            for (bi, branch) in stack.branches.iter().enumerate() {
                count += 1; // branch header
                if !(bi == 0 && has_staged) {
                    count += branch.files.len(); // files (skip first branch if staged)
                }
                count += branch.commits.len();
            }
            if !stack.branches.is_empty() {
                count += 1; // stack footer (╰╯)
            }
            if si + 1 < self.stacks.len() {
                count += 1; // blank line between stacks
            }
        }
        count
    }

    pub fn is_separator(&self, idx: usize) -> bool {
        let mut pos = 0;
        if !self.unassigned_files.is_empty() {
            pos += 1 + self.unassigned_files.len();
            if idx == pos {
                return true; // separator after unassigned files
            }
            pos += 1;
        }
        for (si, stack) in self.stacks.iter().enumerate() {
            let has_staged = stack
                .branches
                .first()
                .map_or(false, |b| !b.files.is_empty());
            if has_staged {
                pos += 1; // staged header
                pos += stack.branches[0].files.len(); // staged files
            }
            for (bi, branch) in stack.branches.iter().enumerate() {
                pos += 1; // branch header
                if !(bi == 0 && has_staged) {
                    pos += branch.files.len();
                }
                pos += branch.commits.len();
            }
            if !stack.branches.is_empty() {
                if idx == pos {
                    return true; // stack footer (╰╯)
                }
                pos += 1;
            }
            if si + 1 < self.stacks.len() {
                if idx == pos {
                    return true; // blank line between stacks
                }
                pos += 1;
            }
        }
        false
    }

    pub fn next_panel(&mut self) {
        self.details_selected = false;
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

    pub fn prev_panel(&mut self) {
        self.details_selected = false;
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

    pub fn select_next(&mut self) {
        match self.active_panel {
            Panel::Status => {
                let total = self.count_status_items();
                if total == 0 {
                    return;
                }
                let current = self.status_state.selected().unwrap_or(0);
                let mut next = (current + 1).min(total.saturating_sub(1));
                while next < total && self.is_separator(next) {
                    next += 1;
                }
                if next >= total {
                    next = current;
                }
                self.status_state.select(Some(next));
            }
            Panel::Oplog => {
                let total = self.oplog_entries.len();
                if total == 0 {
                    return;
                }
                let current = self.oplog_state.selected().unwrap_or(0);
                self.oplog_state
                    .select(Some((current + 1).min(total.saturating_sub(1))));
            }
            Panel::Upstream => {}
        }
        self.details_scroll = 0;
    }

    pub fn select_prev(&mut self) {
        match self.active_panel {
            Panel::Status => {
                let current = self.status_state.selected().unwrap_or(0);
                if current == 0 {
                    return;
                }
                let mut prev = current - 1;
                while prev > 0 && self.is_separator(prev) {
                    prev -= 1;
                }
                if self.is_separator(prev) {
                    return; // stay put
                }
                self.status_state.select(Some(prev));
            }
            Panel::Oplog => {
                let current = self.oplog_state.selected().unwrap_or(0);
                if current > 0 {
                    self.oplog_state.select(Some(current - 1));
                }
            }
            Panel::Upstream => {}
        }
        self.details_scroll = 0;
    }

    pub fn refresh(&mut self, ctx: &mut Context) {
        let status_idx = self.status_state.selected();
        let oplog_idx = self.oplog_state.selected();

        if let Err(e) = self.load_data(ctx) {
            self.command_log.push(format!("Refresh error: {e}"));
            return;
        }

        if let Some(idx) = status_idx {
            let total = self.count_status_items();
            self.status_state
                .select(Some(if idx < total { idx } else { 0 }));
        }
        if let Some(idx) = oplog_idx {
            self.oplog_state.select(Some(
                if idx < self.oplog_entries.len() { idx } else { 0 },
            ));
        }

        self.command_log.push("Refreshed".to_string());
        self.last_refresh = Instant::now();
    }

    pub fn fetch_upstream(&mut self, ctx: &mut Context) {
        self.command_log.push("fetch_from_remotes()".to_string());

        match but_api::legacy::virtual_branches::fetch_from_remotes(
            ctx,
            Some("manual-fetch".to_string()),
        ) {
            Ok(base_branch) => {
                if base_branch.behind > 0 {
                    self.command_log
                        .push(format!("Fetched: {} new commits", base_branch.behind));
                } else {
                    self.command_log.push("Fetched: up to date".to_string());
                }
                self.refresh(ctx);
            }
            Err(e) => {
                self.command_log.push(format!("Fetch error: {e}"));
            }
        }
        self.last_fetch = Instant::now();
    }

    /// Identifies what the currently selected status item is.
    pub fn selected_status_item(&self) -> Option<StatusItem> {
        let idx = self.status_state.selected()?;
        let mut pos = 0;

        if !self.unassigned_files.is_empty() {
            if idx == pos {
                return Some(StatusItem::UnassignedHeader);
            }
            pos += 1;
            for (i, _) in self.unassigned_files.iter().enumerate() {
                if idx == pos {
                    return Some(StatusItem::UnassignedFile(i));
                }
                pos += 1;
            }
            pos += 1; // separator
        }

        for (si, stack) in self.stacks.iter().enumerate() {
            let has_staged = stack
                .branches
                .first()
                .map_or(false, |b| !b.files.is_empty());
            if has_staged {
                if idx == pos {
                    return Some(StatusItem::StagedHeader { stack: si });
                }
                pos += 1;
                for (fi, _) in stack.branches[0].files.iter().enumerate() {
                    if idx == pos {
                        return Some(StatusItem::AssignedFile {
                            stack: si,
                            branch: 0,
                            file: fi,
                        });
                    }
                    pos += 1;
                }
            }
            for (bi, branch) in stack.branches.iter().enumerate() {
                if idx == pos {
                    return Some(StatusItem::Branch {
                        stack: si,
                        branch: bi,
                    });
                }
                pos += 1;
                if !(bi == 0 && has_staged) {
                    for (fi, _) in branch.files.iter().enumerate() {
                        if idx == pos {
                            return Some(StatusItem::AssignedFile {
                                stack: si,
                                branch: bi,
                                file: fi,
                            });
                        }
                        pos += 1;
                    }
                }
                for (ci, _) in branch.commits.iter().enumerate() {
                    if idx == pos {
                        return Some(StatusItem::Commit {
                            stack: si,
                            branch: bi,
                            commit: ci,
                        });
                    }
                    pos += 1;
                }
            }
            if !stack.branches.is_empty() {
                pos += 1; // stack footer
            }
            if si + 1 < self.stacks.len() {
                pos += 1; // blank line between stacks
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
pub(super) enum StatusItem {
    UnassignedHeader,
    UnassignedFile(usize),
    StagedHeader {
        stack: usize,
    },
    Branch {
        stack: usize,
        branch: usize,
    },
    AssignedFile {
        stack: usize,
        branch: usize,
        file: usize,
    },
    Commit {
        stack: usize,
        branch: usize,
        commit: usize,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(ctx: &mut Context) -> Result<()> {
    let mut guard = crate::tui::TerminalGuard::new(true)?;
    let mut app = App::new(ctx)?;

    loop {
        guard.terminal_mut().draw(|f| ui(f, &app))?;

        if app.should_quit {
            break;
        }

        // Auto-refresh every 10s
        if app.last_refresh.elapsed() >= Duration::from_secs(10) {
            app.refresh(ctx);
        }

        // Auto-fetch every 5 min
        if app.last_fetch.elapsed() >= Duration::from_secs(300) {
            app.command_log.push("auto-fetch".to_string());
            app.fetch_upstream(ctx);
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key, ctx),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
    }

    Ok(())
}
