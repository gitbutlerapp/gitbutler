use std::collections::BTreeMap;

use bstr::BString;
use but_core::HunkHeader;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use super::diff_viewer::{DiffLine, parse_hunk_assignment_to_lines};

/// A single hunk with selection state and assignment data for staging.
pub(crate) struct StageHunkEntry {
    pub hunk_header: Option<HunkHeader>,
    pub path_bytes: BString,
    pub selected: bool,
    pub diff_lines: Vec<DiffLine>,
}

/// A file containing selectable hunks.
pub(crate) struct StageFileEntry {
    pub path: String,
    pub hunks: Vec<StageHunkEntry>,
}

#[derive(PartialEq)]
enum CheckState {
    All,
    Some,
    None,
}

impl StageFileEntry {
    fn check_state(&self) -> CheckState {
        let selected = self.hunks.iter().filter(|h| h.selected).count();
        if selected == self.hunks.len() {
            CheckState::All
        } else if selected > 0 {
            CheckState::Some
        } else {
            CheckState::None
        }
    }

    fn toggle_all(&mut self) {
        let new_state = self.check_state() != CheckState::All;
        for hunk in &mut self.hunks {
            hunk.selected = new_state;
        }
    }

    pub fn from_worktree(id_map: &crate::IdMap) -> Vec<StageFileEntry> {
        let mut by_path: BTreeMap<String, Vec<&but_hunk_assignment::HunkAssignment>> = BTreeMap::new();
        for uncommitted_hunk in id_map.uncommitted_hunks.values() {
            let a = &uncommitted_hunk.hunk_assignment;
            by_path.entry(a.path.clone()).or_default().push(a);
        }

        by_path
            .into_iter()
            .map(|(path, assignments)| {
                let hunks = assignments
                    .into_iter()
                    .map(|a| StageHunkEntry {
                        hunk_header: a.hunk_header,
                        path_bytes: a.path_bytes.clone(),
                        selected: true,
                        diff_lines: parse_hunk_assignment_to_lines(a),
                    })
                    .collect();
                StageFileEntry { path, hunks }
            })
            .collect()
    }
}

// --- TUI App ---

#[derive(PartialEq)]
enum Pane {
    FileList,
    DiffView,
}

/// The result of the TUI interaction.
pub(crate) enum StageResult {
    /// User confirmed staging.
    Stage {
        /// Hunks to assign to the target branch.
        selected: Vec<(Option<HunkHeader>, BString)>,
        /// Hunks to explicitly unassign (set to no branch).
        unselected: Vec<(Option<HunkHeader>, BString)>,
    },
    /// User quit without staging.
    Cancelled,
}

struct StageViewerApp {
    files: Vec<StageFileEntry>,
    list_state: ListState,
    diff_scroll: u16,
    active_pane: Pane,
    should_quit: bool,
    should_stage: bool,
    file_list_area: Rect,
    /// Which hunk is focused in the diff view (for per-hunk toggling).
    focused_hunk_index: usize,
    /// Cumulative line offsets for each hunk (for mapping scroll position to hunk index).
    hunk_line_offsets: Vec<u16>,
    branch_name: String,
}

impl StageViewerApp {
    fn new(files: Vec<StageFileEntry>, branch_name: String) -> Self {
        let mut list_state = ListState::default();
        if !files.is_empty() {
            list_state.select(Some(0));
        }
        let hunk_line_offsets = Self::compute_hunk_offsets_for(&files, 0);
        Self {
            files,
            list_state,
            diff_scroll: 0,
            active_pane: Pane::FileList,
            should_quit: false,
            should_stage: false,
            file_list_area: Rect::default(),
            focused_hunk_index: 0,
            hunk_line_offsets,
            branch_name,
        }
    }

    fn compute_hunk_offsets_for(files: &[StageFileEntry], file_idx: usize) -> Vec<u16> {
        let mut offsets = Vec::new();
        if let Some(file) = files.get(file_idx) {
            let mut offset: u16 = 0;
            for hunk in &file.hunks {
                offsets.push(offset);
                // 1 line for the rendered selection header, plus non-HunkHeader diff lines
                // (HunkHeader lines are skipped in rendering since we render our own header)
                let content_lines = hunk
                    .diff_lines
                    .iter()
                    .filter(|dl| !matches!(dl, DiffLine::HunkHeader(_)))
                    .count();
                offset = offset.saturating_add(1 + content_lines as u16);
            }
        }
        offsets
    }

    fn update_hunk_offsets(&mut self) {
        let idx = self.list_state.selected().unwrap_or(0);
        self.hunk_line_offsets = Self::compute_hunk_offsets_for(&self.files, idx);
    }

    fn selected_file(&self) -> Option<usize> {
        self.list_state.selected()
    }

    fn next_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = self
            .selected_file()
            .map_or(0, |i| if i + 1 < self.files.len() { i + 1 } else { i });
        self.list_state.select(Some(i));
        self.diff_scroll = 0;
        self.focused_hunk_index = 0;
        self.update_hunk_offsets();
    }

    fn prev_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = self.selected_file().map_or(0, |i| i.saturating_sub(1));
        self.list_state.select(Some(i));
        self.diff_scroll = 0;
        self.focused_hunk_index = 0;
        self.update_hunk_offsets();
    }

    fn scroll_down(&mut self, amount: u16) {
        self.diff_scroll = self.diff_scroll.saturating_add(amount);
    }

    fn scroll_up(&mut self, amount: u16) {
        self.diff_scroll = self.diff_scroll.saturating_sub(amount);
    }

    /// Scroll so that the focused hunk header is at the top of the diff view.
    fn scroll_to_focused_hunk(&mut self) {
        if let Some(&offset) = self.hunk_line_offsets.get(self.focused_hunk_index) {
            self.diff_scroll = offset;
        }
    }

    fn next_hunk(&mut self) {
        if let Some(file_idx) = self.selected_file() {
            let hunk_count = self.files[file_idx].hunks.len();
            if self.focused_hunk_index + 1 < hunk_count {
                // Next hunk in same file
                self.focused_hunk_index += 1;
                self.scroll_to_focused_hunk();
            } else if file_idx + 1 < self.files.len() {
                // Jump to first hunk of next file
                self.list_state.select(Some(file_idx + 1));
                self.focused_hunk_index = 0;
                self.diff_scroll = 0;
                self.update_hunk_offsets();
            }
        }
    }

    fn prev_hunk(&mut self) {
        if self.focused_hunk_index > 0 {
            // Previous hunk in same file
            self.focused_hunk_index -= 1;
            self.scroll_to_focused_hunk();
        } else if let Some(file_idx) = self.selected_file()
            && file_idx > 0
        {
            // Jump to last hunk of previous file
            self.list_state.select(Some(file_idx - 1));
            self.update_hunk_offsets();
            let prev_hunk_count = self.files[file_idx - 1].hunks.len();
            self.focused_hunk_index = prev_hunk_count.saturating_sub(1);
            self.scroll_to_focused_hunk();
        }
    }

    fn toggle_current_file(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            self.files[idx].toggle_all();
        }
    }

    fn toggle_focused_hunk(&mut self) {
        if let Some(file_idx) = self.list_state.selected() {
            let file = &mut self.files[file_idx];
            if self.focused_hunk_index < file.hunks.len() {
                file.hunks[self.focused_hunk_index].selected = !file.hunks[self.focused_hunk_index].selected;
            }
        }
    }

    fn select_all(&mut self) {
        for file in &mut self.files {
            for hunk in &mut file.hunks {
                hunk.selected = true;
            }
        }
    }

    fn deselect_all(&mut self) {
        for file in &mut self.files {
            for hunk in &mut file.hunks {
                hunk.selected = false;
            }
        }
    }

    fn collect_selected(&self) -> Vec<(Option<HunkHeader>, BString)> {
        self.files
            .iter()
            .flat_map(|f| {
                f.hunks
                    .iter()
                    .filter(|h| h.selected)
                    .map(|h| (h.hunk_header, h.path_bytes.clone()))
            })
            .collect()
    }

    fn collect_unselected(&self) -> Vec<(Option<HunkHeader>, BString)> {
        self.files
            .iter()
            .flat_map(|f| {
                f.hunks
                    .iter()
                    .filter(|h| !h.selected)
                    .map(|h| (h.hunk_header, h.path_bytes.clone()))
            })
            .collect()
    }

    fn total_hunks(&self) -> usize {
        self.files.iter().map(|f| f.hunks.len()).sum()
    }

    fn selected_hunks(&self) -> usize {
        self.files
            .iter()
            .map(|f| f.hunks.iter().filter(|h| h.selected).count())
            .sum()
    }

    fn selected_files(&self) -> usize {
        self.files.iter().filter(|f| f.hunks.iter().any(|h| h.selected)).count()
    }

    fn handle_input(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                    KeyCode::Char('s') | KeyCode::Enter => self.should_stage = true,
                    KeyCode::Char(' ') => match self.active_pane {
                        Pane::FileList => self.toggle_current_file(),
                        Pane::DiffView => self.toggle_focused_hunk(),
                    },
                    KeyCode::Char('a') => self.select_all(),
                    KeyCode::Char('n') => self.deselect_all(),
                    KeyCode::Up | KeyCode::Char('k') => match self.active_pane {
                        Pane::FileList => self.prev_file(),
                        Pane::DiffView => self.prev_hunk(),
                    },
                    KeyCode::Down | KeyCode::Char('j') => match self.active_pane {
                        Pane::FileList => self.next_file(),
                        Pane::DiffView => self.next_hunk(),
                    },
                    KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') | KeyCode::Tab => {
                        self.active_pane = match self.active_pane {
                            Pane::FileList => Pane::DiffView,
                            Pane::DiffView => Pane::FileList,
                        };
                    }
                    KeyCode::PageDown => self.scroll_down(20),
                    KeyCode::PageUp => self.scroll_up(20),
                    _ => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let area = self.file_list_area;
                    if mouse.column > area.x
                        && mouse.column < area.x + area.width.saturating_sub(1)
                        && mouse.row > area.y
                        && mouse.row < area.y + area.height.saturating_sub(1)
                    {
                        let visible_index = (mouse.row - area.y - 1) as usize;
                        let clicked_index = visible_index.saturating_add(self.list_state.offset());
                        if clicked_index < self.files.len() {
                            self.list_state.select(Some(clicked_index));
                            self.diff_scroll = 0;
                            self.focused_hunk_index = 0;
                            self.update_hunk_offsets();
                            self.active_pane = Pane::FileList;
                        }
                    }
                }
                MouseEventKind::ScrollDown => self.scroll_down(3),
                MouseEventKind::ScrollUp => self.scroll_up(3),
                _ => {}
            },
            _ => {}
        }
    }
}

fn ui(frame: &mut ratatui::Frame, app: &mut StageViewerApp) {
    // Vertical split: main area + status line + help footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)])
        .split(frame.area());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(outer[0]);

    // Left pane: file list with checkboxes
    let file_items: Vec<ListItem> = app
        .files
        .iter()
        .map(|f| {
            let check = match f.check_state() {
                CheckState::All => Span::styled("[x] ", Style::default().fg(Color::Green)),
                CheckState::Some => Span::styled("[-] ", Style::default().fg(Color::Yellow)),
                CheckState::None => Span::styled("[ ] ", Style::default().fg(Color::DarkGray)),
            };
            let path = Span::styled(f.path.clone(), Style::default());
            ListItem::new(Line::from(vec![check, path]))
        })
        .collect();

    let file_border_style = if app.active_pane == Pane::FileList {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let file_list = List::new(file_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Files ")
                .border_style(file_border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    app.file_list_area = chunks[0];
    frame.render_stateful_widget(file_list, chunks[0], &mut app.list_state);

    // Right pane: diff view with hunk selection indicators
    let diff_lines: Vec<Line> = if let Some(idx) = app.selected_file() {
        let file = &app.files[idx];
        let mut lines = Vec::new();
        for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
            // Hunk selection header
            let check = if hunk.selected { "[x]" } else { "[ ]" };
            let check_color = if hunk.selected { Color::Green } else { Color::DarkGray };
            let is_focused = app.active_pane == Pane::DiffView && hunk_idx == app.focused_hunk_index;

            // Find the hunk header text from diff_lines
            let header_text = hunk
                .diff_lines
                .iter()
                .find_map(|dl| {
                    if let DiffLine::HunkHeader(text) = dl {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            let header_style = if is_focused {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{check} "), Style::default().fg(check_color)),
                Span::styled(header_text, header_style),
            ]));

            // Render diff lines (skip HunkHeader since we rendered it above)
            // Dim colors when hunk is not selected
            let dim = !hunk.selected;
            for dl in &hunk.diff_lines {
                match dl {
                    DiffLine::HunkHeader(_) => {} // already rendered
                    DiffLine::Added { line_num, content } => {
                        let style = if dim {
                            Style::default().fg(Color::LightBlue)
                        } else {
                            Style::default().fg(Color::Green)
                        };
                        lines.push(Line::from(Span::styled(format!("{line_num:>5} +{content}"), style)));
                    }
                    DiffLine::Removed { line_num, content } => {
                        let style = if dim {
                            Style::default().fg(Color::LightBlue)
                        } else {
                            Style::default().fg(Color::Red)
                        };
                        lines.push(Line::from(Span::styled(format!("{line_num:>5} -{content}"), style)));
                    }
                    DiffLine::Context {
                        old_num,
                        new_num,
                        content,
                    } => {
                        lines.push(Line::from(Span::styled(
                            format!("{old_num:>5} {new_num:>5}  {content}"),
                            Style::default().fg(Color::DarkGray),
                        )));
                    }
                    DiffLine::Info(text) => {
                        lines.push(Line::from(Span::styled(
                            format!("  {text}"),
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
                        )));
                    }
                }
            }
        }
        lines
    } else {
        vec![Line::from(Span::styled(
            "No file selected",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let diff_title = format!(" Stage to [{}] ", app.branch_name);

    let diff_border_style = if app.active_pane == Pane::DiffView {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let diff_view = Paragraph::new(diff_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(diff_title)
                .border_style(diff_border_style),
        )
        .scroll((app.diff_scroll, 0));

    frame.render_widget(diff_view, chunks[1]);

    // Status line
    let selected = app.selected_hunks();
    let total = app.total_hunks();
    let file_count = app.selected_files();
    let status = Line::from(vec![Span::styled(
        format!(" {selected}/{total} hunks selected across {file_count} file(s)"),
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )]);
    frame.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray)),
        outer[1],
    );

    // Help footer
    let help = Line::from(vec![
        Span::styled(" j/k", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("h/l/Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" pane  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Space", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" toggle  ", Style::default().fg(Color::DarkGray)),
        Span::styled("a", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" all  ", Style::default().fg(Color::DarkGray)),
        Span::styled("n", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" none  ", Style::default().fg(Color::DarkGray)),
        Span::styled("s/Enter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" stage  ", Style::default().fg(Color::DarkGray)),
        Span::styled("q", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" quit", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(help), outer[2]);
}

pub(crate) fn run_stage_viewer(files: Vec<StageFileEntry>, branch_name: &str) -> anyhow::Result<StageResult> {
    let mut guard = super::TerminalGuard::new(true)?;
    let mut app = StageViewerApp::new(files, branch_name.to_string());

    loop {
        guard.terminal_mut().draw(|frame| ui(frame, &mut app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
            app.handle_input(ev);
        }

        if app.should_quit || app.should_stage {
            break;
        }
    }

    drop(guard);

    if app.should_stage {
        Ok(StageResult::Stage {
            selected: app.collect_selected(),
            unselected: app.collect_unselected(),
        })
    } else {
        Ok(StageResult::Cancelled)
    }
}

// --- Branch Selector ---

/// Simple TUI to pick a branch from a list.
/// Returns None if the user cancels (q/Esc).
pub(crate) fn run_branch_selector(branches: &[String]) -> anyhow::Result<Option<String>> {
    let mut guard = super::TerminalGuard::new(false)?;

    let mut list_state = ListState::default();
    if !branches.is_empty() {
        list_state.select(Some(0));
    }
    let mut selected: Option<String> = None;
    let mut quit = false;

    loop {
        guard.terminal_mut().draw(|frame| {
            let items: Vec<ListItem> = branches
                .iter()
                .map(|b| ListItem::new(Line::from(Span::raw(b.clone()))))
                .collect();

            let area = frame.area();
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Select branch to stage to ")
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");

            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(area);

            frame.render_stateful_widget(list, layout[0], &mut list_state);

            let help = Line::from(vec![
                Span::styled(" j/k", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Enter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(" select  ", Style::default().fg(Color::DarkGray)),
                Span::styled("q", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
            ]);
            frame.render_widget(Paragraph::new(help), layout[1]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    quit = true;
                    break;
                }
                KeyCode::Enter => {
                    if let Some(idx) = list_state.selected() {
                        selected = Some(branches[idx].clone());
                    }
                    break;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = list_state.selected().map_or(0, |i| i.saturating_sub(1));
                    list_state.select(Some(i));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = list_state
                        .selected()
                        .map_or(0, |i| if i + 1 < branches.len() { i + 1 } else { i });
                    list_state.select(Some(i));
                }
                _ => {}
            }
        }
    }

    drop(guard);

    if quit { Ok(None) } else { Ok(selected) }
}
