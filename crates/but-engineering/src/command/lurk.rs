//! Lurk command — read-only TUI for observing agent chat.

use std::collections::{HashMap, HashSet};
use std::io::{self, Stdout};
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, Utc};
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::conflict;
use crate::db::DbHandle;
use crate::types::{Agent, Claim, Message};

/// How often to poll the database for new data.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// How far back to look on initial load (1 hour).
const INITIAL_WINDOW_SECS: i64 = 60 * 60;

/// Max messages on initial load.
const INITIAL_LIMIT: usize = 50;

/// Max messages per poll.
const POLL_LIMIT: usize = 1000;

/// Maximum retained messages to prevent unbounded memory growth.
/// Kept low enough that total rendered lines stay within u16::MAX (ratatui scroll limit).
const MAX_RETAINED_MESSAGES: usize = 2_000;

/// Colors assigned to agents (cycled sequentially to avoid collisions).
/// Uses standard named colors for broad terminal compatibility.
const AGENT_COLORS: &[Color] = &[
    Color::Cyan,
    Color::Green,
    Color::Yellow,
    Color::Magenta,
    Color::Blue,
    Color::Red,
    Color::LightCyan,
    Color::LightGreen,
    Color::LightMagenta,
    Color::LightBlue,
];

struct App {
    messages: Vec<Message>,
    agents: Vec<Agent>,
    claims: Vec<Claim>,
    /// Scroll offset in lines from the bottom (0 = at bottom).
    scroll_offset: usize,
    auto_scroll: bool,
    last_message_time: DateTime<Utc>,
    should_quit: bool,
    /// Tracks consecutive poll failures for error display.
    consecutive_poll_errors: usize,
    /// Last error message from polling, shown in the status bar.
    last_error: Option<String>,
    /// Maps agent IDs to color indices for collision-free color assignment.
    agent_color_map: HashMap<String, usize>,
    /// Next color index to assign (cycles through AGENT_COLORS).
    next_color_index: usize,
}

impl App {
    fn new() -> Self {
        App {
            messages: Vec::new(),
            agents: Vec::new(),
            claims: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            last_message_time: Utc::now() - chrono::Duration::seconds(INITIAL_WINDOW_SECS),
            should_quit: false,
            consecutive_poll_errors: 0,
            last_error: None,
            agent_color_map: HashMap::new(),
            next_color_index: 0,
        }
    }

    /// Get a color for an agent, assigning a new one if not seen before.
    /// Colors cycle through AGENT_COLORS, ensuring each new agent gets
    /// the next color in sequence (no hash collisions).
    fn agent_color(&mut self, agent_id: &str) -> Color {
        if let Some(&idx) = self.agent_color_map.get(agent_id) {
            return AGENT_COLORS[idx];
        }
        let idx = self.next_color_index % AGENT_COLORS.len();
        self.next_color_index += 1;
        self.agent_color_map.insert(agent_id.to_string(), idx);
        AGENT_COLORS[idx]
    }
}

/// Run the lurk TUI.
pub fn execute(db_path: &Path) -> anyhow::Result<()> {
    let mut terminal = setup_terminal()?;

    // Install a panic hook that restores the terminal before printing the panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal_raw();
        original_hook(info);
    }));

    let result = run_event_loop(&mut terminal, db_path);

    // Remove our custom panic hook. This installs the *default* hook, not the original —
    // the original was moved into the closure and cannot be recovered. Acceptable since
    // execute() is only called from main() and the process is about to exit.
    let _ = std::panic::take_hook();

    // Always attempt terminal restoration, but prefer the event loop error.
    let restore_result = restore_terminal(&mut terminal);
    match (result, restore_result) {
        (Err(e), _) => Err(e),
        (Ok(()), Err(e)) => Err(e),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    // Attempt each step independently so a failure in one doesn't skip the others.
    let r1 = terminal::disable_raw_mode();
    // execute() returns Result<&mut W>; calling .err() drops the mutable reference
    // to avoid a double borrow on `terminal`, while capturing any error.
    let r2 = terminal.backend_mut().execute(LeaveAlternateScreen).err();
    let r3 = terminal.show_cursor();
    r1?;
    if let Some(e) = r2 {
        return Err(e.into());
    }
    r3?;
    Ok(())
}

/// Best-effort restore for panic hook — tries each step independently.
fn restore_terminal_raw() {
    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);
    let _ = io::stdout().execute(crossterm::cursor::Show);
}

fn run_event_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, db_path: &Path) -> anyhow::Result<()> {
    let mut app = App::new();
    let mut last_poll = Instant::now() - POLL_INTERVAL; // force immediate first poll

    loop {
        // 1. Poll for terminal events with short timeout.
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            handle_key(&mut app, key.code, terminal.size()?.height as usize);
        }

        if app.should_quit {
            return Ok(());
        }

        // 2. Poll DB for new messages every POLL_INTERVAL.
        if last_poll.elapsed() >= POLL_INTERVAL {
            // Compute inner_width for line counting. Borders subtract 2 columns,
            // and the agents panel (if shown) takes AGENTS_PANEL_WIDTH from the right.
            let term_width = terminal.size()?.width;
            let messages_width = if term_width >= AGENTS_PANEL_MIN_WIDTH {
                term_width.saturating_sub(AGENTS_PANEL_WIDTH)
            } else {
                term_width
            };
            let inner_width = messages_width.saturating_sub(2) as usize;
            let lines_before = count_rendered_lines(&app.messages, inner_width);

            poll_db(&mut app, db_path);

            // When scrolled up, adjust scroll_offset so the viewport stays on the
            // same content rather than drifting as new messages push the bottom down.
            if !app.auto_scroll {
                let lines_after = count_rendered_lines(&app.messages, inner_width);
                let delta = lines_after.saturating_sub(lines_before);
                app.scroll_offset += delta;
            }

            last_poll = Instant::now();
        }

        // 3. Render.
        terminal.draw(|frame| render(frame, &mut app))?;
    }
}

fn handle_key(app: &mut App, key: KeyCode, viewport_height: usize) {
    // Reserve 2 lines for help bar and 2 for message area borders.
    let page_size = viewport_height.saturating_sub(4);

    match key {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Up => {
            app.scroll_offset = app.scroll_offset.saturating_add(1);
            app.auto_scroll = false;
        }
        KeyCode::Down => {
            app.scroll_offset = app.scroll_offset.saturating_sub(1);
            if app.scroll_offset == 0 {
                app.auto_scroll = true;
            }
        }
        KeyCode::PageUp => {
            app.scroll_offset = app.scroll_offset.saturating_add(page_size);
            app.auto_scroll = false;
        }
        KeyCode::PageDown => {
            app.scroll_offset = app.scroll_offset.saturating_sub(page_size);
            if app.scroll_offset == 0 {
                app.auto_scroll = true;
            }
        }
        KeyCode::Home => {
            app.scroll_offset = usize::MAX; // will be clamped in render
            app.auto_scroll = false;
        }
        KeyCode::End => {
            app.scroll_offset = 0;
            app.auto_scroll = true;
        }
        _ => {}
    }
}

fn poll_db(app: &mut App, db_path: &Path) {
    // Open a fresh connection each poll to see WAL writes from other processes.
    let db = match DbHandle::new_at_path(db_path) {
        Ok(db) => db,
        Err(e) => {
            record_poll_error(app, &e);
            return;
        }
    };

    // Fetch messages.
    let messages_result = if app.messages.is_empty() {
        let since = Utc::now() - chrono::Duration::seconds(INITIAL_WINDOW_SECS);
        db.query_recent_messages(since, INITIAL_LIMIT)
    } else {
        db.query_messages_since(app.last_message_time, Some(POLL_LIMIT))
    };

    match messages_result {
        Ok(new_messages) => {
            if let Some(last) = new_messages.last() {
                app.last_message_time = last.timestamp;
            }
            app.messages.extend(new_messages);

            // Evict oldest messages to prevent unbounded memory growth.
            if app.messages.len() > MAX_RETAINED_MESSAGES {
                let excess = app.messages.len() - MAX_RETAINED_MESSAGES;
                app.messages.drain(..excess);
            }
        }
        Err(e) => {
            record_poll_error(app, &e);
            return;
        }
    }

    // Fetch agents active in the last hour, ordered by most recent first.
    let agents_since = Utc::now() - chrono::Duration::hours(1);
    // Show only active claim leases (same timeout used by conflict checks).
    let claims_since = Utc::now() - chrono::Duration::minutes(conflict::CLAIM_STALE_MINUTES);
    match db.list_agents(Some(agents_since)) {
        Ok(agents) => app.agents = agents,
        Err(e) => {
            record_poll_error(app, &e);
            return;
        }
    }
    match db.list_claims(Some(claims_since)) {
        Ok(claims) => app.claims = claims,
        Err(e) => {
            record_poll_error(app, &e);
            return;
        }
    }

    // All succeeded — clear error state.
    app.consecutive_poll_errors = 0;
    app.last_error = None;
}

fn record_poll_error(app: &mut App, error: &dyn std::fmt::Display) {
    app.consecutive_poll_errors += 1;
    // Only surface persistent errors (transient ones resolve on their own).
    if app.consecutive_poll_errors >= 3 {
        app.last_error = Some(format!("{error}"));
    }
}

// ── Rendering ───────────────────────────────────────────────────────────────

/// Minimum terminal width before we show the agents panel.
const AGENTS_PANEL_MIN_WIDTH: u16 = 70;
/// Width of the agents panel when shown.
const AGENTS_PANEL_WIDTH: u16 = 36;
/// Maximum number of claimed files shown per agent in the side panel.
const MAX_CLAIMS_SHOWN_PER_AGENT: usize = 3;

fn render(frame: &mut ratatui::Frame<'_>, app: &mut App) {
    let area = frame.area();

    // 1. Vertical split: main content area + full-width help bar at bottom.
    let v_chunks = Layout::vertical([
        Constraint::Min(1),    // main content
        Constraint::Length(2), // help bar (border + help text)
    ])
    .split(area);

    // 2. Horizontal split of main content: messages + optional agents panel.
    let show_agents_panel = area.width >= AGENTS_PANEL_MIN_WIDTH;

    if show_agents_panel {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(1),                     // messages
                Constraint::Length(AGENTS_PANEL_WIDTH), // agents panel
            ])
            .split(v_chunks[0]);

        render_messages(frame, app, h_chunks[0]);
        render_agents_panel(frame, app, h_chunks[1]);
    } else {
        render_messages(frame, app, v_chunks[0]);
    }

    // 3. Help bar spans full width at bottom.
    render_help_bar(frame, app, v_chunks[1]);
}

fn render_messages(frame: &mut ratatui::Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let inner_width = area.width.saturating_sub(2) as usize; // account for borders
    let inner_height = area.height.saturating_sub(2) as usize; // account for borders

    // Pre-resolve colors for all agents to avoid borrow conflict.
    let msg_colors: Vec<Color> = app
        .messages
        .iter()
        .map(|m| m.agent_id.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|id| app.agent_color(&id))
        .collect();

    // Ensure agents in the side panel also get color assignments,
    // so @mentions of them resolve correctly.
    for id in app.agents.iter().map(|a| a.id.clone()).collect::<Vec<_>>() {
        app.agent_color(&id);
    }

    // Build agent ID → color lookup for @mention highlighting.
    let mention_colors: HashMap<String, Color> = app
        .agent_color_map
        .iter()
        .map(|(id, &idx)| (id.clone(), AGENT_COLORS[idx % AGENT_COLORS.len()]))
        .collect();

    // Build all display lines from messages.
    let mut lines: Vec<Line<'_>> = Vec::new();
    for (i, msg) in app.messages.iter().enumerate() {
        if i > 0 {
            lines.push(Line::raw(""));
        }

        let color = msg_colors[i];
        let local_time = msg.timestamp.with_timezone(&Local);
        let time_str = local_time.format("%H:%M:%S").to_string();

        // Header line: [HH:MM:SS] agent_id:
        // Truncate to inner_width so our line count matches visual lines exactly.
        let prefix = format!("[{time_str}] ");
        let suffix = ":";
        let prefix_width = UnicodeWidthStr::width(prefix.as_str());
        let header_width = prefix_width + UnicodeWidthStr::width(msg.agent_id.as_str()) + suffix.len();

        if header_width <= inner_width {
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}:", msg.agent_id),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        } else {
            let max_id_width = inner_width.saturating_sub(prefix_width + suffix.len());
            let truncated_id = truncate_agent_id_for_width(&msg.agent_id, max_id_width);
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{truncated_id}:"),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // Content lines, wrapped and indented, with @mentions highlighted.
        let indent = "  ";
        let wrap_width = inner_width.saturating_sub(indent.len());
        for content_line in msg.content.lines() {
            if wrap_width == 0 {
                let mut spans = vec![Span::raw(indent)];
                spans.extend(styled_content_spans(content_line, &mention_colors));
                lines.push(Line::from(spans));
            } else {
                for chunk in wrap_text(content_line, wrap_width) {
                    let mut spans = vec![Span::raw(indent)];
                    spans.extend(styled_content_spans(chunk, &mention_colors));
                    lines.push(Line::from(spans));
                }
            }
        }
    }

    // Calculate scroll. The scroll position is "lines from bottom", while
    // ratatui's Paragraph scroll is (rows_from_top, cols_from_left).
    let total_lines = lines.len();
    let max_scroll_top = total_lines.saturating_sub(inner_height);
    let scroll_offset = if app.auto_scroll {
        0
    } else {
        app.scroll_offset.min(max_scroll_top)
    };
    let scroll_top = max_scroll_top.saturating_sub(scroll_offset);

    let scroll_indicator = if !app.auto_scroll && scroll_offset > 0 {
        format!(" (scroll +{scroll_offset}) ")
    } else {
        String::new()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" #engineering{scroll_indicator}"));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll_top.min(u16::MAX as usize) as u16, 0));

    frame.render_widget(paragraph, area);
}

fn render_help_bar(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let inner = Block::default().borders(Borders::TOP).inner(area);

    // Top border.
    let border = Block::default().borders(Borders::TOP);
    frame.render_widget(border, area);

    // Show error if present, otherwise show help text.
    if let Some(ref err) = app.last_error {
        let error_line = Paragraph::new(Line::from(vec![
            Span::styled("DB error: ", Style::default().fg(Color::Red)),
            Span::styled(err.as_str(), Style::default().fg(Color::Red)),
            Span::styled(" (retrying...)", Style::default().fg(Color::DarkGray)),
        ]));
        frame.render_widget(error_line, inner);
    } else {
        let help = Paragraph::new(Line::from(vec![Span::styled(
            "\u{2191}\u{2193} scroll | PgUp/PgDn page | Home/End | q quit",
            Style::default().fg(Color::DarkGray),
        )]));
        frame.render_widget(help, inner);
    }
}

fn render_agents_panel(frame: &mut ratatui::Frame<'_>, app: &mut App, area: ratatui::layout::Rect) {
    let agent_count = app.agents.len();
    let title = format!(" Agents ({agent_count}) ");
    let block = Block::default()
        .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
        .title(title);
    // Only RIGHT border takes a column (no LEFT border).
    let inner_width = area.width.saturating_sub(1) as usize;

    if app.agents.is_empty() {
        let empty =
            Paragraph::new(Line::from(Span::styled("(none)", Style::default().fg(Color::DarkGray)))).block(block);
        frame.render_widget(empty, area);
        return;
    }

    // Pre-resolve colors for all agents to avoid borrow conflict.
    let agent_colors: Vec<Color> = app
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|id| app.agent_color(&id))
        .collect();

    let mut claims_by_agent: HashMap<String, Vec<String>> = HashMap::new();
    let mut claim_owners: HashMap<String, HashSet<String>> = HashMap::new();
    for claim in &app.claims {
        claims_by_agent
            .entry(claim.agent_id.clone())
            .or_default()
            .push(claim.file_path.clone());
        claim_owners
            .entry(claim.file_path.clone())
            .or_default()
            .insert(claim.agent_id.clone());
    }

    let now = Utc::now();
    let items: Vec<ListItem<'_>> = app
        .agents
        .iter()
        .enumerate()
        .map(|(i, agent)| {
            let color = agent_colors[i];

            let ago = now.signed_duration_since(agent.last_active);
            let ago_str = if ago.num_minutes() < 1 {
                "now".to_string()
            } else if ago.num_minutes() < 60 {
                format!("{}m", ago.num_minutes())
            } else if ago.num_hours() < 24 {
                format!("{}h", ago.num_hours())
            } else {
                format!("{}d", ago.num_days())
            };

            // Build lines for this agent entry.
            let mut lines = Vec::new();

            // First line: agent ID + time ago.
            let id_display = {
                // Truncate ID if needed to leave room for " (Xm)".
                let ago_part = format!(" ({ago_str})");
                let ago_width = UnicodeWidthStr::width(ago_part.as_str());
                let max_id = inner_width.saturating_sub(ago_width);
                truncate_agent_id_for_width(&agent.id, max_id)
            };
            lines.push(Line::from(vec![
                Span::styled(id_display, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ({ago_str})"), Style::default().fg(Color::DarkGray)),
            ]));

            // Second line: status (if present), dimmed and indented.
            if let Some(ref status) = agent.status {
                let max_status_width = inner_width.saturating_sub(2); // 2 for "  " indent
                let boundary = display_width_boundary(status, max_status_width);
                let display_status = &status[..boundary];
                lines.push(Line::from(Span::styled(
                    format!("  {display_status}"),
                    Style::default().fg(Color::DarkGray),
                )));
            }

            let mut claim_files = claims_by_agent
                .get(&agent.id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            if !claim_files.is_empty() {
                claim_files.sort();
                claim_files.dedup();
                let shared_count = claim_files
                    .iter()
                    .filter(|path| claim_owners.get(*path).map(|owners| owners.len()).unwrap_or(0) > 1)
                    .count();

                lines.push(Line::from(Span::styled(
                    if shared_count > 0 {
                        format!("  claims ({}) shared({shared_count})", claim_files.len())
                    } else {
                        format!("  claims ({})", claim_files.len())
                    },
                    if shared_count > 0 {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::Yellow)
                    },
                )));

                for claim in claim_files.iter().take(MAX_CLAIMS_SHOWN_PER_AGENT) {
                    let is_shared = claim_owners.get(claim).map(|owners| owners.len()).unwrap_or(0) > 1;
                    let prefix = if is_shared { "    ! " } else { "    - " };
                    let max_claim_width = inner_width.saturating_sub(prefix.len());
                    let boundary = display_width_boundary(claim, max_claim_width);
                    let display_claim = &claim[..boundary];
                    lines.push(Line::from(Span::styled(
                        format!("{prefix}{display_claim}"),
                        if is_shared {
                            Style::default().fg(Color::Red)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    )));
                }
                if claim_files.len() > MAX_CLAIMS_SHOWN_PER_AGENT {
                    lines.push(Line::from(Span::styled(
                        format!("    +{} more", claim_files.len() - MAX_CLAIMS_SHOWN_PER_AGENT),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Known file extensions for standalone filename detection (e.g., `login.rs`, `Cargo.toml`).
const KNOWN_FILE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "js", "tsx", "jsx", "py", "rb", "go", "java", "c", "cpp", "h", "hpp", "cs", "swift", "kt", "toml",
    "yaml", "yml", "json", "xml", "html", "css", "scss", "md", "txt", "sh", "sql", "lock", "cfg", "conf", "ini",
];

/// Parse text for `@mentions` and file paths, returning styled spans.
/// Mentions get the agent's color (bold), file paths get underlined, rest is plain.
fn styled_content_spans<'a>(text: &'a str, mention_colors: &HashMap<String, Color>) -> Vec<Span<'a>> {
    let mention_spans = mention_styled_spans(text, mention_colors);
    // Second pass: highlight file paths within any unstyled spans.
    let mut result = Vec::with_capacity(mention_spans.len());
    for span in mention_spans {
        if span.style != Style::default() {
            result.push(span);
            continue;
        }
        match span.content {
            std::borrow::Cow::Borrowed(s) => result.extend(path_styled_spans(s)),
            std::borrow::Cow::Owned(_) => result.push(span),
        }
    }
    result
}

/// Parse text for `@mentions` and return styled spans.
/// Known agent mentions get the agent's color (bold); unknown mentions get bold white.
fn mention_styled_spans<'a>(text: &'a str, mention_colors: &HashMap<String, Color>) -> Vec<Span<'a>> {
    // Fast path: no '@' at all.
    if !text.contains('@') {
        return vec![Span::raw(text)];
    }

    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut pos = 0;

    while pos < text.len() {
        match text[pos..].find('@') {
            Some(offset) => {
                let at_pos = pos + offset;
                if at_pos > pos {
                    spans.push(Span::raw(&text[pos..at_pos]));
                }
                let mention_start = at_pos + 1;
                if mention_start < text.len() {
                    let first = bytes[mention_start];
                    if first.is_ascii_alphanumeric() || first == b'_' || first == b'-' {
                        let mention_end = text[mention_start..]
                            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                            .map(|off| mention_start + off)
                            .unwrap_or(text.len());
                        let mention_name = &text[mention_start..mention_end];
                        let full_mention = &text[at_pos..mention_end];

                        if let Some(&color) = mention_colors.get(mention_name) {
                            spans.push(Span::styled(
                                full_mention,
                                Style::default().fg(color).add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            spans.push(Span::styled(
                                full_mention,
                                Style::default().add_modifier(Modifier::BOLD),
                            ));
                        }
                        pos = mention_end;
                        continue;
                    }
                }
                spans.push(Span::raw(&text[at_pos..at_pos + 1]));
                pos = at_pos + 1;
            }
            None => {
                if pos < text.len() {
                    spans.push(Span::raw(&text[pos..]));
                }
                break;
            }
        }
    }

    spans
}

/// Scan text for file-path-like tokens and return styled spans.
/// Paths get underlined; everything else stays as raw text.
fn path_styled_spans(text: &str) -> Vec<Span<'_>> {
    if !text.contains('/') && !text.contains('.') {
        return vec![Span::raw(text)];
    }

    let path_style = Style::default().add_modifier(Modifier::UNDERLINED);
    let bytes = text.as_bytes();
    let mut spans: Vec<Span<'_>> = Vec::new();
    let mut last_end = 0;
    let mut i = 0;

    while i < bytes.len() {
        if is_path_byte(bytes[i]) && (i == 0 || !is_path_byte(bytes[i - 1])) {
            let start = i;
            while i < bytes.len() && is_path_byte(bytes[i]) {
                i += 1;
            }
            let mut end = i;
            // Strip a trailing '.' that looks like end-of-sentence punctuation.
            if end > start && bytes[end - 1] == b'.' {
                let stripped = &text[start..end - 1];
                if is_file_path(stripped) {
                    end -= 1;
                }
            }
            let token = &text[start..end];
            if is_file_path(token) {
                if start > last_end {
                    spans.push(Span::raw(&text[last_end..start]));
                }
                spans.push(Span::styled(token, path_style));
                if end < i {
                    // Stripped trailing punctuation — push it as raw.
                    spans.push(Span::raw(&text[end..i]));
                }
                last_end = i;
            }
        } else {
            i += 1;
        }
    }

    if last_end == 0 && spans.is_empty() {
        return vec![Span::raw(text)];
    }
    if last_end < text.len() {
        spans.push(Span::raw(&text[last_end..]));
    }
    spans
}

fn is_path_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'/' | b'_' | b'-' | b'~')
}

fn is_file_path(token: &str) -> bool {
    if token.len() < 3 {
        return false;
    }
    // Paths containing '/' (e.g., src/auth/login.rs) but not URLs starting with '//'
    if token.contains('/') && !token.starts_with("//") {
        return true;
    }
    // Standalone filenames with known extensions (e.g., login.rs, Cargo.toml)
    if let Some(dot_pos) = token.rfind('.') {
        let ext = &token[dot_pos + 1..];
        let name = &token[..dot_pos];
        if !ext.is_empty()
            && !name.is_empty()
            && KNOWN_FILE_EXTENSIONS.contains(&ext)
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        {
            return true;
        }
    }
    false
}

/// Truncate an agent id to fit a target display width while preserving identity.
/// Uses `prefix…suffix` when space permits so similar prefixes stay distinguishable.
fn truncate_agent_id_for_width(id: &str, max_width: usize) -> String {
    let full_width = UnicodeWidthStr::width(id);
    if full_width <= max_width {
        return id.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "…".to_string();
    }
    if max_width <= 5 {
        let end = display_width_boundary(id, max_width.saturating_sub(1));
        return format!("{}…", &id[..end]);
    }

    // Keep a short suffix so two long IDs with a shared prefix remain distinct.
    let suffix_start = last_n_chars_start(id, 4);
    let suffix = &id[suffix_start..];
    let suffix_width = UnicodeWidthStr::width(suffix);
    let prefix_width = max_width.saturating_sub(1 + suffix_width); // 1 for ellipsis.

    if prefix_width == 0 {
        let end = display_width_boundary(id, max_width.saturating_sub(1));
        return format!("{}…", &id[..end]);
    }

    let prefix_end = display_width_boundary(id, prefix_width);
    let prefix = &id[..prefix_end];
    format!("{prefix}…{suffix}")
}

/// Return byte index for the start of the last `n` chars in a string.
fn last_n_chars_start(s: &str, n: usize) -> usize {
    if n == 0 {
        return s.len();
    }
    let mut remaining = n;
    for (idx, _) in s.char_indices().rev() {
        remaining = remaining.saturating_sub(1);
        if remaining == 0 {
            return idx;
        }
    }
    0
}

/// Count total rendered lines for a set of messages at a given terminal width.
/// Must mirror the line counting logic in `render_messages` exactly.
fn count_rendered_lines(messages: &[Message], inner_width: usize) -> usize {
    let indent_len = 2; // "  "
    let wrap_width = inner_width.saturating_sub(indent_len);
    let mut total = 0;
    for (i, msg) in messages.iter().enumerate() {
        if i > 0 {
            total += 1; // blank line between messages
        }
        total += 1; // header line (always 1 line, truncated not wrapped)
        for content_line in msg.content.lines() {
            if wrap_width == 0 {
                total += 1;
            } else {
                total += wrap_text(content_line, wrap_width).len();
            }
        }
    }
    total
}

/// Word-wrap text into lines of at most `width` display columns, preferring
/// breaks at spaces. Falls back to splitting at character boundaries when no
/// space is found within the width.
fn wrap_text(text: &str, width: usize) -> Vec<&str> {
    if text.is_empty() {
        return vec![""];
    }
    if width == 0 {
        return vec![text];
    }

    let mut result = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        // Find the byte offset where cumulative display width reaches `width`.
        let byte_limit = display_width_boundary(remaining, width);
        if byte_limit >= remaining.len() {
            result.push(remaining);
            break;
        }
        // Try to break at a space within that range.
        let break_at = remaining[..byte_limit]
            .rfind(' ')
            .map(|pos| pos + 1)
            .unwrap_or(byte_limit);
        // Guard against zero-width splits: if the first character is wider than
        // `width` (e.g. a CJK character in a 1-column terminal), byte_limit is 0
        // and no space is found either. Advance past the first character to avoid
        // an infinite loop.
        let break_at = if break_at == 0 {
            next_char_boundary(remaining, 0)
        } else {
            break_at
        };
        result.push(&remaining[..break_at]);
        remaining = &remaining[break_at..];
    }
    result
}

/// Returns the byte offset where cumulative display width reaches `max_width`,
/// or `s.len()` if the entire string fits within `max_width` columns.
fn display_width_boundary(s: &str, max_width: usize) -> usize {
    let mut width = 0;
    for (i, c) in s.char_indices() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if width + w > max_width {
            return i;
        }
        width += w;
    }
    s.len()
}

/// Returns the byte offset of the next character boundary after `pos`.
fn next_char_boundary(s: &str, pos: usize) -> usize {
    s[pos..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| pos + offset)
        .unwrap_or(s.len())
}
