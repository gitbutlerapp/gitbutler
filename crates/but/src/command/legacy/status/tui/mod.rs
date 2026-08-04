#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use std::sync::mpsc::{Receiver, Sender};

use anyhow::Context as _;
use bstr::BString;
use but_api::open::program::ProgramSpec;
use but_ctx::Context;
use but_settings::AppSettingsWithDiskSync;
use crossterm::event::{Event, MouseEventKind};
use gitbutler_operating_modes::OperatingMode;
use gix::refs::FullName;
use ratatui::prelude::*;

use crate::{
    CliId, CliResult,
    args::atoms::ResolvedCliIdArg,
    command::{
        legacy::status::{
            StatusFlags, StatusOutputLine, TuiLaunchOptions, TuiOutcome, TuiRunOptions,
            tui::{
                app::{
                    CherryPickMessage, CommandMessage, CommandModeKind, CommitMessage, JumpMessage,
                    MoveMessage, NormalMode, PickChangesMode, RewordMessage, SquashMessage,
                    StackMessage, UpdateContext,
                },
                backstack::{Backstack, BackstackEntry},
                confirm::ConfirmMessage,
                cursor::Cursor,
                details::{DetailsMessage, ScrollDirection},
                fuzzy_picker::{
                    Col, FuzzyPicker, FuzzyPickerItem, FuzzyPickerMessage, SearchableToken,
                },
                help::HelpMessage,
                key_bind::{KeyBinds, fuzzy_picker_key_binds},
                mode::Mode,
                toast::ToastKind,
            },
        },
        open::Openable,
    },
    id::{BranchId, CommitId, CommittedFileId, UncommittedHunkOrFile},
    tui::{
        Clipboard, CrosstermTerminalGuard, HeadlessTerminalGuard, TerminalGuard, Tui,
        TuiInputOutputChannel,
        event_polling::{CrosstermEventPolling, EventPolling, NoopEventPolling},
    },
    utils::InputOutputChannel,
};

use app::{App, InlineRewordMode, Modal};

mod app;
mod backstack;
mod confirm;
mod copy_selection_picker;
mod cursor;
mod details;
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
mod remember_selection;
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
    operating_mode: OperatingMode,
    flags: StatusFlags,
    status_lines: Vec<StatusOutputLine>,
    launch_options: TuiLaunchOptions,
    initial_target: Option<ResolvedCliIdArg>,
    run_options: TuiRunOptions,
) -> CliResult<(Vec<StatusOutputLine>, TuiOutcome)> {
    let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();

    let head_sha = operations::head_sha(ctx)?;
    let mut app = App::new(
        ctx,
        status_lines,
        flags,
        launch_options,
        initial_target,
        run_options,
        ctx.settings.feature_flags.tui_file_browser,
        Vec::from([watcher_rx]),
        head_sha,
        Clipboard::live(),
        operating_mode,
    )?;

    let messages = Vec::new();

    // second buffer so we can send messages from `App::handle_message`
    let other_messages = Vec::new();

    let outcome = if app.launch_options.headless {
        let mut terminal_guard = HeadlessTerminalGuard::new(240, 240)?;
        let mut event_polling = NoopEventPolling;

        let mut update_ctx = UpdateContext {
            messages,
            other_messages,
            ctx,
        };

        render_loop(
            &mut app,
            &mut terminal_guard,
            &mut event_polling,
            out,
            &mut update_ctx,
        )?
    } else {
        let _watcher_handle =
            start_watcher(ctx, watcher_tx).context("failed to start filesystem watcher")?;

        let mut terminal_guard = CrosstermTerminalGuard::alt_screen(true)?;
        let mut event_polling = CrosstermEventPolling::default();

        let mut update_ctx = UpdateContext {
            messages,
            other_messages,
            ctx,
        };

        render_loop(
            &mut app,
            &mut terminal_guard,
            &mut event_polling,
            out,
            &mut update_ctx,
        )?
    };

    Ok((app.status_lines, outcome))
}

fn render_loop<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: &mut E,
    out: &mut dyn TuiInputOutputChannel,
    update_ctx: &mut UpdateContext<'_>,
) -> anyhow::Result<TuiOutcome>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    for<'a> &'a mut E: EventPolling,
{
    app.render(terminal_guard)?;

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
            out,
            update_ctx,
        )?;

        if let Some(outcome) = app.outcome.take() {
            break Ok(outcome);
        }
    }
}

fn render_loop_once<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: E,
    events: &mut Vec<Event>,
    out: &mut dyn TuiInputOutputChannel,
    update_ctx: &mut UpdateContext<'_>,
) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    E: EventPolling,
{
    count_allocations("update", || {
        app.update(terminal_guard, event_polling, events, out, update_ctx)
    })?;

    app.render(terminal_guard)?;

    app.fps.frame_finished();

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
            for key_bind in key_binds.iter_key_binds_available_in_mode(mode, selection) {
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
                        | Mode::Squash(..)
                        | Mode::Commit(..)
                        | Mode::Stack(..)
                        | Mode::PickChanges(..)
                        | Mode::MoveStack(..)
                        | Mode::CherryPick(..)
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
                | Mode::Squash(..)
                | Mode::Commit(..)
                | Mode::Stack(..)
                | Mode::PickChanges(..)
                | Mode::MoveStack(..)
                | Mode::CherryPick(..)
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

#[derive(Debug)]
pub enum Message {
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
    Squash(SquashMessage),
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
    CherryPick(CherryPickMessage),
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
    OpenInProgram(ProgramSpec, Openable),
    OpenInDefaultProgram,
    PickProgramThenOpen,
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
pub enum DetailsLayoutMessage {
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
pub enum ReloadCause {
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
pub enum FilesMessage {
    ToggleGlobalFilesList,
    ToggleFilesForSelectedCommit,
}

/// What to select after reloading
#[derive(Debug)]
pub enum SelectAfterReload {
    Commit(gix::ObjectId),
    FirstFileInCommit(gix::ObjectId),
    #[expect(dead_code)]
    UncommittedFile {
        path: BString,
    },
    UncommittedDetailsSection {
        index: usize,
        direction: ScrollDirection,
    },
    Branch(String),
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
            Message::Squash(message) => match message {
                SquashMessage::Confirm => true,
                SquashMessage::Start
                | SquashMessage::StartReverse
                | SquashMessage::StartWith(..)
                | SquashMessage::UseTargetMessage => false,
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
            Message::CherryPick(message) => match message {
                CherryPickMessage::Confirm | CherryPickMessage::CherryPickToNewBranch => true,
                CherryPickMessage::Start | CherryPickMessage::ToggleInsertSide => false,
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
                | Modal::ProgramPicker { .. }
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
            | Message::OpenInProgram(..)
            | Message::PickProgramThenOpen
            | Message::OpenInDefaultProgram
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

#[derive(Debug, Clone)]
pub enum Selectable {
    UncommittedHunkOrFile(UncommittedHunkOrFile),
    CommittedFile(CommittedFileId),
    Branch(BranchId),
    Commit(CommitId),
    Uncommitted,
}

impl PartialEq<CliId> for Selectable {
    fn eq(&self, other: &CliId) -> bool {
        match self {
            Selectable::UncommittedHunkOrFile(this) => {
                if let CliId::UncommittedHunkOrFile(other) = other {
                    return this == other;
                }
            }
            Selectable::CommittedFile(this) => {
                if let CliId::CommittedFile {
                    committed_file: other,
                    id: _,
                } = other
                {
                    return this == other;
                }
            }
            Selectable::Branch(this) => {
                if let CliId::Branch(other) = other {
                    return this == other;
                }
            }
            Selectable::Commit(this) => {
                if let CliId::Commit {
                    commit: other,
                    id: _,
                } = other
                {
                    return this == other;
                }
            }
            Selectable::Uncommitted => {
                return matches!(other, CliId::Uncommitted { .. });
            }
        }
        false
    }
}
