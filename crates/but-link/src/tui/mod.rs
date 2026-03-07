//! Read-only terminal UI for observing `but link` coordination state.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, IsTerminal as _};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use rusqlite::{Connection, OpenFlags, params};

use crate::payloads::{
    AckHistory, AcquireHistory, BlockHistory, DiscoveryPayload, ResolveHistory, SurfacePayload,
};
use crate::text;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DB_POLL_INTERVAL: Duration = Duration::from_millis(250);
const INITIAL_WINDOW_MS: i64 = 60 * 60 * 1000;
const INITIAL_LIMIT: i64 = 250;
const POLL_LIMIT: i64 = 1000;
const MAX_RETAINED_MESSAGES: usize = 2_000;
const AGENT_STALE_MS: i64 = 10 * 60 * 1000;
const AGENTS_PANEL_MIN_WIDTH: u16 = 72;
const AGENTS_PANEL_WIDTH: u16 = 42;

type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;

#[derive(Clone, Debug)]
struct MessageEntry {
    id: i64,
    created_at_ms: i64,
    agent_id: String,
    kind: String,
    content: String,
}

#[derive(Clone, Debug)]
struct AgentEntry {
    agent_id: String,
    status: Option<String>,
    plan: Option<String>,
    last_seen_at_ms: i64,
    last_progress_at_ms: i64,
}

#[derive(Clone, Debug)]
struct ClaimEntry {
    path: String,
    agent_id: String,
}

#[derive(Clone, Debug)]
struct BlockEntry {
    id: i64,
    agent_id: String,
    mode: String,
    reason: String,
    paths: Vec<String>,
}

#[derive(Debug)]
struct App {
    messages: Vec<MessageEntry>,
    agents: Vec<AgentEntry>,
    claims: Vec<ClaimEntry>,
    blocks: Vec<BlockEntry>,
    last_message_id: i64,
    scroll_offset: usize,
    auto_scroll: bool,
    should_quit: bool,
    consecutive_poll_errors: usize,
    last_error: Option<String>,
}

impl App {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            agents: Vec::new(),
            claims: Vec::new(),
            blocks: Vec::new(),
            last_message_id: 0,
            scroll_offset: 0,
            auto_scroll: true,
            should_quit: false,
            consecutive_poll_errors: 0,
            last_error: None,
        }
    }
}

/// RAII guard that restores terminal state, including on panic.
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    original_hook: Arc<Mutex<Option<PanicHook>>>,
}

impl TerminalGuard {
    fn new() -> anyhow::Result<Self> {
        let original_hook: Arc<Mutex<Option<PanicHook>>> =
            Arc::new(Mutex::new(Some(std::panic::take_hook())));

        let hook_ref = Arc::clone(&original_hook);
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
            if let Ok(hook_guard) = hook_ref.lock()
                && let Some(hook) = hook_guard.as_ref()
            {
                hook(panic_info);
            }
        }));

        let terminal = (|| -> anyhow::Result<Terminal<CrosstermBackend<io::Stdout>>> {
            enable_raw_mode()?;
            let mut stdout = io::stdout();
            crossterm::execute!(stdout, EnterAlternateScreen)?;
            let backend = CrosstermBackend::new(stdout);
            Ok(Terminal::new(backend)?)
        })();

        match terminal {
            Ok(terminal) => Ok(Self {
                terminal,
                original_hook,
            }),
            Err(err) => {
                // Clean up terminal state on error so we don't leave it broken.
                let _ = disable_raw_mode();
                let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
                if let Some(hook) = original_hook.lock().ok().and_then(|mut h| h.take()) {
                    std::panic::set_hook(hook);
                }
                Err(err)
            }
        }
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
        if let Some(hook) = self.original_hook.lock().ok().and_then(|mut h| h.take()) {
            std::panic::set_hook(hook);
        }
    }
}

pub(crate) fn run(current_dir: &Path) -> anyhow::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("but link tui requires an interactive terminal (TTY)");
    }

    let git_dir = crate::repo::discover_git_dir(current_dir)?;
    let db_path = git_dir.join("gitbutler").join("but-link.db");
    if !db_path.is_file() {
        anyhow::bail!(
            "no link database found at {} (run a `but link` command first)",
            db_path.display()
        );
    }

    let mut guard = TerminalGuard::new()?;
    run_event_loop(guard.terminal_mut(), &db_path)
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    db_path: &Path,
) -> anyhow::Result<()> {
    let mut app = App::new();
    let mut last_poll = Instant::now() - DB_POLL_INTERVAL;

    loop {
        if event::poll(EVENT_POLL_INTERVAL)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            let page_size = terminal
                .size()
                .map(|s| s.height.saturating_sub(6) as usize)
                .unwrap_or(10);
            handle_key(&mut app, key.code, page_size);
        }

        if app.should_quit {
            return Ok(());
        }

        if last_poll.elapsed() >= DB_POLL_INTERVAL {
            poll_db(&mut app, db_path);
            last_poll = Instant::now();
        }

        terminal.draw(|frame| render(frame, &app))?;
    }
}

fn handle_key(app: &mut App, key: KeyCode, page_size: usize) {
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
            app.scroll_offset = app.scroll_offset.saturating_add(page_size.max(1));
            app.auto_scroll = false;
        }
        KeyCode::PageDown => {
            app.scroll_offset = app.scroll_offset.saturating_sub(page_size.max(1));
            if app.scroll_offset == 0 {
                app.auto_scroll = true;
            }
        }
        KeyCode::Home => {
            app.scroll_offset = usize::MAX;
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
    let conn = match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(err) => {
            record_poll_error(app, &format!("open db: {err}"));
            return;
        }
    };

    if let Err(err) = poll_messages(app, &conn) {
        record_poll_error(app, &format!("messages: {err}"));
        return;
    }
    if let Err(err) = poll_agents(app, &conn) {
        record_poll_error(app, &format!("agents: {err}"));
        return;
    }
    if let Err(err) = poll_claims(app, &conn) {
        record_poll_error(app, &format!("claims: {err}"));
        return;
    }
    if let Err(err) = poll_blocks(app, &conn) {
        record_poll_error(app, &format!("blocks: {err}"));
        return;
    }

    // Hide agents inactive beyond AGENT_STALE_MS that hold no active claims.
    if let Ok(now_ms) = crate::db::now_unix_ms() {
        let cutoff_ms = now_ms.saturating_sub(AGENT_STALE_MS);
        let agents_with_claims: BTreeSet<String> =
            app.claims.iter().map(|c| c.agent_id.clone()).collect();
        let agents_with_blocks: BTreeSet<String> =
            app.blocks.iter().map(|b| b.agent_id.clone()).collect();
        app.agents.retain(|a| {
            a.last_progress_at_ms >= cutoff_ms
                || agents_with_claims.contains(&a.agent_id)
                || agents_with_blocks.contains(&a.agent_id)
        });
    }

    app.consecutive_poll_errors = 0;
    app.last_error = None;
}

fn record_poll_error(app: &mut App, err: &str) {
    app.consecutive_poll_errors = app.consecutive_poll_errors.saturating_add(1);
    if app.consecutive_poll_errors >= 3 {
        app.last_error = Some(err.to_owned());
    }
}

fn poll_messages(app: &mut App, conn: &Connection) -> anyhow::Result<()> {
    if app.messages.is_empty() {
        let now_ms = crate::db::now_unix_ms()?;
        let since_ms = now_ms.saturating_sub(INITIAL_WINDOW_MS);
        let mut stmt = conn.prepare(
            "SELECT id, created_at_ms, agent_id, kind, body_json FROM messages \
             WHERE created_at_ms >= ?1 \
             ORDER BY id ASC \
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![since_ms, INITIAL_LIMIT], map_message_row)?;
        for row in rows {
            let entry = row?;
            app.last_message_id = app.last_message_id.max(entry.id);
            app.messages.push(entry);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, created_at_ms, agent_id, kind, body_json FROM messages \
             WHERE id > ?1 \
             ORDER BY id ASC \
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![app.last_message_id, POLL_LIMIT], map_message_row)?;
        for row in rows {
            let entry = row?;
            app.last_message_id = app.last_message_id.max(entry.id);
            app.messages.push(entry);
        }
    }

    if app.messages.len() > MAX_RETAINED_MESSAGES {
        let excess = app.messages.len() - MAX_RETAINED_MESSAGES;
        app.messages.drain(..excess);
    }

    Ok(())
}

fn map_message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageEntry> {
    let body_json = row.get::<_, String>(4)?;
    let kind = row.get::<_, String>(3)?;
    let content = format_message_content(&kind, &body_json);
    Ok(MessageEntry {
        id: row.get::<_, i64>(0)?,
        created_at_ms: row.get::<_, i64>(1)?,
        agent_id: row.get::<_, String>(2)?,
        kind,
        content,
    })
}

fn poll_agents(app: &mut App, conn: &Connection) -> anyhow::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT agent_id, status, plan, last_seen_at_ms, last_progress_at_ms
         FROM agent_state \
         ORDER BY last_progress_at_ms DESC, agent_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(AgentEntry {
            agent_id: row.get::<_, String>(0)?,
            status: row.get::<_, Option<String>>(1)?,
            plan: row.get::<_, Option<String>>(2)?,
            last_seen_at_ms: row.get::<_, i64>(3)?,
            last_progress_at_ms: row.get::<_, i64>(4)?,
        })
    })?;

    app.agents = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(())
}

fn poll_claims(app: &mut App, conn: &Connection) -> anyhow::Result<()> {
    let now_ms = crate::db::now_unix_ms()?;
    let mut stmt = conn.prepare(
        "SELECT path, agent_id, expires_at_ms FROM claims \
         WHERE expires_at_ms > ?1 \
         ORDER BY path ASC, agent_id ASC",
    )?;
    let rows = stmt.query_map(params![now_ms], |row| {
        Ok(ClaimEntry {
            path: row.get::<_, String>(0)?,
            agent_id: row.get::<_, String>(1)?,
        })
    })?;

    app.claims = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(())
}

fn poll_blocks(app: &mut App, conn: &Connection) -> anyhow::Result<()> {
    let now_ms = crate::db::now_unix_ms()?;
    let mut stmt = conn.prepare(
        "SELECT b.id, b.agent_id, b.mode, b.reason, bp.path
         FROM blocks b
         JOIN block_paths bp ON bp.block_id = b.id
         WHERE b.resolved_at_ms IS NULL
           AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?1)
         ORDER BY b.id ASC, bp.path ASC",
    )?;
    let rows = stmt.query_map(params![now_ms], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut grouped: BTreeMap<i64, BlockEntry> = BTreeMap::new();
    for row in rows {
        let (id, agent_id, mode, reason, path) = row?;
        let entry = grouped.entry(id).or_insert_with(|| BlockEntry {
            id,
            agent_id,
            mode,
            reason,
            paths: Vec::new(),
        });
        entry.paths.push(path);
    }
    app.blocks = grouped.into_values().collect();
    Ok(())
}

fn render(frame: &mut ratatui::Frame<'_>, app: &App) {
    let root = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(root);

    let show_agents_panel = root.width >= AGENTS_PANEL_MIN_WIDTH;
    if show_agents_panel {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(AGENTS_PANEL_WIDTH)])
            .split(vertical[0]);
        render_messages(frame, app, horizontal[0]);
        render_agents(frame, app, horizontal[1]);
    } else {
        render_messages(frame, app, vertical[0]);
    }
    render_footer(frame, app, vertical[1]);
}

fn render_messages(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let mut lines: Vec<Line<'_>> = Vec::new();
    for (idx, msg) in app.messages.iter().enumerate() {
        if idx > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", format_hms_utc(msg.created_at_ms)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{} ", msg.agent_id),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({})", msg.kind),
                Style::default().fg(kind_color(&msg.kind)),
            ),
        ]));
        for line in msg.content.lines() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::raw(line.to_owned()),
            ]));
        }
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    let total_lines = lines.len();
    let max_scroll_top = total_lines.saturating_sub(inner_height);
    let scroll_offset = if app.auto_scroll {
        0
    } else {
        app.scroll_offset.min(max_scroll_top)
    };
    let scroll_top = max_scroll_top.saturating_sub(scroll_offset);
    let title_suffix = if app.auto_scroll {
        String::new()
    } else {
        format!(" (scroll +{scroll_offset})")
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Messages{}", title_suffix)),
        )
        .scroll((scroll_top.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, area);
}

fn render_agents(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let mut claims_by_agent: BTreeMap<&str, Vec<&ClaimEntry>> = BTreeMap::new();
    let mut owners_by_path: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for claim in &app.claims {
        claims_by_agent
            .entry(claim.agent_id.as_str())
            .or_default()
            .push(claim);
        owners_by_path
            .entry(claim.path.as_str())
            .or_default()
            .insert(claim.agent_id.as_str());
    }

    let mut lines: Vec<Line<'_>> = Vec::new();
    if !app.blocks.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("open blocks: {}", app.blocks.len()),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        for block in app.blocks.iter().take(3) {
            lines.push(Line::from(Span::styled(
                format!(
                    "  #{} {} {} [{}]",
                    block.id,
                    block.agent_id,
                    block.reason,
                    block.paths.join(", ")
                ),
                if block.mode == "hard" {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            )));
        }
        lines.push(Line::raw(""));
    }

    for agent in &app.agents {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(vec![
            Span::styled(
                agent.agent_id.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " (seen {} · progress {})",
                    relative_age(agent.last_seen_at_ms),
                    relative_age(agent.last_progress_at_ms)
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        if let Some(status) = &agent.status {
            lines.push(Line::from(Span::styled(
                format!("  status: {status}"),
                Style::default().fg(Color::Gray),
            )));
        }
        if let Some(plan) = &agent.plan {
            lines.push(Line::from(Span::styled(
                format!("  plan: {plan}"),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if let Some(claims) = claims_by_agent.get(agent.agent_id.as_str()) {
            lines.push(Line::from(Span::styled(
                format!("  claims: {}", claims.len()),
                Style::default().fg(Color::Yellow),
            )));
            for claim in claims.iter().take(3) {
                let shared = owners_by_path
                    .get(claim.path.as_str())
                    .is_some_and(|owners| owners.len() > 1);
                let prefix = if shared { "  ! " } else { "  - " };
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{}", claim.path),
                    if shared {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                )));
            }
            let shared_count = claims
                .iter()
                .filter(|claim| {
                    owners_by_path
                        .get(claim.path.as_str())
                        .is_some_and(|owners| owners.len() > 1)
                })
                .count();
            if shared_count > 0 {
                lines.push(Line::from(Span::styled(
                    format!("  shared collisions: {shared_count}"),
                    Style::default().fg(Color::Red),
                )));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no active agents)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
            .title(format!(
                " Agents · {}m ({})",
                AGENT_STALE_MS / 60_000,
                app.agents.len()
            )),
    );
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let border = Block::default().borders(Borders::TOP);
    let inner = border.inner(area);
    frame.render_widget(border, area);

    let paragraph = if let Some(err) = &app.last_error {
        Paragraph::new(Line::from(vec![
            Span::styled("DB error: ", Style::default().fg(Color::Red)),
            Span::styled(err.clone(), Style::default().fg(Color::Red)),
            Span::styled(" (retrying)", Style::default().fg(Color::DarkGray)),
        ]))
    } else {
        Paragraph::new(Line::from(Span::styled(
            "q quit | Up/Down scroll | PgUp/PgDn page | Home/End",
            Style::default().fg(Color::DarkGray),
        )))
    };
    frame.render_widget(paragraph, inner);
}

fn kind_color(kind: &str) -> Color {
    match kind {
        "discovery" => Color::Cyan,
        "intent" => Color::Yellow,
        "declaration" => Color::Magenta,
        "block" => Color::Red,
        "resolve" => Color::Green,
        "ack" => Color::Yellow,
        "acquire" => Color::Green,
        "claim" => Color::Green,
        "release" => Color::DarkGray,
        _ => Color::Gray,
    }
}

fn format_hms_utc(created_at_ms: i64) -> String {
    let seconds = created_at_ms.div_euclid(1000);
    let sec_in_day = seconds.rem_euclid(86_400);
    let hour = sec_in_day / 3_600;
    let minute = (sec_in_day % 3_600) / 60;
    let second = sec_in_day % 60;
    format!("{hour:02}:{minute:02}:{second:02}")
}

fn relative_age(updated_at_ms: i64) -> String {
    let now_ms = crate::db::now_unix_ms().unwrap_or(updated_at_ms);
    let delta_ms = now_ms.saturating_sub(updated_at_ms).max(0);
    let delta_s = delta_ms / 1000;
    if delta_s < 60 {
        "now".to_owned()
    } else if delta_s < 3_600 {
        format!("{}m", delta_s / 60)
    } else if delta_s < 86_400 {
        format!("{}h", delta_s / 3_600)
    } else {
        format!("{}d", delta_s / 86_400)
    }
}

fn format_message_content(kind: &str, body_json: &str) -> String {
    let (obj, fallback) = text::parse_body(body_json);
    match kind {
        "discovery" => DiscoveryPayload::from_json_str(body_json)
            .map(|payload| {
                let mut out = payload.title.clone();
                if !payload.evidence.is_empty() {
                    out.push_str(&format!(" ({} evidence)", payload.evidence.len()));
                }
                if let Some(cmd) = payload.command() {
                    out.push_str(&format!("\n  action: {cmd}"));
                }
                out
            })
            .unwrap_or(fallback),
        "intent" | "declaration" => SurfacePayload::from_json_str(body_json)
            .map(|payload| {
                format!(
                    "{} [{}] surface: {}",
                    payload.scope,
                    payload.tags.join(", "),
                    payload.surface.join(", ")
                )
            })
            .unwrap_or(fallback),
        "block" => BlockHistory::from_json_str(body_json)
            .map(|payload| {
                format!(
                    "{} block: {}\n  paths: {}",
                    payload.mode,
                    payload.reason,
                    payload.paths.join(", ")
                )
            })
            .unwrap_or_else(|_| {
                let reason = obj
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&fallback);
                let mode = obj
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("advisory");
                let paths: Vec<&str> = obj
                    .get("paths")
                    .and_then(|v| v.as_array())
                    .map_or(Vec::new(), |a| {
                        a.iter().filter_map(|v| v.as_str()).collect()
                    });
                format!("{mode} block: {reason}\n  paths: {}", paths.join(", "))
            }),
        "ack" => AckHistory::from_json_str(body_json)
            .map(|payload| format!("ack -> {}: {}", payload.target_agent_id, payload.text))
            .unwrap_or_else(|_| {
                let target = obj
                    .get("target_agent_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                format!("ack -> {target}: {fallback}")
            }),
        "resolve" => ResolveHistory::from_json_str(body_json)
            .map(|payload| format!("resolved block #{}", payload.block_id))
            .unwrap_or_else(|_| {
                let block_id = obj
                    .get("block_id")
                    .and_then(|v| v.as_i64())
                    .unwrap_or_default();
                format!("resolved block #{block_id}")
            }),
        "acquire" => AcquireHistory::from_json_str(body_json)
            .map(|payload| {
                if payload.acquired_paths.is_empty() {
                    payload.text
                } else {
                    format!("acquired: {}", payload.acquired_paths.join(", "))
                }
            })
            .unwrap_or(fallback),
        _ => fallback,
    }
}
