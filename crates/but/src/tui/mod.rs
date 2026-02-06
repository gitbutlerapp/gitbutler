//! A library of TUI components to perform a certain operations.
//!
//! These may be interactive or static, with interactive ones containing *verbs*, and static ones being *nouns*.

pub mod table;
pub use table::types::Table;

pub mod get_text;

pub(crate) mod diff_viewer;
pub(crate) mod stage_viewer;

use std::io;
use std::sync::{Arc, Mutex};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;

/// RAII guard that ensures the terminal is restored to its original state,
/// even if an error occurs or a panic is caught.
pub(crate) struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    mouse_captured: bool,
    /// Holds the original panic hook so we can restore it on drop.
    /// `None` if a panic already fired (the hook consumed itself).
    original_hook: Arc<Mutex<Option<PanicHook>>>,
}

impl TerminalGuard {
    /// Enter raw mode, alternate screen, and optionally enable mouse capture.
    /// Returns a guard that will restore the terminal on drop.
    pub fn new(enable_mouse: bool) -> anyhow::Result<Self> {
        let original_hook: Arc<Mutex<Option<PanicHook>>> = Arc::new(Mutex::new(Some(std::panic::take_hook())));

        // Install panic hook to restore terminal on panic
        let hook_ref = Arc::clone(&original_hook);
        let mouse = enable_mouse;
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            if mouse {
                let _ = crossterm::execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
            } else {
                let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
            }
            // Take the original hook so it won't be restored on drop after a panic
            if let Some(hook) = hook_ref.lock().ok().and_then(|mut h| h.take()) {
                hook(panic_info);
            }
        }));

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if enable_mouse {
            crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        } else {
            crossterm::execute!(stdout, EnterAlternateScreen)?;
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            mouse_captured: enable_mouse,
            original_hook,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        if self.mouse_captured {
            let _ = crossterm::execute!(self.terminal.backend_mut(), DisableMouseCapture, LeaveAlternateScreen);
        } else {
            let _ = crossterm::execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        }
        let _ = self.terminal.show_cursor();

        // Restore the original panic hook if it hasn't been consumed by a panic
        if let Some(hook) = self.original_hook.lock().ok().and_then(|mut h| h.take()) {
            std::panic::set_hook(hook);
        }
    }
}
