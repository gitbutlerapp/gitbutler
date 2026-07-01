#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use std::{
    rc::Rc,
    sync::{Arc, atomic::AtomicBool, mpsc::Receiver},
    time::Duration,
};

use anyhow::Context as _;
use bstr::BString;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_settings::AppSettingsWithDiskSync;
use crossterm::event::Event;
use gitbutler_operating_modes::OperatingMode;
use gix::refs::FullName;
use nonempty::NonEmpty;
use ratatui::prelude::*;

use crate::{
    CliId,
    command::legacy::status::{
        StatusFlags, StatusOutputLine, TuiLaunchOptions, TuiOutcome, TuiRunOptions,
        tui::{
            app::{
                CommandMessage, CommandModeKind, CommitMessage, JumpMessage, MoveMessage,
                NormalMode, PickChangesMode, RewordMessage, RubMessage, StackMessage,
            },
            backstack::{Backstack, BackstackEntry},
            confirm::ConfirmMessage,
            cursor::Cursor,
            details::{DetailsMessage, RenderNextChunkResult},
            event_polling::{CrosstermEventPolling, EventPolling, NoopEventPolling},
            fuzzy_picker::{
                Col, FuzzyPicker, FuzzyPickerItem, FuzzyPickerMessage, SearchableToken,
            },
            help::HelpMessage,
            key_bind::{KeyBinds, fuzzy_picker_key_binds},
            marking::{Markable, Marks},
            message_on_drop::MessageOnDrop,
            mode::{Mode, ModeDiscriminant},
            operations::stack_has_assigned_changes,
            toast::ToastKind,
        },
    },
    tui::{CrosstermTerminalGuard, HeadlessTerminalGuard, TerminalGuard},
    utils::{DebugAsType, InputOutputChannel, WriteWithUtils},
};

use render::render_app;

use app::{App, InlineRewordMode, Modal, format_error_for_tui};

mod app;
mod backstack;
mod confirm;
mod copy_selection_picker;
mod cursor;
mod details;
mod event_polling;
mod file_browser;
mod fps;
mod fuzzy_picker;
mod graph_extension;
mod help;
mod highlight;
mod key_bind;
mod marking;
mod message_on_drop;
mod mode;
mod operations;
mod popup;
mod render;
mod toast;

#[cfg(test)]
mod tests;

const NOOP: &str = "noop";
const CURSOR_CONTEXT_ROWS: usize = 3;

/// How much does the detail area grow/shrink with when adjusted
const DETAILS_SIZE_ADJUSTMENT_PERCENTAGE: u16 = 5;

const DETAILS_MIN_SIZE_PERCENTAGE: u16 = 30;
const DETAILS_MAX_SIZE_PERCENTAGE: u16 = 90;

/// How long to ignore watcher reloads after a TUI mutation.
///
/// Covers the watcher's normal idle debounce, with extra time for Windows'
/// slower watcher tick. Intentionally shorter than the max debounce timeout.
const WATCHER_SELF_ECHO_SUPPRESSION: Duration = if cfg!(windows) {
    Duration::from_secs(1)
} else {
    Duration::from_millis(500)
};

pub fn render_tui(
    ctx: &mut Context,
    out: &mut InputOutputChannel<'_>,
    mode: &OperatingMode,
    flags: StatusFlags,
    status_lines: Vec<StatusOutputLine>,
    launch_options: TuiLaunchOptions,
    run_options: TuiRunOptions,
) -> anyhow::Result<(Vec<StatusOutputLine>, TuiOutcome)> {
    let mut app = App::new(
        status_lines,
        flags,
        launch_options,
        run_options,
        ctx.settings.feature_flags.tui_file_browser,
    );

    let mut messages = Vec::new();

    // second buffer so we can send messages from `App::handle_message`
    let mut other_messages = Vec::new();

    let outcome = if app.launch_options.headless {
        let mut terminal_guard = HeadlessTerminalGuard::new(240, 240)?;
        let event_polling = NoopEventPolling;

        render_loop(
            &mut app,
            &mut terminal_guard,
            event_polling,
            &mut messages,
            &mut other_messages,
            <Arc<AtomicBool>>::default(),
            ctx,
            out,
            mode,
        )?
    } else {
        let (_watcher_handle, received_watcher_event) =
            start_watcher(ctx).context("failed to start filesystem watcher")?;

        let mut terminal_guard = CrosstermTerminalGuard::alt_screen(true)?;
        let event_polling = CrosstermEventPolling;

        render_loop(
            &mut app,
            &mut terminal_guard,
            event_polling,
            &mut messages,
            &mut other_messages,
            received_watcher_event,
            ctx,
            out,
            mode,
        )?
    };

    Ok((app.status_lines, outcome))
}

fn render_loop<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: E,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
    received_watcher_event: Arc<AtomicBool>,
    ctx: &mut Context,
    out: &mut dyn TuiInputOutputChannel,
    mode: &OperatingMode,
) -> anyhow::Result<TuiOutcome>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    E: EventPolling + Copy,
{
    render(app, terminal_guard)?;

    loop {
        if app
            .launch_options
            .quit_after
            .is_some_and(|quit_after| quit_after <= app.updates)
        {
            break Ok(TuiOutcome::None);
        }

        render_loop_once(
            app,
            terminal_guard,
            event_polling,
            messages,
            other_messages,
            &received_watcher_event,
            ctx,
            out,
            mode,
        )?;

        if let Some(outcome) = app.outcome.take() {
            break Ok(outcome);
        }
    }
}

#[expect(clippy::too_many_arguments)]
fn render_loop_once<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: E,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
    received_watcher_event: &AtomicBool,
    ctx: &mut Context,
    out: &mut dyn TuiInputOutputChannel,
    mode: &OperatingMode,
) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    E: EventPolling,
{
    update(
        app,
        terminal_guard,
        event_polling,
        messages,
        other_messages,
        received_watcher_event,
        ctx,
        out,
        mode,
    )?;

    render(app, terminal_guard)?;

    app.fps.frame_finished();

    Ok(())
}

#[expect(clippy::too_many_arguments)]
fn update<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: E,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
    received_watcher_event: &AtomicBool,
    ctx: &mut Context,
    out: &mut dyn TuiInputOutputChannel,
    mode: &OperatingMode,
) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    E: EventPolling,
{
    app.updates += 1;

    // update at full speed while we're rendering the diff
    let event_poll_timeout = if app.details.needs_update(app.is_details_visible) {
        Duration::from_millis(0)
    } else {
        Duration::from_millis(30)
    };
    // poll terminal events
    for event in event_polling.poll(event_poll_timeout)? {
        let picker_shown = match &app.modal {
            Some(
                Modal::GotoBranchPicker { .. }
                | Modal::ApplyStackPicker { .. }
                | Modal::CopySelectionPicker { .. },
            ) => true,
            Some(Modal::Confirm { .. } | Modal::Help { .. }) | None => false,
        };
        event_to_messages(
            event,
            app.active_key_binds(),
            &app.mode,
            picker_shown,
            messages,
        );
    }

    // check for any out of band messages
    app.incoming_out_of_band_messages
        .retain(|rx| match rx.try_recv() {
            Ok(msg) => {
                messages.push(msg);
                false
            }
            Err(err) => match err {
                std::sync::mpsc::TryRecvError::Empty => true,
                std::sync::mpsc::TryRecvError::Disconnected => false,
            },
        });

    // check for events from the watcher
    if received_watcher_event
        .compare_exchange(
            true,
            false,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_ok_and(|value| value)
        && app
            .previous_reload_caused_by_mutation_timestamp
            .is_none_or(|timestamp| timestamp.elapsed() > WATCHER_SELF_ECHO_SUPPRESSION)
    {
        messages.push(Message::Reload(None, ReloadCause::Watcher));
    }

    // handle messages
    let mut did_reload = false;
    messages.append(&mut app.delayed_messages);
    loop {
        if messages.is_empty() {
            break;
        }
        for msg in messages.drain(..) {
            if matches!(msg, Message::Reload(..)) {
                if did_reload && cfg!(feature = "tui-profiling") && !cfg!(test) {
                    app.toasts
                        .insert(ToastKind::Error, "Double reload".to_owned());
                } else {
                    did_reload = true;
                }
            }
            app.handle_message(ctx, out, mode, terminal_guard, other_messages, msg);
        }
        std::mem::swap(messages, other_messages);
    }

    if app.toasts.update() {
        app.should_render = true;
    }

    if app.highlight.update() {
        app.should_render = true;
    }

    if app.details.update_highlight() {
        app.should_render = true;
    }

    let selection = app
        .cursor
        .selected_line(&app.status_lines)
        .and_then(|line| line.data.cli_id())
        .map(|id| &**id);

    if app.details.needs_update(app.is_details_visible) {
        match app.details.update(ctx, selection) {
            Ok(Some(result)) => match result {
                RenderNextChunkResult::Done => {
                    if app.launch_options.quit_after_rendering_full_diff {
                        app.outcome = Some(TuiOutcome::None);
                    }
                }
                RenderNextChunkResult::Meta | RenderNextChunkResult::Diff => {}
            },
            Ok(None) => {}
            Err(err) => {
                messages.push(Message::ShowError(Arc::new(err)));
            }
        }
        app.should_render = true;
    }

    if let Some(file_browser) = &mut app.file_browser
        && let Mode::Details(details_mode) = &*app.mode
        && file_browser.needs_update(app.is_details_visible && details_mode.full_screen)
    {
        match file_browser.update(ctx, selection) {
            Ok(true) => {
                app.should_render = true;
            }
            Ok(false) => {}
            Err(err) => {
                messages.push(Message::ShowError(Arc::new(err)));
            }
        }
    }

    if app.fps.update() {
        app.should_render = true;
    }

    Ok(())
}

fn render<T>(app: &mut App, terminal_guard: &mut T) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
{
    if std::mem::take(&mut app.should_render) {
        let _span = tracing::trace_span!("render").entered();
        terminal_guard.terminal_mut().draw(|frame| {
            app.renders += 1;
            render_app(app, frame)
        })?;
    }

    Ok(())
}

fn event_to_messages(
    ev: Event,
    key_binds: &KeyBinds,
    mode: &Mode,
    picker_shown: bool,
    messages: &mut Vec<Message>,
) {
    match ev {
        Event::Key(key) => {
            let mut handled = false;
            for key_bind in key_binds.iter_key_binds_available_in_mode(ModeDiscriminant::from(mode))
            {
                if key_bind.matches(&key) {
                    messages.push(key_bind.message());
                    handled = true;
                }
            }

            if !handled {
                if picker_shown {
                    messages.push(Message::FuzzyPicker(FuzzyPickerMessage::Input(ev)));
                } else {
                    match mode {
                        Mode::InlineReword(..) => {
                            messages.push(Message::Reword(RewordMessage::InlineInput(ev)));
                        }
                        Mode::Command(..) => {
                            messages.push(Message::Command(CommandMessage::Input(ev)));
                        }
                        Mode::Jump(..) => {
                            messages.push(Message::Jump(JumpMessage::Input(ev)));
                        }
                        Mode::Normal(..)
                        | Mode::Details(..)
                        | Mode::Rub(..)
                        | Mode::Commit(..)
                        | Mode::Stack(..)
                        | Mode::PickChanges(..)
                        | Mode::MoveStack(..)
                        | Mode::Move(..) => {}
                    }
                }
            }
        }
        Event::Resize(_, _) => {
            messages.push(Message::JustRender);
        }
        Event::Paste(_) => match mode {
            Mode::InlineReword(..) => {
                messages.push(Message::Reword(RewordMessage::InlineInput(ev)));
            }
            Mode::Command(..) => {
                messages.push(Message::Command(CommandMessage::Input(ev)));
            }
            Mode::Jump(..) => {
                messages.push(Message::Jump(JumpMessage::Input(ev)));
            }
            Mode::Normal(..)
            | Mode::Details(..)
            | Mode::Rub(..)
            | Mode::Commit(..)
            | Mode::Stack(..)
            | Mode::PickChanges(..)
            | Mode::MoveStack(..)
            | Mode::Move(..) => {
                messages.push(Message::JustRender);
            }
        },
        Event::FocusGained => {
            messages.push(Message::SetHasFocus(true));
        }
        Event::FocusLost => {
            messages.push(Message::SetHasFocus(false));
        }
        Event::Mouse(_) => {}
    }
}

fn nonempty_from_refs<'a, T>(head: &'a T, tail: impl Iterator<Item = &'a T>) -> NonEmpty<&'a T> {
    let mut nonempty = NonEmpty::new(head);
    nonempty.extend(tail);
    nonempty
}

fn start_watcher(
    ctx: &mut Context,
) -> anyhow::Result<(gitbutler_watcher::WatcherHandle, Arc<AtomicBool>)> {
    let app_settings = app_settings_sync()?;
    let watch_mode = gitbutler_watcher::WatchMode::from_env_or_settings(
        &app_settings.get()?.feature_flags.watch_mode,
        |key| std::env::var(key).ok(),
    );

    let received_watcher_event = Arc::new(AtomicBool::new(false));

    let handler = gitbutler_watcher::Handler::new({
        let received_watcher_event = Arc::clone(&received_watcher_event);
        move |_change| {
            received_watcher_event.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    });

    let project_id = ctx.legacy_project.id.clone();

    let watcher = gitbutler_watcher::watch_in_background(
        handler,
        ctx.workdir_or_fail()?,
        project_id,
        app_settings,
        watch_mode,
    )?;

    Ok((watcher, received_watcher_event))
}

fn app_settings_sync() -> anyhow::Result<AppSettingsWithDiskSync> {
    let config_dir = but_path::app_config_dir().context("missing app config dir")?;
    std::fs::create_dir_all(&config_dir).with_context(|| {
        format!(
            "failed to create app config dir at '{}'",
            config_dir.display()
        )
    })?;
    AppSettingsWithDiskSync::new_with_customization(config_dir, None)
}

mod private {
    pub trait Sealed {}
    impl Sealed for crate::utils::InputOutputChannel<'_> {}
}

/// Required to abstract over input/output channels for the TUI.
///
/// In production we want to require `InputOutputChannel`. This means the caller must check that
/// input is actually supported and return an error otherwise. However in tests we don't want to
/// enforce that.
///
/// So this trait exists such that we can make a fake to use in tests that panics on
/// `prompt_single_line`.
pub trait TuiInputOutputChannel: WriteWithUtils + private::Sealed {
    fn prompt_single_line(&mut self, prompt: &str) -> anyhow::Result<Option<String>>;
}

impl TuiInputOutputChannel for InputOutputChannel<'_> {
    fn prompt_single_line(&mut self, prompt: &str) -> anyhow::Result<Option<String>> {
        InputOutputChannel::prompt_single_line(self, prompt)
    }
}

#[derive(Debug, Clone)]
enum Message {
    // Lifecycle
    JustRender,
    Quit,
    ConfirmAndQuit,
    EnterNormalModeAfterConfirmingOperation,
    Reload(Option<SelectAfterReload>, ReloadCause),
    ShowError(Arc<anyhow::Error>),
    ShowToast {
        kind: ToastKind,
        text: Text<'static>,
    },
    Confirm(ConfirmMessage),
    Discard,
    DropToBeDiscarded,
    GrowDetails,
    ShrinkDetails,
    SetHasFocus(bool),
    Back,
    UnfocusDetails,

    // Cursor movement
    MoveCursorUp(usize),
    MoveCursorDown(usize),
    MoveCursorPreviousSection,
    MoveCursorNextSection,
    SelectUncommitted,
    SelectMergeBase,
    PickAndGotoBranch,
    SelectBranch(FullName),

    // Features
    Commit(CommitMessage),
    Rub(RubMessage),
    Reword(RewordMessage),
    Command(CommandMessage),
    Files(FilesMessage),
    Move(MoveMessage),
    Stack(StackMessage),
    Details(DetailsMessage),
    DetailsLayout(DetailsLayoutMessage),
    FuzzyPicker(FuzzyPickerMessage),
    Help(HelpMessage),
    Jump(JumpMessage),
    NewBranch,
    ToggleHelp,
    Mark,
    ClearNormalModeMarks,
    Undo,
    Redo,

    // Utilities
    CopySelection,
    CopySelectionPicker,
    #[expect(clippy::enum_variant_names)]
    RegisterOutOfBandMessage(Rc<Receiver<Message>>),
    WithOneFrameDelay(Box<Message>),
    AndThen {
        lhs: Box<Message>,
        rhs: Box<Message>,
    },
    #[allow(dead_code)]
    Debug(&'static str),
}

impl Message {
    /// Delay a message so it wont be handled until the next frame.
    pub fn with_one_frame_delay(self) -> Self {
        Self::WithOneFrameDelay(Box::new(self))
    }

    /// Send another message only if handling the first succeeds.
    #[expect(dead_code)]
    pub fn and_then(self, other: Self) -> Self {
        Self::AndThen {
            lhs: Box::new(self),
            rhs: Box::new(other),
        }
    }
}

#[derive(Debug, Clone)]
enum DetailsLayoutMessage {
    /// Focus the details view, showing it first if needed.
    ///
    /// `full_screen` controls whether focus enters the split view or expands details to cover the
    /// status view.
    Focus { full_screen: bool },
    /// Toggle between split details and full-screen details.
    ToggleFullScreen,
    /// Show or hide the details view without necessarily focusing it.
    ToggleVisibility,
    /// Close the full-screen details view if active, otherwise toggle details visibility.
    Dismiss,
}

/// The cause for a reload.
///
/// Used to surpress watcher triggered reloads that happen after an operation from the TUI. Otherwise
/// we'd get double reloads after performing an operation from the TUI since that changes the git
/// repo which triggers the watcher.
#[derive(Debug, Clone, Copy)]
enum ReloadCause {
    /// Reloading because some mutation was made by the TUI.
    Mutation,
    /// Reloading because the watcher came back with an event.
    Watcher,
    /// Reloading only because some TUI view state changed, not because any real data changed.
    ViewOnly,
    /// The user manually triggered a reload.
    Manual,
}

#[derive(Debug, Clone)]
enum FilesMessage {
    ToggleGlobalFilesList,
    ToggleFilesForCommit,
}

/// What to select after reloading
#[derive(Debug, Clone)]
enum SelectAfterReload {
    Commit(gix::ObjectId),
    FirstFileInCommit(gix::ObjectId),
    UncommittedFile {
        path: BString,
        stack_id: Option<StackId>,
    },
    Branch(String),
    Stack(StackId),
    CliId(Arc<CliId>),
    Uncommitted,
}
