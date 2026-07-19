#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use std::{
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use anyhow::Context as _;
use bstr::BString;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_settings::AppSettingsWithDiskSync;
use crossterm::event::{Event, MouseEventKind};
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
            copy_selection_picker::Clipboard,
            cursor::Cursor,
            details::{DetailsMessage, ScrollDirection},
            event_polling::{CrosstermEventPolling, EventPolling, NoopEventPolling},
            fuzzy_picker::{
                Col, FuzzyPicker, FuzzyPickerItem, FuzzyPickerMessage, SearchableToken,
            },
            help::HelpMessage,
            key_bind::{KeyBinds, fuzzy_picker_key_binds},
            mode::{Mode, ModeDiscriminant},
            toast::ToastKind,
        },
    },
    tui::{CrosstermTerminalGuard, HeadlessTerminalGuard, TerminalGuard},
    utils::{InputOutputChannel, WriteWithUtils},
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

pub fn render_tui(
    ctx: &mut Context,
    out: &mut InputOutputChannel<'_>,
    mode: &OperatingMode,
    flags: StatusFlags,
    status_lines: Vec<StatusOutputLine>,
    launch_options: TuiLaunchOptions,
    run_options: TuiRunOptions,
) -> anyhow::Result<(Vec<StatusOutputLine>, TuiOutcome)> {
    let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();

    let head_sha = operations::head_sha(ctx)?;
    let mut app = App::new(
        status_lines,
        flags,
        launch_options,
        run_options,
        ctx.settings.feature_flags.tui_file_browser,
        Vec::from([watcher_rx]),
        head_sha,
        Clipboard::live(),
    );

    let mut messages = Vec::new();

    // second buffer so we can send messages from `App::handle_message`
    let mut other_messages = Vec::new();

    let outcome = if app.launch_options.headless {
        let mut terminal_guard = HeadlessTerminalGuard::new(240, 240)?;
        let mut event_polling = NoopEventPolling;

        render_loop(
            &mut app,
            &mut terminal_guard,
            &mut event_polling,
            &mut messages,
            &mut other_messages,
            ctx,
            out,
            mode,
        )?
    } else {
        let _watcher_handle =
            start_watcher(ctx, watcher_tx).context("failed to start filesystem watcher")?;

        let mut terminal_guard = CrosstermTerminalGuard::alt_screen(true)?;
        let mut event_polling = CrosstermEventPolling::default();

        render_loop(
            &mut app,
            &mut terminal_guard,
            &mut event_polling,
            &mut messages,
            &mut other_messages,
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
    event_polling: &mut E,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
    ctx: &mut Context,
    out: &mut dyn TuiInputOutputChannel,
    mode: &OperatingMode,
) -> anyhow::Result<TuiOutcome>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    for<'a> &'a mut E: EventPolling,
{
    render(app, terminal_guard)?;

    let mut events = Vec::with_capacity(128);

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
            &mut *event_polling,
            &mut events,
            messages,
            other_messages,
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
    events: &mut Vec<Event>,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
    ctx: &mut Context,
    out: &mut dyn TuiInputOutputChannel,
    mode: &OperatingMode,
) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    E: EventPolling,
{
    count_allocations("update", || {
        update(
            app,
            terminal_guard,
            event_polling,
            events,
            messages,
            other_messages,
            ctx,
            out,
            mode,
        )
    })?;

    render(app, terminal_guard)?;

    app.fps.frame_finished();

    Ok(())
}

#[expect(clippy::too_many_arguments)]
fn update<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: E,
    events: &mut Vec<Event>,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
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
    let event_poll_timeout = if app.details.is_polling_thread() {
        Duration::from_millis(0)
    } else {
        Duration::from_millis(30)
    };
    // poll terminal events
    events.clear();
    event_polling.poll_into(event_poll_timeout, events)?;
    for event in events.drain(..) {
        let terminal_area: Rect = terminal_guard.terminal_mut().size()?.into();
        event_to_messages(event, app, terminal_area, messages);
    }
    dedup_mutation_messages(messages, other_messages);

    // check for any out of band messages
    app.incoming_out_of_band_messages
        .retain(|rx| match rx.try_recv() {
            Ok(msg) => {
                messages.push(msg);
                true
            }
            Err(err) => match err {
                std::sync::mpsc::TryRecvError::Empty => true,
                std::sync::mpsc::TryRecvError::Disconnected => false,
            },
        });

    // handle messages
    let mut did_reload = false;
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

    if app.details.update_highlights() {
        app.should_render = true;
    }

    let selection = app
        .cursor
        .selected_line(&app.status_lines)
        .and_then(|line| line.data.cli_id())
        .map(|id| &**id);

    if app.details.update(ctx, selection, app.is_details_visible)? {
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
                messages.push(Message::ShowError(err));
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
            count_allocations("render", || render_app(app, frame))
        })?;
    }

    Ok(())
}

fn event_to_messages(ev: Event, app: &App, terminal_area: Rect, messages: &mut Vec<Message>) {
    let key_binds = app.active_key_binds();
    let mode = &*app.mode;
    match ev {
        Event::Key(key) => {
            let mut handled = false;
            let selection = app
                .cursor
                .selected_line(&app.status_lines)
                .and_then(|line| Some(&**line.data.cli_id()?));
            for key_bind in
                key_binds.iter_key_binds_available_in_mode(ModeDiscriminant::from(mode), selection)
            {
                if key_bind.matches(&key) {
                    messages.push(key_bind.message());
                    handled = true;
                }
            }

            if !handled {
                if let Some(message) = app
                    .modal
                    .as_ref()
                    .and_then(|modal| modal.input_message(ev.clone()))
                {
                    messages.push(message);
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
        Event::Paste(_) => {
            if let Some(message) = app
                .modal
                .as_ref()
                .and_then(|modal| modal.input_message(ev.clone()))
            {
                messages.push(message);
                return;
            }

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
                | Mode::Move(..) => {
                    messages.push(Message::JustRender);
                }
            }
        }
        Event::FocusGained => {
            messages.push(Message::SetHasFocus(true));
        }
        Event::FocusLost => {
            messages.push(Message::SetHasFocus(false));
        }
        Event::Mouse(event) => match event.kind {
            MouseEventKind::ScrollDown => {
                if app.modal.is_none() {
                    if mouse_is_over_debug(app, terminal_area, event.column, event.row) {
                        messages.push(Message::DebugScrollDown(3));
                    } else if mouse_is_over_details(app, terminal_area, event.column, event.row) {
                        messages.push(Message::Details(DetailsMessage::ScrollDown(3)));
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if app.modal.is_none() {
                    if mouse_is_over_debug(app, terminal_area, event.column, event.row) {
                        messages.push(Message::DebugScrollUp(3));
                    } else if mouse_is_over_details(app, terminal_area, event.column, event.row) {
                        messages.push(Message::Details(DetailsMessage::ScrollUp(3)));
                    }
                }
            }
            MouseEventKind::Moved
            | MouseEventKind::Down(..)
            | MouseEventKind::Up(..)
            | MouseEventKind::Drag(..)
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight => {}
        },
    }
}

fn mouse_is_over_debug(app: &App, terminal_area: Rect, column: u16, row: u16) -> bool {
    render::debug_content_area_for_app(app, terminal_area)
        .is_some_and(|area| area.contains(Position { x: column, y: row }))
}

fn mouse_is_over_details(app: &App, terminal_area: Rect, column: u16, row: u16) -> bool {
    render::details_content_area_for_app(app, terminal_area)
        .is_some_and(|area| area.contains(Position { x: column, y: row }))
}

fn nonempty_from_refs<'a, T>(head: &'a T, tail: impl Iterator<Item = &'a T>) -> NonEmpty<&'a T> {
    let mut nonempty = NonEmpty::new(head);
    nonempty.extend(tail);
    nonempty
}

fn start_watcher(
    ctx: &mut Context,
    tx: Sender<Message>,
) -> anyhow::Result<gitbutler_watcher::WatcherHandle> {
    let app_settings = app_settings_sync()?;
    let watch_mode = gitbutler_watcher::WatchMode::from_env_or_settings(
        &app_settings.get()?.feature_flags.watch_mode,
        |key| std::env::var(key).ok(),
    );

    let handler = gitbutler_watcher::Handler::new(move |change| {
        _ = tx.send(Message::WatcherEvent(change));
        Ok(())
    });

    let project_id = ctx.legacy_project.id.clone();

    let watcher = gitbutler_watcher::watch_in_background(
        handler,
        ctx.workdir_or_fail()?,
        project_id,
        app_settings,
        watch_mode,
    )?;

    Ok(watcher)
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

#[derive(Debug)]
enum Message {
    // Lifecycle
    JustRender,
    Quit,
    ConfirmAndQuit,
    EnterNormalModeAfterConfirmingOperation,
    Reload(Option<SelectAfterReload>, ReloadCause),
    ShowError(anyhow::Error),
    ShowToast {
        kind: ToastKind,
        text: Text<'static>,
    },
    Confirm(ConfirmMessage),
    Discard,
    DropToBeDiscarded,
    GrowDetails,
    ShrinkDetails,
    DebugScrollUp(usize),
    DebugScrollDown(usize),
    SetHasFocus(bool),
    Back,
    UnfocusDetails,
    WatcherEvent(gitbutler_watcher::Change),

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
    ClearMarks,
    Undo,
    Redo,
    ShowModal(Modal),

    // Utilities
    CopySelection,
    CopySelectionPicker,
    CopyToClipboard(String),
    #[expect(clippy::enum_variant_names)]
    RegisterOutOfBandMessage(Receiver<Message>),
    AndThen {
        lhs: Box<Message>,
        rhs: Box<Message>,
    },
    #[allow(dead_code)]
    Debug(&'static str),
}

#[test]
fn message_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Message>();
}

impl Message {
    /// Send another message only if handling the first succeeds.
    #[expect(dead_code)]
    pub fn and_then(self, other: Self) -> Self {
        Self::AndThen {
            lhs: Box::new(self),
            rhs: Box::new(other),
        }
    }
}

#[derive(Debug)]
enum DetailsLayoutMessage {
    /// Focus the details view, showing it first if needed.
    ///
    /// `full_screen` controls whether focus enters the split view or expands details to cover the
    /// status view.
    Focus { full_screen: bool },
    /// Toggle between split details and full-screen details.
    ToggleFullScreen,
    /// Switch full-screen details to the focused split details view.
    SwitchToSplit,
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
    Watcher { details_selection_changed: bool },
    /// Reloading only because some TUI view state changed, not because any real data changed.
    ViewOnly,
    /// The user manually triggered a reload.
    Manual,
}

#[derive(Debug)]
enum FilesMessage {
    ToggleGlobalFilesList,
    ToggleFilesForSelectedCommit,
}

/// What to select after reloading
#[derive(Debug)]
enum SelectAfterReload {
    Commit(gix::ObjectId),
    FirstFileInCommit(gix::ObjectId),
    UncommittedFile {
        path: BString,
        stack_id: Option<StackId>,
    },
    UncommittedDetailsSection {
        index: usize,
        direction: ScrollDirection,
    },
    Branch(String),
    Stack(StackId),
    CliId(Box<CliId>),
    Uncommitted,
}

#[inline(always)]
#[track_caller]
#[allow(dead_code)]
fn count_allocations<F, T>(_tag: &'static str, f: F) -> T
where
    F: FnOnce() -> T,
{
    #[cfg(feature = "tui-profiling")]
    {
        let mut result = None;
        let loc = std::panic::Location::caller();
        let start = std::time::Instant::now();
        let info = allocation_counter::measure(|| {
            result = Some(f());
        });
        let duration = start.elapsed();
        tracing::debug!(
            "{}:{}:{}: {}: {} allocation(s), {:?}",
            loc.file(),
            loc.line(),
            loc.column(),
            _tag,
            info.count_total,
            duration,
        );
        result.unwrap()
    }

    #[cfg(not(feature = "tui-profiling"))]
    {
        f()
    }
}

/// If the user has a high key repeat and hold down a key that causes a mutation (like 'n') the app
/// will slow down significantly while running multiple mutations per update. Messages that don't
/// mutate the git repo, such as scrolling are safe to execute multiple times per update.
///
/// This function fixes that by only allowing one mutation per update.
fn dedup_mutation_messages(messages: &mut Vec<Message>, other_messages: &mut Vec<Message>) {
    fn is_repo_mutation(m: &Message) -> bool {
        match m {
            Message::AndThen { lhs, rhs } => is_repo_mutation(lhs) || is_repo_mutation(rhs),
            Message::Reload(_, cause) => match cause {
                ReloadCause::Mutation
                | ReloadCause::Watcher { .. }
                | ReloadCause::ViewOnly
                | ReloadCause::Manual => true,
            },
            Message::Confirm(message) => match message {
                ConfirmMessage::Confirm | ConfirmMessage::Yes => true,
                ConfirmMessage::Left | ConfirmMessage::Right | ConfirmMessage::No => false,
            },
            Message::Commit(message) => match message {
                CommitMessage::CreateEmpty
                | CommitMessage::Confirm
                | CommitMessage::CommitToNewBranch => true,
                CommitMessage::Start
                | CommitMessage::StartWithSource(..)
                | CommitMessage::ToggleMessageComposer(_)
                | CommitMessage::ToggleInsertSide => false,
            },
            Message::Rub(message) => match message {
                RubMessage::Confirm => true,
                RubMessage::Start
                | RubMessage::StartWithSource(..)
                | RubMessage::StartReverse
                | RubMessage::UseTargetMessage
                | RubMessage::UseSourceMessage => false,
            },
            Message::Reword(message) => match message {
                RewordMessage::WithEditor
                | RewordMessage::OpenEditor
                | RewordMessage::InlineConfirm => true,
                RewordMessage::InlineStart | RewordMessage::InlineInput(_) => false,
            },
            Message::Command(message) => match message {
                CommandMessage::Confirm => true,
                CommandMessage::Start(_) | CommandMessage::Input(_) => false,
            },
            Message::Files(message) => match message {
                FilesMessage::ToggleGlobalFilesList
                | FilesMessage::ToggleFilesForSelectedCommit => false,
            },
            Message::Move(message) => match message {
                MoveMessage::Confirm => true,
                MoveMessage::Start | MoveMessage::ToggleInsertSide => false,
            },
            Message::Stack(message) => match message {
                StackMessage::Unapply | StackMessage::MoveConfirm => true,
                StackMessage::Enter | StackMessage::ShowApplyPicker | StackMessage::MoveStart => {
                    false
                }
            },
            Message::Details(message) => match message {
                DetailsMessage::Deselect
                | DetailsMessage::SelectFirstSection
                | DetailsMessage::CopyCurrentHunk
                | DetailsMessage::SelectNextSection
                | DetailsMessage::SelectPrevSection
                | DetailsMessage::ScrollUp(_)
                | DetailsMessage::ScrollDown(_)
                | DetailsMessage::GotoTop
                | DetailsMessage::GotoBottom
                | DetailsMessage::Discard
                | DetailsMessage::Mark
                | DetailsMessage::DropToBeDiscarded => false,
            },
            Message::DetailsLayout(message) => match message {
                DetailsLayoutMessage::Focus { .. }
                | DetailsLayoutMessage::ToggleFullScreen
                | DetailsLayoutMessage::SwitchToSplit
                | DetailsLayoutMessage::ToggleVisibility
                | DetailsLayoutMessage::Dismiss => false,
            },
            Message::FuzzyPicker(message) => match message {
                FuzzyPickerMessage::Confirm => true,
                FuzzyPickerMessage::MoveCursorDown
                | FuzzyPickerMessage::MoveCursorUp
                | FuzzyPickerMessage::Input(_)
                | FuzzyPickerMessage::Close => false,
            },
            Message::Help(message) => match message {
                HelpMessage::Close
                | HelpMessage::Escape
                | HelpMessage::ToggleSearch
                | HelpMessage::Input(_)
                | HelpMessage::ScrollUp(_)
                | HelpMessage::ScrollDown(_) => false,
            },
            Message::Jump(message) => match message {
                JumpMessage::Enter
                | JumpMessage::Input(_)
                | JumpMessage::Previous
                | JumpMessage::Next
                | JumpMessage::Confirm => false,
            },
            Message::ShowModal(modal) => match modal {
                Modal::Confirm { .. }
                | Modal::CopySelectionPicker { .. }
                | Modal::GotoBranchPicker { .. }
                | Modal::ApplyStackPicker { .. }
                | Modal::Help { .. } => false,
            },
            Message::Undo | Message::Redo | Message::Discard | Message::NewBranch => true,
            Message::JustRender
            | Message::Quit
            | Message::ConfirmAndQuit
            | Message::EnterNormalModeAfterConfirmingOperation
            | Message::ShowError(..)
            | Message::ShowToast { .. }
            | Message::DropToBeDiscarded
            | Message::GrowDetails
            | Message::ShrinkDetails
            | Message::DebugScrollUp(_)
            | Message::DebugScrollDown(_)
            | Message::SetHasFocus(_)
            | Message::Back
            | Message::UnfocusDetails
            | Message::MoveCursorUp(_)
            | Message::MoveCursorDown(_)
            | Message::MoveCursorPreviousSection
            | Message::MoveCursorNextSection
            | Message::SelectUncommitted
            | Message::SelectMergeBase
            | Message::PickAndGotoBranch
            | Message::SelectBranch(..)
            | Message::ToggleHelp
            | Message::Mark
            | Message::ClearMarks
            | Message::CopySelection
            | Message::CopySelectionPicker
            | Message::CopyToClipboard(..)
            | Message::RegisterOutOfBandMessage(..)
            | Message::Debug(_) => false,
            // these are never generated by the tui itself
            Message::WatcherEvent(..) => false,
        }
    }

    assert!(other_messages.is_empty());

    let mut seen_mutation = false;
    for m in messages.drain(..) {
        if is_repo_mutation(&m) {
            if seen_mutation {
                continue;
            } else {
                seen_mutation = true;
            }
        }
        other_messages.push(m);
    }

    std::mem::swap(messages, other_messages);
}
