use std::collections::BTreeMap;

use bstr::{BString, ByteSlice};
use but_core::unified_diff::DiffHunk;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::id::UncommittedCliId;

/// A single file's diff information for TUI display.
pub(crate) struct DiffFileEntry {
    pub path: String,
    pub status: char,
    pub diff_lines: Vec<DiffLine>,
}

/// A single line within a diff, pre-parsed for rendering.
pub(crate) enum DiffLine {
    HunkHeader(String),
    Added {
        line_num: u32,
        content: String,
    },
    Removed {
        line_num: u32,
        content: String,
    },
    Context {
        old_num: u32,
        new_num: u32,
        content: String,
    },
    Info(String),
}

/// Filter for worktree diffs, mirroring the non-TUI filter logic.
pub(crate) enum WorktreeFilter {
    Unassigned,
    Uncommitted(Box<UncommittedCliId>),
    Stack(gitbutler_stack::StackId),
}

impl DiffFileEntry {
    pub fn from_worktree(id_map: &crate::IdMap, filter: Option<&WorktreeFilter>) -> Vec<DiffFileEntry> {
        // Group hunks by path, applying filter
        let mut by_path: BTreeMap<String, Vec<&but_hunk_assignment::HunkAssignment>> = BTreeMap::new();
        for uncommitted_hunk in id_map.uncommitted_hunks.values() {
            let a = &uncommitted_hunk.hunk_assignment;
            let include = match filter {
                None => true,
                Some(WorktreeFilter::Unassigned) => a.stack_id.is_none(),
                Some(WorktreeFilter::Uncommitted(id)) => {
                    if id.is_entire_file {
                        a.path_bytes == id.hunk_assignments.first().path_bytes
                    } else {
                        a.eq(id.hunk_assignments.first())
                    }
                }
                Some(WorktreeFilter::Stack(stack_id)) => a.stack_id.as_ref() == Some(stack_id),
            };
            if include {
                by_path.entry(a.path.clone()).or_default().push(a);
            }
        }

        by_path
            .into_iter()
            .map(|(path, assignments)| {
                let mut diff_lines = Vec::new();
                for assignment in &assignments {
                    diff_lines.extend(parse_hunk_assignment_to_lines(assignment));
                }
                DiffFileEntry {
                    path,
                    status: 'M',
                    diff_lines,
                }
            })
            .collect()
    }

    pub fn from_commit(
        ctx: &mut but_ctx::Context,
        commit_id: gix::ObjectId,
        path_filter: Option<BString>,
    ) -> anyhow::Result<Vec<DiffFileEntry>> {
        let result = but_api::diff::commit_details(ctx, commit_id, but_api::diff::ComputeLineStats::No)?;

        result
            .diff_with_first_parent
            .into_iter()
            .filter(|change| path_filter.as_ref().is_none_or(|p| p == &change.path))
            .map(|change| {
                // Convert but_core::TreeChange -> but_core::ui::TreeChange
                let ui_change: but_core::ui::TreeChange = change.into();
                let status = status_char(&ui_change.status);
                let path = ui_change.path_bytes.to_string();
                let patch = but_api::legacy::diff::tree_change_diffs(ctx, ui_change).ok().flatten();
                let diff_lines = match patch {
                    Some(p) => parse_unified_patch(&p),
                    None => vec![DiffLine::Info("(no diff available)".to_string())],
                };
                Ok(DiffFileEntry {
                    path,
                    status,
                    diff_lines,
                })
            })
            .collect()
    }

    pub fn from_branch(ctx: &but_ctx::Context, short_name: String) -> anyhow::Result<Vec<DiffFileEntry>> {
        let result = but_api::branch::branch_diff(ctx, short_name)?;

        result
            .changes
            .into_iter()
            .map(|change| {
                let status = status_char(&change.status);
                let path = change.path_bytes.to_string();
                let patch = but_api::legacy::diff::tree_change_diffs(ctx, change).ok().flatten();
                let diff_lines = match patch {
                    Some(p) => parse_unified_patch(&p),
                    None => vec![DiffLine::Info("(no diff available)".to_string())],
                };
                Ok(DiffFileEntry {
                    path,
                    status,
                    diff_lines,
                })
            })
            .collect()
    }
}

fn status_char(status: &but_core::ui::TreeStatus) -> char {
    match status {
        but_core::ui::TreeStatus::Addition { .. } => 'A',
        but_core::ui::TreeStatus::Deletion { .. } => 'D',
        but_core::ui::TreeStatus::Modification { .. } => 'M',
        but_core::ui::TreeStatus::Rename { .. } => 'R',
    }
}

fn parse_unified_patch(patch: &but_core::UnifiedPatch) -> Vec<DiffLine> {
    match patch {
        but_core::UnifiedPatch::Binary => {
            vec![DiffLine::Info("Binary file - no diff available".to_string())]
        }
        but_core::UnifiedPatch::TooLarge { size_in_bytes } => {
            vec![DiffLine::Info(format!(
                "File too large ({size_in_bytes} bytes) - no diff available"
            ))]
        }
        but_core::UnifiedPatch::Patch {
            hunks,
            is_result_of_binary_to_text_conversion,
            ..
        } => {
            let mut lines = Vec::new();
            if *is_result_of_binary_to_text_conversion {
                lines.push(DiffLine::Info(
                    "(diff generated from binary-to-text conversion)".to_string(),
                ));
            }
            for hunk in hunks {
                lines.extend(parse_hunk_to_lines(hunk));
            }
            lines
        }
    }
}

pub(crate) fn parse_hunk_assignment_to_lines(assignment: &but_hunk_assignment::HunkAssignment) -> Vec<DiffLine> {
    if let (Some(diff), Some(header)) = (&assignment.diff, &assignment.hunk_header) {
        let hunk = DiffHunk {
            old_start: header.old_start,
            old_lines: header.old_lines,
            new_start: header.new_start,
            new_lines: header.new_lines,
            diff: diff.clone(),
        };
        parse_hunk_to_lines(&hunk)
    } else {
        vec![DiffLine::Info("(no detailed diff available)".to_string())]
    }
}

fn parse_hunk_to_lines(hunk: &DiffHunk) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut old_line = hunk.old_start;
    let mut new_line = hunk.new_start;

    lines.push(DiffLine::HunkHeader(format!(
        "@@ -{},{} +{},{} @@",
        hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
    )));

    for line in hunk.diff.lines() {
        if line.is_empty() || line.starts_with(b"@@") {
            continue;
        }

        if let Some(rest) = line.strip_prefix(b"+") {
            lines.push(DiffLine::Added {
                line_num: new_line,
                content: rest.to_str_lossy().into_owned(),
            });
            new_line += 1;
        } else if let Some(rest) = line.strip_prefix(b"-") {
            lines.push(DiffLine::Removed {
                line_num: old_line,
                content: rest.to_str_lossy().into_owned(),
            });
            old_line += 1;
        } else {
            let content = line.strip_prefix(b" ").unwrap_or(line);
            lines.push(DiffLine::Context {
                old_num: old_line,
                new_num: new_line,
                content: content.to_str_lossy().into_owned(),
            });
            old_line += 1;
            new_line += 1;
        }
    }

    lines
}

// --- TUI App ---

#[derive(PartialEq)]
enum Pane {
    FileList,
    DiffView,
}

struct DiffViewerApp {
    files: Vec<DiffFileEntry>,
    list_state: ListState,
    diff_scroll: u16,
    active_pane: Pane,
    should_quit: bool,
    file_list_area: Rect,
}

impl DiffViewerApp {
    fn new(files: Vec<DiffFileEntry>) -> Self {
        let mut list_state = ListState::default();
        if !files.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            files,
            list_state,
            diff_scroll: 0,
            active_pane: Pane::FileList,
            should_quit: false,
            file_list_area: Rect::default(),
        }
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
    }

    fn prev_file(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = self.selected_file().map_or(0, |i| i.saturating_sub(1));
        self.list_state.select(Some(i));
        self.diff_scroll = 0;
    }

    fn scroll_down(&mut self, amount: u16) {
        self.diff_scroll = self.diff_scroll.saturating_add(amount);
    }

    fn scroll_up(&mut self, amount: u16) {
        self.diff_scroll = self.diff_scroll.saturating_sub(amount);
    }

    fn handle_input(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return;
                }
                match key.code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Up | KeyCode::Char('k') => match self.active_pane {
                        Pane::FileList => self.prev_file(),
                        Pane::DiffView => self.scroll_up(1),
                    },
                    KeyCode::Down | KeyCode::Char('j') => match self.active_pane {
                        Pane::FileList => self.next_file(),
                        Pane::DiffView => self.scroll_down(1),
                    },
                    KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') | KeyCode::Tab => {
                        self.active_pane = match self.active_pane {
                            Pane::FileList => Pane::DiffView,
                            Pane::DiffView => Pane::FileList,
                        };
                    }
                    KeyCode::Char(' ') if self.active_pane == Pane::DiffView => self.scroll_down(20),
                    KeyCode::PageDown => self.scroll_down(20),
                    KeyCode::PageUp => self.scroll_up(20),
                    _ => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let area = self.file_list_area;
                    // Check if click is inside the file list area (accounting for border)
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

fn ui(frame: &mut ratatui::Frame, app: &mut DiffViewerApp) {
    // Vertical split: main area + help footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(outer[0]);

    // Left pane: file list
    let file_items: Vec<ListItem> = app
        .files
        .iter()
        .map(|f| {
            let style = match f.status {
                'A' => Style::default().fg(Color::Green),
                'D' => Style::default().fg(Color::Red),
                'R' => Style::default().fg(Color::Yellow),
                _ => Style::default(),
            };
            ListItem::new(Line::from(Span::styled(format!("{} {}", f.status, f.path), style)))
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

    // Right pane: diff view
    let diff_lines: Vec<Line> = if let Some(idx) = app.selected_file() {
        app.files[idx]
            .diff_lines
            .iter()
            .map(|dl| match dl {
                DiffLine::HunkHeader(text) => Line::from(Span::styled(text.clone(), Style::default().fg(Color::Cyan))),
                DiffLine::Added { line_num, content } => Line::from(Span::styled(
                    format!("{line_num:>5} +{content}"),
                    Style::default().fg(Color::Green),
                )),
                DiffLine::Removed { line_num, content } => Line::from(Span::styled(
                    format!("{line_num:>5} -{content}"),
                    Style::default().fg(Color::Red),
                )),
                DiffLine::Context {
                    old_num,
                    new_num,
                    content,
                } => Line::from(vec![Span::styled(
                    format!("{old_num:>5} {new_num:>5}  {content}"),
                    Style::default().fg(Color::DarkGray),
                )]),
                DiffLine::Info(text) => Line::from(Span::styled(
                    format!("  {text}"),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
                )),
            })
            .collect()
    } else {
        vec![Line::from(Span::styled(
            "No file selected",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let diff_title = if let Some(idx) = app.selected_file() {
        format!(" {} ", app.files[idx].path)
    } else {
        " Diff ".to_string()
    };

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

    // Help footer
    let help = Line::from(vec![
        Span::styled(" j/k", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "h/l/←/→/Tab",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" switch pane  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Space/PgDn",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" page down  ", Style::default().fg(Color::DarkGray)),
        Span::styled("PgUp", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" page up  ", Style::default().fg(Color::DarkGray)),
        Span::styled("q", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" quit", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(help), outer[1]);
}

pub(crate) fn run_diff_viewer(files: Vec<DiffFileEntry>) -> anyhow::Result<()> {
    let mut guard = super::TerminalGuard::new(true)?;
    let mut app = DiffViewerApp::new(files);

    loop {
        guard.terminal_mut().draw(|frame| ui(frame, &mut app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
            app.handle_input(ev);
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
