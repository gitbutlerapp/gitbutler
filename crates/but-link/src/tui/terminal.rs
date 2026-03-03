//! Terminal setup and teardown helpers for the `but link` TUI.

use std::io;
use std::sync::{Arc, Mutex};

use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// Stored panic hook type restored when the TUI exits.
type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;

/// RAII guard that restores terminal state, including on panic.
pub(crate) struct TerminalGuard {
    /// Active ratatui terminal instance.
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    /// Previously installed panic hook to restore on drop.
    original_hook: Arc<Mutex<Option<PanicHook>>>,
}

impl TerminalGuard {
    /// Enter alternate-screen/raw-mode terminal operation.
    ///
    /// # Errors
    ///
    /// Returns an error when terminal setup fails.
    pub(crate) fn new() -> anyhow::Result<Self> {
        let original_hook: Arc<Mutex<Option<PanicHook>>> =
            Arc::new(Mutex::new(Some(std::panic::take_hook())));

        let hook_ref = Arc::clone(&original_hook);
        std::panic::set_hook(Box::new(move |panic_info| {
            disable_raw_mode().ok();
            crossterm::execute!(io::stdout(), LeaveAlternateScreen).ok();
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
                disable_raw_mode().ok();
                crossterm::execute!(io::stdout(), LeaveAlternateScreen).ok();
                if let Some(hook) = original_hook.lock().ok().and_then(|mut hook| hook.take()) {
                    std::panic::set_hook(hook);
                }
                Err(err)
            }
        }
    }

    /// Return a mutable reference to the active terminal.
    pub(crate) fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        disable_raw_mode().ok();
        crossterm::execute!(self.terminal.backend_mut(), LeaveAlternateScreen).ok();
        self.terminal.show_cursor().ok();
        if let Some(hook) = self
            .original_hook
            .lock()
            .ok()
            .and_then(|mut hook| hook.take())
        {
            std::panic::set_hook(hook);
        }
    }
}
