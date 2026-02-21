//! A built-in TUI text editor inspired by Microsoft's `edit`.
//!
//! Used as the fallback when no external editor (`GIT_EDITOR`, `core.editor`, `EDITOR`) is configured,
//! and also available directly via `but edit <file>`.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

// ── Hard wrap utility for commit messages ───────────────────────────────────

/// Hard wraps commit message text at 72 characters.
/// First line is preserved as-is (the summary line), subsequent lines are wrapped.
fn hard_wrap_commit_message(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let mut result = Vec::new();

    // First line (summary) - keep as-is
    if let Some(first) = lines.first() {
        result.push(first.to_string());
    }

    // Process remaining lines
    for line in lines.iter().skip(1) {
        // Empty lines and comment lines are preserved
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            result.push(line.to_string());
            continue;
        }

        // Wrap line at 72 characters
        let words: Vec<&str> = line.split_whitespace().collect();
        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= 72 {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                result.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            result.push(current_line);
        }
    }

    result.join("\n")
}

// ── Colour palette (inspired by the screenshot) ──────────────────────────────

const MENU_BAR_BG: Color = Color::Rgb(90, 90, 140);
const MENU_BAR_FG: Color = Color::Rgb(220, 220, 220);
const MENU_ACTIVE_BG: Color = Color::Rgb(180, 200, 60);
const MENU_ACTIVE_FG: Color = Color::Rgb(30, 30, 30);
const EDITOR_BG: Color = Color::Rgb(40, 42, 54);
const EDITOR_FG: Color = Color::Rgb(200, 200, 200);
const LINE_NUM_FG: Color = Color::Rgb(100, 100, 140);
const LINE_NUM_BG: Color = Color::Rgb(30, 32, 44);
const STATUS_BAR_BG: Color = Color::Rgb(80, 80, 140);
const STATUS_BAR_FG: Color = Color::Rgb(220, 220, 220);
const DROPDOWN_BG: Color = Color::Rgb(70, 72, 90);
const DROPDOWN_FG: Color = Color::Rgb(210, 210, 210);
const DROPDOWN_HIGHLIGHT_BG: Color = Color::Rgb(100, 110, 160);
const CURSOR_BG: Color = Color::Rgb(200, 200, 200);
const CURSOR_FG: Color = Color::Rgb(30, 30, 30);

// ── Editor mode ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EditorMode {
    CommitMessage,
    PullRequest,
    BranchName,
}

// ── Menu definitions ─────────────────────────────────────────────────────────

struct MenuItem {
    label: &'static str,
    shortcut: &'static str,
    action: MenuAction,
}

#[derive(Clone, Copy, PartialEq)]
enum MenuAction {
    Save,
    SaveAndQuit,
    Cancel,
    ShowHelp,
    ShowAbout,
}

const MENU_TITLES: &[&str] = &["File", "Help"];

fn menu_items(menu_index: usize) -> &'static [MenuItem] {
    match menu_index {
        0 => &FILE_MENU,
        1 => &HELP_MENU,
        _ => &[],
    }
}

static FILE_MENU: [MenuItem; 3] = [
    MenuItem {
        label: "Save",
        shortcut: "Ctrl+S",
        action: MenuAction::Save,
    },
    MenuItem {
        label: "Save & Quit",
        shortcut: "Ctrl+Q",
        action: MenuAction::SaveAndQuit,
    },
    MenuItem {
        label: "Cancel",
        shortcut: "Esc",
        action: MenuAction::Cancel,
    },
];

static HELP_MENU: [MenuItem; 2] = [
    MenuItem {
        label: "Keyboard Shortcuts",
        shortcut: "",
        action: MenuAction::ShowHelp,
    },
    MenuItem {
        label: "About",
        shortcut: "",
        action: MenuAction::ShowAbout,
    },
];

// ── Help overlay ─────────────────────────────────────────────────────────────

struct HelpOverlay {
    active: bool,
}

impl Default for HelpOverlay {
    fn default() -> Self {
        Self { active: false }
    }
}

// ── About overlay ────────────────────────────────────────────────────────────

struct AboutOverlay {
    active: bool,
}

impl Default for AboutOverlay {
    fn default() -> Self {
        Self { active: false }
    }
}

// ── Editor state ─────────────────────────────────────────────────────────────

struct EditorApp {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
    scroll_row: usize,
    scroll_col: usize,
    modified: bool,
    should_quit: bool,
    save_on_quit: bool,
    mode: EditorMode,
    // Menus
    active_menu: Option<usize>,
    menu_item_index: usize,
    // Overlays
    help_overlay: HelpOverlay,
    about_overlay: AboutOverlay,
    // Layout cache (set during render)
    editor_area: Rect,
    gutter_width: u16,
    // Esc key hint highlighting
    highlight_save_hint: bool,
    hint_highlight_frames: u8,
}

impl EditorApp {
    fn new(_filename: &str, content: &str, mode: EditorMode) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };
        // Ensure at least one line
        let lines = if lines.is_empty() { vec![String::new()] } else { lines };
        Self {
            lines,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            modified: false,
            should_quit: false,
            save_on_quit: false,
            mode,
            active_menu: None,
            menu_item_index: 0,
            help_overlay: HelpOverlay::default(),
            about_overlay: AboutOverlay::default(),
            editor_area: Rect::default(),
            gutter_width: 4,
            highlight_save_hint: false,
            hint_highlight_frames: 0,
        }
    }

    fn content(&self) -> String {
        let text = self.lines.join("\n");
        // Apply hard wrap for commit messages
        if self.mode == EditorMode::CommitMessage {
            hard_wrap_commit_message(&text)
        } else {
            text
        }
    }

    // ── Cursor helpers ───────────────────────────────────────────────────

    fn clamp_cursor(&mut self) {
        if self.cursor_row >= self.lines.len() {
            self.cursor_row = self.lines.len() - 1;
        }
        let line_len = self.lines[self.cursor_row].len();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let visible_rows = self.editor_area.height as usize;
        let visible_cols = (self.editor_area.width.saturating_sub(self.gutter_width)) as usize;

        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        } else if self.cursor_row >= self.scroll_row + visible_rows {
            self.scroll_row = self.cursor_row - visible_rows + 1;
        }
        if self.cursor_col < self.scroll_col {
            self.scroll_col = self.cursor_col;
        } else if self.cursor_col >= self.scroll_col + visible_cols {
            self.scroll_col = self.cursor_col - visible_cols + 1;
        }
    }

    // ── Edit operations ──────────────────────────────────────────────────

    fn insert_char(&mut self, ch: char) {
        self.lines[self.cursor_row].insert(self.cursor_col, ch);
        self.cursor_col += ch.len_utf8();
        self.modified = true;
    }

    fn insert_newline(&mut self) {
        let rest = self.lines[self.cursor_row][self.cursor_col..].to_string();
        self.lines[self.cursor_row].truncate(self.cursor_col);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, rest);
        self.cursor_col = 0;
        self.modified = true;
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            // Find the start of the previous character
            let prev = self.lines[self.cursor_row][..self.cursor_col]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.lines[self.cursor_row].remove(prev);
            self.cursor_col = prev;
            self.modified = true;
        } else if self.cursor_row > 0 {
            let line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&line);
            self.modified = true;
        }
    }

    fn delete(&mut self) {
        let line_len = self.lines[self.cursor_row].len();
        if self.cursor_col < line_len {
            self.lines[self.cursor_row].remove(self.cursor_col);
            self.modified = true;
        } else if self.cursor_row + 1 < self.lines.len() {
            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
            self.modified = true;
        }
    }

    // ── Menu action dispatch ─────────────────────────────────────────────

    fn execute_menu_action(&mut self, action: MenuAction) {
        self.active_menu = None;
        match action {
            MenuAction::Save => {
                self.save_on_quit = true; /* save handled externally */
            }
            MenuAction::SaveAndQuit => {
                self.save_on_quit = true;
                self.should_quit = true;
            }
            MenuAction::Cancel => {
                self.save_on_quit = false;
                self.should_quit = true;
            }
            MenuAction::ShowHelp => {
                self.help_overlay.active = !self.help_overlay.active;
            }
            MenuAction::ShowAbout => {
                self.about_overlay.active = !self.about_overlay.active;
            }
        }
    }

    // ── Input handling ───────────────────────────────────────────────────

    fn handle_event(&mut self, ev: Event) {
        // Decrement hint highlight counter
        if self.hint_highlight_frames > 0 {
            self.hint_highlight_frames = self.hint_highlight_frames.saturating_sub(1);
            if self.hint_highlight_frames == 0 {
                self.highlight_save_hint = false;
            }
        }

        // If help overlay is active, any key dismisses it
        if self.help_overlay.active {
            if let Event::Key(key) = ev
                && key.kind == KeyEventKind::Press
            {
                self.help_overlay.active = false;
            }
            return;
        }

        // If about overlay is active, any key dismisses it
        if self.about_overlay.active {
            if let Event::Key(key) = ev
                && key.kind == KeyEventKind::Press
            {
                self.about_overlay.active = false;
            }
            return;
        }

        if self.active_menu.is_some() {
            self.handle_menu_event(ev);
            return;
        }

        match ev {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let mods = key.modifiers;
                let ctrl = mods.contains(KeyModifiers::CONTROL);

                match key.code {
                    // ── Ctrl shortcuts ────────────────────────────────
                    KeyCode::Char('s') if ctrl => {
                        self.execute_menu_action(MenuAction::Save);
                    }
                    KeyCode::Char('q') if ctrl => {
                        self.execute_menu_action(MenuAction::SaveAndQuit);
                    }

                    // ── Navigation ────────────────────────────────────
                    KeyCode::Up => {
                        if self.cursor_row > 0 {
                            self.cursor_row -= 1;
                        }
                        self.clamp_cursor();
                    }
                    KeyCode::Down => {
                        if self.cursor_row + 1 < self.lines.len() {
                            self.cursor_row += 1;
                        }
                        self.clamp_cursor();
                    }
                    KeyCode::Left => {
                        if ctrl {
                            self.move_word_left();
                        } else if self.cursor_col > 0 {
                            // Move back by one character
                            self.cursor_col = self.lines[self.cursor_row][..self.cursor_col]
                                .char_indices()
                                .next_back()
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                        } else if self.cursor_row > 0 {
                            self.cursor_row -= 1;
                            self.cursor_col = self.lines[self.cursor_row].len();
                        }
                    }
                    KeyCode::Right => {
                        if ctrl {
                            self.move_word_right();
                        } else {
                            let line_len = self.lines[self.cursor_row].len();
                            if self.cursor_col < line_len {
                                // Move forward by one character
                                let ch = self.lines[self.cursor_row][self.cursor_col..].chars().next().unwrap();
                                self.cursor_col += ch.len_utf8();
                            } else if self.cursor_row + 1 < self.lines.len() {
                                self.cursor_row += 1;
                                self.cursor_col = 0;
                            }
                        }
                    }
                    KeyCode::Home => {
                        if ctrl {
                            self.cursor_row = 0;
                        }
                        self.cursor_col = 0;
                    }
                    KeyCode::End => {
                        if ctrl {
                            self.cursor_row = self.lines.len() - 1;
                        }
                        self.cursor_col = self.lines[self.cursor_row].len();
                    }
                    KeyCode::PageUp => {
                        let page = self.editor_area.height as usize;
                        self.cursor_row = self.cursor_row.saturating_sub(page);
                        self.clamp_cursor();
                    }
                    KeyCode::PageDown => {
                        let page = self.editor_area.height as usize;
                        self.cursor_row = (self.cursor_row + page).min(self.lines.len() - 1);
                        self.clamp_cursor();
                    }

                    // ── Text editing ──────────────────────────────────
                    KeyCode::Char(ch) => {
                        self.insert_char(ch);
                    }
                    KeyCode::Enter => self.insert_newline(),
                    KeyCode::Backspace => self.backspace(),
                    KeyCode::Delete => self.delete(),
                    KeyCode::Tab => {
                        // Insert 4 spaces
                        self.lines[self.cursor_row].insert_str(self.cursor_col, "    ");
                        self.cursor_col += 4;
                        self.modified = true;
                    }

                    // ── Esc: highlight save hint instead of quitting ──
                    KeyCode::Esc => {
                        self.highlight_save_hint = true;
                        self.hint_highlight_frames = 6; // Flash for ~6 frames
                    }
                    KeyCode::F(10) => {
                        self.active_menu = Some(0);
                        self.menu_item_index = 0;
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => {}
        }
    }

    fn move_word_left(&mut self) {
        let line = &self.lines[self.cursor_row];
        if self.cursor_col == 0 {
            if self.cursor_row > 0 {
                self.cursor_row -= 1;
                self.cursor_col = self.lines[self.cursor_row].len();
            }
            return;
        }
        // Skip whitespace, then skip word characters
        let bytes = line.as_bytes();
        let mut col = self.cursor_col;
        while col > 0 && bytes[col - 1].is_ascii_whitespace() {
            col -= 1;
        }
        while col > 0 && !bytes[col - 1].is_ascii_whitespace() {
            col -= 1;
        }
        self.cursor_col = col;
    }

    fn move_word_right(&mut self) {
        let line = &self.lines[self.cursor_row];
        let len = line.len();
        if self.cursor_col >= len {
            if self.cursor_row + 1 < self.lines.len() {
                self.cursor_row += 1;
                self.cursor_col = 0;
            }
            return;
        }
        let bytes = line.as_bytes();
        let mut col = self.cursor_col;
        while col < len && !bytes[col].is_ascii_whitespace() {
            col += 1;
        }
        while col < len && bytes[col].is_ascii_whitespace() {
            col += 1;
        }
        self.cursor_col = col;
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if click is on the menu bar (row 0 relative to terminal)
                if mouse.row == 0 {
                    self.handle_menu_bar_click(mouse.column);
                    return;
                }
                // Click in editor area — set cursor
                if mouse.row > 0
                    && mouse.row < self.editor_area.y + self.editor_area.height
                    && mouse.column >= self.editor_area.x + self.gutter_width
                {
                    let row = (mouse.row as usize - 1) + self.scroll_row;
                    let col = (mouse.column - self.editor_area.x - self.gutter_width) as usize + self.scroll_col;
                    self.cursor_row = row.min(self.lines.len() - 1);
                    self.cursor_col = col.min(self.lines[self.cursor_row].len());
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll_row = self.scroll_row.saturating_sub(3);
            }
            MouseEventKind::ScrollDown => {
                let max = self.lines.len().saturating_sub(1);
                self.scroll_row = (self.scroll_row + 3).min(max);
            }
            _ => {}
        }
    }

    fn handle_menu_bar_click(&mut self, col: u16) {
        // Determine which menu title was clicked
        let mut x = 1u16;
        for (i, title) in MENU_TITLES.iter().enumerate() {
            let w = title.len() as u16 + 2; // padding
            if col >= x && col < x + w {
                if self.active_menu == Some(i) {
                    self.active_menu = None;
                } else {
                    self.active_menu = Some(i);
                    self.menu_item_index = 0;
                }
                return;
            }
            x += w;
        }
        self.active_menu = None;
    }

    fn handle_menu_event(&mut self, ev: Event) {
        match ev {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc => self.active_menu = None,
                KeyCode::Up => {
                    if self.menu_item_index > 0 {
                        self.menu_item_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if let Some(mi) = self.active_menu {
                        let items = menu_items(mi);
                        if self.menu_item_index + 1 < items.len() {
                            self.menu_item_index += 1;
                        }
                    }
                }
                KeyCode::Left => {
                    if let Some(mi) = self.active_menu {
                        self.active_menu = Some(if mi == 0 { MENU_TITLES.len() - 1 } else { mi - 1 });
                        self.menu_item_index = 0;
                    }
                }
                KeyCode::Right => {
                    if let Some(mi) = self.active_menu {
                        self.active_menu = Some((mi + 1) % MENU_TITLES.len());
                        self.menu_item_index = 0;
                    }
                }
                KeyCode::Enter => {
                    if let Some(mi) = self.active_menu {
                        let items = menu_items(mi);
                        if self.menu_item_index < items.len() {
                            let action = items[self.menu_item_index].action;
                            self.execute_menu_action(action);
                        }
                    }
                }
                _ => {}
            },
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                // Check if click is on menu bar
                if mouse.row == 0 {
                    self.handle_menu_bar_click(mouse.column);
                    return;
                }
                // Check if click is inside the dropdown
                if let Some(mi) = self.active_menu {
                    let dropdown_rect = self.dropdown_rect(mi);
                    if mouse.row >= dropdown_rect.y
                        && mouse.row < dropdown_rect.y + dropdown_rect.height
                        && mouse.column >= dropdown_rect.x
                        && mouse.column < dropdown_rect.x + dropdown_rect.width
                    {
                        let item_idx = (mouse.row - dropdown_rect.y - 1) as usize; // -1 for border
                        let items = menu_items(mi);
                        if item_idx < items.len() {
                            let action = items[item_idx].action;
                            self.execute_menu_action(action);
                        }
                        return;
                    }
                }
                // Click outside menu → close it and handle as editor click
                self.active_menu = None;
                self.handle_mouse(mouse);
            }
            _ => {}
        }
    }

    // ── Dropdown rect calculation ────────────────────────────────────────

    fn dropdown_rect(&self, menu_index: usize) -> Rect {
        let items = menu_items(menu_index);
        let width = items
            .iter()
            .map(|it| it.label.len() + it.shortcut.len() + 6)
            .max()
            .unwrap_or(20) as u16
            + 2; // borders
        let height = items.len() as u16 + 2; // borders
        let mut x = 1u16;
        for i in 0..menu_index {
            x += MENU_TITLES[i].len() as u16 + 2;
        }
        Rect::new(x, 1, width, height)
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

fn render(frame: &mut ratatui::Frame, app: &mut EditorApp) {
    let area = frame.area();

    // Layout: menu bar (1) | editor (fill) | status bar (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    render_menu_bar(frame, app, chunks[0]);
    render_editor(frame, app, chunks[1]);
    render_status_bar(frame, app, chunks[2]);

    // Render dropdown if active (on top of everything)
    if let Some(mi) = app.active_menu {
        render_dropdown(frame, app, mi);
    }

    // Render help overlay if active
    if app.help_overlay.active {
        render_help_overlay(frame);
    }

    // Render about overlay if active
    if app.about_overlay.active {
        render_about_overlay(frame);
    }
}

fn render_menu_bar(frame: &mut ratatui::Frame, app: &EditorApp, area: Rect) {
    // Fill the entire bar with the menu background
    let bar_bg = Paragraph::new("").style(Style::default().bg(MENU_BAR_BG));
    frame.render_widget(bar_bg, area);

    let mut spans = Vec::new();
    spans.push(Span::styled(" ", Style::default().bg(MENU_BAR_BG)));
    for (i, title) in MENU_TITLES.iter().enumerate() {
        let style = if app.active_menu == Some(i) {
            Style::default().fg(MENU_ACTIVE_FG).bg(MENU_ACTIVE_BG)
        } else {
            Style::default().fg(MENU_BAR_FG).bg(MENU_BAR_BG)
        };
        spans.push(Span::styled(format!("{title}"), style));
        spans.push(Span::styled("  ", Style::default().bg(MENU_BAR_BG)));
    }

    let line = Line::from(spans);
    let menu_bar = Paragraph::new(line);
    frame.render_widget(menu_bar, area);
}

fn render_editor(frame: &mut ratatui::Frame, app: &mut EditorApp, area: Rect) {
    app.editor_area = area;

    // Calculate gutter width based on line count
    let line_count = app.lines.len();
    let digits = format!("{line_count}").len().max(2);
    app.gutter_width = digits as u16 + 2; // digit space + padding

    let visible_rows = area.height as usize;
    let text_width = area.width.saturating_sub(app.gutter_width) as usize;
    app.ensure_cursor_visible();

    // 72-char guide line position (only for commit message mode)
    let guide_col = if app.mode == EditorMode::CommitMessage {
        Some(72)
    } else {
        None
    };

    for row_offset in 0..visible_rows {
        let line_idx = app.scroll_row + row_offset;
        let y = area.y + row_offset as u16;

        // Gutter (line number)
        let gutter_area = Rect::new(area.x, y, app.gutter_width, 1);
        if line_idx < app.lines.len() {
            let num_str = format!("{:>width$} ", line_idx + 1, width = digits);
            let gutter = Paragraph::new(Span::styled(num_str, Style::default().fg(LINE_NUM_FG).bg(LINE_NUM_BG)));
            frame.render_widget(gutter, gutter_area);
        } else {
            let gutter = Paragraph::new(Span::styled(
                " ".repeat(app.gutter_width as usize),
                Style::default().bg(LINE_NUM_BG),
            ));
            frame.render_widget(gutter, gutter_area);
        }

        // Text content
        let text_area = Rect::new(area.x + app.gutter_width, y, text_width as u16, 1);

        if line_idx < app.lines.len() {
            let line = &app.lines[line_idx];
            let display_start = app.scroll_col;

            let mut spans = Vec::new();
            let mut byte_col = 0usize; // byte offset — used for cursor detection
            let mut vis_col = 0usize;  // visual column accounting for tab expansion
            let mut rendered = 0usize; // visual columns placed in text_area so far

            for ch in line.chars() {
                // Tab width: advance to the next 4-space tab stop
                let tab_w = if ch == '\t' { 4 - (vis_col % 4) } else { 1 };

                // Skip chars before the horizontal scroll offset (byte-based)
                if byte_col < display_start {
                    byte_col += ch.len_utf8();
                    vis_col += tab_w;
                    continue;
                }

                if rendered >= text_width { break; }

                let is_cursor = line_idx == app.cursor_row && byte_col == app.cursor_col;

                if ch == '\t' {
                    // Render tab as spaces up to the next tab stop (clamped to area width)
                    let spaces = tab_w.min(text_width - rendered);
                    for i in 0..spaces {
                        let is_guide = guide_col.map_or(false, |g| vis_col + i == g);
                        let style = if is_cursor && i == 0 {
                            Style::default().fg(CURSOR_FG).bg(CURSOR_BG)
                        } else if is_guide {
                            Style::default().fg(EDITOR_FG).bg(Color::Rgb(60, 62, 74))
                        } else {
                            Style::default().fg(EDITOR_FG).bg(EDITOR_BG)
                        };
                        spans.push(Span::styled(" ", style));
                    }
                    rendered += spaces;
                } else {
                    let is_guide = guide_col.map_or(false, |g| vis_col == g);
                    let style = if is_cursor {
                        Style::default().fg(CURSOR_FG).bg(CURSOR_BG)
                    } else if is_guide {
                        Style::default().fg(EDITOR_FG).bg(Color::Rgb(60, 62, 74))
                    } else {
                        Style::default().fg(EDITOR_FG).bg(EDITOR_BG)
                    };
                    spans.push(Span::styled(ch.to_string(), style));
                    rendered += 1;
                }

                byte_col += ch.len_utf8();
                vis_col += tab_w;
            }

            // Cursor at end of line
            if line_idx == app.cursor_row && app.cursor_col >= byte_col && app.cursor_col <= line.len() {
                if rendered < text_width {
                    spans.push(Span::styled(" ", Style::default().fg(CURSOR_FG).bg(CURSOR_BG)));
                    rendered += 1;
                    vis_col += 1;
                }
            }

            // Fill remaining space
            while rendered < text_width {
                let is_guide = guide_col.map_or(false, |g| vis_col == g);
                let bg = if is_guide { Color::Rgb(60, 62, 74) } else { EDITOR_BG };
                spans.push(Span::styled(" ", Style::default().bg(bg)));
                rendered += 1;
                vis_col += 1;
            }

            frame.render_widget(Paragraph::new(Line::from(spans)), text_area);
        } else {
            // Empty line beyond end of document - show guide line if applicable
            let mut col = 0;
            let mut spans = Vec::new();
            for _ in 0..text_width {
                let is_guide = guide_col.map_or(false, |g| col == g);
                let bg = if is_guide { Color::Rgb(60, 62, 74) } else { EDITOR_BG };
                spans.push(Span::styled(" ", Style::default().bg(bg)));
                col += 1;
            }
            frame.render_widget(Paragraph::new(Line::from(spans)), text_area);
        }
    }
}

fn render_status_bar(frame: &mut ratatui::Frame, app: &EditorApp, area: Rect) {
    let modified_indicator = if app.modified { "*" } else { "" };

    // Mode name for display
    let mode_name = match app.mode {
        EditorMode::CommitMessage => "Commit Message",
        EditorMode::PullRequest => "Plain",
        EditorMode::BranchName => "Branch Name",
    };

    let left = format!(
        " [{}] {}:{} {}",
        mode_name,
        app.cursor_row + 1,
        app.cursor_col + 1,
        modified_indicator,
    );

    // Highlight ctrl-q if Esc was pressed
    let ctrl_q_style = if app.highlight_save_hint {
        Style::default()
            .fg(Color::Rgb(255, 255, 100))
            .bg(Color::Rgb(100, 100, 50))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(STATUS_BAR_FG).bg(STATUS_BAR_BG)
    };

    let ctrl_q_hint = "Ctrl-Q to save ";

    let padding = area.width.saturating_sub(left.len() as u16 + ctrl_q_hint.len() as u16);

    let line = Line::from(vec![
        Span::styled(&left, Style::default().fg(STATUS_BAR_FG).bg(STATUS_BAR_BG)),
        Span::styled(" ".repeat(padding as usize), Style::default().bg(STATUS_BAR_BG)),
        Span::styled(ctrl_q_hint, ctrl_q_style),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_dropdown(frame: &mut ratatui::Frame, app: &EditorApp, menu_index: usize) {
    let items = menu_items(menu_index);
    let rect = app.dropdown_rect(menu_index);

    // Clamp to screen
    let screen = frame.area();
    let rect = Rect::new(
        rect.x.min(screen.width.saturating_sub(rect.width)),
        rect.y,
        rect.width.min(screen.width),
        rect.height.min(screen.height.saturating_sub(rect.y)),
    );

    // Clear the area behind the dropdown
    frame.render_widget(Clear, rect);

    let inner_width = rect.width.saturating_sub(2) as usize; // inside borders
    let mut lines = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let shortcut_len = item.shortcut.len();
        let label_space = inner_width.saturating_sub(shortcut_len + 4);
        let text = format!("  {:<width$}  {}", item.label, item.shortcut, width = label_space,);
        let style = if i == app.menu_item_index {
            Style::default().fg(DROPDOWN_FG).bg(DROPDOWN_HIGHLIGHT_BG)
        } else {
            Style::default().fg(DROPDOWN_FG).bg(DROPDOWN_BG)
        };
        lines.push(Line::styled(text, style));
    }

    let dropdown = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(140, 140, 180)).bg(DROPDOWN_BG))
            .style(Style::default().bg(DROPDOWN_BG)),
    );
    frame.render_widget(dropdown, rect);
}

fn render_help_overlay(frame: &mut ratatui::Frame) {
    let screen = frame.area();
    let width = 50u16.min(screen.width.saturating_sub(4));
    let height = 12u16.min(screen.height.saturating_sub(4));
    let x = (screen.width.saturating_sub(width)) / 2;
    let y = (screen.height.saturating_sub(height)) / 2;
    let rect = Rect::new(x, y, width, height);

    frame.render_widget(Clear, rect);

    let help_text = vec![
        Line::styled(
            "  Keyboard Shortcuts",
            Style::default().fg(MENU_ACTIVE_BG).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled("  Ctrl+Q        Save & Quit", Style::default().fg(DROPDOWN_FG)),
        Line::styled("  Ctrl+S        Save", Style::default().fg(DROPDOWN_FG)),
        Line::styled("  Esc           Highlight save hint", Style::default().fg(DROPDOWN_FG)),
        Line::styled("  F10           Open Menu", Style::default().fg(DROPDOWN_FG)),
        Line::raw(""),
        Line::styled("  Press any key to close", Style::default().fg(Color::DarkGray)),
    ];

    let help = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .border_style(Style::default().fg(Color::Rgb(140, 140, 180)).bg(DROPDOWN_BG))
            .style(Style::default().bg(DROPDOWN_BG)),
    );
    frame.render_widget(help, rect);
}

fn render_about_overlay(frame: &mut ratatui::Frame) {
    let screen = frame.area();
    let width = 40u16.min(screen.width.saturating_sub(4));
    let height = 7u16.min(screen.height.saturating_sub(4));
    let x = (screen.width.saturating_sub(width)) / 2;
    let y = (screen.height.saturating_sub(height)) / 2;
    let rect = Rect::new(x, y, width, height);

    frame.render_widget(Clear, rect);

    let about_text = vec![
        Line::raw(""),
        Line::styled(
            "  Made with ❤️ in Berlin",
            Style::default().fg(DROPDOWN_FG).add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            "  by GitButler",
            Style::default().fg(DROPDOWN_FG).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled("  Press any key to close", Style::default().fg(Color::DarkGray)),
    ];

    let about = Paragraph::new(about_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" About ")
            .border_style(Style::default().fg(Color::Rgb(140, 140, 180)).bg(DROPDOWN_BG))
            .style(Style::default().bg(DROPDOWN_BG)),
    );
    frame.render_widget(about, rect);
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Opens the built-in TUI text editor.
///
/// Returns `Some(content)` if the user saved, or `None` if they cancelled.
pub fn run_builtin_editor(filename: &str, initial_content: &str, mode: EditorMode) -> anyhow::Result<Option<String>> {
    let mut guard = super::TerminalGuard::new(true)?;
    let mut app = EditorApp::new(filename, initial_content, mode);

    loop {
        guard.terminal_mut().draw(|frame| render(frame, &mut app))?;

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;
            app.handle_event(ev);
        }

        if app.should_quit {
            break;
        }
    }

    if app.save_on_quit {
        Ok(Some(app.content()))
    } else {
        Ok(None)
    }
}

/// Opens the built-in editor for a file on disk.
/// Reads the file, lets the user edit, and writes it back on save.
pub fn edit_file(path: &std::path::Path) -> anyhow::Result<()> {
    let content = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string());

    // Generic file editing defaults to PR mode (no hard wrap)
    if let Some(new_content) = run_builtin_editor(&filename, &content, EditorMode::PullRequest)? {
        std::fs::write(path, new_content)?;
    }

    Ok(())
}
