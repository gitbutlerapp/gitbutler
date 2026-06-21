#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use std::{
    borrow::Cow,
    ffi::OsString,
    process::Command,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool, mpsc::Receiver},
    time::{Duration, Instant, SystemTime},
};

use anyhow::Context as _;
use bstr::{BString, ByteSlice};
use but_api::{diff::ComputeLineStats, legacy::oplog::RestoreKind};
use but_core::{DryRun, ref_metadata::StackId};
use but_core::{diff::CommitDetails, tree::create_tree::RejectionReason};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use but_settings::AppSettingsWithDiskSync;
use but_transaction::DynamicOutcome;
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use gitbutler_branch_actions::BranchListingFilter;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{
    prelude::ObjectIdExt as _,
    refs::{Category, FullName},
};
use nonempty::NonEmpty;
use ratatui::prelude::*;
use ratatui_textarea::{CursorMove, TextArea};
use tracing::Level;

use crate::{
    CliId, IdMap,
    command::legacy::{
        self, ShowDiffInEditor,
        reword::get_branch_name_from_editor,
        status::{
            StatusFlags, StatusOutputLine, TuiLaunchOptions, TuiOutcome, TuiRunOptions,
            tui::{
                backstack::{Backstack, BackstackEntry, RememberToUpdateBackstack},
                confirm::{Confirm, ConfirmMessage},
                cursor::{Cursor, is_selectable_in_mode},
                details::{Details, DetailsMessage, RenderNextChunkResult},
                event_polling::{CrosstermEventPolling, EventPolling, NoopEventPolling},
                fps::FpsCounter,
                fuzzy_picker::{
                    Col, FuzzyPicker, FuzzyPickerItem, FuzzyPickerMessage, SearchableToken,
                },
                help::{Help, HelpMessage},
                highlight::Highlights,
                key_bind::{
                    KeyBinds, confirm_key_binds, default_key_binds, fuzzy_picker_key_binds,
                    help_key_binds, normal_with_marks_key_binds,
                },
                marking::{MarkClasses, Markable, Marks},
                message_on_drop::MessageOnDrop,
                mode::{
                    CommandMode, CommandModeKind, CommitMessageComposer, CommitMode, CommitSource,
                    DetailsMode, DetailsReturnMode, InlineRewordMode, Mode, ModeDiscriminant,
                    MoveMode, MoveSource, MoveStackMode, NormalMode, PickUncommittedMode,
                    ReorderStackSource, RubMode, RubSource, StackCommitSource, StackMode,
                    UnassignedCommitSource,
                },
                operations::stack_has_assigned_changes,
                toast::{ToastKind, Toasts},
            },
        },
    },
    id::UNASSIGNED,
    resolve_legacy_top_level_apply_branch_name,
    theme::Theme,
    tui::{CrosstermTerminalGuard, HeadlessTerminalGuard, TerminalGuard},
    utils::{
        DebugAsType, InputOutputChannel, WriteWithUtils, binary_path::current_exe_for_but_exec,
        diff_specs::DiffSpecBuilder, time::format_relative_time,
    },
};

use super::{FilesStatusFlag, output::StatusOutputLineData};

use render::{details_viewport, ensure_cursor_visible, render_app, status_viewport_height};

mod backstack;
mod confirm;
mod cursor;
mod details;
mod event_polling;
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
mod rub;
mod rub_from_detail_view;
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

pub(super) fn render_tui(
    ctx: &mut Context,
    out: &mut InputOutputChannel<'_>,
    mode: &OperatingMode,
    flags: StatusFlags,
    status_lines: Vec<StatusOutputLine>,
    launch_options: TuiLaunchOptions,
    run_options: TuiRunOptions,
) -> anyhow::Result<(Vec<StatusOutputLine>, TuiOutcome)> {
    let mut app = App::new(status_lines, flags, launch_options, run_options);

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
            Some(Modal::GotoBranchPicker { .. } | Modal::ApplyStackPicker { .. }) => true,
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

    if app.details.needs_update(app.is_details_visible) {
        let selection = app
            .cursor
            .selected_line(&app.status_lines)
            .and_then(|line| line.data.cli_id())
            .map(|id| &**id);
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

#[derive(Debug)]
struct App {
    status_lines: Vec<StatusOutputLine>,
    flags: StatusFlags,
    outcome: Option<TuiOutcome>,
    should_render: bool,
    cursor: Cursor,
    scroll_top: usize,
    mode: RememberToUpdateBackstack<Mode>,
    toasts: Toasts,
    renders: u64,
    updates: u64,
    app_key_binds: AppKeyBinds,
    highlight: Highlights<CliId>,
    modal: Option<Modal>,
    details: Details,
    is_details_visible: bool,
    launch_options: TuiLaunchOptions,
    delayed_messages: Vec<Message>,
    incoming_out_of_band_messages: Vec<Rc<Receiver<Message>>>,
    fps: FpsCounter,
    to_be_discarded: Vec<Arc<CliId>>,
    status_width_percentage: u16,
    theme: &'static Theme,
    has_focus: bool,
    backstack: Backstack,
    previous_reload_caused_by_mutation_timestamp: Option<Instant>,
}

#[derive(Debug)]
enum Modal {
    Confirm {
        confirm: Confirm,
        key_binds: KeyBinds,
    },
    GotoBranchPicker {
        picker: Box<FuzzyPicker<GotoBranchItem>>,
        key_binds: KeyBinds,
    },
    ApplyStackPicker {
        picker: Box<FuzzyPicker<ApplyBranchItem>>,
        key_binds: KeyBinds,
    },
    Help {
        help: Help,
        key_binds: KeyBinds,
    },
}

#[derive(Debug)]
struct AppKeyBinds {
    key_binds: KeyBinds,
    normal_with_marks_key_binds: KeyBinds,
}

impl App {
    fn new(
        status_lines: Vec<StatusOutputLine>,
        flags: StatusFlags,
        launch_options: TuiLaunchOptions,
        run_options: TuiRunOptions,
    ) -> Self {
        let cursor = if let Some(object_id) = launch_options.select_commit {
            Cursor::select_commit(object_id, &status_lines)
                .unwrap_or_else(|| Cursor::new(&status_lines))
        } else {
            Cursor::new(&status_lines)
        };

        let theme = crate::theme::get();

        let (mut details, is_details_visible) = (Details::new(theme), launch_options.show_diff);
        if is_details_visible {
            details.mark_dirty();
        }

        let app_key_binds = AppKeyBinds {
            key_binds: default_key_binds(),
            normal_with_marks_key_binds: normal_with_marks_key_binds(),
        };

        let mode = RememberToUpdateBackstack::new(match run_options {
            TuiRunOptions::Normal => Mode::default(),
            TuiRunOptions::PickChanges => Mode::PickChanges(Default::default()),
        });

        Self {
            status_lines,
            flags,
            cursor,
            scroll_top: 0,
            outcome: None,
            should_render: true,
            mode,
            toasts: Default::default(),
            renders: 0,
            updates: 0,
            app_key_binds,
            highlight: Default::default(),
            delayed_messages: Default::default(),
            incoming_out_of_band_messages: Default::default(),
            to_be_discarded: Default::default(),
            modal: Default::default(),
            backstack: Default::default(),
            previous_reload_caused_by_mutation_timestamp: Default::default(),
            fps: FpsCounter::new(),
            details,
            is_details_visible,
            launch_options,
            status_width_percentage: 50,
            theme,
            has_focus: true,
        }
    }

    fn active_key_binds(&self) -> &KeyBinds {
        match &self.modal {
            Some(Modal::Confirm { key_binds, .. })
            | Some(Modal::GotoBranchPicker { key_binds, .. })
            | Some(Modal::ApplyStackPicker { key_binds, .. })
            | Some(Modal::Help { key_binds, .. }) => key_binds,
            None => {
                if let Mode::Normal(NormalMode { marks }) = &*self.mode
                    && !marks.is_empty()
                {
                    &self.app_key_binds.normal_with_marks_key_binds
                } else {
                    &self.app_key_binds.key_binds
                }
            }
        }
    }

    #[tracing::instrument(level = Level::TRACE, skip(self, ctx, out, mode, terminal_guard, messages))]
    fn handle_message<T>(
        &mut self,
        ctx: &mut Context,
        out: &mut dyn TuiInputOutputChannel,
        mode: &OperatingMode,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
        msg: Message,
    ) where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        if let Err(err) = self.try_handle_message(ctx, out, mode, terminal_guard, messages, msg) {
            messages.push(Message::ShowError(Arc::new(err)));
        }
    }

    fn try_handle_message<T>(
        &mut self,
        ctx: &mut Context,
        out: &mut dyn TuiInputOutputChannel,
        mode: &OperatingMode,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
        msg: Message,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let start = Instant::now();
        let discriminant = MessageDiscriminant::from(&msg);

        self.should_render = true;
        let terminal_area: Rect = terminal_guard.terminal_mut().size()?.into();
        let visible_height = status_viewport_height(self, terminal_area);

        if self
            .details
            .needs_update_after_message(self.is_details_visible, &msg)
        {
            self.details.mark_dirty();
        }

        match msg {
            Message::Quit => {
                self.handle_quit();
            }
            Message::ConfirmAndQuit => {
                self.handle_confirm_and_quit();
            }
            Message::JustRender => {}
            Message::MoveCursorUp(count) => {
                for _ in 0..count {
                    if let Some(new_cursor) =
                        self.cursor
                            .move_up(&self.status_lines, &self.mode, self.flags.show_files)
                    {
                        self.cursor = new_cursor;
                    } else {
                        break;
                    }
                }
            }
            Message::MoveCursorDown(count) => {
                for _ in 0..count {
                    if let Some(new_cursor) =
                        self.cursor
                            .move_down(&self.status_lines, &self.mode, self.flags.show_files)
                    {
                        self.cursor = new_cursor;
                    } else {
                        break;
                    }
                }
            }
            Message::MoveCursorPreviousSection => {
                if let Some(new_cursor) = self.cursor.move_previous_section(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                }
            }
            Message::MoveCursorNextSection => {
                if let Some(new_cursor) = self.cursor.move_next_section(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                }
            }
            Message::SelectBranch(branch_name) => {
                if let Some(new_cursor) =
                    Cursor::select_branch(&branch_name.shorten().to_str_lossy(), &self.status_lines)
                {
                    self.cursor = if matches!(&*self.mode, Mode::Rub(_)) {
                        new_cursor
                            .move_down_within_section(
                                &self.status_lines,
                                &self.mode,
                                self.flags.show_files,
                            )
                            .unwrap_or(new_cursor)
                    } else {
                        new_cursor
                    };
                }
            }
            Message::SelectUnassigned => {
                let new_cursor = Cursor::new(&self.status_lines);
                if let Some(unassigned_line) = new_cursor.selected_line(&self.status_lines)
                    && cursor::is_selectable_in_mode(
                        unassigned_line,
                        &self.mode,
                        self.flags.show_files,
                    )
                {
                    self.cursor = new_cursor;
                }
            }
            Message::SelectMergeBase => {
                let Some(new_cursor) = Cursor::select_merge_base(&self.status_lines) else {
                    return Ok(());
                };
                if let Some(merge_base_line) = new_cursor.selected_line(&self.status_lines)
                    && cursor::is_selectable_in_mode(
                        merge_base_line,
                        &self.mode,
                        self.flags.show_files,
                    )
                {
                    self.cursor = new_cursor;
                }
            }
            Message::Rub(rub_message) => match rub_message {
                RubMessage::Start => self.handle_rub_start(),
                RubMessage::StartWithSource {
                    source,
                    unlock_details,
                } => {
                    self.handle_rub_start_with_source(source, unlock_details);
                }
                RubMessage::StartReverse => {
                    self.handle_rub_start_reverse(ctx)?;
                }
                RubMessage::Confirm => self.handle_rub_confirm(ctx, messages)?,
                RubMessage::UseTargetMessage => {
                    self.handle_rub_use_target_message();
                }
                RubMessage::UseSourceMessage => {
                    self.handle_rub_use_source_message();
                }
            },
            Message::Back => {
                self.handle_back(messages);
            }
            Message::UnfocusDetails => {
                self.handle_unfocus_details(messages);
            }
            Message::EnterNormalModeAfterConfirmingOperation => {
                self.handle_enter_normal_mode_after_confirming_operation(messages);
            }
            Message::DetailsLayout(details_layout_message) => match details_layout_message {
                DetailsLayoutMessage::Focus { full_screen } => {
                    self.handle_focus_details(full_screen, messages);
                }
                DetailsLayoutMessage::ToggleFullScreen => {
                    self.handle_toggle_details_full_screen(messages);
                }
                DetailsLayoutMessage::ToggleVisibility => {
                    self.handle_toggle_details_visibility(messages);
                }
                DetailsLayoutMessage::Dismiss => {
                    self.handle_dismiss_details(messages);
                }
            },
            Message::Files(files_message) => match files_message {
                FilesMessage::ToggleGlobalFilesList => {
                    self.handle_files_toggle_global_files_list(messages)
                }
                FilesMessage::ToggleFilesForCommit => {
                    self.handle_files_toggle_files_for_commit(ctx, messages)?
                }
            },
            Message::Reload(select_after_reload, cause) => {
                self.handle_reload(ctx, out, mode, select_after_reload, cause)?
            }
            Message::ShowError(err) => self.handle_show_error(err, messages),
            Message::Commit(commit_message) => match commit_message {
                CommitMessage::CreateEmpty => self.handle_commit_create_empty(ctx, messages)?,
                CommitMessage::Start => self.handle_commit_start(ctx)?,
                CommitMessage::Confirm => {
                    self.handle_commit_confirm(ctx, terminal_guard, messages)?
                }
                CommitMessage::ToggleMessageComposer(composer) => {
                    self.handle_commit_toggle_message_composer(composer);
                }
                CommitMessage::CommitToNewBranch => {
                    self.handle_commit_to_new_branch(messages);
                }
                CommitMessage::ToggleInsertSide => {
                    self.handle_commit_toggle_insert_side();
                }
            },
            Message::Reword(reword_message) => match reword_message {
                RewordMessage::WithEditor => {
                    self.handle_reword_with_editor(ctx, terminal_guard, messages)?;
                }
                RewordMessage::InlineStart => self.handle_reword_inline_start(ctx, messages)?,
                RewordMessage::InlineInput(ev) => self.handle_reword_inline_input(ev),
                RewordMessage::InlineConfirm => self.handle_reword_inline_confirm(ctx, messages)?,
                RewordMessage::OpenEditor => {
                    self.handle_reword_open_editor(ctx, terminal_guard, messages)?;
                }
            },
            Message::Command(command_message) => match command_message {
                CommandMessage::Start(kind) => self.handle_command_start(kind),
                CommandMessage::Input(ev) => self.handle_command_input(ev),
                CommandMessage::Confirm => {
                    self.handle_command_confirm(terminal_guard, out, messages)?
                }
            },
            Message::Move(move_message) => match move_message {
                MoveMessage::Start => self.handle_move_start(),
                MoveMessage::ToggleInsertSide => self.handle_move_toggle_insert_side(),
                MoveMessage::Confirm => self.handle_move_confirm(ctx, messages)?,
            },
            Message::NewBranch => {
                self.handle_new_branch(ctx, messages)?;
            }
            Message::CopySelection => {
                self.handle_copy_selection()?;
            }
            Message::ShowToast { kind, text } => {
                self.toasts.insert(kind, text);
            }
            Message::Confirm(confirm_message) => match self.modal.take() {
                Some(Modal::Confirm { confirm, key_binds }) => {
                    self.modal = confirm
                        .handle_message(confirm_message, ctx, messages)?
                        .map(|confirm| Modal::Confirm { confirm, key_binds });
                }
                modal => self.modal = modal,
            },
            Message::FuzzyPicker(fuzzy_picker_message) => {
                if let Some(modal) = self.modal.take() {
                    match modal {
                        Modal::GotoBranchPicker { picker, key_binds } => {
                            self.modal = picker
                                .handle_message(fuzzy_picker_message, ctx, messages)?
                                .map(|picker| Modal::GotoBranchPicker {
                                    picker: Box::new(picker),
                                    key_binds,
                                });
                        }
                        Modal::ApplyStackPicker { picker, key_binds } => {
                            self.modal = picker
                                .handle_message(fuzzy_picker_message, ctx, messages)?
                                .map(|picker| Modal::ApplyStackPicker {
                                    picker: Box::new(picker),
                                    key_binds,
                                });
                        }
                        Modal::Confirm { .. } | Modal::Help { .. } => {
                            self.modal = Some(modal);
                        }
                    }
                }
            }
            Message::Help(help_message) => match self.modal.take() {
                Some(Modal::Help { help, key_binds }) => {
                    self.modal = help
                        .handle_message(help_message, terminal_area)?
                        .map(|help| Modal::Help { help, key_binds });
                }
                modal => self.modal = modal,
            },
            Message::Details(details_message) => {
                let details_viewport = details_viewport(self, terminal_area);
                self.details
                    .try_handle_message(details_message, details_viewport, messages)?;
            }
            Message::RegisterOutOfBandMessage(rx) => {
                self.incoming_out_of_band_messages.push(rx);
            }
            Message::WithOneFrameDelay(msg) => {
                self.delayed_messages.push(*msg);
            }
            Message::Discard => {
                self.handle_discard(ctx, messages)?;
            }
            Message::DropToBeDiscarded => {
                self.to_be_discarded.clear();
            }
            Message::AndThen { lhs, rhs } => {
                self.try_handle_message(ctx, out, mode, terminal_guard, messages, *lhs)?;

                // Push `rhs` to the end of the queue. That way any messages enqueued by `lhs` will
                // be handled first.
                messages.push(*rhs);
            }
            Message::Debug(text) => {
                messages.push(Message::ShowToast {
                    kind: ToastKind::Debug,
                    text: text.to_owned(),
                });
            }
            Message::GrowDetails => {
                self.update_status_width_percentage(
                    self.status_width_percentage
                        .saturating_sub(DETAILS_SIZE_ADJUSTMENT_PERCENTAGE),
                    terminal_area,
                );
            }
            Message::ShrinkDetails => {
                self.update_status_width_percentage(
                    self.status_width_percentage
                        .saturating_add(DETAILS_SIZE_ADJUSTMENT_PERCENTAGE),
                    terminal_area,
                );
            }
            Message::PickAndGotoBranch => {
                self.handle_pick_and_goto_branch(ctx)?;
            }
            Message::ToggleHelp => {
                self.handle_toggle_help();
            }
            Message::Mark => {
                self.handle_mark(ctx)?;
            }
            Message::ClearNormalModeMarks => {
                self.handle_clear_normal_mode_marks();
            }
            Message::SetHasFocus(has_focus) => {
                self.has_focus = has_focus;
            }
            Message::Undo => {
                self.handle_undo(ctx)?;
            }
            Message::Redo => {
                self.handle_redo(ctx)?;
            }
            Message::Stack(stack_message) => match stack_message {
                StackMessage::Enter => self.handle_stack_enter(ctx)?,
                StackMessage::ShowApplyPicker => self.handle_stack_show_apply_picker(ctx)?,
                StackMessage::Unapply => self.handle_stack_unapply(),
                StackMessage::MoveStart => self.handle_stack_move_start(),
                StackMessage::MoveConfirm => self.handle_stack_move_confirm(ctx, messages)?,
            },
        }

        ensure_cursor_visible(self, visible_height);

        if cfg!(feature = "tui-profiling") && !cfg!(test) {
            let elapsed_ms = start.elapsed().as_millis();
            if !matches!(
                discriminant,
                MessageDiscriminant::Reload | MessageDiscriminant::Command
            ) && elapsed_ms > 60
            {
                self.toasts.insert(
                    ToastKind::Debug,
                    format!("Slow message: {discriminant:?} {elapsed_ms:?} ms"),
                );
            }
        }

        Ok(())
    }

    fn handle_quit(&mut self) {
        self.outcome = Some(TuiOutcome::None);
    }

    fn handle_confirm_and_quit(&mut self) {
        self.outcome = match &*self.mode {
            Mode::Normal(..)
            | Mode::Rub(..)
            | Mode::InlineReword(..)
            | Mode::Command(..)
            | Mode::Commit(..)
            | Mode::Move(..)
            | Mode::Details(..)
            | Mode::Stack(..)
            | Mode::MoveStack(..) => Some(TuiOutcome::None),
            Mode::PickChanges(PickUncommittedMode { marks }) => {
                let ids = marks
                    .iter()
                    .cloned()
                    .map(|mark| mark.into_cli_id())
                    .collect();
                Some(TuiOutcome::CliIds(ids))
            }
        };
    }

    fn handle_unfocus_details(&mut self, messages: &mut Vec<Message>) {
        if let Mode::Details(DetailsMode { full_screen, .. }) = &*self.mode {
            if *full_screen {
                return;
            }

            self.restore_mode_before_details(messages);
            self.maybe_move_cursor_into_file_list();
            return;
        }

        self.unfocus_details_regardless_if_we_are_full_screen_or_not(messages);
    }

    fn unfocus_details_regardless_if_we_are_full_screen_or_not(
        &mut self,
        messages: &mut Vec<Message>,
    ) {
        self.mode.update(&mut self.backstack, |backstack, mode| {
            *mode = Mode::Normal(Default::default());
            backstack.remove_leave_normal_mode();
        });

        messages.push(Message::Details(DetailsMessage::Deselect));
    }

    fn handle_enter_normal_mode_after_confirming_operation(&mut self, messages: &mut Vec<Message>) {
        let mut entries_to_handle = Vec::new();
        self.mode.update(&mut self.backstack, |backstack, mode| {
            backstack.retain(|entry| match entry {
                BackstackEntry::ShowFileList => {
                    // this keeps the global file list open after performing operations such as
                    // committing or rubbing
                    true
                }
                BackstackEntry::LeaveNormalMode | BackstackEntry::Mark => {
                    entries_to_handle.push(entry);
                    false
                }
                BackstackEntry::OpenSplitDetailsView => true,
                BackstackEntry::OpenFullScreenDetailsView => {
                    entries_to_handle.push(entry);
                    false
                }
            });

            *mode = Mode::Normal(NormalMode::default());
        });

        for entry in entries_to_handle {
            self.handle_backstack_entry(entry, messages);
        }

        self.maybe_move_cursor_into_file_list();
    }

    fn handle_back(&mut self, messages: &mut Vec<Message>) {
        if let Some(entry) = self.backstack.pop() {
            self.handle_backstack_entry(entry, messages);
        }
    }

    fn handle_backstack_entry(&mut self, entry: BackstackEntry, messages: &mut Vec<Message>) {
        match entry {
            BackstackEntry::LeaveNormalMode => {
                if !self.restore_mode_before_details(messages) {
                    let marks = self.marks().cloned().unwrap_or_default();
                    self.mode.update(&mut self.backstack, |backstack, mode| {
                        let _ = backstack;
                        *mode = Mode::Normal(NormalMode { marks });
                    });
                }
                self.maybe_move_cursor_into_file_list();
            }
            BackstackEntry::ShowFileList => {
                self.flags.show_files = FilesStatusFlag::None;
                messages.push(Message::Reload(None, ReloadCause::ViewOnly));
            }
            BackstackEntry::Mark => match self
                .mode
                .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
            {
                Mode::Normal(normal_mode) => {
                    normal_mode.marks.clear();
                }
                Mode::PickChanges(pick_uncommitted_mode) => {
                    pick_uncommitted_mode.marks.clear();
                }
                Mode::Rub(..) => {
                    *self
                        .mode
                        .get_mut_without_updating_backstack_and_i_promise_not_to_change_state() =
                        Mode::Normal(NormalMode::default());
                    self.backstack.remove_mark();
                    self.backstack.remove_leave_normal_mode();
                    self.maybe_move_cursor_into_file_list();
                }
                Mode::InlineReword(..)
                | Mode::Command(..)
                | Mode::Commit(..)
                | Mode::Move(..)
                | Mode::Stack(..)
                | Mode::MoveStack(..)
                | Mode::Details(..) => {}
            },
            BackstackEntry::OpenSplitDetailsView | BackstackEntry::OpenFullScreenDetailsView => {
                messages.push(Message::DetailsLayout(
                    DetailsLayoutMessage::ToggleVisibility,
                ));
            }
        }
    }

    fn restore_mode_before_details(&mut self, messages: &mut Vec<Message>) -> bool {
        self.mode.update(&mut self.backstack, |backstack, mode| {
            let previous_mode = std::mem::replace(mode, Mode::Normal(NormalMode::default()));
            let Mode::Details(details_mode) = previous_mode else {
                *mode = previous_mode;
                return false;
            };

            backstack.remove_leave_normal_mode();
            if details_mode.full_screen {
                backstack.remove_open_details_view();
                messages.push(Message::DetailsLayout(
                    DetailsLayoutMessage::ToggleVisibility,
                ));
            } else {
                messages.push(Message::Details(DetailsMessage::Deselect));
            }

            *mode = match details_mode.return_mode {
                DetailsReturnMode::Normal(normal_mode) => Mode::Normal(normal_mode),
                DetailsReturnMode::PickChanges(pick_uncommitted_mode) => {
                    Mode::PickChanges(pick_uncommitted_mode)
                }
            };
            true
        })
    }

    fn maybe_move_cursor_into_file_list(&mut self) {
        match self.flags.show_files {
            FilesStatusFlag::Commit(object_id) => {
                // When viewing files in a commit cursor movement is constrained to only those
                // files. However you can start a rub which then enables moving outside the file
                // list, while keeping the file list visible. Thus when entering normal mode
                // (perhaps from cancelling the rub) we need to potentially move the cursor back to
                // the file list.
                let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
                    return;
                };

                if let Some(cli_id) = selection.data.cli_id()
                    && let CliId::CommittedFile { commit_id, .. } = &**cli_id
                    && *commit_id == object_id
                {
                    // cursor is already within the file list
                } else {
                    self.cursor =
                        Cursor::select_first_file_in_commit(object_id, &self.status_lines)
                            .unwrap_or(self.cursor);
                }
            }
            FilesStatusFlag::None | FilesStatusFlag::All => {}
        }
    }

    fn handle_focus_details(&mut self, full_screen: bool, messages: &mut Vec<Message>) {
        if !full_screen
            && let Mode::Details(DetailsMode {
                full_screen: false, ..
            }) = &*self.mode
        {
            self.restore_mode_before_details(messages);
            return;
        }

        if self.is_details_visible {
            messages.push(Message::Details(DetailsMessage::SelectFirstSection));
        } else {
            messages.push(Message::DetailsLayout(
                DetailsLayoutMessage::ToggleVisibility,
            ));

            // We can't select the first section on the same frame that we show the detail view.
            // The incremental diff rendering introduces a one frame delay before the first section
            // is shown.
            messages
                .push(Message::Details(DetailsMessage::SelectFirstSection).with_one_frame_delay());

            self.backstack.push_open_details_view(full_screen);
        }

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                let previous_mode = std::mem::replace(mode, Mode::Normal(NormalMode::default()));
                let return_mode = match previous_mode {
                    Mode::PickChanges(pick_uncommitted_mode) => {
                        DetailsReturnMode::PickChanges(pick_uncommitted_mode)
                    }
                    Mode::Details(details_mode) => details_mode.return_mode,
                    Mode::Normal(normal_mode) => DetailsReturnMode::Normal(normal_mode),
                    Mode::Rub(..)
                    | Mode::InlineReword(..)
                    | Mode::Command(..)
                    | Mode::Commit(..)
                    | Mode::Move(..)
                    | Mode::MoveStack(..)
                    | Mode::Stack(..) => DetailsReturnMode::Normal(NormalMode::default()),
                };
                *mode = Mode::Details(DetailsMode {
                    full_screen,
                    return_mode,
                });
            });
    }

    fn handle_toggle_details_full_screen(&mut self, messages: &mut Vec<Message>) {
        match &*self.mode {
            Mode::Normal(..) | Mode::PickChanges(..) => {
                messages.push(Message::DetailsLayout(DetailsLayoutMessage::Focus {
                    full_screen: true,
                }));
            }
            Mode::Details(DetailsMode { full_screen, .. }) => {
                if *full_screen {
                    self.restore_mode_before_details(messages);
                } else {
                    messages.push(Message::DetailsLayout(DetailsLayoutMessage::Focus {
                        full_screen: true,
                    }));
                }
            }
            Mode::Rub(..)
            | Mode::InlineReword(..)
            | Mode::Command(..)
            | Mode::Commit(..)
            | Mode::Stack(..)
            | Mode::MoveStack(..)
            | Mode::Move(..) => {}
        }
    }

    fn handle_toggle_details_visibility(&mut self, messages: &mut Vec<Message>) {
        self.is_details_visible = !self.is_details_visible;

        if self.is_details_visible {
            self.details.mark_dirty();

            if matches!(&*self.mode, Mode::Normal(..)) {
                self.backstack.push_open_details_view(false);
            }
        } else {
            self.backstack.remove_open_details_view();
            self.details.reset_scroll();
            if matches!(&*self.mode, Mode::Details(..)) {
                messages.push(Message::UnfocusDetails);
            }
        }
    }

    fn handle_dismiss_details(&mut self, messages: &mut Vec<Message>) {
        if let Mode::Details(details_mode) = &*self.mode
            && details_mode.full_screen
        {
            messages.push(Message::DetailsLayout(
                DetailsLayoutMessage::ToggleFullScreen,
            ));
        } else {
            messages.push(Message::DetailsLayout(
                DetailsLayoutMessage::ToggleVisibility,
            ));
        }
    }

    fn handle_rub_start(&mut self) {
        let Mode::Normal(normal_mode) = &*self.mode else {
            return;
        };
        let Some(selected_line) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };
        let Some(cli_id) = selected_line.data.cli_id() else {
            return;
        };
        if normal_mode.marks.is_empty() {
            self.handle_rub_start_with_source(RubSource::CliId(Arc::clone(cli_id)), None);
        } else {
            self.handle_rub_start_with_source(RubSource::Marks(normal_mode.marks.clone()), None);
        }
    }

    fn available_targets_for_rub_mode(&self, source: &RubSource) -> Vec<Arc<CliId>> {
        match &source {
            RubSource::CliId(source) => self
                .status_lines
                .iter()
                .filter_map(|line| line.data.cli_id())
                .filter(|target| {
                    source == *target
                        || rub::route_operation(
                            NonEmpty::new(source),
                            target,
                            MessageCombinationStrategy::KeepBoth,
                        )
                        .is_some()
                })
                .cloned()
                .collect::<Vec<_>>(),
            RubSource::CommittedHunk(hunk) => self
                .status_lines
                .iter()
                .filter_map(|line| line.data.cli_id())
                .filter(|target| {
                    source.contains(target)
                        || rub_from_detail_view::route_operation(hunk, target).is_some()
                })
                .cloned()
                .collect::<Vec<_>>(),
            RubSource::Marks(marks) => {
                let marks = marks
                    .iter()
                    .cloned()
                    .map(|mark| mark.into_cli_id())
                    .collect::<Vec<_>>();
                self.status_lines
                    .iter()
                    .filter_map(|line| line.data.cli_id())
                    .filter(|target| {
                        source.contains(target) || {
                            marks.iter().all(|mark| {
                                rub::route_operation(
                                    NonEmpty::new(mark),
                                    target,
                                    MessageCombinationStrategy::KeepBoth,
                                )
                                .is_some()
                            })
                        }
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            }
        }
    }

    fn handle_rub_start_with_source(
        &mut self,
        source: RubSource,
        unlock_details: Option<MessageOnDrop>,
    ) {
        match &source {
            RubSource::CliId(cli_id) => {
                if !rub::supports_rubbing(cli_id) {
                    return;
                }
            }
            RubSource::Marks(marks) => {
                let MarkClasses {
                    marked_commits,
                    marked_uncommitted,
                } = marks.classify();
                if marked_commits && marked_uncommitted {
                    return;
                }

                for mark in marks {
                    if !rub::mark_supports_rubbing(mark) {
                        return;
                    }
                }
            }
            RubSource::CommittedHunk(..) => {}
        }

        let available_targets = self.available_targets_for_rub_mode(&source);

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Rub(RubMode {
                    source,
                    available_targets,
                    how_to_combine_messages: MessageCombinationStrategy::KeepBoth,
                    _unlock_details: unlock_details,
                });
            });

        if self
            .cursor
            .selected_line(&self.status_lines)
            .is_some_and(|line| {
                cursor::is_selectable_in_mode(line, &self.mode, self.flags.show_files)
            })
        {
            return;
        }

        if let Some(new_cursor) =
            self.cursor
                .move_down(&self.status_lines, &self.mode, self.flags.show_files)
        {
            self.cursor = new_cursor;
        } else if let Some(new_cursor) =
            self.cursor
                .move_up(&self.status_lines, &self.mode, self.flags.show_files)
        {
            self.cursor = new_cursor;
        }
    }

    fn handle_rub_start_reverse(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
        else {
            return Ok(());
        };

        let CliId::Commit { commit_id, .. } = &**selection else {
            return Ok(());
        };

        let stack_id = {
            let (_guard, _, ws, _) = ctx.workspace_and_db()?;
            ws.find_commit_and_containers(*commit_id)
                .and_then(|(stack, _, _)| stack.id)
        };

        let source = if let Some(stack_id) = stack_id
            && operations::stack_has_assigned_changes(ctx, stack_id)?
            && let Some(id) = self
                .status_lines
                .iter()
                .filter_map(|line| line.data.cli_id())
                .find_map(|id| {
                    if let CliId::Stack { id, stack_id: sid } = &**id
                        && *sid == stack_id
                    {
                        Some(id)
                    } else {
                        None
                    }
                }) {
            RubSource::CliId(Arc::new(CliId::Stack {
                id: id.to_owned(),
                stack_id,
            }))
        } else {
            RubSource::CliId(Arc::new(CliId::Unassigned {
                id: UNASSIGNED.to_owned(),
            }))
        };

        let available_targets = self.available_targets_for_rub_mode(&source);

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Rub(RubMode {
                    source,
                    available_targets,
                    how_to_combine_messages: MessageCombinationStrategy::KeepBoth,
                    _unlock_details: None,
                });
            });

        Ok(())
    }

    fn handle_files_toggle_global_files_list(&mut self, messages: &mut Vec<Message>) {
        self.flags.show_files = match self.flags.show_files {
            FilesStatusFlag::None => {
                self.backstack.push_show_file_list();
                FilesStatusFlag::All
            }
            FilesStatusFlag::All | FilesStatusFlag::Commit(_) => {
                self.backstack.remove_show_file_list();
                FilesStatusFlag::None
            }
        };
        messages.push(Message::Reload(None, ReloadCause::ViewOnly));
    }

    fn handle_files_toggle_files_for_commit(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if let Mode::Normal(normal_mode) = &*self.mode
            && !normal_mode.marks.is_empty()
        {
            let MarkClasses {
                marked_commits,
                // with marked uncommitted files the cursor cannot move out of the unassigned
                // changes section, thus you can never toggle files for a commit because that
                // requires selecting the commit
                marked_uncommitted: _,
            } = normal_mode.marks.classify();
            if marked_commits {
                match self.flags.show_files {
                    FilesStatusFlag::None => {
                        return Ok(());
                    }
                    FilesStatusFlag::Commit(_) => {}
                    FilesStatusFlag::All => {
                        self.flags.show_files = FilesStatusFlag::None;
                        self.backstack.remove_show_file_list();
                        messages.push(Message::Reload(None, ReloadCause::ViewOnly));
                        return Ok(());
                    }
                }
            }
        }

        if let Some(selection) = self.cursor.selected_line(&self.status_lines)
            && let Some(cli_id) = selection.data.cli_id()
            && let CliId::Commit { commit_id, .. } = &**cli_id
        {
            if !operations::commit_is_empty(ctx, *commit_id)? {
                let select_after_reload = match self.flags.show_files {
                    FilesStatusFlag::None => {
                        self.flags.show_files = FilesStatusFlag::Commit(*commit_id);
                        self.backstack.push_show_file_list();
                        Some(SelectAfterReload::FirstFileInCommit(*commit_id))
                    }
                    FilesStatusFlag::All | FilesStatusFlag::Commit(_) => {
                        self.flags.show_files = FilesStatusFlag::None;
                        self.backstack.remove_show_file_list();
                        Some(SelectAfterReload::Commit(*commit_id))
                    }
                };
                messages.push(Message::Reload(select_after_reload, ReloadCause::ViewOnly));
            }
        } else {
            self.flags.show_files = FilesStatusFlag::None;
            self.backstack.remove_show_file_list();
            messages.push(Message::Reload(None, ReloadCause::ViewOnly));
        };

        Ok(())
    }

    fn handle_rub_use_target_message(&mut self) {
        let Mode::Rub(RubMode {
            how_to_combine_messages,
            ..
        }) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };
        *how_to_combine_messages = match *how_to_combine_messages {
            MessageCombinationStrategy::KeepBoth | MessageCombinationStrategy::KeepSubject => {
                MessageCombinationStrategy::KeepTarget
            }
            MessageCombinationStrategy::KeepTarget => MessageCombinationStrategy::KeepBoth,
        };
    }

    fn handle_rub_use_source_message(&mut self) {
        let Mode::Rub(RubMode {
            how_to_combine_messages,
            ..
        }) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };
        *how_to_combine_messages = match *how_to_combine_messages {
            MessageCombinationStrategy::KeepBoth | MessageCombinationStrategy::KeepTarget => {
                MessageCombinationStrategy::KeepSubject
            }
            MessageCombinationStrategy::KeepSubject => MessageCombinationStrategy::KeepBoth,
        };
    }

    /// Handles confirming the currently selected rub operation.
    fn handle_rub_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Rub(RubMode {
            source,
            how_to_combine_messages,
            available_targets: _,
            _unlock_details: _,
        }) = &*self.mode
        else {
            return Ok(());
        };

        let Some(target) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
        else {
            return Ok(());
        };

        let reload_message = match source {
            RubSource::CliId(source) => {
                if let Some(operation) =
                    rub::route_operation(NonEmpty::new(source), target, *how_to_combine_messages)
                {
                    let what_to_select = operations::rub(ctx, &operation)?;
                    Message::Reload(what_to_select, ReloadCause::Mutation)
                } else {
                    return Ok(());
                }
            }
            RubSource::CommittedHunk(hunk) => {
                if let Some(operation) = rub_from_detail_view::route_operation(hunk, target) {
                    Message::Reload(Some(operation.execute(ctx)?), ReloadCause::Mutation)
                } else {
                    return Ok(());
                }
            }
            RubSource::Marks(marks) => {
                let sources = marks
                    .iter()
                    .cloned()
                    .map(|mark| mark.into_cli_id())
                    .filter(|source| source != &**target)
                    .collect::<Vec<_>>();
                let mut iter = sources.iter();
                if let Some(sources) = iter.next().map(|first| nonempty_from_refs(first, iter))
                    && let Some(operation) =
                        rub::route_operation(sources, target, *how_to_combine_messages)
                {
                    let what_to_select = operations::rub(ctx, &operation)?;
                    Message::Reload(what_to_select, ReloadCause::Mutation)
                } else {
                    return Ok(());
                }
            }
        };

        match self.flags.show_files {
            FilesStatusFlag::Commit(..) => {
                self.backstack.remove_show_file_list();
                self.flags.show_files = FilesStatusFlag::None;
            }
            FilesStatusFlag::None | FilesStatusFlag::All => {}
        }

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            reload_message,
        ]);

        Ok(())
    }

    /// Handles reloading status output and restoring selection.
    fn handle_reload(
        &mut self,
        ctx: &mut Context,
        out: &mut dyn TuiInputOutputChannel,
        mode: &OperatingMode,
        select_after_reload: Option<SelectAfterReload>,
        cause: ReloadCause,
    ) -> anyhow::Result<()> {
        let new_lines = operations::reload_legacy(ctx, out, mode, self.flags, self.launch_options)?;

        self.cursor = if let Some(select_after_reload) = select_after_reload {
            match select_after_reload {
                SelectAfterReload::Commit(commit_id) => {
                    Cursor::select_commit(commit_id, &new_lines)
                }
                SelectAfterReload::Branch(branch) => Cursor::select_branch(&branch, &new_lines),
                SelectAfterReload::Unassigned => Cursor::select_unassigned(&new_lines),
                SelectAfterReload::UncommittedFile { path, stack_id } => {
                    Cursor::select_uncommitted_file(path.as_ref(), stack_id, &new_lines)
                }
                SelectAfterReload::FirstFileInCommit(commit_id) => {
                    Cursor::select_first_file_in_commit(commit_id, &new_lines)
                }
                SelectAfterReload::Stack(stack_id) => Cursor::select_stack(stack_id, &new_lines),
                SelectAfterReload::CliId(cli_id) => Cursor::restore(&cli_id, &new_lines),
            }
        } else {
            let selected_merge_base = self
                .cursor
                .selected_line(&self.status_lines)
                .is_some_and(|line| matches!(line.data, StatusOutputLineData::MergeBase));

            let default_restore = || {
                self.cursor
                    .selection_cli_id_for_reload(&self.status_lines, self.flags.show_files)
                    .and_then(|previously_selected_cli_id| {
                        Cursor::restore(previously_selected_cli_id, &new_lines)
                    })
            };

            if selected_merge_base {
                Cursor::select_merge_base(&new_lines).or_else(default_restore)
            } else {
                default_restore()
            }
        }
        .unwrap_or_else(|| Cursor::new(&new_lines));

        self.status_lines = new_lines;

        match cause {
            ReloadCause::Watcher | ReloadCause::ViewOnly | ReloadCause::Manual => {}
            ReloadCause::Mutation => {
                self.previous_reload_caused_by_mutation_timestamp = Some(Instant::now());
            }
        }

        Ok(())
    }

    /// Handles showing a transient UI error.
    fn handle_show_error(&mut self, err: Arc<anyhow::Error>, messages: &mut Vec<Message>) {
        self.toasts
            .insert(ToastKind::Error, format_error_for_tui(&err));

        // ensure we always enter normal mode when something does wrong
        // so we don't get stuck in whatever mode we were in previously
        messages.push(Message::EnterNormalModeAfterConfirmingOperation);
    }

    fn select_top_branch_for_stack_after_reload(
        &self,
        stack_id: StackId,
    ) -> Option<SelectAfterReload> {
        self.status_lines.iter().find_map(|line| {
            let cli_id = line.data.cli_id()?;
            if let CliId::Branch {
                stack_id: Some(branch_stack_id),
                ..
            } = &**cli_id
                && *branch_stack_id == stack_id
            {
                Some(SelectAfterReload::CliId(Arc::clone(cli_id)))
            } else {
                None
            }
        })
    }

    fn handle_discard(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if self.marks().is_none_or(|marks| marks.is_empty()) {
            self.handle_discard_selection(ctx, messages)
        } else {
            self.handle_discard_marks(ctx, messages)
        }
    }

    fn handle_discard_selection(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };
        let Some(cli_id) = selection.data.cli_id() else {
            return Ok(());
        };

        self.modal = Some(Modal::Confirm {
            confirm: match &**cli_id {
                CliId::Unassigned { .. } => {
                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new("Discard unassigned changes?".into()),
                        self.theme,
                        move |ctx, messages| {
                            operations::discard_unassigned_legacy(ctx)?;
                            messages.push(Message::Reload(
                                Some(SelectAfterReload::Unassigned),
                                ReloadCause::Mutation,
                            ));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Uncommitted(uncommitted) => {
                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let uncommitted = uncommitted.clone();

                    let select_after_reload = if uncommitted.is_entire_file
                    // Discarding a whole file: handle stack-specific cursor fallback.
                    && let Some(stack_id) = uncommitted.hunk_assignments.first().stack_id
                    // If this is the last file on the stack, jump to the stack's top branch.
                    && operations::assigned_file_count_for_stack(ctx, stack_id)
                        .is_ok_and(|count| count == 1)
                    {
                        self.select_top_branch_for_stack_after_reload(stack_id)
                            .unwrap_or(SelectAfterReload::Stack(stack_id))
                    } else {
                        // Discarding only part of a file: select the previous selectable line.
                        self.cursor.select_previous_cli_id_or_unassigned(
                            &self.status_lines,
                            &self.mode,
                            self.flags.show_files,
                        )
                    };

                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new("Discard uncommitted file?".into()),
                        self.theme,
                        move |ctx, messages| {
                            let hunk_assignments = uncommitted
                                .hunk_assignments
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>();
                            operations::discard_uncommitted_legacy(ctx, hunk_assignments)?;
                            messages.push(Message::Reload(
                                Some(select_after_reload),
                                ReloadCause::Mutation,
                            ));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Stack { stack_id, .. } => {
                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let stack_id = *stack_id;
                    let select_after_reload = self
                        .select_top_branch_for_stack_after_reload(stack_id)
                        .unwrap_or(SelectAfterReload::Stack(stack_id));
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new("Discard staged changes in this stack?".into()),
                        self.theme,
                        move |ctx, messages| {
                            operations::discard_stack(ctx, stack_id)?;
                            messages.push(Message::Reload(
                                Some(select_after_reload),
                                ReloadCause::Mutation,
                            ));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Commit { commit_id, .. } => {
                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let commit_id = *commit_id;
                    let select_after_reload = self
                        .cursor
                        .select_after_discarded_commit(&self.status_lines);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new(
                            format!("Discard commit {}?", commit_id.to_hex_with_len(7)).into(),
                        ),
                        self.theme,
                        move |ctx, messages| {
                            let discard_result = operations::commit_discard(ctx, commit_id)?;
                            let select_after_reload =
                                select_after_reload.map(|selection| match selection {
                                    SelectAfterReload::Commit(target_commit_id) => {
                                        let remapped_target_commit_id = discard_result
                                            .workspace
                                            .replaced_commits
                                            .get(&target_commit_id)
                                            .copied()
                                            .unwrap_or(target_commit_id);
                                        SelectAfterReload::Commit(remapped_target_commit_id)
                                    }
                                    other => other,
                                });
                            messages
                                .push(Message::Reload(select_after_reload, ReloadCause::Mutation));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Branch { name, stack_id, .. } => {
                    let Some(stack_id) = *stack_id else {
                        return Ok(());
                    };

                    let name = name.to_owned();

                    let commits = commits_on_branch(ctx, stack_id, &name)?;

                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let select_after_reload = self
                        .cursor
                        .select_after_discarded_branch(&self.status_lines);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);

                    Confirm::new(
                        NonEmpty::new(format!("Discard branch {name}?").into()),
                        self.theme,
                        move |ctx, messages| {
                            let mut meta = ctx.meta()?;
                            let snapshot_details =
                                SnapshotDetails::new(OperationKind::DeleteBranch);

                            let refname = Category::LocalBranch.to_full_name(&*name)?;
                            but_transaction::with_transaction(
                                ctx,
                                &mut meta,
                                snapshot_details,
                                DryRun::No,
                                |mut tx| {
                                    tx.remove_reference(refname.as_ref())?;
                                    if !commits.is_empty() {
                                        tx.discard_commits(
                                            commits.into_iter().map(|(commit, _)| commit),
                                        )?;
                                    }
                                    Ok(())
                                },
                            )?;

                            messages
                                .push(Message::Reload(select_after_reload, ReloadCause::Mutation));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::PathPrefix { .. } | CliId::CommittedFile { .. } => return Ok(()),
            },
            key_binds: confirm_key_binds(),
        });

        Ok(())
    }

    fn handle_discard_marks(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Normal(normal_mode) = &*self.mode else {
            return Ok(());
        };

        if normal_mode.marks.is_empty() {
            return Ok(());
        }

        let commits = normal_mode
            .marks
            .iter()
            .filter_map(|mark| match mark {
                Markable::Commit { commit_id, .. } => Some(*commit_id),
                Markable::Uncommitted(..) => None,
            })
            .collect::<Vec<_>>();

        let uncommitted_diff_specs = {
            let context_lines = ctx.settings.context_lines;
            let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);

            for mark in &normal_mode.marks {
                match mark {
                    Markable::Uncommitted(uncommitted) => {
                        builder.push_changes_from_uncommitted(uncommitted)?;
                    }
                    Markable::Commit { .. } => {}
                }
            }

            builder.into_diff_specs()
        };

        self.to_be_discarded = normal_mode
            .marks
            .iter()
            .cloned()
            .map(|mark| Arc::new(mark.into_cli_id()))
            .collect::<Vec<_>>();

        let select_after_reload = self
            .cursor
            .select_after_discarded_marks(&self.status_lines, &normal_mode.marks);

        let drop_to_be_discarded =
            message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);

        let confirm = Confirm::new(
            NonEmpty::new("Discard?".into()),
            self.theme,
            move |ctx, messages| {
                let mut meta = ctx.meta()?;
                let snapshot_details = SnapshotDetails::new(OperationKind::Discard);
                let workspace = but_transaction::with_transaction(
                    ctx,
                    &mut meta,
                    snapshot_details,
                    DryRun::No,
                    |mut tx| {
                        if !commits.is_empty() {
                            tx.discard_commits(commits)?;
                        }

                        if !uncommitted_diff_specs.is_empty() {
                            but_workspace::discard_workspace_changes(
                                tx.repo(),
                                uncommitted_diff_specs,
                                tx.context_lines(),
                            )?;
                        }

                        Ok(())
                    },
                )?;
                let select_after_reload = select_after_reload.map(|selection| match selection {
                    SelectAfterReload::Commit(target_commit_id) => {
                        let remapped_target_commit_id = workspace
                            .replaced_commits
                            .get(&target_commit_id)
                            .copied()
                            .unwrap_or(target_commit_id);
                        SelectAfterReload::Commit(remapped_target_commit_id)
                    }
                    other => other,
                });

                drop(drop_to_be_discarded);
                messages.extend([
                    Message::ClearNormalModeMarks,
                    Message::Reload(select_after_reload, ReloadCause::Mutation),
                ]);
                Ok(())
            },
        );

        self.modal = Some(Modal::Confirm {
            confirm,
            key_binds: confirm_key_binds(),
        });

        Ok(())
    }

    /// Handles creating an empty commit relative to the current selection.
    fn handle_commit_create_empty(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        match &selection.data {
            StatusOutputLineData::Branch { cli_id } => {
                let CliId::Branch { name, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_result = operations::create_empty_commit_relative_to_branch(ctx, name)?;

                messages.push(Message::Reload(
                    Some(SelectAfterReload::Commit(commit_result.new_commit)),
                    ReloadCause::Mutation,
                ));
            }
            StatusOutputLineData::Commit { cli_id, .. } => {
                let CliId::Commit { commit_id, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_result =
                    operations::create_empty_commit_relative_to_commit(ctx, *commit_id)?;

                messages.push(Message::Reload(
                    Some(SelectAfterReload::Commit(commit_result.new_commit)),
                    ReloadCause::Mutation,
                ));
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UnassignedChanges { .. }
            | StatusOutputLineData::UnassignedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => {}
        }

        Ok(())
    }

    fn handle_commit_start(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        if self.marks().is_none_or(|marks| marks.is_empty()) {
            self.handle_commit_start_selection(ctx)?;
        } else {
            self.handle_commit_start_marks();
        }
        Ok(())
    }

    fn handle_commit_start_selection(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let commit_mode = match &selection.data {
            StatusOutputLineData::UnassignedChanges { cli_id } => {
                let Some(source) = CommitSource::try_new(Arc::unwrap_or_clone(Arc::clone(cli_id)))
                else {
                    return Ok(());
                };
                CommitMode {
                    source: Arc::new(source),
                    insert_side: InsertSide::Below,
                    scope_to_stack: None,
                    message_composer: CommitMessageComposer::default(),
                }
            }
            StatusOutputLineData::UnassignedFile { cli_id }
            | StatusOutputLineData::StagedChanges { cli_id }
            | StatusOutputLineData::StagedFile { cli_id } => {
                let Some(source) = CommitSource::try_new(Arc::unwrap_or_clone(Arc::clone(cli_id)))
                else {
                    return Ok(());
                };
                CommitMode {
                    source: Arc::new(source),
                    insert_side: InsertSide::Below,
                    scope_to_stack: cli_id.stack_id(),
                    message_composer: CommitMessageComposer::default(),
                }
            }
            StatusOutputLineData::Commit { stack_id, .. } => {
                let (source, scope_to_stack) = if let Some(stack_id) = *stack_id
                    && stack_has_assigned_changes(ctx, stack_id)?
                {
                    (
                        CommitSource::Stack(StackCommitSource { stack_id }),
                        Some(stack_id),
                    )
                } else {
                    (
                        CommitSource::Unassigned(UnassignedCommitSource {
                            id: UNASSIGNED.to_string(),
                        }),
                        None,
                    )
                };
                CommitMode {
                    scope_to_stack,
                    insert_side: InsertSide::Below,
                    message_composer: CommitMessageComposer::default(),
                    source: Arc::new(source),
                }
            }
            StatusOutputLineData::Branch { cli_id } => {
                let CliId::Branch { stack_id, .. } = &**cli_id else {
                    return Ok(());
                };
                let (source, scope_to_stack) = if let Some(stack_id) = *stack_id
                    && stack_has_assigned_changes(ctx, stack_id)?
                {
                    (
                        CommitSource::Stack(StackCommitSource { stack_id }),
                        Some(stack_id),
                    )
                } else {
                    (
                        CommitSource::Unassigned(UnassignedCommitSource {
                            id: UNASSIGNED.to_string(),
                        }),
                        None,
                    )
                };
                CommitMode {
                    source: Arc::new(source),
                    insert_side: InsertSide::Below,
                    scope_to_stack,
                    message_composer: CommitMessageComposer::default(),
                }
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Commit(commit_mode);
            });

        Ok(())
    }

    fn handle_commit_start_marks(&mut self) {
        let Mode::Normal(normal_mode) = &*self.mode else {
            return;
        };

        if normal_mode.marks.is_empty() {
            return;
        }

        let uncommitted = normal_mode
            .marks
            .iter()
            .cloned()
            .map(|mark| match mark {
                Markable::Uncommitted(uncommitted_cli_id) => Some(uncommitted_cli_id),
                Markable::Commit { .. } => None,
            })
            .collect::<Option<Vec<_>>>();
        let Some(uncommitted) = uncommitted else {
            return;
        };

        if uncommitted.is_empty() {
            return;
        }

        let source = Arc::new(CommitSource::Marks(normal_mode.marks.clone()));

        if let Some(cursor) = self
            .cursor
            .select_closest_commit_source(&self.status_lines, &source)
        {
            self.cursor = cursor;
        }

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Commit(CommitMode {
                    source,
                    insert_side: InsertSide::Below,
                    scope_to_stack: None,
                    message_composer: CommitMessageComposer::default(),
                });
            });
    }

    fn handle_commit_confirm<T>(
        &mut self,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Mode::Commit(CommitMode {
            source,
            insert_side,
            scope_to_stack,
            message_composer,
        }) = &*self.mode
        else {
            return Ok(());
        };

        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        if selection
            .data
            .cli_id()
            .is_some_and(|target| source.contains(target))
        {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        let target = match &selection.data {
            StatusOutputLineData::Branch { cli_id }
            | StatusOutputLineData::Commit { cli_id, .. } => cli_id,
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UnassignedChanges { .. }
            | StatusOutputLineData::UnassignedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => {
                return Ok(());
            }
        };

        let Some((insert_commit_relative_to, insert_side)) =
            operations::where_to_place_commit(ctx, target, *insert_side)?
        else {
            return Ok(());
        };

        let changes_to_commit = {
            let context_lines = ctx.settings.context_lines;
            let guard = ctx.shared_worktree_access();
            let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(guard.read_permission())?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines)
                .with_scope_to_stack(*scope_to_stack);
            match &**source {
                CommitSource::Marks(marks) => {
                    for mark in marks {
                        let Markable::Uncommitted(uncommitted) = mark else {
                            unreachable!(
                                "commit marks were validated to contain only uncommitted files"
                            );
                        };
                        builder.push_changes_from_uncommitted(uncommitted)?;
                    }
                }
                CommitSource::Unassigned(UnassignedCommitSource { id }) => {
                    builder.push_changes_from_unassigned(id)?;
                }
                CommitSource::Uncommitted(uncommitted) => {
                    builder.push_changes_from_uncommitted(uncommitted)?;
                }
                CommitSource::Stack(StackCommitSource { stack_id }) => {
                    builder.push_changes_from_stack(*stack_id)?;
                }
            }
            builder.into_diff_specs()
        };

        let mut meta = ctx.meta()?;
        let snapshot_details = SnapshotDetails::new(OperationKind::CreateCommit);
        let commit_create_result = but_transaction::with_transaction(
            ctx,
            &mut meta,
            snapshot_details,
            DryRun::No,
            |mut tx| {
                let commit_create_result = tx.create_commit(
                    insert_commit_relative_to,
                    insert_side,
                    changes_to_commit,
                    String::new(),
                )?;

                if commit_create_result.rejected_specs.is_empty() {
                    if let Some(new_commit) = commit_create_result.new_commit {
                        match message_composer {
                            CommitMessageComposer::Editor => {
                                let repo = tx.repo();
                                let commit_details = CommitDetails::from_commit_id(
                                    new_commit.attach(repo),
                                    ComputeLineStats::No.into(),
                                )?;
                                let current_message =
                                    commit_details.commit.inner.message.to_string();

                                let _suspend_guard = terminal_guard.suspend()?;

                                let message = legacy::reword::get_commit_message_from_editor(
                                    tx.repo(),
                                    tx.context_lines(),
                                    commit_details,
                                    String::new(),
                                    &current_message,
                                    ShowDiffInEditor::Unspecified,
                                )?
                                .unwrap_or_default();

                                let reworded_commit =
                                    tx.reword_commit(new_commit, BString::from(message).as_ref())?;

                                Ok(DynamicOutcome::Commit((Some(reworded_commit), None)))
                            }
                            CommitMessageComposer::Inline => {
                                Ok(DynamicOutcome::Commit((
                                    commit_create_result.new_commit,
                                    // TODO(david): rewording separately isn't great because it
                                    // results in two oplog entries. One for creating the commit
                                    // and one for rewording it.
                                    //
                                    // Fixing that is a little tricky because we'd have to show a
                                    // "temp" commit in the status while composing the message and
                                    // then only commit when the user confirms.
                                    Some(Message::Reword(RewordMessage::InlineStart)),
                                )))
                            }
                            CommitMessageComposer::Empty => Ok(DynamicOutcome::Commit((
                                commit_create_result.new_commit,
                                None,
                            ))),
                        }
                    } else {
                        Ok(DynamicOutcome::Commit((
                            commit_create_result.new_commit,
                            None,
                        )))
                    }
                } else {
                    Ok(DynamicOutcome::Rollback(
                        commit_create_result.rejected_specs,
                    ))
                }
            },
        )?;

        match commit_create_result {
            DynamicOutcome::Commit(((new_commit, reword_msg), _workspace)) => {
                messages.extend(
                    [
                        Message::EnterNormalModeAfterConfirmingOperation,
                        Message::Reload(
                            new_commit.map(SelectAfterReload::Commit),
                            ReloadCause::Mutation,
                        ),
                    ]
                    .into_iter()
                    .chain(reword_msg),
                );
            }
            DynamicOutcome::Rollback(rejected_specs) => {
                let mut full_error_msg =
                    "Some selected changes could not be committed:\n".to_owned();
                let mut errors_per_diff_spec = rejected_specs
                .iter()
                .map(|(rejection_reason, diff_spec)| {
                    let human_reason = match rejection_reason {
                        RejectionReason::NoEffectiveChanges => "Changes were a no-op",
                        RejectionReason::CherryPickMergeConflict
                        | RejectionReason::WorkspaceMergeConflict
                        | RejectionReason::WorkspaceMergeConflictOfUnrelatedFile => {
                            "Failed with a conflict. Try committing to a different stack"
                        }
                        RejectionReason::WorktreeFileMissingForObjectConversion => "File was deleted",
                        RejectionReason::FileToLargeOrBinary => "File is too large or binary",
                        RejectionReason::PathNotFoundInBaseTree => {
                            "A change with multiple hunks to be applied wasn't present in the base-tree"
                        }
                        RejectionReason::UnsupportedDirectoryEntry => "Path is not a file",
                        RejectionReason::UnsupportedTreeEntry => "Undiffable entry type",
                        RejectionReason::MissingDiffSpecAssociation => "Missing association between diff and file",
                    };
                    (human_reason, diff_spec)
                }).map(|(human_reason, diff_spec)| {
                    let mut out = format!("- {}: {human_reason}", diff_spec.path);
                    if let Some(previous_path) = &diff_spec.previous_path {
                        out.push_str(&format!(" (previously {previous_path})"));
                    }
                    out
                })
                .peekable();
                while let Some(line) = errors_per_diff_spec.next() {
                    full_error_msg.push_str(&line);
                    if errors_per_diff_spec.peek().is_some() {
                        full_error_msg.push('\n');
                    }
                }

                messages.push(Message::ShowToast {
                    kind: ToastKind::Error,
                    text: full_error_msg,
                });
            }
        }

        Ok(())
    }

    fn handle_commit_to_new_branch(&mut self, messages: &mut Vec<Message>) {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };
        match &selection.data {
            StatusOutputLineData::UnassignedFile { .. }
            | StatusOutputLineData::Branch { .. }
            | StatusOutputLineData::UnassignedChanges { .. } => {}
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return,
        }
        messages.push(Message::NewBranch.and_then(Message::Commit(CommitMessage::Confirm)));
    }

    fn handle_commit_toggle_insert_side(&mut self) {
        let Mode::Commit(commit_mode) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };
        commit_mode.insert_side = match commit_mode.insert_side {
            InsertSide::Above => InsertSide::Below,
            InsertSide::Below => InsertSide::Above,
        };
    }

    fn handle_commit_toggle_message_composer(&mut self, composer: CommitMessageComposer) {
        if let Mode::Commit(mode) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        {
            match composer {
                CommitMessageComposer::Editor => {
                    // you can't toggle the editor composer, that is always the default
                }
                CommitMessageComposer::Empty => {
                    mode.message_composer = match mode.message_composer {
                        CommitMessageComposer::Editor | CommitMessageComposer::Inline => {
                            CommitMessageComposer::Empty
                        }
                        CommitMessageComposer::Empty => CommitMessageComposer::Editor,
                    };
                }
                CommitMessageComposer::Inline => {
                    mode.message_composer = match mode.message_composer {
                        CommitMessageComposer::Editor | CommitMessageComposer::Empty => {
                            CommitMessageComposer::Inline
                        }
                        CommitMessageComposer::Inline => CommitMessageComposer::Editor,
                    };
                }
            }
        }
    }

    fn handle_move_start(&mut self) {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };

        let move_mode = if let Some(marks) = self.marks()
            && !marks.is_empty()
        {
            let MarkClasses {
                marked_commits,
                marked_uncommitted,
            } = marks.classify();
            if !marked_commits || marked_uncommitted {
                return;
            }
            MoveMode {
                source: Arc::new(MoveSource::Marks(marks.clone())),
                insert_side: InsertSide::Above,
            }
        } else {
            match &selection.data {
                StatusOutputLineData::Branch { cli_id }
                | StatusOutputLineData::Commit { cli_id, .. } => {
                    let Ok(source) = MoveSource::try_from(Arc::unwrap_or_clone(Arc::clone(cli_id)))
                    else {
                        return;
                    };
                    MoveMode {
                        source: Arc::new(source),
                        insert_side: InsertSide::Above,
                    }
                }
                StatusOutputLineData::UpdateNotice
                | StatusOutputLineData::Connector
                | StatusOutputLineData::BetweenStacks
                | StatusOutputLineData::StagedChanges { .. }
                | StatusOutputLineData::StagedFile { .. }
                | StatusOutputLineData::UnassignedChanges { .. }
                | StatusOutputLineData::UnassignedFile { .. }
                | StatusOutputLineData::CommitMessage
                | StatusOutputLineData::EmptyCommitMessage
                | StatusOutputLineData::File { .. }
                | StatusOutputLineData::MergeBase
                | StatusOutputLineData::UpstreamChanges
                | StatusOutputLineData::Warning
                | StatusOutputLineData::Hint
                | StatusOutputLineData::NoAssignmentsUnstaged => return,
            }
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Move(move_mode);
            });
    }

    fn handle_move_toggle_insert_side(&mut self) {
        let Mode::Move(move_mode) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };
        move_mode.insert_side = match move_mode.insert_side {
            InsertSide::Above => InsertSide::Below,
            InsertSide::Below => InsertSide::Above,
        };
    }

    fn handle_move_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Move(MoveMode {
            source,
            insert_side,
        }) = &*self.mode
        else {
            return Ok(());
        };

        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        if selection
            .data
            .cli_id()
            .is_some_and(|target| source.contains(target))
        {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        let target = match &selection.data {
            StatusOutputLineData::Branch { cli_id } => {
                if let CliId::Branch { name, .. } = &**cli_id {
                    MoveTarget::Branch { name }
                } else {
                    return Ok(());
                }
            }
            StatusOutputLineData::Commit { cli_id, .. } => {
                if let CliId::Commit { commit_id, .. } = &**cli_id {
                    MoveTarget::Commit {
                        commit_id: *commit_id,
                    }
                } else {
                    return Ok(());
                }
            }
            StatusOutputLineData::MergeBase => MoveTarget::MergeBase,
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UnassignedChanges { .. }
            | StatusOutputLineData::UnassignedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => {
                return Ok(());
            }
        };

        let selection_after_reload = match &**source {
            MoveSource::Commit {
                commit_id: source_commit_id,
                ..
            } => {
                let commit_move_result = match target {
                    MoveTarget::Branch { name } => operations::move_commit_to_branch(
                        ctx,
                        Vec::from([*source_commit_id]),
                        name,
                    )?,
                    MoveTarget::Commit {
                        commit_id: target_commit_id,
                    } => operations::move_commit_to_commit(
                        ctx,
                        Vec::from([*source_commit_id]),
                        target_commit_id,
                        *insert_side,
                    )?,
                    MoveTarget::MergeBase => return Ok(()),
                };

                commit_move_result
                    .workspace
                    .replaced_commits
                    .get(source_commit_id)
                    .copied()
                    .map(SelectAfterReload::Commit)
            }
            MoveSource::Marks(marks) => {
                let Some(sources) = marks
                    .iter()
                    .map(|mark| match mark {
                        Markable::Commit { commit_id, .. } => Some(*commit_id),
                        Markable::Uncommitted(..) => None,
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return Ok(());
                };

                let commit_move_result = match target {
                    MoveTarget::Branch { name } => {
                        operations::move_commit_to_branch(ctx, sources.clone(), name)?
                    }
                    MoveTarget::Commit {
                        commit_id: target_commit_id,
                    } => operations::move_commit_to_commit(
                        ctx,
                        sources.clone(),
                        target_commit_id,
                        *insert_side,
                    )?,
                    MoveTarget::MergeBase => return Ok(()),
                };

                sources
                    .iter()
                    .find_map(|source| {
                        commit_move_result
                            .workspace
                            .replaced_commits
                            .get(source)
                            .copied()
                    })
                    .map(SelectAfterReload::Commit)
            }
            MoveSource::Branch {
                name: source_branch_name,
                ..
            } => match target {
                MoveTarget::Branch {
                    name: target_branch_name,
                } => {
                    operations::move_branch_onto_branch(
                        ctx,
                        source_branch_name,
                        target_branch_name,
                    )?;
                    Some(SelectAfterReload::Branch(source_branch_name.to_owned()))
                }
                MoveTarget::MergeBase => {
                    operations::tear_off_branch(ctx, source_branch_name)?;
                    Some(SelectAfterReload::Branch(source_branch_name.to_owned()))
                }
                MoveTarget::Commit { .. } => return Ok(()),
            },
        };

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(selection_after_reload, ReloadCause::Mutation),
        ]);

        Ok(())
    }

    fn handle_new_branch(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let new_name = match &selection.data {
            StatusOutputLineData::Branch { cli_id } => {
                let CliId::Branch { name, .. } = &**cli_id else {
                    return Ok(());
                };
                operations::create_branch_anchored_legacy(ctx, name.to_owned())?
            }
            StatusOutputLineData::UnassignedChanges { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UnassignedFile { .. } => operations::create_branch_legacy(ctx)?,
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        messages.push(Message::Reload(
            Some(SelectAfterReload::Branch(new_name)),
            ReloadCause::Mutation,
        ));

        Ok(())
    }

    fn handle_copy_selection(&mut self) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };
        let Some(cli_id) = selection.data.cli_id() else {
            return Ok(());
        };

        let what_to_copy = match &**cli_id {
            CliId::Branch { name, .. } => Cow::Borrowed(&**name),
            CliId::Commit { commit_id, .. } => Cow::Owned(commit_id.to_hex_with_len(7).to_string()),
            CliId::CommittedFile { path, .. } => path.to_str_lossy(),
            CliId::Uncommitted(uncommitted) => {
                Cow::Borrowed(&*uncommitted.hunk_assignments.first().path)
            }
            CliId::PathPrefix { .. } | CliId::Unassigned { .. } | CliId::Stack { .. } => {
                return Ok(());
            }
        };

        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(what_to_copy))
            .context("failed to copy to system clipboard")?;

        self.highlight
            .insert(Arc::unwrap_or_clone(Arc::clone(cli_id)));

        Ok(())
    }

    /// Handles opening the full-screen commit reword editor for the selected commit.
    fn handle_reword_with_editor<T>(
        &mut self,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Some(commit_id) = self.selected_commit_id() else {
            return Ok(());
        };

        let _suspend_guard = terminal_guard.suspend()?;

        let Some(reword_result) = operations::reword_commit_with_editor_legacy(ctx, commit_id)?
        else {
            return Ok(());
        };

        messages.push(Message::Reload(
            Some(SelectAfterReload::Commit(reword_result.new_commit)),
            ReloadCause::Mutation,
        ));

        Ok(())
    }

    fn handle_reword_inline_start(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };
        let Some(cli_id) = selection.data.cli_id() else {
            return Ok(());
        };

        let inline_reword_mode = match &**cli_id {
            CliId::Branch { name, stack_id, .. } => {
                let Some(stack_id) = stack_id else {
                    return Ok(());
                };
                let mut textarea = TextArea::from([name]);
                textarea.set_cursor_line_style(self.theme.local_branch);
                textarea.move_cursor(CursorMove::End);

                InlineRewordMode::Branch {
                    name: name.to_owned(),
                    stack_id: *stack_id,
                    textarea: Box::new(textarea),
                }
            }
            CliId::Commit { commit_id, .. } => {
                let current_message = operations::current_commit_message(ctx, *commit_id)?;

                if operations::commit_message_has_multiple_lines_legacy(&current_message) {
                    messages.push(Message::Reword(RewordMessage::WithEditor));
                    return Ok(());
                }

                let first_line = current_message.lines().next().unwrap_or("").to_string();
                let mut textarea = TextArea::from([first_line]);
                textarea.set_cursor_line_style(self.theme.default);
                textarea.move_cursor(CursorMove::End);

                InlineRewordMode::Commit {
                    commit_id: *commit_id,
                    textarea: Box::new(textarea),
                }
            }
            CliId::Uncommitted(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Unassigned { .. }
            | CliId::Stack { .. } => return Ok(()),
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::InlineReword(inline_reword_mode);
            });

        Ok(())
    }

    /// Handles key input while inline reword mode is active.
    fn handle_reword_inline_input(&mut self, ev: Event) {
        if let Mode::InlineReword(inline_reword_mode) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        {
            let ev = match inline_reword_mode {
                InlineRewordMode::Branch { .. } => {
                    if let Event::Key(key_ev) = ev
                        && key_ev.is_press()
                        && key_ev.modifiers == event::KeyModifiers::NONE
                        && let KeyCode::Char(' ') = key_ev.code
                    {
                        Event::Key(KeyEvent {
                            code: KeyCode::Char('-'),
                            modifiers: key_ev.modifiers,
                            kind: key_ev.kind,
                            state: key_ev.state,
                        })
                    } else {
                        ev
                    }
                }
                InlineRewordMode::Commit { .. } => ev,
            };

            inline_reword_mode.textarea_mut().input(ev);
        }
    }

    fn handle_reword_inline_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let inline_reword_mode = if let Mode::InlineReword(inline_reword_mode) = &*self.mode {
            inline_reword_mode
        } else {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        };

        let first_line = inline_reword_mode
            .textarea()
            .lines()
            .first()
            .map(std::string::String::as_str)
            .unwrap_or("");

        match inline_reword_mode {
            InlineRewordMode::Commit { commit_id, .. } => {
                let Some(reword_result) =
                    operations::reword_commit_legacy(ctx, *commit_id, first_line)?
                else {
                    messages.push(Message::EnterNormalModeAfterConfirmingOperation);
                    return Ok(());
                };

                messages.extend([
                    Message::EnterNormalModeAfterConfirmingOperation,
                    Message::Reload(
                        Some(SelectAfterReload::Commit(reword_result.new_commit)),
                        ReloadCause::Mutation,
                    ),
                ]);
            }
            InlineRewordMode::Branch { name, stack_id, .. } => {
                let new_name = operations::reword_branch_legacy(
                    ctx,
                    *stack_id,
                    name.to_owned(),
                    first_line.to_owned(),
                )?;

                messages.extend([
                    Message::EnterNormalModeAfterConfirmingOperation,
                    Message::Reload(
                        Some(SelectAfterReload::Branch(new_name)),
                        ReloadCause::Mutation,
                    ),
                ]);
            }
        }

        Ok(())
    }

    fn handle_reword_open_editor<T>(
        &mut self,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Mode::InlineReword(inline_reword_mode) = &*self.mode else {
            return Ok(());
        };

        let textarea = inline_reword_mode.textarea();
        let Some(line) = textarea.lines().first() else {
            return Ok(());
        };

        let _suspend_guard = terminal_guard.suspend()?;
        let what_to_select = match inline_reword_mode {
            InlineRewordMode::Commit { commit_id, .. } => {
                let commit_details =
                    but_api::diff::commit_details(ctx, *commit_id, ComputeLineStats::No)?;
                if let Some(reword_result) =
                    operations::reword_commit_with_editor_with_message_legacy(
                        ctx,
                        commit_details,
                        line.to_owned(),
                    )?
                {
                    SelectAfterReload::Commit(reword_result.new_commit)
                } else {
                    SelectAfterReload::Commit(*commit_id)
                }
            }
            InlineRewordMode::Branch { name, stack_id, .. } => {
                let new_name = get_branch_name_from_editor(line)?;
                let normalized_name =
                    operations::reword_branch_legacy(ctx, *stack_id, name.clone(), new_name)?;
                SelectAfterReload::Branch(normalized_name)
            }
        };
        drop(_suspend_guard);

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(Some(what_to_select), ReloadCause::Mutation),
        ]);

        Ok(())
    }

    fn handle_command_start(&mut self, kind: CommandModeKind) {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(self.theme.default);
        textarea.move_cursor(CursorMove::End);

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Command(CommandMode {
                    textarea: Box::new(textarea),
                    kind,
                });
            });
    }

    fn handle_command_input(&mut self, ev: Event) {
        if let Mode::Command(CommandMode { textarea, .. }) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        {
            textarea.input(ev);
        }
    }

    fn handle_command_confirm<T>(
        &mut self,
        terminal_guard: &mut T,
        out: &mut dyn TuiInputOutputChannel,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        //
        // `cfg!(test)` is false for integration tests but we currently don't have integration
        // tests of the TUI so thats fine for now.
        const IN_TEST: bool = cfg!(test);

        let Mode::Command(CommandMode { textarea, kind }) = &*self.mode else {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        };

        let Some(input) = textarea.lines().first() else {
            return Ok(());
        };

        let _suspend_guard = terminal_guard.suspend()?;

        let mut cmd = match kind {
            CommandModeKind::But => {
                let binary_path = current_exe_for_but_exec()?;
                let args = shell_words::split(input)?.into_iter().map(OsString::from);
                let mut cmd = Command::new(binary_path);
                cmd.args(args);
                cmd
            }
            CommandModeKind::Shell => {
                let mut args = shell_words::split(input)?.into_iter().map(OsString::from);
                let Some(binary) = args.next() else {
                    messages.push(Message::EnterNormalModeAfterConfirmingOperation);
                    return Ok(());
                };
                let mut cmd = Command::new(binary);
                cmd.args(args);
                cmd
            }
        };

        let status = cmd.spawn()?.wait()?;

        if !IN_TEST {
            out.prompt_single_line("\npress enter to continue...")?;
        }

        if status.success() {
            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(None, ReloadCause::Mutation),
            ]);
        } else {
            self.push_transient_error(anyhow::Error::msg(format!(
                "command exited with status {}",
                format_exit_status(status)
            )));
        }

        drop(_suspend_guard);

        Ok(())
    }

    /// Adds a transient error toast message that auto-dismisses after a short duration.
    fn push_transient_error(&mut self, err: anyhow::Error) {
        self.toasts
            .insert(ToastKind::Error, format_error_for_tui(&err));
    }

    /// Returns the currently selected commit id when the selected line is a commit.
    fn selected_commit_id(&self) -> Option<gix::ObjectId> {
        let selection = self.cursor.selected_line(&self.status_lines)?;

        let StatusOutputLineData::Commit { cli_id, .. } = &selection.data else {
            return None;
        };

        let CliId::Commit { commit_id, .. } = &**cli_id else {
            return None;
        };

        Some(*commit_id)
    }

    fn update_status_width_percentage(&mut self, new: u16, terminal_area: Rect) {
        if !self.is_details_visible {
            return;
        }

        self.status_width_percentage = new.clamp(
            100 - DETAILS_MAX_SIZE_PERCENTAGE,
            100 - DETAILS_MIN_SIZE_PERCENTAGE,
        );

        let details_viewport = details_viewport(self, terminal_area);
        self.details.ensure_selection_visible(details_viewport);
    }

    fn handle_pick_and_goto_branch(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        match self.flags.show_files {
            FilesStatusFlag::None | FilesStatusFlag::All => {}
            FilesStatusFlag::Commit(_) => return Ok(()),
        }

        let head_info = {
            let meta = ctx.meta()?;
            but_workspace::head_info(
                &*ctx.repo.get()?,
                &meta,
                but_workspace::ref_info::Options {
                    project_meta: ctx.project_meta()?,
                    ..Default::default()
                },
            )?
        };

        let branch_names = head_info
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .filter_map(|segment| {
                let ref_info = segment.ref_info.as_ref()?;
                Some(&ref_info.ref_name)
            })
            .filter(|name| {
                if matches!(&*self.mode, Mode::Rub(_)) {
                    true
                } else {
                    // not all branches are selectable all the time, for example if we're committing
                    // changes assigned to a stack then we cannot select branches outside the stack
                    self.status_lines
                        .iter()
                        .find(|line| {
                            if let Some(id) = line.data.cli_id()
                                && let CliId::Branch {
                                    name: name_on_line, ..
                                } = &**id
                                && name_on_line == name.shorten()
                            {
                                true
                            } else {
                                false
                            }
                        })
                        .is_none_or(|line| {
                            is_selectable_in_mode(line, &self.mode, self.flags.show_files)
                        })
                }
            })
            .map(|name| name.to_owned())
            .collect::<Vec<_>>();

        if let Some(branch_names) = NonEmpty::from_vec(branch_names) {
            let include_unassigned = Cursor::select_unassigned(&self.status_lines)
                .and_then(|cursor| cursor.selected_line(&self.status_lines))
                .is_some_and(|unassigned| {
                    is_selectable_in_mode(unassigned, &self.mode, self.flags.show_files)
                });

            let picker_items = if include_unassigned {
                let mut mapped_items = NonEmpty::new(GotoBranchItem::Unassigned);
                mapped_items.extend(branch_names.map(GotoBranchItem::Branch));
                mapped_items
            } else {
                branch_names.map(GotoBranchItem::Branch)
            };

            self.modal = Some(Modal::GotoBranchPicker {
                picker: Box::new(FuzzyPicker::new(
                    picker_items,
                    self.theme,
                    |item, _ctx, messages| {
                        match item {
                            GotoBranchItem::Branch(branch_name) => {
                                messages.push(Message::SelectBranch(branch_name));
                            }
                            GotoBranchItem::Unassigned => {
                                messages.push(Message::SelectUnassigned);
                            }
                        }
                        Ok(())
                    },
                )),
                key_binds: fuzzy_picker_key_binds(),
            });
        }

        Ok(())
    }

    fn handle_toggle_help(&mut self) {
        if matches!(self.modal, Some(Modal::Help { .. })) {
            self.modal = None;
        } else {
            self.modal = Some(Modal::Help {
                help: Help::new([&self.app_key_binds.key_binds], self.theme),
                key_binds: help_key_binds(),
            });
        }
    }

    fn handle_mark(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|selection| selection.data.cli_id())
        else {
            return Ok(());
        };

        match &**selection {
            CliId::Commit { .. } | CliId::Uncommitted(..) => {
                if handle_mark_cli_id(
                    selection,
                    self.mode
                        .get_mut_without_updating_backstack_and_i_promise_not_to_change_state(),
                ) && let Some(new_cursor) = self.cursor.move_down_within_section(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                }
            }
            CliId::Branch {
                name,
                id: _,
                stack_id,
            } => {
                // you cannot select branches in rub mode so we don't need to care about that
                if let Some(stack_id) = *stack_id {
                    match self
                        .mode
                        .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
                    {
                        Mode::Normal(NormalMode { marks })
                        | Mode::PickChanges(PickUncommittedMode { marks }) => {
                            handle_mark_branch(marks, ctx, stack_id, name)?;
                        }
                        Mode::Rub(..)
                        | Mode::InlineReword(..)
                        | Mode::Command(..)
                        | Mode::Commit(..)
                        | Mode::Move(..)
                        | Mode::Details(..)
                        | Mode::MoveStack(..)
                        | Mode::Stack(..) => {}
                    }
                }
            }
            CliId::Unassigned { .. } => {
                // you cannot select unassigned changes in rub mode so we don't need to care about that
                match self
                    .mode
                    .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
                {
                    Mode::Normal(NormalMode { marks })
                    | Mode::PickChanges(PickUncommittedMode { marks }) => {
                        handle_mark_unassigned(marks, &self.status_lines);
                    }
                    Mode::Rub(..)
                    | Mode::InlineReword(..)
                    | Mode::Command(..)
                    | Mode::Commit(..)
                    | Mode::Move(..)
                    | Mode::Details(..)
                    | Mode::MoveStack(..)
                    | Mode::Stack(..) => {}
                }
            }
            CliId::PathPrefix { .. } | CliId::CommittedFile { .. } | CliId::Stack { .. } => {}
        }

        if let Some(marks) = self.marks() {
            if marks.is_empty() {
                self.backstack.remove_mark();
            } else {
                self.backstack.push_mark();
            }
        }

        Ok(())
    }

    fn handle_clear_normal_mode_marks(&mut self) {
        let Mode::Normal(normal_mode) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };

        normal_mode.marks.clear();
        self.backstack.remove_mark();
    }

    fn marks(&self) -> Option<&Marks> {
        self.mode.marks()
    }

    fn handle_undo(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        self.restore_to_target_snapshot(UndoOrRedo::Undo, ctx)
    }

    fn handle_redo(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        self.restore_to_target_snapshot(UndoOrRedo::Redo, ctx)
    }

    fn selected_stack_id(&self) -> Option<StackId> {
        let selected_line = self.cursor.selected_line(&self.status_lines)?;
        stack_id_for_line(selected_line, &self.status_lines)
    }

    fn selected_line_uses_top_stack_for_stack_mode(&self) -> bool {
        self.cursor
            .selected_line(&self.status_lines)
            .is_some_and(line_uses_top_stack_for_stack_mode)
    }

    fn handle_stack_enter(&mut self, ctx: &Context) -> anyhow::Result<()> {
        match self.flags.show_files {
            FilesStatusFlag::Commit(..) => return Ok(()),
            FilesStatusFlag::None | FilesStatusFlag::All => {}
        }

        let head_info = but_api::legacy::workspace::head_info(ctx)?;

        let stack_heads = head_info
            .stacks
            .iter()
            .filter_map(|stack| stack.ref_name().cloned())
            .collect::<Vec<_>>();

        let Some(top_stack_head) = stack_heads.first().cloned() else {
            self.mode
                .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                    *mode = Mode::Stack(StackMode {
                        stack_heads: Default::default(),
                    });
                });
            return Ok(());
        };

        let selected_stack_head = if self.selected_line_uses_top_stack_for_stack_mode() {
            Some(top_stack_head)
        } else {
            self.selected_stack_id().and_then(|selected_stack_id| {
                head_info
                    .stacks
                    .iter()
                    .find(|stack| stack.id == Some(selected_stack_id))
                    .and_then(|stack| stack.ref_name().cloned())
            })
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Stack(StackMode { stack_heads });
            });

        if let Some(selected_stack_head) = selected_stack_head {
            let branch_name = selected_stack_head.shorten().to_str_lossy();
            if let Some(cursor) = Cursor::select_branch(&branch_name, &self.status_lines) {
                self.cursor = cursor;
            }
        }

        Ok(())
    }

    fn handle_stack_show_apply_picker(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let branch_listings = but_api::legacy::virtual_branches::list_branches(
            ctx,
            Some(BranchListingFilter {
                local: None,
                applied: Some(false),
            }),
        )
        .context("Failed to list branches available to apply")?
        .into_iter();

        let now = SystemTime::now();
        let mut branches = branch_listings
            .map(|listing| ApplyBranchItem {
                name: listing.name.0.to_string(),
                has_local: listing.has_local,
                updated_at: listing.updated_at,
                updated_at_display: format_relative_time(now, (listing.updated_at / 1000) as i64),
                last_commiter: listing
                    .last_commiter
                    .name
                    .map(|name| name.to_string())
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();

        branches.sort_by(|a, b| {
            b.has_local
                .cmp(&a.has_local)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
                .then_with(|| a.name.cmp(&b.name))
        });

        let Some(items) = NonEmpty::from_vec(branches) else {
            return Ok(());
        };
        let picker = FuzzyPicker::new(items, self.theme, |item, ctx, messages| {
            let reference = {
                let repo = ctx.repo.get()?;
                let name = resolve_legacy_top_level_apply_branch_name(&repo, &item.name)
                    .with_context(|| format!("Failed to resolve branch '{}'", item.name))?;
                repo.find_reference(&name)
                    .with_context(|| format!("Failed to find branch '{name}'"))?
                    .detach()
            };

            let outcome = but_api::branch::apply(ctx, reference.name.as_ref())
                .with_context(|| format!("Failed to apply '{}'", reference.name.shorten()))?;

            if !outcome.conflicting_stacks.is_empty() {
                let conflicting_stack_names = outcome
                    .conflicting_stacks
                    .iter()
                    .map(|stack| stack.ref_name.shorten().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!(
                    "'{}' conflicts with existing stack in the workspace: {conflicting_stack_names}",
                    reference.name.shorten(),
                );
            }

            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(
                    Some(SelectAfterReload::Branch(item.name)),
                    ReloadCause::Mutation,
                ),
            ]);

            Ok(())
        });
        self.modal = Some(Modal::ApplyStackPicker {
            picker: Box::new(picker),
            key_binds: fuzzy_picker_key_binds(),
        });

        Ok(())
    }

    fn handle_stack_unapply(&mut self) {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };
        let Some(selection) = selection.data.cli_id() else {
            return;
        };

        let (stack_id, name) = match &**selection {
            CliId::Branch {
                stack_id: Some(stack_id),
                name,
                ..
            } => (*stack_id, name),
            CliId::Branch { .. }
            | CliId::Uncommitted(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Commit { .. }
            | CliId::Unassigned { .. }
            | CliId::Stack { .. } => return,
        };

        self.modal = Some(Modal::Confirm {
            confirm: Confirm::new(
                NonEmpty::new(format!("Unapply '{name}'?").into()),
                self.theme,
                move |ctx, messages| {
                    but_api::legacy::virtual_branches::unapply_stack(ctx, stack_id)?;
                    messages.extend([
                        Message::EnterNormalModeAfterConfirmingOperation,
                        Message::Reload(None, ReloadCause::Mutation),
                    ]);
                    Ok(())
                },
            ),
            key_binds: confirm_key_binds(),
        });
    }

    fn handle_stack_move_start(&mut self) {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };
        let Some(CliId::Branch {
            name,
            stack_id: Some(stack),
            ..
        }) = selection.data.cli_id().map(|id| &**id)
        else {
            return;
        };
        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                let source = ReorderStackSource {
                    stack: *stack,
                    branch: name.to_owned(),
                };
                *mode = Mode::MoveStack(MoveStackMode { source });
            });
    }

    fn handle_stack_move_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::MoveStack(MoveStackMode { source }) = &*self.mode else {
            return Ok(());
        };

        let selection_index = self.cursor.index();
        let Some(selection) = self.status_lines.get(selection_index) else {
            return Ok(());
        };

        if selection
            .data
            .cli_id()
            .is_some_and(|target| source.matches(target))
        {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        if !matches!(selection.data, StatusOutputLineData::BetweenStacks) {
            return Ok(());
        }

        let current_stack_order = stack_ids_in_display_order(&self.status_lines);
        let Some(source_index) = current_stack_order
            .iter()
            .position(|stack| *stack == source.stack)
        else {
            return Ok(());
        };

        let target_index = stack_ids_in_display_order(&self.status_lines[..selection_index]).len();
        let mut new_stack_order = current_stack_order.clone();
        let source_stack = new_stack_order.remove(source_index);
        let insert_index = if target_index > source_index {
            target_index - 1
        } else {
            target_index
        };
        new_stack_order.insert(insert_index.min(new_stack_order.len()), source_stack);

        if new_stack_order == current_stack_order {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        let updates = new_stack_order
            .into_iter()
            .enumerate()
            .map(|(order, stack)| gitbutler_branch::BranchUpdateRequest {
                id: Some(stack),
                order: Some(order),
            })
            .collect();

        but_api::legacy::virtual_branches::update_stack_order(ctx, updates)?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Branch(source.branch.clone())),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }

    fn restore_to_target_snapshot(
        &mut self,
        kind: UndoOrRedo,
        ctx: &mut Context,
    ) -> anyhow::Result<()> {
        let target_snapshot = match kind {
            UndoOrRedo::Undo => operations::get_undo_target_snapshot_legacy(ctx)?,
            UndoOrRedo::Redo => operations::get_redo_target_snapshot_legacy(ctx)?,
        };
        let Some(target_snapshot) = target_snapshot else {
            return Ok(());
        };

        let text = {
            let restore_from = if let Ok(Some(snapshot)) =
                operations::peel_restore_snapshot_legacy(ctx, target_snapshot.commit_id)
                && snapshot.commit_id != target_snapshot.commit_id
                && snapshot.details.is_some()
            {
                Cow::Owned(snapshot)
            } else {
                Cow::Borrowed(&target_snapshot)
            };

            let time = restore_from
                .created_at
                .format_or_unix(gix::date::time::format::DEFAULT);

            let commit = restore_from.commit_id.to_hex_with_len(7);

            Line::from_iter(
                [
                    Span::raw(match kind {
                        UndoOrRedo::Undo => "Undo ",
                        UndoOrRedo::Redo => "Redo ",
                    }),
                    Span::raw(commit.to_string()).style(self.theme.cli_id),
                ]
                .into_iter()
                .chain([Span::raw(" "), Span::raw(time).style(self.theme.time)])
                .chain(restore_from.details.iter().flat_map(|details| {
                    [
                        Span::raw(" "),
                        Span::raw(details.operation.title()).style(self.theme.attention),
                    ]
                }))
                .chain([Span::raw("?")]),
            )
        };

        let commit = target_snapshot.commit_id;
        self.modal = Some(Modal::Confirm {
            confirm: Confirm::new(NonEmpty::new(text), self.theme, move |ctx, messages| {
                operations::restore_snapshot_with_kind_legacy(
                    ctx,
                    match kind {
                        UndoOrRedo::Undo => RestoreKind::RestoreFromSnapshotViaUndo,
                        UndoOrRedo::Redo => RestoreKind::RestoreFromSnapshotViaRedo,
                    },
                    commit,
                )?;
                messages.push(Message::Reload(None, ReloadCause::Mutation));
                Ok(())
            }),
            key_binds: confirm_key_binds(),
        });

        Ok(())
    }
}

#[derive(Copy, Clone)]
enum UndoOrRedo {
    Undo,
    Redo,
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

fn handle_mark_cli_id(commit: &CliId, mode: &mut Mode) -> bool {
    let Some(markable) = Markable::try_from_cli_id(commit) else {
        return false;
    };

    match mode {
        Mode::Normal(normal_mode) => {
            normal_mode.marks.toggle(markable);
        }
        Mode::PickChanges(pick_uncommitted_mode) => {
            pick_uncommitted_mode.marks.toggle(markable);
        }
        Mode::Rub(rub_mode) => {
            match &mut rub_mode.source {
                RubSource::CliId(cli_id) => {
                    match &**cli_id {
                        CliId::Commit { .. } => {
                            // we only support rubbing commits, meaning the source
                            // also most be a commit
                            let mut marks = Marks::default();
                            if let Some(previous_source) = Markable::try_from_cli_id(cli_id)
                                && markable != previous_source
                            {
                                marks.toggle(previous_source);
                            }
                            marks.toggle(markable);
                            rub_mode.source = RubSource::Marks(marks);
                        }
                        CliId::Uncommitted(..)
                        | CliId::PathPrefix { .. }
                        | CliId::CommittedFile { .. }
                        | CliId::Branch { .. }
                        | CliId::Unassigned { .. }
                        | CliId::Stack { .. } => return false,
                    }
                }
                RubSource::CommittedHunk(..) => return false,
                RubSource::Marks(marks) => {
                    marks.toggle(markable.clone());

                    match marks.len() {
                        0 => {
                            rub_mode.source = RubSource::CliId(Arc::new(markable.into_cli_id()));
                        }
                        1 => {
                            let only_remaining_mark = marks.iter().next().cloned();
                            if let Some(mark) = only_remaining_mark {
                                rub_mode.source = RubSource::CliId(Arc::new(mark.into_cli_id()));
                            }
                        }
                        _ => {
                            //
                        }
                    }
                }
            }
        }
        Mode::InlineReword(..)
        | Mode::Command(..)
        | Mode::Commit(..)
        | Mode::Move(..)
        | Mode::Stack(..)
        | Mode::MoveStack(..)
        | Mode::Details(..) => {
            return false;
        }
    }

    true
}

fn handle_mark_branch(
    marks: &mut Marks,
    ctx: &Context,
    stack_id: StackId,
    name: &str,
) -> anyhow::Result<()> {
    let Some(commits) = commits_on_branch(ctx, stack_id, name)?
        .into_iter()
        .map(|(commit_id, short_id)| {
            Markable::try_from_cli_id(&CliId::Commit {
                commit_id,
                id: short_id,
            })
        })
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(());
    };

    toggle_markables(marks, commits);

    Ok(())
}

fn line_uses_top_stack_for_stack_mode(line: &StatusOutputLine) -> bool {
    match &line.data {
        StatusOutputLineData::UnassignedChanges { .. } => true,
        StatusOutputLineData::UnassignedFile { cli_id }
        | StatusOutputLineData::StagedFile { cli_id }
        | StatusOutputLineData::File { cli_id } => {
            matches!(&**cli_id, CliId::Uncommitted(..) | CliId::PathPrefix { .. })
        }
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::Branch { .. }
        | StatusOutputLineData::Commit { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::MergeBase
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => false,
    }
}

fn stack_ids_in_display_order(status_lines: &[StatusOutputLine]) -> Vec<StackId> {
    let mut stack_ids = Vec::new();
    for line in status_lines {
        if let StatusOutputLineData::Branch { cli_id } = &line.data
            && let CliId::Branch {
                stack_id: Some(stack_id),
                ..
            } = &**cli_id
            && !stack_ids.contains(stack_id)
        {
            stack_ids.push(*stack_id);
        }
    }
    stack_ids
}

fn stack_id_for_line(
    line: &StatusOutputLine,
    status_lines: &[StatusOutputLine],
) -> Option<StackId> {
    match &line.data {
        StatusOutputLineData::Branch { cli_id }
        | StatusOutputLineData::StagedChanges { cli_id }
        | StatusOutputLineData::StagedFile { cli_id }
        | StatusOutputLineData::UnassignedFile { cli_id }
        | StatusOutputLineData::File { cli_id } => stack_id_for_cli_id(cli_id, status_lines),
        StatusOutputLineData::Commit { stack_id, .. } => *stack_id,
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::UnassignedChanges { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::MergeBase
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    }
}

fn stack_id_for_cli_id(cli_id: &CliId, status_lines: &[StatusOutputLine]) -> Option<StackId> {
    match cli_id {
        CliId::Uncommitted(uncommitted) => uncommitted.hunk_assignments.first().stack_id,
        CliId::PathPrefix {
            hunk_assignments, ..
        } => hunk_assignments.first().1.stack_id,
        CliId::CommittedFile { commit_id, .. } | CliId::Commit { commit_id, .. } => {
            status_lines.iter().find_map(|line| match &line.data {
                StatusOutputLineData::Commit {
                    cli_id, stack_id, ..
                } => match &**cli_id {
                    CliId::Commit {
                        commit_id: line_commit_id,
                        ..
                    } if line_commit_id == commit_id => *stack_id,
                    CliId::Uncommitted(..)
                    | CliId::PathPrefix { .. }
                    | CliId::CommittedFile { .. }
                    | CliId::Branch { .. }
                    | CliId::Commit { .. }
                    | CliId::Unassigned { .. }
                    | CliId::Stack { .. } => None,
                },
                StatusOutputLineData::UpdateNotice
                | StatusOutputLineData::Connector
                | StatusOutputLineData::BetweenStacks
                | StatusOutputLineData::StagedChanges { .. }
                | StatusOutputLineData::StagedFile { .. }
                | StatusOutputLineData::UnassignedChanges { .. }
                | StatusOutputLineData::UnassignedFile { .. }
                | StatusOutputLineData::Branch { .. }
                | StatusOutputLineData::CommitMessage
                | StatusOutputLineData::EmptyCommitMessage
                | StatusOutputLineData::File { .. }
                | StatusOutputLineData::MergeBase
                | StatusOutputLineData::UpstreamChanges
                | StatusOutputLineData::Warning
                | StatusOutputLineData::Hint
                | StatusOutputLineData::NoAssignmentsUnstaged => None,
            })
        }
        CliId::Branch { stack_id, .. } => *stack_id,
        CliId::Stack { stack_id, .. } => Some(*stack_id),
        CliId::Unassigned { .. } => None,
    }
}

fn handle_mark_unassigned(marks: &mut Marks, status_lines: &[StatusOutputLine]) {
    let unassigned_files = status_lines.iter().filter_map(|line| match &line.data {
        StatusOutputLineData::UnassignedFile { cli_id } => Markable::try_from_cli_id(cli_id),
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UnassignedChanges { .. }
        | StatusOutputLineData::Branch { .. }
        | StatusOutputLineData::Commit { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::MergeBase
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    });

    toggle_markables(marks, unassigned_files);
}

fn toggle_markables(marks: &mut Marks, markables: impl IntoIterator<Item = Markable>) {
    let (marked, unmarked) = markables
        .into_iter()
        .partition::<Vec<_>, _>(|markable| marks.contains(markable));

    match (marked.is_empty(), unmarked.is_empty()) {
        (true, false) => {
            for markable in unmarked {
                marks.insert(markable);
            }
        }
        (false, true) => {
            for markable in marked {
                marks.remove(&markable);
            }
        }
        _ => {
            for markable in unmarked {
                marks.insert(markable);
            }
        }
    }
}

fn commits_on_branch(
    ctx: &Context,
    stack_id: StackId,
    name: &str,
) -> anyhow::Result<Vec<(gix::ObjectId, String)>> {
    let guard = ctx.shared_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    let segment = id_map
        .stacks()
        .iter()
        .filter(|stack| stack.id.is_some_and(|id| id == stack_id))
        .flat_map(|stack| &stack.segments)
        .find(|segment| {
            segment
                .branch_name()
                .is_some_and(|branch_name| branch_name == name)
        })
        .context("segment not found")?;

    let commits = segment
        .workspace_commits
        .iter()
        .map(|commit| (commit.commit_id(), commit.short_id.clone()))
        .collect::<Vec<_>>();

    Ok(commits)
}

#[derive(Debug, Clone, strum::EnumDiscriminants)]
#[strum_discriminants(name(MessageDiscriminant))]
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
        text: String,
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
    SelectUnassigned,
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
    NewBranch,
    ToggleHelp,
    Mark,
    ClearNormalModeMarks,
    Undo,
    Redo,

    // Utilities
    CopySelection,
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

impl Message {
    /// Delay a message so it wont be handled until the next frame.
    pub(super) fn with_one_frame_delay(self) -> Self {
        Self::WithOneFrameDelay(Box::new(self))
    }

    /// Send another message only if handling the first succeeds.
    pub(super) fn and_then(self, other: Self) -> Self {
        Self::AndThen {
            lhs: Box::new(self),
            rhs: Box::new(other),
        }
    }
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
enum RubMessage {
    Start,
    StartWithSource {
        source: RubSource,
        unlock_details: Option<MessageOnDrop>,
    },
    StartReverse,
    UseTargetMessage,
    UseSourceMessage,
    Confirm,
}

#[derive(Debug, Clone)]
enum RewordMessage {
    WithEditor,
    OpenEditor,
    InlineStart,
    InlineInput(Event),
    InlineConfirm,
}

#[derive(Debug, Clone)]
enum CommandMessage {
    Start(CommandModeKind),
    Input(Event),
    Confirm,
}

#[derive(Debug, Clone)]
enum CommitMessage {
    CreateEmpty,
    Start,
    ToggleMessageComposer(CommitMessageComposer),
    Confirm,
    CommitToNewBranch,
    ToggleInsertSide,
}

#[derive(Debug, Clone)]
enum MoveMessage {
    Start,
    ToggleInsertSide,
    Confirm,
}

#[derive(Debug, Clone)]
enum StackMessage {
    Enter,
    ShowApplyPicker,
    Unapply,
    MoveStart,
    MoveConfirm,
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
    Unassigned,
}

/// Formats an error for display in the terminal UI without including backtraces.
///
/// The output always starts with the top-level error message and, when available,
/// appends a `Caused by:` section containing every error in the cause chain.
fn format_error_for_tui(err: &anyhow::Error) -> String {
    let mut causes = err.chain();

    let Some(top_level) = causes.next() else {
        return "unknown error".to_owned();
    };

    let cause_lines: Vec<String> = causes.map(|cause| cause.to_string()).collect();
    if cause_lines.is_empty() {
        return top_level.to_string();
    }

    let mut output = top_level.to_string();
    output.push_str("\n\nCaused by:\n");

    for (idx, cause) in cause_lines.iter().enumerate() {
        output.push_str(&format!("    {idx}: {cause}"));
        if idx + 1 < cause_lines.len() {
            output.push('\n');
        }
    }

    output
}

/// Formats an exit status for human-readable error messages.
fn format_exit_status(status: std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        code.to_string()
    } else {
        status.to_string()
    }
}

enum MoveTarget<'a> {
    Branch { name: &'a str },
    Commit { commit_id: gix::ObjectId },
    MergeBase,
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
enum GotoBranchItem {
    Branch(FullName),
    Unassigned,
}

impl FuzzyPickerItem for GotoBranchItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        match self {
            Self::Branch(full_name) => [Col {
                text: full_name.shorten().to_str_lossy(),
                searchable: Some(searchable),
            }],
            Self::Unassigned => [Col {
                text: "unassigned changes".into(),
                searchable: Some(searchable),
            }],
        }
    }

    fn style(&self, theme: &'static Theme) -> Style {
        match self {
            Self::Branch(..) => theme.local_branch,
            Self::Unassigned => theme.info,
        }
    }
}

#[derive(Debug, Clone)]
struct ApplyBranchItem {
    name: String,
    last_commiter: String,
    has_local: bool,
    updated_at: u128,
    updated_at_display: String,
}

impl FuzzyPickerItem for ApplyBranchItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        [
            Col {
                text: if self.has_local { "local" } else { "remote" }.into(),
                searchable: None,
            },
            Col {
                text: self.name.as_str().into(),
                searchable: Some(searchable),
            },
            Col {
                text: self.updated_at_display.as_str().into(),
                searchable: None,
            },
            Col {
                text: self.last_commiter.as_str().into(),
                searchable: None,
            },
        ]
    }

    fn style(&self, theme: &'static Theme) -> Style {
        if self.has_local {
            theme.local_branch
        } else {
            theme.remote_branch
        }
    }
}
