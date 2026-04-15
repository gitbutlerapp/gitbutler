use std::{
    borrow::Cow,
    cell::RefCell,
    ffi::OsString,
    iter::once,
    process::Command,
    rc::Rc,
    sync::{Arc, LazyLock, mpsc::Receiver},
    time::Duration,
};

use anyhow::Context as _;
use bstr::{BString, ByteSlice};
use but_core::{DryRun, tree::create_tree::RejectionReason};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use gitbutler_operating_modes::OperatingMode;
use gitbutler_stack::StackId;
use itertools::Either;
use ratatui::{
    Frame,
    palette::Hsl,
    prelude::*,
    widgets::{Block, BorderType, Borders, List, ListItem},
};
use ratatui_textarea::{CursorMove, TextArea};
use tracing::Level;
use unicode_width::UnicodeWidthStr;

use crate::{
    CliId,
    command::legacy::{
        rub::RubOperationDiscriminants,
        status::{
            CommitLineContent, FileLineContent, StatusFlags, StatusOutputLine, TuiLaunchOptions,
            output::BranchLineContent,
            tui::{
                confirm::{Confirm, ConfirmMessage},
                cursor::{Cursor, is_selectable_in_mode},
                details::{Details, DetailsMessage, DetailsVisibility, RenderNextChunkResult},
                dry_run_workspace::DryRunWorkspace,
                event_polling::{CrosstermEventPolling, EventPolling, NoopEventPolling},
                fps::FpsCounter,
                graph_extension::{ExtensionDirection, extend_connector_spans},
                highlight::{Highlights, with_highlight},
                key_bind::{KeyBinds, confirm_key_binds, default_key_binds},
                message_on_drop::MessageOnDrop,
                mode::{
                    CommandMode, CommitMode, CommitSource, InlineRewordMode, Mode, MoveMode,
                    MoveSource, RubMode, RubSource, StackCommitSource, UnassignedCommitSource,
                },
                operations::stack_has_assigned_changes,
                toast::{ToastKind, Toasts},
            },
        },
    },
    id::UNASSIGNED,
    tui::{CrosstermTerminalGuard, HeadlessTerminalGuard, TerminalGuard},
    utils::{DebugAsType, OutputChannel, binary_path::current_exe_for_but_exec},
};

use super::{
    FilesStatusFlag,
    output::{StatusOutputContent, StatusOutputLineData},
};

mod confirm;
mod cursor;
mod details;
mod dry_run_workspace;
mod event_polling;
mod fps;
mod graph_extension;
mod highlight;
mod key_bind;
mod message_on_drop;
mod mode;
mod operations;
mod rub;
mod rub_from_detail_view;
mod toast;

#[cfg(test)]
mod tests;

static CURSOR_BG: LazyLock<Color> = LazyLock::new(|| Color::Rgb(69, 71, 90));

static DETAILS_CURSOR_BG: LazyLock<Color> =
    LazyLock::new(|| Color::from_hsl(Hsl::new(236.8, 0.162, 0.229)));

const NOOP: &str = "noop";
const CURSOR_CONTEXT_ROWS: usize = 3;

/// How much does the detail area grow/shrink with when adjusted
const DETAILS_SIZE_ADJUSTMENT_PERCENTAGE: u16 = 5;

const DETAILS_MIN_SIZE_PERCENTAGE: u16 = 30;
const DETAILS_MAX_SIZE_PERCENTAGE: u16 = 90;

pub(super) async fn render_tui(
    ctx: &mut Context,
    out: &mut OutputChannel,
    mode: &OperatingMode,
    flags: StatusFlags,
    status_lines: Vec<StatusOutputLine>,
    options: TuiLaunchOptions,
) -> anyhow::Result<Vec<StatusOutputLine>> {
    let mut app = App::new(status_lines, flags, options);

    let mut messages = Vec::new();

    // second buffer so we can send messages from `App::handle_message`
    let mut other_messages = Vec::new();

    if app.options.headless {
        let mut terminal_guard = HeadlessTerminalGuard::new(240, 240)?;
        let event_polling = NoopEventPolling;

        render_loop(
            &mut app,
            &mut terminal_guard,
            event_polling,
            &mut messages,
            &mut other_messages,
            ctx,
            out,
            mode,
        )
        .await?;
    } else {
        let mut terminal_guard = CrosstermTerminalGuard::new(true)?;
        let event_polling = CrosstermEventPolling;

        render_loop(
            &mut app,
            &mut terminal_guard,
            event_polling,
            &mut messages,
            &mut other_messages,
            ctx,
            out,
            mode,
        )
        .await?;
    }

    Ok(app.status_lines)
}

#[expect(clippy::too_many_arguments)]
async fn render_loop<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: E,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
    ctx: &mut Context,
    out: &mut OutputChannel,
    mode: &OperatingMode,
) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    E: EventPolling + Copy,
{
    loop {
        if app
            .options
            .quit_after
            .is_some_and(|quit_after| quit_after <= app.updates)
        {
            break Ok(());
        }

        render_loop_once(
            app,
            terminal_guard,
            event_polling,
            messages,
            other_messages,
            ctx,
            out,
            mode,
        )
        .await?;

        if app.should_quit {
            break Ok(());
        }
    }
}

#[expect(clippy::too_many_arguments)]
async fn render_loop_once<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: E,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
    ctx: &mut Context,
    out: &mut OutputChannel,
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
        ctx,
        out,
        mode,
    )
    .await?;

    render(app, terminal_guard)?;

    app.fps.frame_finished();

    Ok(())
}

#[expect(clippy::too_many_arguments)]
async fn update<T, E>(
    app: &mut App,
    terminal_guard: &mut T,
    event_polling: E,
    messages: &mut Vec<Message>,
    other_messages: &mut Vec<Message>,
    ctx: &mut Context,
    out: &mut OutputChannel,
    mode: &OperatingMode,
) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
    E: EventPolling,
{
    app.updates += 1;

    // update at full speed while we're rendering the diff
    let event_poll_timeout = if app.details.needs_update() {
        Duration::from_millis(0)
    } else {
        Duration::from_millis(30)
    };
    // poll terminal events
    for event in event_polling.poll(event_poll_timeout)? {
        event_to_messages(event, app.active_key_binds(), &app.mode, messages);
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

    // handle messages
    messages.append(&mut app.delayed_messages);
    loop {
        if messages.is_empty() {
            break;
        }
        for msg in messages.drain(..) {
            app.handle_message(ctx, out, mode, terminal_guard, other_messages, msg)
                .await;
        }
        std::mem::swap(messages, other_messages);
    }

    if app.toasts.update() {
        app.should_render = true;
    }

    if app.highlight.update() {
        app.should_render = true;
    }

    if app.details.needs_update() {
        let selection = app
            .cursor
            .selected_line(&app.status_lines)
            .and_then(|line| line.data.cli_id())
            .map(|id| &**id);
        match app.details.update(ctx, selection) {
            Ok(Some(result)) => match result {
                RenderNextChunkResult::Done => {
                    if app.options.quit_after_rendering_full_diff {
                        app.should_quit = true;
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
            app.render(frame)
        })?;
    }

    Ok(())
}

#[derive(Debug)]
struct App {
    status_lines: Vec<StatusOutputLine>,
    flags: StatusFlags,
    should_quit: bool,
    should_render: bool,
    cursor: Cursor,
    scroll_top: usize,
    mode: Mode,
    key_binds: KeyBinds,
    confirm_key_binds: KeyBinds,
    toasts: Toasts,
    renders: u64,
    updates: u64,
    highlight: Highlights,
    confirm: Option<Confirm>,
    details: Details,
    options: TuiLaunchOptions,
    delayed_messages: Vec<Message>,
    incoming_out_of_band_messages: Vec<Rc<Receiver<Message>>>,
    fps: FpsCounter,
    to_be_discarded: Option<Arc<CliId>>,
    status_width_percentage: u16,
    dry_run_workspace: DryRunWorkspace,
}

impl App {
    fn new(
        status_lines: Vec<StatusOutputLine>,
        flags: StatusFlags,
        options: TuiLaunchOptions,
    ) -> Self {
        let cursor = if let Some(object_id) = options.select_commit {
            Cursor::select_commit(object_id, &status_lines)
                .unwrap_or_else(|| Cursor::new(&status_lines))
        } else {
            Cursor::new(&status_lines)
        };

        let details = if options.show_diff {
            Details::new_visible()
        } else {
            Details::new_hidden()
        };

        Self {
            status_lines,
            flags,
            cursor,
            scroll_top: 0,
            should_quit: false,
            should_render: true,
            mode: Mode::default(),
            key_binds: default_key_binds(),
            confirm_key_binds: confirm_key_binds(),
            toasts: Default::default(),
            renders: 0,
            updates: 0,
            highlight: Default::default(),
            delayed_messages: Default::default(),
            incoming_out_of_band_messages: Default::default(),
            to_be_discarded: Default::default(),
            dry_run_workspace: Default::default(),
            fps: FpsCounter::new(),
            confirm: None,
            details,
            options,
            status_width_percentage: 50,
        }
    }

    fn active_key_binds(&self) -> &KeyBinds {
        if self.confirm.is_some() {
            &self.confirm_key_binds
        } else {
            &self.key_binds
        }
    }

    fn status_content_area(&self, terminal_area: Rect) -> Rect {
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(terminal_area)[0]
    }

    fn details_viewport(&self, terminal_area: Rect) -> Rect {
        let content_area = self.status_content_area(terminal_area);
        self.status_layout(content_area)
            .details_area
            .unwrap_or(content_area)
    }

    /// Returns the number of terminal rows available for rendering the status list.
    fn status_viewport_height(&self, terminal_area: Rect) -> usize {
        let content_area = self.status_content_area(terminal_area);
        let status_area = self.status_layout(content_area).status_area;

        // The status pane uses a bottom border, so the inner list viewport is one row shorter
        // than the outer area.
        usize::from(status_area.height.saturating_sub(1)).max(1)
    }

    /// Returns the rendered height in terminal rows for the given status line.
    fn rendered_height_for_status_line(&self, line_idx: usize) -> usize {
        self.status_lines
            .get(line_idx)
            .map(|line| {
                self.render_status_list_item(line, self.cursor.index() == line_idx)
                    .into_iter()
                    .count()
            })
            .unwrap_or(0)
    }

    /// Returns the total rendered height of the entire status list.
    fn total_rendered_height(&self) -> usize {
        (0..self.status_lines.len())
            .map(|idx| self.rendered_height_for_status_line(idx))
            .sum()
    }

    /// Returns the rendered row range occupied by the selected line.
    fn selected_row_range(&self) -> Option<std::ops::Range<usize>> {
        let selected_idx = self.cursor.index();
        let selected_line = self.status_lines.get(selected_idx)?;
        let start = (0..selected_idx)
            .map(|idx| self.rendered_height_for_status_line(idx))
            .sum();
        let len = self
            .render_status_list_item(selected_line, true)
            .into_iter()
            .count();
        Some(start..start.saturating_add(len))
    }

    /// Clamps the topmost visible rendered row to the available content height.
    fn clamp_scroll_top(&mut self, visible_height: usize) {
        let max_scroll_top = self.total_rendered_height().saturating_sub(visible_height);
        self.scroll_top = self.scroll_top.min(max_scroll_top);
    }

    /// Adjusts the viewport so the selected line stays visible with context rows above and below
    /// whenever possible.
    fn ensure_cursor_visible(&mut self, visible_height: usize) {
        self.clamp_scroll_top(visible_height);

        let Some(selected_rows) = self.selected_row_range() else {
            return;
        };

        let selected_height = selected_rows.end.saturating_sub(selected_rows.start);
        let context_rows =
            CURSOR_CONTEXT_ROWS.min(visible_height.saturating_sub(selected_height) / 2);

        let min_scroll_top = selected_rows
            .end
            .saturating_add(context_rows)
            .saturating_sub(visible_height);
        let max_scroll_top = selected_rows.start.saturating_sub(context_rows);

        if self.scroll_top < min_scroll_top {
            self.scroll_top = min_scroll_top;
        } else if self.scroll_top > max_scroll_top {
            self.scroll_top = max_scroll_top;
        }

        self.clamp_scroll_top(visible_height);
    }

    #[tracing::instrument(level = Level::TRACE, skip(self, ctx, out, mode, terminal_guard, messages))]
    async fn handle_message<T>(
        &mut self,
        ctx: &mut Context,
        out: &mut OutputChannel,
        mode: &OperatingMode,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
        msg: Message,
    ) where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        if let Err(err) = self
            .try_handle_message(ctx, out, mode, terminal_guard, messages, msg)
            .await
        {
            messages.push(Message::ShowError(Arc::new(err)));
        }
    }

    async fn try_handle_message<T>(
        &mut self,
        ctx: &mut Context,
        out: &mut OutputChannel,
        mode: &OperatingMode,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
        msg: Message,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        self.should_render = true;
        let terminal_area: Rect = terminal_guard.terminal_mut().size()?.into();
        let visible_height = self.status_viewport_height(terminal_area);

        if self.details.needs_update_after_message(&msg) {
            self.details.mark_dirty();
        }

        match msg {
            Message::Quit => {
                self.should_quit = true;
            }
            Message::JustRender => {}
            Message::MoveCursorUp => {
                if let Some(new_cursor) =
                    self.cursor
                        .move_up(&self.status_lines, &self.mode, self.flags.show_files)
                {
                    self.cursor = new_cursor;
                    self.update_dry_run_state(ctx);
                }
            }
            Message::MoveCursorDown => {
                if let Some(new_cursor) =
                    self.cursor
                        .move_down(&self.status_lines, &self.mode, self.flags.show_files)
                {
                    self.cursor = new_cursor;
                    self.update_dry_run_state(ctx);
                }
            }
            Message::MoveCursorPreviousSection => {
                if let Some(new_cursor) = self.cursor.move_previous_section(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                    self.update_dry_run_state(ctx);
                }
            }
            Message::MoveCursorNextSection => {
                if let Some(new_cursor) = self.cursor.move_next_section(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                    self.update_dry_run_state(ctx);
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
                    self.update_dry_run_state(ctx);
                }
            }
            Message::Rub(rub_message) => match rub_message {
                RubMessage::Start => self.handle_start_rub(),
                RubMessage::StartWithSource {
                    source,
                    unlock_details,
                } => {
                    self.handle_start_rub_with_source(source, unlock_details);
                }
                RubMessage::Confirm => self.handle_confirm_rub(ctx, messages)?,
            },
            Message::EnterNormalMode => {
                self.handle_enter_normal_mode(messages);
            }
            Message::EnterDetailsMode => {
                self.handle_enter_details_mode(messages);
            }
            Message::LeaveDetailsMode => {
                self.handle_leave_details_mode(messages);
            }
            Message::Files(files_message) => match files_message {
                FilesMessage::ToggleGlobalFilesList => {
                    self.handle_toggle_global_files_list(messages)
                }
                FilesMessage::ToggleFilesForCommit => {
                    self.handle_toggle_files_for_commit(ctx, messages)?
                }
            },
            Message::Reload(select_after_reload) => {
                self.handle_reload(ctx, out, mode, select_after_reload)
                    .await?
            }
            Message::ShowError(err) => self.handle_show_error(err, messages),
            Message::Commit(commit_message) => match commit_message {
                CommitMessage::CreateEmpty => self.handle_commit_create_empty(ctx, messages)?,
                CommitMessage::Start => self.handle_commit_start(ctx)?,
                CommitMessage::Confirm => self.handle_commit_confirm(ctx, messages)?,
                CommitMessage::SetInsertSide(insert_side) => {
                    self.handle_commit_set_insert_side(insert_side);
                }
                CommitMessage::ToggleEmptyMessage => {
                    self.handle_commit_toggle_empty_message();
                }
            },
            Message::Reword(reword_message) => match reword_message {
                RewordMessage::WithEditor => {
                    self.handle_reword_with_editor(ctx, terminal_guard, messages)?;
                }
                RewordMessage::InlineStart => self.handle_start_reword_inline(ctx, messages)?,
                RewordMessage::InlineInput(ev) => self.handle_reword_inline_input(ev),
                RewordMessage::InlineConfirm => self.handle_confirm_inline_reword(ctx, messages)?,
            },
            Message::Command(command_message) => match command_message {
                CommandMessage::Start => self.handle_enter_command_mode(),
                CommandMessage::Input(ev) => self.handle_command_input(ev),
                CommandMessage::Confirm => {
                    self.handle_run_command(terminal_guard, out, messages)?
                }
            },
            Message::Move(move_message) => match move_message {
                MoveMessage::Start => self.handle_move_start(),
                MoveMessage::SetInsertSide(insert_side) => {
                    self.handle_move_set_insert_side(insert_side)
                }
                MoveMessage::Confirm => self.handle_move_confirm(ctx, messages)?,
            },
            Message::Branch(branch_message) => match branch_message {
                BranchMessage::Start => {
                    self.handle_start_branch_mode(messages);
                }
                BranchMessage::New => {
                    self.handle_create_new_branch(ctx, messages)?;
                }
            },
            Message::CopySelection => {
                self.handle_copy_selection()?;
            }
            Message::ShowToast { kind, text } => {
                self.toasts.insert(kind, text);
            }
            Message::Confirm(confirm_message) => {
                self.confirm = self
                    .confirm
                    .take()
                    .and_then(|confirm| confirm.handle_message(confirm_message, messages));
            }
            Message::RunAfterConfirmation(f) => {
                if let Some(f) = f.0.try_borrow_mut().ok().and_then(|mut f| f.take()) {
                    (f)(self, ctx, messages)?;
                }
            }
            Message::Details(details_message) => {
                let details_viewport = self.details_viewport(terminal_area);
                self.details
                    .try_handle_message(details_message, details_viewport, messages)?;
            }
            Message::RegisterMessageOnDrop(rx) => {
                self.incoming_out_of_band_messages.push(rx);
            }
            Message::WithOneFrameDelay(msg) => {
                self.delayed_messages.push(*msg);
            }
            Message::Discard => {
                self.handle_discard(ctx, messages);
            }
            Message::DropToBeDiscarded => {
                self.to_be_discarded = None;
            }
            Message::AndThen { lhs, rhs } => {
                Box::pin(self.try_handle_message(ctx, out, mode, terminal_guard, messages, *lhs))
                    .await?;
                Box::pin(self.try_handle_message(ctx, out, mode, terminal_guard, messages, *rhs))
                    .await?;
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
        }

        self.ensure_cursor_visible(visible_height);

        Ok(())
    }

    fn handle_enter_normal_mode(&mut self, messages: &mut Vec<Message>) {
        if matches!(self.mode, Mode::Normal) {
            match self.flags.show_files {
                FilesStatusFlag::None => {}
                FilesStatusFlag::All => {
                    messages.push(Message::Files(FilesMessage::ToggleGlobalFilesList));
                }
                FilesStatusFlag::Commit(_) => {
                    messages.push(Message::Files(FilesMessage::ToggleFilesForCommit));
                }
            }
        }

        if matches!(self.mode, Mode::Details) {
            messages.push(Message::Details(DetailsMessage::Deselect));
        }

        self.dry_run_workspace.clear();

        self.mode = Mode::Normal;

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

    fn handle_enter_details_mode(&mut self, messages: &mut Vec<Message>) {
        self.mode = Mode::Details;
        if self.details.is_visible() {
            messages.push(Message::Details(DetailsMessage::SelectFirstSection));
        } else {
            messages.push(Message::Details(DetailsMessage::ToggleVisibility));

            // We can't select the first section on the same frame that we show the detail view.
            // The incremental diff rendering introduces a one frame delay before the first section
            // is shown.
            messages
                .push(Message::Details(DetailsMessage::SelectFirstSection).with_one_frame_delay());
        }
    }

    fn handle_leave_details_mode(&mut self, messages: &mut Vec<Message>) {
        if matches!(self.mode, Mode::Details) {
            messages.push(Message::EnterNormalMode);
        }
    }

    fn handle_start_rub(&mut self) {
        let Some(selected_line) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };
        let Some(cli_id) = selected_line.data.cli_id() else {
            return;
        };
        self.handle_start_rub_with_source(RubSource::CliId(Arc::clone(cli_id)), None);
    }

    fn handle_start_rub_with_source(
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
            RubSource::CommittedHunk(..) => {}
        }

        let available_targets = self
            .status_lines
            .iter()
            .filter_map(|line| line.data.cli_id())
            .filter(|target| {
                source == ***target
                    || match &source {
                        RubSource::CliId(source) => rub::route_operation(source, target).is_some(),
                        RubSource::CommittedHunk(hunk) => {
                            rub_from_detail_view::route_operation(hunk, target).is_some()
                        }
                    }
            })
            .cloned()
            .collect::<Vec<_>>();

        self.mode = Mode::Rub(RubMode {
            source,
            available_targets,
            _unlock_details: unlock_details,
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

    /// Handles toggling file visibility and requests a status reload.
    fn handle_toggle_global_files_list(&mut self, messages: &mut Vec<Message>) {
        self.flags.show_files = match self.flags.show_files {
            FilesStatusFlag::None => FilesStatusFlag::All,
            FilesStatusFlag::All | FilesStatusFlag::Commit(_) => FilesStatusFlag::None,
        };
        messages.push(Message::Reload(None));
    }

    fn handle_toggle_files_for_commit(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if let Some(selection) = self.cursor.selected_line(&self.status_lines)
            && let Some(cli_id) = selection.data.cli_id()
            && let CliId::Commit { commit_id, .. } = &**cli_id
        {
            if !operations::commit_is_empty(ctx, *commit_id)? {
                let select_after_reload = match self.flags.show_files {
                    FilesStatusFlag::None => {
                        self.flags.show_files = FilesStatusFlag::Commit(*commit_id);
                        Some(SelectAfterReload::FirstFileInCommit(*commit_id))
                    }
                    FilesStatusFlag::All | FilesStatusFlag::Commit(_) => {
                        self.flags.show_files = FilesStatusFlag::None;
                        Some(SelectAfterReload::Commit(*commit_id))
                    }
                };
                messages.push(Message::Reload(select_after_reload));
            }
        } else {
            self.flags.show_files = FilesStatusFlag::None;
            messages.push(Message::Reload(None));
        };

        Ok(())
    }

    /// Handles confirming the currently selected rub operation.
    fn handle_confirm_rub(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let reload_message = match &self.mode {
            Mode::Rub(RubMode {
                source,
                available_targets: _,
                _unlock_details: _,
            }) => {
                if let Some(selected_line) = self.cursor.selected_line(&self.status_lines)
                    && let Some(target) = selected_line.data.cli_id()
                {
                    match source {
                        RubSource::CliId(source) => {
                            if let Some(operation) = rub::route_operation(source, target) {
                                if let Some(what_to_select) = operations::rub(ctx, &operation)? {
                                    if self.options.debug {
                                        messages.push(Message::ShowToast {
                                            kind: ToastKind::Debug,
                                            text: format!(
                                                "Performed `{:?}`",
                                                RubOperationDiscriminants::from(operation)
                                            ),
                                        });
                                    }
                                    Some(Message::Reload(Some(what_to_select)))
                                } else {
                                    messages.push(Message::ShowError(Arc::new(
                                        anyhow::Error::from(rub::OperationNotSupported::new(
                                            &operation,
                                        )),
                                    )));
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        RubSource::CommittedHunk(hunk) => {
                            if let Some(operation) =
                                rub_from_detail_view::route_operation(hunk, target)
                            {
                                Some(Message::Reload(Some(operation.execute(ctx)?)))
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                }
            }
            Mode::Normal
            | Mode::Branch
            | Mode::Details
            | Mode::InlineReword(..)
            | Mode::Command(..)
            | Mode::Commit(..)
            | Mode::Move(..) => None,
        };

        self.flags.show_files = FilesStatusFlag::None;

        messages.extend([
            Message::EnterNormalMode,
            reload_message.unwrap_or(Message::Reload(None)),
        ]);

        Ok(())
    }

    /// Handles reloading status output and restoring selection.
    async fn handle_reload(
        &mut self,
        ctx: &mut Context,
        out: &mut OutputChannel,
        mode: &OperatingMode,
        select_after_reload: Option<SelectAfterReload>,
    ) -> anyhow::Result<()> {
        let new_lines = operations::reload_legacy(ctx, out, mode, self.flags, self.options).await?;

        self.cursor = if let Some(select_after_reload) = select_after_reload {
            match select_after_reload {
                SelectAfterReload::Commit(commit_id) => {
                    Cursor::select_commit(commit_id, &new_lines)
                }
                SelectAfterReload::Branch(branch) => Cursor::select_branch(branch, &new_lines),
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
            let default_restore = || {
                self.cursor
                    .selection_cli_id_for_reload(&self.status_lines, self.flags.show_files)
                    .and_then(|previously_selected_cli_id| {
                        Cursor::restore(previously_selected_cli_id, &new_lines)
                    })
            };

            let selected_merge_base_in_branch_mode = matches!(self.mode, Mode::Branch)
                && self
                    .cursor
                    .selected_line(&self.status_lines)
                    .is_some_and(|line| matches!(line.data, StatusOutputLineData::MergeBase));

            if selected_merge_base_in_branch_mode {
                Cursor::select_merge_base(&new_lines).or_else(default_restore)
            } else {
                default_restore()
            }
        }
        .unwrap_or_else(|| Cursor::new(&new_lines));

        self.status_lines = new_lines;
        Ok(())
    }

    /// Handles showing a transient UI error.
    fn handle_show_error(&mut self, err: Arc<anyhow::Error>, messages: &mut Vec<Message>) {
        self.toasts
            .insert(ToastKind::Error, format_error_for_tui(&err));

        // ensure we always enter normal mode when something does wrong
        // so we don't get stuck in whatever mode we were in previously
        messages.push(Message::EnterNormalMode);
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

    fn handle_discard(&mut self, ctx: &mut Context, messages: &mut Vec<Message>) {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };
        let Some(cli_id) = selection.data.cli_id() else {
            return;
        };

        self.confirm = Some(match &**cli_id {
            CliId::Unassigned { .. } => {
                self.to_be_discarded = Some(Arc::clone(cli_id));
                let drop_to_be_discarded =
                    message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                Confirm::new(
                    "Discard unassigned changes?",
                    run_after_confirmation_msg(move |_, ctx, messages| {
                        operations::discard_unassigned_legacy(ctx)?;
                        messages.push(Message::Reload(Some(SelectAfterReload::Unassigned)));
                        drop(drop_to_be_discarded);
                        Ok(())
                    }),
                )
            }
            CliId::Uncommitted(uncommitted) => {
                self.to_be_discarded = Some(Arc::clone(cli_id));
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
                    format!("Discard {}?", uncommitted.describe()),
                    run_after_confirmation_msg(move |_, ctx, messages| {
                        let hunk_assignments = uncommitted
                            .hunk_assignments
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>();
                        operations::discard_uncommitted_legacy(ctx, hunk_assignments)?;
                        messages.push(Message::Reload(Some(select_after_reload)));
                        drop(drop_to_be_discarded);
                        Ok(())
                    }),
                )
            }
            CliId::Stack { stack_id, .. } => {
                self.to_be_discarded = Some(Arc::clone(cli_id));
                let stack_id = *stack_id;
                let select_after_reload = self
                    .select_top_branch_for_stack_after_reload(stack_id)
                    .unwrap_or(SelectAfterReload::Stack(stack_id));
                let drop_to_be_discarded =
                    message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                Confirm::new(
                    "Discard staged changes in this stack?",
                    run_after_confirmation_msg(move |_, ctx, messages| {
                        operations::discard_stack(ctx, stack_id)?;
                        messages.push(Message::Reload(Some(select_after_reload)));
                        drop(drop_to_be_discarded);
                        Ok(())
                    }),
                )
            }
            CliId::Commit { commit_id, .. } => {
                self.to_be_discarded = Some(Arc::clone(cli_id));
                let commit_id = *commit_id;
                let select_after_reload = self
                    .cursor
                    .select_after_discarded_commit(&self.status_lines);
                let drop_to_be_discarded =
                    message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                Confirm::new(
                    format!("Discard commit {}?", commit_id.to_hex_with_len(7)),
                    run_after_confirmation_msg(move |_, ctx, messages| {
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
                        messages.push(Message::Reload(select_after_reload));
                        drop(drop_to_be_discarded);
                        Ok(())
                    }),
                )
            }
            CliId::Branch { name, stack_id, .. } => {
                let Some(stack_id) = *stack_id else {
                    return;
                };

                let name = name.to_owned();
                self.to_be_discarded = Some(Arc::clone(cli_id));
                let select_after_reload = self
                    .cursor
                    .select_after_discarded_branch(&self.status_lines);
                let drop_to_be_discarded =
                    message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                Confirm::new(
                    format!("Discard branch {name}?"),
                    run_after_confirmation_msg(move |_, ctx, messages| {
                        operations::remove_branch_legacy(ctx, stack_id, name)?;
                        messages.push(Message::Reload(select_after_reload));
                        drop(drop_to_be_discarded);
                        Ok(())
                    }),
                )
            }
            CliId::PathPrefix { .. } | CliId::CommittedFile { .. } => return,
        });
    }

    /// Handles creating an empty commit relative to the current selection.
    fn handle_commit_create_empty(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if !matches!(self.mode, Mode::Normal) {
            return Ok(());
        }

        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        match &selection.data {
            StatusOutputLineData::Branch { cli_id } => {
                let CliId::Branch { name, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_result = operations::create_empty_commit_relative_to_branch(ctx, name)?;

                messages.push(Message::Reload(Some(SelectAfterReload::Commit(
                    commit_result.new_commit,
                ))));
            }
            StatusOutputLineData::Commit { cli_id, .. } => {
                let CliId::Commit { commit_id, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_result =
                    operations::create_empty_commit_relative_to_commit(ctx, *commit_id)?;

                messages.push(Message::Reload(Some(SelectAfterReload::Commit(
                    commit_result.new_commit,
                ))));
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
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
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let insert_side = InsertSide::Above;

        let commit_mode = match &selection.data {
            StatusOutputLineData::UnassignedChanges { cli_id } => {
                let Some(source) = CommitSource::try_new(Arc::unwrap_or_clone(Arc::clone(cli_id)))
                else {
                    return Ok(());
                };
                CommitMode {
                    source: Arc::new(source),
                    scope_to_stack: None,
                    insert_side,
                    empty_message: false,
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
                    scope_to_stack: cli_id.stack_id(),
                    insert_side,
                    empty_message: false,
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
                    insert_side,
                    empty_message: false,
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
                    scope_to_stack,
                    insert_side,
                    empty_message: false,
                }
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        self.mode = Mode::Commit(commit_mode);

        Ok(())
    }

    fn handle_commit_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Commit(CommitMode {
            source,
            scope_to_stack,
            insert_side,
            empty_message,
        }) = &self.mode
        else {
            return Ok(());
        };

        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        if selection
            .data
            .cli_id()
            .is_some_and(|target| **source == **target)
        {
            messages.push(Message::EnterNormalMode);
            return Ok(());
        }

        let target = match &selection.data {
            StatusOutputLineData::Branch { cli_id }
            | StatusOutputLineData::Commit { cli_id, .. } => cli_id,
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
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

        let Some(commit_create_result) = operations::create_commit(
            ctx,
            target,
            source,
            *scope_to_stack,
            *insert_side,
            DryRun::No,
        )?
        else {
            return Ok(());
        };

        let rejected_specs_error_msg = if !commit_create_result.rejected_specs.is_empty() {
            let mut full_error_msg = "Some selected changes could not be committed:\n".to_owned();
            let mut errors_per_diff_spec = commit_create_result
                .rejected_specs
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
            Some(full_error_msg)
        } else {
            None
        };

        messages.extend(
            [
                Message::EnterNormalMode,
                Message::Reload(
                    commit_create_result
                        .new_commit
                        .map(SelectAfterReload::Commit),
                ),
            ]
            .into_iter()
            // TODO(david): don't use a separate reword step, instead get message before creating
            // commit. However that requires computing the diff which I haven't yet figured out how
            // to do
            .chain(
                (!empty_message && commit_create_result.new_commit.is_some())
                    .then_some(Message::Reword(RewordMessage::WithEditor)),
            )
            .chain(rejected_specs_error_msg.map(|text| Message::ShowToast {
                kind: ToastKind::Error,
                text,
            })),
        );

        Ok(())
    }

    fn handle_commit_set_insert_side(&mut self, insert_side: InsertSide) {
        if let Mode::Commit(mode) = &mut self.mode {
            mode.insert_side = insert_side;
        }
    }

    fn handle_commit_toggle_empty_message(&mut self) {
        if let Mode::Commit(mode) = &mut self.mode {
            mode.empty_message = !mode.empty_message;
        }
    }

    fn update_dry_run_state(&mut self, ctx: &mut Context) {
        self.dry_run_workspace.clear();

        let Mode::Commit(CommitMode {
            source,
            scope_to_stack,
            insert_side,
            ..
        }) = &self.mode
        else {
            return;
        };
        let Some(target) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|selection| selection.data.cli_id())
        else {
            return;
        };

        let Ok(Some(result)) = operations::create_commit(
            ctx,
            target,
            source,
            *scope_to_stack,
            *insert_side,
            DryRun::Yes,
        ) else {
            return;
        };

        let conflicted_commits = result
            .workspace
            .head_info
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| &segment.commits)
            .filter(|commit| commit.has_conflicts)
            .collect::<Vec<_>>();

        self.toasts
            .insert(ToastKind::Debug, format!("conflicted commits: {}", conflicted_commits.len()));

        for conflicted_commit in conflicted_commits {
            if let Some(commit_id_in_current_workspace) =
                result.workspace.replaced_commits.iter().find_map(
                    |(old_commit_id, new_commit_id)| {
                        (*new_commit_id == conflicted_commit.id).then_some(*old_commit_id)
                    },
                )
            {
                self.dry_run_workspace
                    .insert_conflicted_commit(commit_id_in_current_workspace);
            }
        }
    }

    fn handle_move_start(&mut self) {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };

        let move_mode = match &selection.data {
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
        };

        self.mode = Mode::Move(move_mode);
    }

    fn handle_move_set_insert_side(&mut self, insert_side: InsertSide) {
        if let Mode::Move(mode) = &mut self.mode {
            mode.insert_side = insert_side;
        }
    }

    fn handle_move_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Move(MoveMode {
            source,
            insert_side,
        }) = &self.mode
        else {
            return Ok(());
        };

        // find target
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        if selection
            .data
            .cli_id()
            .is_some_and(|target| **source == **target)
        {
            messages.push(Message::EnterNormalMode);
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
                    MoveTarget::Branch { name } => {
                        operations::move_commit_to_branch(ctx, *source_commit_id, name)?
                    }
                    MoveTarget::Commit {
                        commit_id: target_commit_id,
                    } => operations::move_commit_to_commit(
                        ctx,
                        *source_commit_id,
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
            Message::EnterNormalMode,
            Message::Reload(selection_after_reload),
        ]);

        Ok(())
    }

    fn handle_start_branch_mode(&mut self, messages: &mut Vec<Message>) {
        let Some(new_cursor) = self.cursor.closest_branch_cursor(&self.status_lines) else {
            return;
        };

        let Some(selection) = new_cursor.selected_line(&self.status_lines) else {
            return;
        };

        match &selection.data {
            StatusOutputLineData::Branch { .. } | StatusOutputLineData::MergeBase => {}
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UnassignedChanges { .. }
            | StatusOutputLineData::UnassignedFile { .. }
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return,
        }

        self.cursor = new_cursor;
        self.mode = Mode::Branch;

        if !self.flags.show_files.is_none() {
            self.flags.show_files = FilesStatusFlag::None;
            messages.push(Message::Reload(None));
        }
    }

    fn handle_create_new_branch(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if !matches!(self.mode, Mode::Branch) {
            return Ok(());
        }

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
            StatusOutputLineData::MergeBase => operations::create_branch_legacy(ctx)?,
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UnassignedChanges { .. }
            | StatusOutputLineData::UnassignedFile { .. }
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        messages.extend([
            Message::EnterNormalMode,
            Message::Reload(Some(SelectAfterReload::Branch(new_name))),
        ]);

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

        self.highlight.insert(Arc::clone(cli_id));

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
        if !matches!(self.mode, Mode::Normal) {
            return Ok(());
        }

        let Some(commit_id) = self.selected_commit_id() else {
            return Ok(());
        };

        let _suspend_guard = terminal_guard.suspend()?;

        let Some(reword_result) = operations::reword_commit_with_editor_legacy(ctx, commit_id)?
        else {
            return Ok(());
        };

        messages.push(Message::Reload(Some(SelectAfterReload::Commit(
            reword_result.new_commit,
        ))));

        Ok(())
    }

    fn handle_start_reword_inline(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if !matches!(self.mode, Mode::Normal) {
            return Ok(());
        }
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
                textarea.set_cursor_line_style(Style::default().green());
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
                textarea.set_cursor_line_style(Style::default());
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

        self.mode = Mode::InlineReword(inline_reword_mode);

        Ok(())
    }

    /// Handles key input while inline reword mode is active.
    fn handle_reword_inline_input(&mut self, ev: Event) {
        if let Mode::InlineReword(inline_reword_mode) = &mut self.mode {
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

    fn handle_confirm_inline_reword(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let inline_reword_mode = if let Mode::InlineReword(inline_reword_mode) = &self.mode {
            inline_reword_mode
        } else {
            messages.push(Message::EnterNormalMode);
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
                    operations::reword_commit_inline_legacy(ctx, *commit_id, first_line)?
                else {
                    messages.push(Message::EnterNormalMode);
                    return Ok(());
                };

                messages.extend([
                    Message::EnterNormalMode,
                    Message::Reload(Some(SelectAfterReload::Commit(reword_result.new_commit))),
                ]);
            }
            InlineRewordMode::Branch { name, stack_id, .. } => {
                let new_name = operations::reword_branch_inline_legacy(
                    ctx,
                    *stack_id,
                    name.to_owned(),
                    first_line.to_owned(),
                )?;

                messages.extend([
                    Message::EnterNormalMode,
                    Message::Reload(Some(SelectAfterReload::Branch(new_name))),
                ]);
            }
        }

        Ok(())
    }

    fn handle_enter_command_mode(&mut self) {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.move_cursor(CursorMove::End);

        self.mode = Mode::Command(CommandMode {
            textarea: Box::new(textarea),
        });
    }

    fn handle_command_input(&mut self, ev: Event) {
        if let Mode::Command(CommandMode { textarea }) = &mut self.mode {
            textarea.input(ev);
        }
    }

    fn handle_run_command<T>(
        &mut self,
        terminal_guard: &mut T,
        out: &mut OutputChannel,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Mode::Command(CommandMode { textarea }) = &self.mode else {
            messages.push(Message::EnterNormalMode);
            return Ok(());
        };

        let Some(input) = textarea.lines().first() else {
            return Ok(());
        };

        let binary_path = current_exe_for_but_exec()?;
        let args = shell_words::split(input)?.into_iter().map(OsString::from);

        let mut cmd = Command::new(binary_path);
        cmd.args(args);

        let _suspend_guard = terminal_guard.suspend()?;
        let status = cmd.spawn()?.wait()?;

        self.prompt_to_continue(out)?;

        drop(_suspend_guard);

        if status.success() {
            messages.extend([Message::EnterNormalMode, Message::Reload(None)]);
        } else {
            self.push_transient_error(anyhow::Error::msg(format!(
                "command exited with status {}",
                format_exit_status(status)
            )));
        }

        Ok(())
    }

    /// Prompts the user to press enter before returning from a command execution.
    fn prompt_to_continue(&mut self, out: &mut OutputChannel) -> anyhow::Result<()> {
        // don't prompt for user input during tests
        //
        // `cfg!(test)` is false for integration tests but we currently don't have integration
        // tests of the TUI so thats fine for now.
        const IN_TEST: bool = cfg!(test);

        if !IN_TEST && let Some(mut input_channel) = out.prepare_for_terminal_input() {
            input_channel.prompt_single_line("\npress enter to continue...")?;
        }

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

    fn render(&self, frame: &mut Frame) {
        let content_layout =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(frame.area());
        let main_content_area = content_layout[0];

        let (main_content_area, debug_area) = if self.options.debug {
            let layout =
                Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .split(main_content_area);
            (layout[0], Some(layout[1]))
        } else {
            (main_content_area, None)
        };

        let hotbar_area = content_layout[1];

        let status_layout = self.status_layout(main_content_area);

        let dimmed_block = Block::bordered()
            .border_style(Style::default().dark_gray())
            .border_type(BorderType::Plain)
            .borders(Borders::BOTTOM);
        let focused_block = Block::bordered()
            .border_style(Style::default().fg(self.mode.bg()))
            .border_type(BorderType::Thick)
            .borders(Borders::BOTTOM);

        let (status_block, details_block) = if matches!(self.mode, Mode::Details) {
            (dimmed_block, focused_block)
        } else {
            (focused_block, dimmed_block)
        };

        {
            let inner_area = status_block.inner(status_layout.status_area);
            frame.render_widget(status_block, status_layout.status_area);
            self.render_status(inner_area, frame);
        }

        if let Some(details_area) = status_layout.details_area {
            let inner_area = details_block.inner(details_area);
            frame.render_widget(details_block, details_area);
            self.details.render(inner_area, frame);
        }

        if let Some(debug_area) = debug_area {
            let outer_block = Block::bordered()
                .border_style(Style::default().dark_gray())
                .border_type(BorderType::Thick)
                .borders(Borders::LEFT);
            let inner_area = outer_block.inner(debug_area);
            frame.render_widget(outer_block, debug_area);
            self.render_debug(inner_area, frame);
        }

        self.render_hotbar(hotbar_area, frame);
    }

    fn status_layout(&self, area: Rect) -> StatusLayout {
        let (status_area, details_area) = match self.details.visibility() {
            DetailsVisibility::Hidden => (area, None),
            DetailsVisibility::VisibleVertical => {
                let layout = Layout::horizontal([
                    Constraint::Percentage(self.status_width_percentage),
                    Constraint::Percentage(100 - self.status_width_percentage),
                ])
                .split(area);
                (layout[0], Some(layout[1]))
            }
        };

        StatusLayout {
            status_area,
            details_area,
        }
    }

    fn render_status(&self, content_area: Rect, frame: &mut Frame) {
        let visible_height = content_area.height as usize;
        let items = self
            .status_lines
            .iter()
            .enumerate()
            .flat_map(|(idx, tui_line)| {
                self.render_status_list_item(tui_line, self.cursor.index() == idx)
            })
            .skip(self.scroll_top)
            .take(visible_height);
        let list = List::new(items);

        frame.render_widget(list, content_area);

        self.render_inline_reword(content_area, frame);

        self.render_toasts(content_area, frame);

        if let Some(confirm) = &self.confirm {
            confirm.render(content_area, frame);
        }
    }

    fn render_status_list_item(
        &self,
        tui_line: &StatusOutputLine,
        is_selected: bool,
    ) -> StatusListItem {
        let StatusOutputLine {
            connector,
            content,
            data,
        } = tui_line;

        let mut line = Line::default();

        if let Some(connector) = connector {
            line.extend(connector.clone());
        }

        let line_is_to_be_discarded =
            self.to_be_discarded
                .as_ref()
                .is_some_and(|to_be_discarded| {
                    data.cli_id()
                        .is_some_and(|selection| to_be_discarded == selection)
                });

        if line_is_to_be_discarded {
            line.extend([Span::raw("<< discard >>").black().on_red(), Span::raw(" ")]);
        } else if is_selected {
            match &self.mode {
                Mode::Normal | Mode::InlineReword(..) | Mode::Command(..) | Mode::Details => {}
                Mode::Rub(RubMode {
                    source,
                    available_targets: _,
                    _unlock_details: _,
                }) => {
                    self.render_rub_inline_labels_for_selected_line(data, source, &mut line);
                }
                Mode::Commit(commit_mode) => {
                    if data
                        .cli_id()
                        .is_some_and(|target| *commit_mode.source == **target)
                        // only target branches here, and not commits. Commits are handled at the
                        // end of this function because they require [`extend_connector_spans`]
                        || matches!(data, StatusOutputLineData::Branch { .. })
                    {
                        self.render_commit_labels_for_selected_line(data, commit_mode, &mut line);
                    }
                }
                Mode::Move(move_mode) => {
                    if data
                        .cli_id()
                        .is_some_and(|target| *move_mode.source == **target)
                        // only target branches here, and not commits. Commits are handled at the
                        // end of this function because they require [`extend_connector_spans`]
                        || matches!(data, StatusOutputLineData::Branch { .. })
                        || matches!(data, StatusOutputLineData::MergeBase)
                    {
                        self.render_move_labels_for_selected_line(data, move_mode, &mut line);
                    }
                }
                Mode::Branch => {
                    self.render_branch_labels_for_selected_line(data, &mut line);
                }
            }
        } else {
            match &self.mode {
                Mode::Normal
                | Mode::InlineReword(..)
                | Mode::Command(..)
                | Mode::Branch
                | Mode::Details => {}
                Mode::Rub(RubMode {
                    source,
                    available_targets: _,
                    _unlock_details: _,
                }) => {
                    if let Some(cli_id) = data.cli_id()
                        && source == &**cli_id
                    {
                        line.extend([source_span(), Span::raw(" ")]);
                    }
                }
                Mode::Commit(CommitMode { source, .. }) => {
                    if let Some(cli_id) = data.cli_id()
                        && **source == **cli_id
                    {
                        line.extend([source_span(), Span::raw(" ")]);
                    }
                }
                Mode::Move(MoveMode { source, .. }) => {
                    if let Some(cli_id) = data.cli_id()
                        && **source == **cli_id
                    {
                        line.extend([source_span(), Span::raw(" ")]);
                    }
                }
            }
        }

        let mut content_spans = match content {
            StatusOutputContent::Plain(spans) => spans.clone(),
            StatusOutputContent::Commit(CommitLineContent {
                sha,
                author,
                message,
                suffix,
            }) => {
                let mut spans =
                    Vec::with_capacity(sha.len() + author.len() + message.len() + suffix.len());
                if data.cli_id().is_some_and(|id| self.highlight.contains(id)) {
                    spans.extend(sha.iter().cloned().map(with_highlight));
                } else {
                    spans.extend(sha.iter().cloned());
                }
                spans.extend(author.iter().cloned());
                spans.extend(message.iter().cloned());
                spans.extend(suffix.iter().cloned());
                spans
            }
            StatusOutputContent::Branch(BranchLineContent {
                id,
                decoration_start,
                branch_name,
                decoration_end,
                suffix,
            }) => {
                let mut spans = Vec::with_capacity(
                    id.len()
                        + decoration_start.len()
                        + branch_name.len()
                        + decoration_end.len()
                        + suffix.len(),
                );
                spans.extend(id.iter().cloned());
                spans.extend(decoration_start.iter().cloned());
                if data.cli_id().is_some_and(|id| self.highlight.contains(id)) {
                    spans.extend(branch_name.iter().cloned().map(with_highlight));
                } else {
                    spans.extend(branch_name.iter().cloned());
                }
                spans.extend(decoration_end.iter().cloned());
                spans.extend(suffix.iter().cloned());
                spans
            }
            StatusOutputContent::File(FileLineContent { id, status, path }) => {
                let mut spans = Vec::with_capacity(id.len() + status.len() + path.len());
                spans.extend(id.iter().cloned());
                spans.extend(status.iter().cloned());
                if data.cli_id().is_some_and(|id| self.highlight.contains(id)) {
                    spans.extend(path.iter().cloned().map(with_highlight));
                } else {
                    spans.extend(path.iter().cloned());
                }
                spans
            }
        };

        if line_is_to_be_discarded {
            content_spans = content_spans
                .into_iter()
                .map(|span| span.crossed_out())
                .collect();
        }

        match &self.mode {
            Mode::InlineReword(inline_reword_mode) => {
                if is_selected {
                    match inline_reword_mode {
                        InlineRewordMode::Commit { .. } => {
                            if let StatusOutputContent::Commit(commit_content) = content {
                                line.extend(commit_content.sha.iter().cloned());
                            }
                        }
                        InlineRewordMode::Branch { textarea, .. } => {
                            if let StatusOutputContent::Branch(branch_content) = content {
                                line.extend(branch_content.id.iter().cloned());
                                line.extend(branch_content.decoration_start.iter().cloned());

                                let len = textarea
                                    .lines()
                                    .first()
                                    .map(|line| line.width())
                                    .unwrap_or(0);
                                line.push_span(Span::raw(" ".repeat(len + 1)));

                                line.extend(branch_content.decoration_end.iter().cloned());
                                line.extend(branch_content.suffix.iter().cloned());
                            }
                        }
                    }
                } else {
                    line.extend(content_spans);
                }
            }
            Mode::Normal
            | Mode::Branch
            | Mode::Details
            | Mode::Move(..)
            | Mode::Command(..)
            | Mode::Rub(..)
            | Mode::Commit(..) => {
                if is_selectable_in_mode(tui_line, &self.mode, self.flags.show_files) {
                    line.extend(content_spans);
                } else {
                    line.extend(
                        content_spans
                            .into_iter()
                            .map(|span| span.style(Style::default().dim())),
                    );
                }
            }
        }

        let is_conflicted_in_dry_run_workspace = data
            .cli_id()
            .and_then(|id| {
                if let CliId::Commit { commit_id, .. } = &**id {
                    Some(*commit_id)
                } else {
                    None
                }
            })
            .is_some_and(|commit| self.dry_run_workspace.is_commit_conflicted(commit));
        if is_conflicted_in_dry_run_workspace {
            line.push_span(Span::raw(" {will conflict}").red());
        }

        if is_selected {
            line = line.bg(*CURSOR_BG);
        }

        if is_selected {
            match &self.mode {
                Mode::Commit(commit_mode)
                    if matches!(data, StatusOutputLineData::Commit { .. }) =>
                {
                    let mut extension_line = Line::default().bg(*CURSOR_BG);
                    extend_connector_spans(
                        connector.as_deref().unwrap_or_default(),
                        match commit_mode.insert_side {
                            InsertSide::Above => ExtensionDirection::Above,
                            InsertSide::Below => ExtensionDirection::Below,
                        },
                        &mut extension_line,
                    );
                    self.render_commit_labels_for_selected_line(
                        data,
                        commit_mode,
                        &mut extension_line,
                    );
                    return match commit_mode.insert_side {
                        InsertSide::Above => StatusListItem::Double(extension_line, line),
                        InsertSide::Below => StatusListItem::Double(line, extension_line),
                    };
                }
                Mode::Move(move_mode) => {
                    if let StatusOutputLineData::Commit { cli_id: target, .. } = data
                        && *move_mode.source != **target
                    {
                        let mut extension_line = Line::default().bg(*CURSOR_BG);
                        extend_connector_spans(
                            connector.as_deref().unwrap_or_default(),
                            match move_mode.insert_side {
                                InsertSide::Above => ExtensionDirection::Above,
                                InsertSide::Below => ExtensionDirection::Below,
                            },
                            &mut extension_line,
                        );
                        self.render_move_labels_for_selected_line(
                            data,
                            move_mode,
                            &mut extension_line,
                        );
                        return match move_mode.insert_side {
                            InsertSide::Above => StatusListItem::Double(extension_line, line),
                            InsertSide::Below => StatusListItem::Double(line, extension_line),
                        };
                    }
                }
                Mode::Commit(..)
                | Mode::Branch
                | Mode::Normal
                | Mode::Details
                | Mode::Rub(..)
                | Mode::InlineReword(..)
                | Mode::Command(..) => {}
            }
        }

        StatusListItem::Single(line)
    }

    fn render_rub_inline_labels_for_selected_line(
        &self,
        data: &StatusOutputLineData,
        source: &RubSource,
        line: &mut Line<'static>,
    ) {
        let Some(target) = data.cli_id() else {
            return;
        };

        if source == &**target {
            line.extend([source_span(), Span::raw(" ")]);
        }

        let display = match source {
            RubSource::CliId(source) => {
                Cow::Borrowed(rub::rub_operation_display(source, target).unwrap_or("invalid"))
            }
            RubSource::CommittedHunk(hunk) => Cow::Borrowed(
                rub_from_detail_view::rub_operation_display(hunk, target).unwrap_or("invalid"),
            ),
        };
        line.extend([
            Span::raw("<< ").mode_colors(&self.mode),
            Span::raw(display).mode_colors(&self.mode),
            Span::raw(" >>").mode_colors(&self.mode),
            Span::raw(" "),
        ]);
    }

    fn render_commit_labels_for_selected_line(
        &self,
        data: &StatusOutputLineData,
        mode: &CommitMode,
        line: &mut Line<'static>,
    ) {
        let Some(target) = data.cli_id() else {
            return;
        };

        if *mode.source == **target {
            line.extend([source_span(), Span::raw(" ")]);
            line.extend(
                [
                    Span::raw("<< ").mode_colors(&self.mode),
                    Span::raw(NOOP).mode_colors(&self.mode),
                ]
                .into_iter()
                .chain(
                    mode.empty_message
                        .then(|| Span::raw(" (empty message)").mode_colors(&self.mode)),
                )
                .chain([Span::raw(" >>").mode_colors(&self.mode), Span::raw(" ")]),
            );
        } else if let Some(display) = commit_operation_display(data, mode) {
            line.extend(
                [
                    Span::raw("<< ").mode_colors(&self.mode),
                    Span::raw(display).mode_colors(&self.mode),
                ]
                .into_iter()
                .chain(
                    mode.empty_message
                        .then(|| Span::raw(" (empty message)").mode_colors(&self.mode)),
                )
                .chain([Span::raw(" >>").mode_colors(&self.mode), Span::raw(" ")]),
            );
        }
    }

    fn render_move_labels_for_selected_line(
        &self,
        data: &StatusOutputLineData,
        mode: &MoveMode,
        line: &mut Line<'static>,
    ) {
        if data.cli_id().is_some_and(|target| *mode.source == **target) {
            line.extend([source_span(), Span::raw(" ")]);
            line.extend([
                Span::raw("<< ").mode_colors(&self.mode),
                Span::raw(NOOP).mode_colors(&self.mode),
                Span::raw(" >>").mode_colors(&self.mode),
                Span::raw(" "),
            ]);
        } else if let Some(display) = move_operation_display(data, mode) {
            line.extend([
                Span::raw("<< ").mode_colors(&self.mode),
                Span::raw(display).mode_colors(&self.mode),
                Span::raw(" >>").mode_colors(&self.mode),
                Span::raw(" "),
            ]);
        }
    }

    fn render_branch_labels_for_selected_line(
        &self,
        data: &StatusOutputLineData,
        line: &mut Line<'static>,
    ) {
        let Some(display) = branch_operation_display(data) else {
            return;
        };
        line.extend([
            Span::raw("<< ").mode_colors(&self.mode),
            Span::raw(display).mode_colors(&self.mode),
            Span::raw(" >>").mode_colors(&self.mode),
            Span::raw(" "),
        ]);
    }

    fn render_hotbar(&self, area: Rect, frame: &mut Frame) {
        let mode_span = Span::raw(format!(
            "  {}  ",
            match self.mode {
                Mode::Normal => "normal",
                Mode::Rub(..) => "rub",
                Mode::InlineReword(..) => "reword",
                Mode::Command(..) => "command",
                Mode::Commit(..) => "commit",
                Mode::Move(..) => "move",
                Mode::Branch => "branch",
                Mode::Details => "details",
            }
        ))
        .mode_colors(&self.mode);

        let layout = Layout::horizontal([
            Constraint::Length(mode_span.width() as _),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

        frame.render_widget(mode_span, layout[0]);

        frame.render_widget(" ", layout[1]);

        match &self.mode {
            Mode::Normal
            | Mode::Branch
            | Mode::Details
            | Mode::Rub(..)
            | Mode::Commit(..)
            | Mode::Move(..)
            | Mode::InlineReword(..) => {
                let mut line = Line::default();
                let mut key_binds_iter = self
                    .active_key_binds()
                    .iter_key_binds_available_in_mode(&self.mode)
                    .filter(|key_bind| !key_bind.hide_from_hotbar())
                    .peekable();
                while let Some(key_bind) = key_binds_iter.next() {
                    line.extend([
                        Span::styled(key_bind.chord_display(), Style::default().blue()),
                        Span::raw(" "),
                        Span::styled(key_bind.short_description(), Style::default().dim()),
                    ]);

                    if key_binds_iter.peek().is_some() {
                        line.push_span(Span::styled(" • ", Style::default().dim()));
                    }
                }

                frame.render_widget(line, layout[2]);
            }
            Mode::Command(CommandMode { textarea }) => {
                let command_layout =
                    Layout::horizontal([Constraint::Length(4), Constraint::Min(1)])
                        .split(layout[2]);

                frame.render_widget("but ", command_layout[0]);
                frame.render_widget(&**textarea, command_layout[1]);
            }
        }
    }

    /// Renders transient toasts stacked in the content area.
    fn render_toasts(&self, area: Rect, frame: &mut Frame) {
        toast::render_toasts(frame, area, &self.toasts);
    }

    fn render_inline_reword(&self, area: Rect, frame: &mut Frame) {
        let inline_reword_mode = if let Mode::InlineReword(inline_reword_mode) = &self.mode {
            inline_reword_mode
        } else {
            return;
        };

        let selected_idx = self.cursor.index();
        let Some(selected_rows) = self.selected_row_range() else {
            return;
        };
        if selected_rows.start < self.scroll_top {
            return;
        }
        let idx = selected_rows.start - self.scroll_top;
        if idx >= area.height as usize {
            return;
        }
        let Some(line) = self.status_lines.get(selected_idx) else {
            return;
        };

        match inline_reword_mode {
            InlineRewordMode::Commit { textarea, .. } => {
                let StatusOutputLineData::Commit { .. } = &line.data else {
                    return;
                };
                let Some(connector) = &line.connector else {
                    return;
                };
                let StatusOutputContent::Commit(commit_content) = &line.content else {
                    return;
                };
                let connector_and_prefix = connector
                    .iter()
                    .chain(&commit_content.sha)
                    .map(|span| span.width() as u16)
                    .sum::<u16>();
                let padding = 1;

                let start_x = connector_and_prefix + padding;
                let x = area.x.saturating_add(start_x);
                let width = area.right().saturating_sub(x);
                let area = Rect::new(x, area.y.saturating_add(idx as u16), width, 1);
                frame.render_widget(&**textarea, area);
            }
            InlineRewordMode::Branch { textarea, .. } => {
                let StatusOutputLineData::Branch { .. } = &line.data else {
                    return;
                };
                let Some(connector) = &line.connector else {
                    return;
                };
                let StatusOutputContent::Branch(branch_content) = &line.content else {
                    return;
                };

                let connector_and_prefix = connector
                    .iter()
                    .chain(&branch_content.id)
                    .chain(&branch_content.decoration_start)
                    .map(|span| span.width() as u16)
                    .sum::<u16>();

                let padding = 0;

                let start_x = connector_and_prefix + padding;
                let x = area.x.saturating_add(start_x);
                let width = area.right().saturating_sub(x);
                let area = Rect::new(x, area.y.saturating_add(idx as u16), width, 1);
                frame.render_widget(&**textarea, area);
            }
        }
    }

    fn render_debug(&self, area: Rect, frame: &mut Frame) {
        let renders = once(ListItem::new("FPS").black().on_blue()).chain(once(ListItem::new(
            format!("{} FPS ({} renders)", self.fps.fps(), self.renders),
        )));

        let dry_run_workspace = format!("{:#?}", self.dry_run_workspace);
        let dry_run_workspace = once(ListItem::new("Dry run workspace").black().on_blue()).chain(
            dry_run_workspace
                .lines()
                .take(100)
                .map(|line| ListItem::new(line.to_owned())),
        );

        let details_selection = format!("{:#?}", self.details.selection());
        let details_selection = once(ListItem::new("Details selection").black().on_blue()).chain(
            details_selection
                .lines()
                .take(100)
                .map(|line| ListItem::new(line.to_owned())),
        );

        let status_selection = format!("{:#?}", self.cursor.selected_line(&self.status_lines));
        let status_selection = once(ListItem::new("Status selection").black().on_blue()).chain(
            status_selection
                .lines()
                .take(100)
                .map(|line| ListItem::new(line.to_owned())),
        );

        let list = List::new(
            renders
                .chain(once(ListItem::new("")))
                .chain(dry_run_workspace)
                .chain(once(ListItem::new("")))
                .chain(details_selection)
                .chain(once(ListItem::new("")))
                .chain(status_selection),
        );

        frame.render_widget(list, area);
    }

    fn update_status_width_percentage(&mut self, new: u16, terminal_area: Rect) {
        if !self.details.is_visible() {
            return;
        }

        self.status_width_percentage = new.clamp(
            100 - DETAILS_MAX_SIZE_PERCENTAGE,
            100 - DETAILS_MIN_SIZE_PERCENTAGE,
        );

        let details_viewport = self.details_viewport(terminal_area);
        self.details.ensure_selection_visible(details_viewport);
    }
}

fn event_to_messages(ev: Event, key_binds: &KeyBinds, mode: &Mode, messages: &mut Vec<Message>) {
    match ev {
        Event::Key(key) => {
            let mut handled = false;
            for key_bind in key_binds.iter_key_binds_available_in_mode(mode) {
                if key_bind.matches(&key) {
                    messages.push(key_bind.message());
                    handled = true;
                }
            }

            if !handled {
                match mode {
                    Mode::InlineReword(..) => {
                        messages.push(Message::Reword(RewordMessage::InlineInput(ev)));
                    }
                    Mode::Command(..) => {
                        messages.push(Message::Command(CommandMessage::Input(ev)));
                    }
                    Mode::Normal
                    | Mode::Branch
                    | Mode::Details
                    | Mode::Rub(..)
                    | Mode::Commit(..)
                    | Mode::Move(..) => {}
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
            Mode::Normal
            | Mode::Branch
            | Mode::Details
            | Mode::Rub(..)
            | Mode::Commit(..)
            | Mode::Move(..) => {
                messages.push(Message::JustRender);
            }
        },
        Event::FocusGained => {
            messages.push(Message::Reload(None));
        }
        Event::FocusLost | Event::Mouse(_) => {}
    }
}

#[derive(Debug, Clone)]
enum Message {
    // Lifecycle
    JustRender,
    Quit,
    EnterNormalMode,
    Reload(Option<SelectAfterReload>),
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

    // Cursor movement
    MoveCursorUp,
    MoveCursorDown,
    MoveCursorPreviousSection,
    MoveCursorNextSection,
    SelectUnassigned,

    // Features
    Commit(CommitMessage),
    Rub(RubMessage),
    Reword(RewordMessage),
    Command(CommandMessage),
    Files(FilesMessage),
    Move(MoveMessage),
    Branch(BranchMessage),
    Details(DetailsMessage),
    EnterDetailsMode,
    LeaveDetailsMode,

    // Utilities
    CopySelection,
    #[expect(clippy::type_complexity)]
    RunAfterConfirmation(
        DebugAsType<
            Rc<
                RefCell<
                    Option<
                        Box<
                            dyn FnOnce(
                                &mut App,
                                &mut Context,
                                &mut Vec<Message>,
                            ) -> anyhow::Result<()>,
                        >,
                    >,
                >,
            >,
        >,
    ),
    RegisterMessageOnDrop(Rc<Receiver<Message>>),
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
    pub(super) fn with_one_frame_delay(self) -> Self {
        Self::WithOneFrameDelay(Box::new(self))
    }

    /// Send another message only if handling the first succeeds.
    #[expect(dead_code)]
    pub(super) fn and_then(self, other: Self) -> Self {
        Self::AndThen {
            lhs: Box::new(self),
            rhs: Box::new(other),
        }
    }
}

#[derive(Debug, Clone)]
enum RubMessage {
    Start,
    StartWithSource {
        source: RubSource,
        unlock_details: Option<MessageOnDrop>,
    },
    Confirm,
}

#[derive(Debug, Clone)]
enum RewordMessage {
    WithEditor,
    InlineStart,
    InlineInput(Event),
    InlineConfirm,
}

#[derive(Debug, Clone)]
enum CommandMessage {
    Start,
    Input(Event),
    Confirm,
}

#[derive(Debug, Clone)]
enum CommitMessage {
    CreateEmpty,
    Start,
    SetInsertSide(InsertSide),
    ToggleEmptyMessage,
    Confirm,
}

#[derive(Debug, Clone)]
enum MoveMessage {
    Start,
    SetInsertSide(InsertSide),
    Confirm,
}

#[derive(Debug, Clone)]
enum BranchMessage {
    Start,
    New,
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

fn commit_operation_display(
    data: &StatusOutputLineData,
    mode: &CommitMode,
) -> Option<&'static str> {
    match data {
        StatusOutputLineData::Branch { cli_id } => {
            if let Some(stack_scope) = mode.scope_to_stack
                && let Some(stack_id) = cli_id.stack_id()
                && stack_scope != stack_id
            {
                // don't allow selecting branches outside the scoped stack
                None
            } else {
                Some("commit to branch")
            }
        }
        StatusOutputLineData::Commit { stack_id, .. } => {
            if let Some(stack_scope) = mode.scope_to_stack
                && Some(stack_scope) != *stack_id
            {
                // don't allow selecting commits outside the scoped stack
                None
            } else {
                match mode.insert_side {
                    InsertSide::Above => Some("insert commit above"),
                    InsertSide::Below => Some("insert commit below"),
                }
            }
        }
        StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UnassignedChanges { .. }
        | StatusOutputLineData::UnassignedFile { .. }
        | StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::MergeBase
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    }
}

fn move_operation_display(data: &StatusOutputLineData, mode: &MoveMode) -> Option<&'static str> {
    match &*mode.source {
        MoveSource::Commit { .. } => match data {
            StatusOutputLineData::Commit { .. } => match mode.insert_side {
                InsertSide::Above => Some("move commit above"),
                InsertSide::Below => Some("move commit below"),
            },
            StatusOutputLineData::Branch { .. } => Some("move commit to branch"),
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
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
            | StatusOutputLineData::NoAssignmentsUnstaged => None,
        },
        MoveSource::Branch { .. } => match data {
            StatusOutputLineData::Branch { .. } => Some("move branch"),
            StatusOutputLineData::MergeBase => Some("unstack branch"),
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::Connector
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
            | StatusOutputLineData::NoAssignmentsUnstaged => None,
        },
    }
}

fn branch_operation_display(data: &StatusOutputLineData) -> Option<&'static str> {
    match data {
        StatusOutputLineData::Branch { .. } | StatusOutputLineData::MergeBase => Some("target"),
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UnassignedChanges { .. }
        | StatusOutputLineData::UnassignedFile { .. }
        | StatusOutputLineData::Commit { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    }
}

fn source_span() -> Span<'static> {
    Span::raw("<< source >>").mode_colors(&Mode::Normal)
}

trait SpanExt {
    fn mode_colors(self, mode: &Mode) -> Self;
}

impl SpanExt for Span<'_> {
    fn mode_colors(self, mode: &Mode) -> Self {
        self.fg(mode.fg()).bg(mode.bg())
    }
}

enum StatusListItem {
    Single(Line<'static>),
    Double(Line<'static>, Line<'static>),
}

impl IntoIterator for StatusListItem {
    type Item = ListItem<'static>;
    type IntoIter =
        Either<std::iter::Once<ListItem<'static>>, std::array::IntoIter<ListItem<'static>, 2>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            StatusListItem::Single(line) => Either::Left(once(ListItem::new(line))),
            StatusListItem::Double(line1, line2) => {
                Either::Right([ListItem::new(line1), ListItem::new(line2)].into_iter())
            }
        }
    }
}

enum MoveTarget<'a> {
    Branch { name: &'a str },
    Commit { commit_id: gix::ObjectId },
    MergeBase,
}

fn run_after_confirmation_msg<F>(f: F) -> Message
where
    F: FnOnce(&mut App, &mut Context, &mut Vec<Message>) -> anyhow::Result<()> + 'static,
{
    Message::RunAfterConfirmation(DebugAsType(Rc::new(RefCell::new(Some(Box::new(
        move |app, ctx, messages| f(app, ctx, messages),
    ))))))
}

struct StatusLayout {
    status_area: Rect,
    details_area: Option<Rect>,
}
