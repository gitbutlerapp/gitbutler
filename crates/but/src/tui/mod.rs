//! A library of TUI components to perform a certain operations.
//!
//! These may be interactive or static, with interactive ones containing *verbs*, and static ones being *nouns*.

pub mod table;
pub use table::types::Table;

pub mod text;

pub mod get_text;

pub(crate) mod diff_viewer;
pub mod editor;
pub(crate) mod stage_viewer;

mod picker;
pub use picker::*;

use std::{
    io,
    sync::{Arc, Mutex},
};

use crossterm::{
    event::{DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal, TerminalOptions, Viewport,
    backend::{CrosstermBackend, TestBackend},
};

type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;

/// RAII guard that ensures the terminal is restored to its original state,
/// even if an error occurs or a panic is caught.
#[must_use]
pub(crate) struct CrosstermTerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    config: CrosstermTerminalConfig,
    /// Holds the original panic hook so we can restore it on drop.
    /// `None` if a panic already fired (the hook consumed itself).
    original_hook: Arc<Mutex<Option<PanicHook>>>,
}

#[derive(Clone)]
struct CrosstermTerminalConfig {
    terminal_options: TerminalOptions,
    uses_alt_screen: bool,
    captures_mouse: bool,
    changes_focus: bool,
}

impl CrosstermTerminalGuard {
    /// Enter raw mode and the alternate screen, optionally enabling mouse capture.
    /// Returns a guard that will restore the terminal on drop.
    pub fn alt_screen(enable_mouse: bool) -> anyhow::Result<Self> {
        Self::new(CrosstermTerminalConfig {
            terminal_options: TerminalOptions {
                viewport: Viewport::Fullscreen,
            },
            uses_alt_screen: true,
            captures_mouse: enable_mouse,
            changes_focus: true,
        })
    }

    /// Enter raw mode and render inline at the current cursor position.
    /// Returns a guard that will restore the terminal on drop.
    pub fn inline(height: u16) -> anyhow::Result<Self> {
        Self::new(CrosstermTerminalConfig {
            terminal_options: TerminalOptions {
                viewport: Viewport::Inline(height),
            },
            uses_alt_screen: false,
            captures_mouse: false,
            changes_focus: false,
        })
    }

    fn new(config: CrosstermTerminalConfig) -> anyhow::Result<Self> {
        let original_hook: Arc<Mutex<Option<PanicHook>>> =
            Arc::new(Mutex::new(Some(std::panic::take_hook())));

        // Install panic hook to restore terminal on panic
        let hook_ref = Arc::clone(&original_hook);
        let hook_config = config.clone();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let _ = leave_terminal_mode(&mut io::stdout(), &hook_config);
            // Take the original hook so it won't be restored on drop after a panic
            if let Some(hook) = hook_ref.lock().ok().and_then(|mut h| h.take()) {
                hook(panic_info);
            }
        }));

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        enter_terminal_mode(&mut stdout, &config)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::with_options(backend, config.terminal_options.clone())?;

        Ok(Self {
            terminal,
            config,
            original_hook,
        })
    }
}

impl Drop for CrosstermTerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = leave_terminal_mode(self.terminal.backend_mut(), &self.config);
        let _ = self.terminal.show_cursor();

        // Restore the original panic hook if it hasn't been consumed by a panic
        if let Some(hook) = self.original_hook.lock().ok().and_then(|mut h| h.take()) {
            std::panic::set_hook(hook);
        }
    }
}

fn enter_terminal_mode<W: io::Write>(
    writer: &mut W,
    config: &CrosstermTerminalConfig,
) -> io::Result<()> {
    if config.uses_alt_screen && config.captures_mouse {
        crossterm::execute!(
            writer,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableFocusChange
        )?;
    } else if config.uses_alt_screen {
        crossterm::execute!(writer, EnterAlternateScreen, EnableFocusChange)?;
    } else if config.changes_focus {
        crossterm::execute!(writer, EnableFocusChange)?;
    }

    Ok(())
}

fn leave_terminal_mode<W: io::Write>(
    writer: &mut W,
    config: &CrosstermTerminalConfig,
) -> io::Result<()> {
    if config.uses_alt_screen && config.captures_mouse {
        crossterm::execute!(
            writer,
            DisableMouseCapture,
            DisableFocusChange,
            LeaveAlternateScreen
        )?;
    } else if config.uses_alt_screen {
        crossterm::execute!(writer, DisableFocusChange, LeaveAlternateScreen)?;
    } else if config.changes_focus {
        crossterm::execute!(writer, DisableFocusChange)?;
    }

    Ok(())
}

/// A terminal guard that renders into an in-memory backend without touching the real terminal.
///
/// This is useful for non-interactive runs (for example profiling with `xctrace`) where terminal
/// input/output APIs can stop the target process due to job-control semantics.
#[must_use]
pub(crate) struct HeadlessTerminalGuard {
    /// The in-memory terminal used by ratatui during headless rendering.
    terminal: Terminal<TestBackend>,
}

impl HeadlessTerminalGuard {
    /// Create a headless terminal guard with a fixed terminal size.
    pub fn new(width: u16, height: u16) -> anyhow::Result<Self> {
        let backend = TestBackend::new(width, height);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl TerminalGuard for HeadlessTerminalGuard {
    type Backend = TestBackend;

    type SuspendGuard<'a>
        = HeadlessSuspendGuard
    where
        Self: 'a;

    fn suspend(&mut self) -> anyhow::Result<Self::SuspendGuard<'_>> {
        Ok(HeadlessSuspendGuard)
    }

    fn terminal_mut(&mut self) -> &mut Terminal<Self::Backend> {
        &mut self.terminal
    }
}

/// A no-op suspend guard used by [`HeadlessTerminalGuard`].
#[must_use]
pub(crate) struct HeadlessSuspendGuard;

impl Drop for HeadlessSuspendGuard {
    fn drop(&mut self) {}
}

pub(crate) trait TerminalGuard {
    type Backend: ratatui::backend::Backend;

    type SuspendGuard<'a>
    where
        Self: 'a;

    /// Temporarily leaves raw mode and restores terminal state to run an external interactive program.
    ///
    /// This can for example be used to suspend a TUI and bring up an editor or run an external
    /// command.
    ///
    /// Returns a RAII guard that restores terminal state when dropped.
    fn suspend(&mut self) -> anyhow::Result<Self::SuspendGuard<'_>>;

    /// Get a mutable reference to the guard's terminal.
    fn terminal_mut(&mut self) -> &mut Terminal<Self::Backend>;
}

impl TerminalGuard for CrosstermTerminalGuard {
    type Backend = CrosstermBackend<io::Stdout>;

    type SuspendGuard<'a> = SuspendGuard<'a>;

    fn suspend(&mut self) -> anyhow::Result<Self::SuspendGuard<'_>> {
        disable_raw_mode()?;
        leave_terminal_mode(self.terminal.backend_mut(), &self.config)?;

        Ok(SuspendGuard(self))
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

/// RAII guard that resumes terminal state suspended by [`TerminalGuard::suspend`].
#[must_use]
pub(crate) struct SuspendGuard<'a>(&'a mut CrosstermTerminalGuard);

impl Drop for SuspendGuard<'_> {
    fn drop(&mut self) {
        _ = enable_raw_mode();
        _ = enter_terminal_mode(self.0.terminal.backend_mut(), &self.0.config);
        if self.0.config.uses_alt_screen {
            _ = self.0.terminal.clear();
        }
    }
}
