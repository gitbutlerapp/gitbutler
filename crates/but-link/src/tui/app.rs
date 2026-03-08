//! TUI state and polling/event-loop behavior.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rusqlite::{Connection, OpenFlags};

use crate::read::{
    AgentPanelEntry, BlockListEntry, ClaimListEntry, DiscoveryListEntry, MessageDisplayEntry,
    SurfaceListEntry,
};

use super::render;

/// Keyboard event polling interval.
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Database polling interval for refreshed snapshots.
const DB_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Initial recent-message window shown by the TUI.
const INITIAL_WINDOW_MS: i64 = 60 * 60 * 1000;

/// Maximum number of transcript messages loaded into the TUI.
const MESSAGE_LIMIT: i64 = 500;

/// In-memory application state for the `but link` TUI.
#[derive(Debug)]
pub(crate) struct App {
    /// Free-text transcript messages shown in the main pane.
    pub messages: Vec<MessageDisplayEntry>,
    /// Discovery rows shown above the transcript.
    pub discoveries: Vec<DiscoveryListEntry>,
    /// Surface declarations shown above the transcript.
    pub surfaces: Vec<SurfaceListEntry>,
    /// Agent panel rows.
    pub agents: Vec<AgentPanelEntry>,
    /// Active claim rows.
    pub claims: Vec<ClaimListEntry>,
    /// Open typed block rows.
    pub blocks: Vec<BlockListEntry>,
    /// Scroll offset from the bottom of the transcript pane.
    pub scroll_offset: usize,
    /// Whether the transcript automatically follows the newest content.
    pub auto_scroll: bool,
    /// Whether the event loop should exit.
    pub should_quit: bool,
    /// Number of consecutive database polling failures.
    pub consecutive_poll_errors: usize,
    /// Most recent persistent polling error, when present.
    pub last_error: Option<String>,
}

impl App {
    /// Create a fresh empty TUI state.
    pub(crate) fn new() -> Self {
        Self {
            messages: Vec::new(),
            discoveries: Vec::new(),
            surfaces: Vec::new(),
            agents: Vec::new(),
            claims: Vec::new(),
            blocks: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            should_quit: false,
            consecutive_poll_errors: 0,
            last_error: None,
        }
    }
}

/// Run the TUI event loop against the provided database path.
///
/// # Errors
///
/// Returns an error when terminal drawing or input polling fails.
pub(crate) fn run_event_loop(
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
                .map(|size| size.height.saturating_sub(6) as usize)
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

        terminal.draw(|frame| render::render(frame, &app))?;
    }
}

/// Update TUI state in response to a pressed key.
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

/// Poll the database and refresh the in-memory snapshot.
fn poll_db(app: &mut App, db_path: &Path) {
    let conn = match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(err) => {
            record_poll_error(app, &format!("open db: {err}"));
            return;
        }
    };

    let now_ms = crate::db::now_unix_ms().unwrap_or_default();
    let since_ms = now_ms.saturating_sub(INITIAL_WINDOW_MS);
    let snapshot = match crate::services::read::tui_snapshot(&conn, since_ms, MESSAGE_LIMIT, now_ms)
    {
        Ok(snapshot) => snapshot,
        Err(err) => {
            record_poll_error(app, &format!("snapshot: {err}"));
            return;
        }
    };

    app.messages = snapshot.messages;
    app.discoveries = snapshot.discoveries;
    app.surfaces = snapshot.surfaces;
    app.agents = snapshot.agents;
    app.claims = snapshot.claims;
    app.blocks = snapshot.blocks;
    app.consecutive_poll_errors = 0;
    app.last_error = None;
}

/// Record a polling error and surface it after repeated failures.
fn record_poll_error(app: &mut App, err: &str) {
    app.consecutive_poll_errors = app.consecutive_poll_errors.saturating_add(1);
    if app.consecutive_poll_errors >= 3 {
        app.last_error = Some(err.to_owned());
    }
}
