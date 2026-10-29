//! A library of TUI components to perform a certain operations.
//!
//! These may be interactive or static, with interactive ones containing *verbs*, and static ones being *nouns*.

pub mod table;
use anyhow::Context as _;
pub use table::types::Table;

pub mod text;

pub mod event_polling;

pub mod get_text;

pub mod editor;

mod picker;
pub use picker::*;

use std::{
    borrow::Cow,
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
    prelude::Backend,
};

use crate::{
    tui::event_polling::EventPolling,
    utils::{DebugAsType, InputOutputChannel, WriteWithUtils},
};

#[cfg(test)]
pub mod test_utils;

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
            LeaveAlternateScreen,
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
        self.terminal.show_cursor()?;

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
        _ = self.0.terminal.hide_cursor();
        if self.0.config.uses_alt_screen {
            _ = self.0.terminal.clear();
        }
    }
}

pub struct EmptyContext;

impl<'a> From<&'a mut but_ctx::Context> for EmptyContext {
    fn from(_ctx: &'a mut but_ctx::Context) -> Self {
        Self
    }
}

pub trait Tui {
    type UpdateContext<'a>;

    fn update<T, E>(
        &mut self,
        terminal_guard: &mut T,
        event_polling: E,
        events: &mut Vec<crossterm::event::Event>,
        out: &mut dyn TuiInputOutputChannel,
        update_ctx: &mut Self::UpdateContext<'_>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
        E: EventPolling;

    fn render<T>(&mut self, terminal_guard: &mut T) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>;
}

/// Required to abstract over input/output channels for the TUI.
///
/// In production we want to require `InputOutputChannel`. This means the caller must check that
/// input is actually supported and return an error otherwise. However in tests we don't want to
/// enforce that.
///
/// So this trait exists such that we can make a fake to use in tests that panics on
/// `prompt_single_line`.
pub trait TuiInputOutputChannel: WriteWithUtils {
    fn prompt_single_line(&mut self, prompt: &str) -> anyhow::Result<Option<String>>;
}

impl TuiInputOutputChannel for InputOutputChannel<'_> {
    fn prompt_single_line(&mut self, prompt: &str) -> anyhow::Result<Option<String>> {
        InputOutputChannel::prompt_single_line(self, prompt)
    }
}

#[derive(Clone, Debug)]
pub struct Clipboard(DebugAsType<Arc<dyn ClipboardImpl + Send>>);

impl Clipboard {
    pub fn live() -> Self {
        struct Live;

        impl ClipboardImpl for Live {
            fn set_text(&self, text: Cow<'_, str>) -> anyhow::Result<()> {
                arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(text))
                    .context("failed to copy to system clipboard")?;
                Ok(())
            }
        }

        Self::new(Live)
    }

    #[cfg(test)]
    pub fn test() -> (Self, Arc<std::sync::Mutex<String>>) {
        struct Test(Arc<std::sync::Mutex<String>>);

        let shared = <Arc<std::sync::Mutex<String>>>::default();

        impl ClipboardImpl for Test {
            fn set_text(&self, text: Cow<'_, str>) -> anyhow::Result<()> {
                *self.0.lock().unwrap() = text.to_string();
                Ok(())
            }
        }

        (Self::new(Test(Arc::clone(&shared))), shared)
    }

    fn new<C>(clipboard_impl: C) -> Self
    where
        C: ClipboardImpl + Send + 'static,
    {
        Self(DebugAsType(Arc::new(clipboard_impl)))
    }

    pub fn set_text<'a>(&self, text: impl Into<Cow<'a, str>>) -> anyhow::Result<()> {
        self.0.set_text(text.into())
    }
}

trait ClipboardImpl {
    fn set_text(&self, text: Cow<'_, str>) -> anyhow::Result<()>;
}
