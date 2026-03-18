use std::{
    ffi::OsString,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Context as _;
use bstr::BString;
use but_api::{
    commit::{insert_blank::commit_insert_blank, json::RelativeTo},
    diff::ComputeLineStats,
};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use crossterm::event::{self, Event};
use gitbutler_operating_modes::OperatingMode;
use ratatui::{
    Frame,
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
};
use ratatui_textarea::{CursorMove, TextArea};

use crate::{
    CliId,
    args::OutputFormat,
    command::legacy::{
        ShowDiffInEditor,
        commit_message_prep::{
            commit_message_has_multiple_lines, normalize_commit_message,
            should_update_commit_message,
        },
        reword::get_commit_message_from_editor,
        rub::{RubOperation, route_operation},
        status::{
            StatusFlags, StatusOutput, StatusOutputLine, build_status_context, build_status_output,
            tui::{
                cursor::Cursor,
                key_bind::{KeyBinds, default_key_binds},
            },
        },
    },
    tui::{CrosstermTerminalGuard, TerminalGuard},
    utils::OutputChannel,
};

use super::output::{StatusOutputContent, StatusOutputLineData};

mod cursor;
mod key_bind;

#[cfg(test)]
mod tests;

const CURSOR_BG: Color = Color::Rgb(69, 71, 90);

pub(super) async fn render_tui(
    ctx: &mut Context,
    out: &mut OutputChannel,
    mode: &OperatingMode,
    flags: StatusFlags,
    status_lines: Vec<StatusOutputLine>,
    debug: bool,
) -> anyhow::Result<Vec<StatusOutputLine>> {
    let mut terminal_guard = CrosstermTerminalGuard::new(true)?;

    let mut app = App::new(status_lines, flags, debug);

    let mut messages = Vec::new();

    // second buffer so we can send messages from `App::handle_message`
    let mut other_messages = Vec::new();

    let event_polling = CrosstermEventPolling;

    loop {
        render_loop_once(
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

        if app.should_quit {
            break;
        }
    }

    Ok(app.status_lines)
}

/// Trait for abstracting event polling so we can hardcode events in tests.
trait EventPolling {
    type Error: std::error::Error + Send + Sync + 'static;

    fn poll(self) -> Result<impl IntoIterator<Item = Event>, Self::Error>;
}

/// An [`EventPolling`] implementation that polls events for real using crossterm.
#[derive(Copy, Clone)]
struct CrosstermEventPolling;

impl EventPolling for CrosstermEventPolling {
    type Error = std::io::Error;

    fn poll(self) -> Result<impl IntoIterator<Item = Event>, Self::Error> {
        if event::poll(Duration::from_millis(30))? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
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
    // poll events
    for event in event_polling.poll()? {
        event_to_messages(event, &app.key_binds, &app.mode, messages);
    }

    // handle messages
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

    // dismiss errors
    let now = Instant::now();
    let errors_before = app.errors.len();
    app.errors.retain(|err| err.dismiss_at > now);
    if app.errors.len() != errors_before {
        app.should_render = true;
    }

    // render
    if std::mem::take(&mut app.should_render) {
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
    mode: Mode,
    key_binds: KeyBinds,
    debug_enabled: bool,
    errors: Vec<AppError>,
    renders: u64,
}

impl App {
    fn new(status_lines: Vec<StatusOutputLine>, flags: StatusFlags, debug: bool) -> Self {
        let cursor = Cursor::new(&status_lines);

        Self {
            status_lines,
            flags,
            cursor,
            should_quit: false,
            should_render: true,
            mode: Mode::default(),
            key_binds: default_key_binds(),
            debug_enabled: debug,
            errors: Vec::new(),
            renders: 0,
        }
    }

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

        match msg {
            Message::Quit => {
                self.should_quit = true;
            }
            Message::Noop => {}
            Message::MoveCursorUp => self.cursor.move_up(&self.status_lines, &self.mode),
            Message::MoveCursorDown => self.cursor.move_down(&self.status_lines, &self.mode),
            Message::MoveCursorPreviousSection => self
                .cursor
                .move_previous_section(&self.status_lines, &self.mode),
            Message::MoveCursorNextSection => self
                .cursor
                .move_next_section(&self.status_lines, &self.mode),
            Message::StartRub => self.handle_start_rub(),
            Message::EnterNormalMode => {
                self.mode = Mode::Normal;
            }
            Message::ToggleFiles => self.handle_toggle_files(messages),
            Message::ConfirmRub => self.handle_confirm_rub(ctx, messages)?,
            Message::Reload(select_after_reload) => {
                self.handle_reload(ctx, out, mode, select_after_reload)
                    .await?
            }
            Message::ShowError(err) => self.handle_show_error(err, messages),
            Message::CreateEmptyCommit => self.handle_create_empty_commit(ctx, messages)?,
            Message::RewordWithEditor => {
                self.handle_reword_with_editor(ctx, terminal_guard, messages)?;
            }
            Message::StartRewordInline => self.handle_start_reword_inline(ctx, messages)?,
            Message::RewordInlineInput(ev) => self.handle_reword_inline_input(ev),
            Message::ConfirmInlineReword => self.handle_confirm_inline_reword(ctx, messages)?,
            Message::EnterCommandMode => self.handle_enter_command_mode(),
            Message::CommandInput(ev) => self.handle_command_input(ev),
            Message::RunCommand => self.handle_run_command(terminal_guard, out, messages)?,
        }

        Ok(())
    }

    /// Handles transitioning into rub mode and selecting a valid rub target.
    fn handle_start_rub(&mut self) {
        if !matches!(self.mode, Mode::Normal) {
            return;
        }

        let Some(selected_line) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };

        let Some(cli_id) = selected_line.data.cli_id() else {
            return;
        };

        let available_targets = self
            .status_lines
            .iter()
            .filter_map(|line| line.data.cli_id())
            .filter(|target| *target == cli_id || route_operation(cli_id, target).is_some())
            .cloned()
            .collect::<Vec<_>>();

        self.mode = Mode::Rub {
            source: Arc::clone(cli_id),
            available_targets,
        };

        if self
            .cursor
            .selected_line(&self.status_lines)
            .is_some_and(|line| cursor::is_selectable_in_mode(line, &self.mode))
        {
            return;
        }

        let previous_cursor = self.cursor;
        self.cursor.move_down(&self.status_lines, &self.mode);
        if self.cursor == previous_cursor {
            self.cursor.move_up(&self.status_lines, &self.mode);
        }
    }

    /// Handles toggling file visibility and requests a status reload.
    fn handle_toggle_files(&mut self, messages: &mut Vec<Message>) {
        self.flags.show_files = !self.flags.show_files;
        messages.push(Message::Reload(None));
    }

    /// Handles confirming the currently selected rub operation.
    fn handle_confirm_rub(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if let Mode::Rub { source, .. } = &self.mode
            && let Some(selected_line) = self.cursor.selected_line(&self.status_lines)
            && let Some(target) = selected_line.data.cli_id()
            && let Some(operation) = route_operation(source, target)
        {
            with_noop_output(|out| operation.execute(ctx, out))?;
        }

        messages.extend([Message::EnterNormalMode, Message::Reload(None)]);
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
        {
            let meta = ctx.meta()?;
            let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
            ws.refresh_from_head(&repo, &meta)?;
        }

        let mut new_lines = Vec::new();

        build_status_context(
            ctx,
            out,
            mode,
            self.flags,
            crate::command::legacy::status::StatusRenderMode::Tui {
                debug: self.debug_enabled,
            },
        )
        .await
        .and_then(|status_ctx| {
            build_status_output(
                ctx,
                &status_ctx,
                &mut StatusOutput::Buffer {
                    lines: &mut new_lines,
                },
            )
        })?;

        self.cursor = if let Some(select_after_reload) = select_after_reload {
            match select_after_reload {
                SelectAfterReload::Commit(commit_id) => Cursor::select(commit_id, &new_lines),
            }
        } else {
            self.cursor
                .selection_cli_id_for_reload(&self.status_lines, self.flags.show_files)
                .and_then(|previously_selected_cli_id| {
                    Cursor::restore(previously_selected_cli_id, &new_lines)
                })
        }
        .unwrap_or_else(|| Cursor::new(&new_lines));

        self.status_lines = new_lines;
        Ok(())
    }

    /// Handles showing a transient UI error.
    fn handle_show_error(&mut self, err: Arc<anyhow::Error>, messages: &mut Vec<Message>) {
        self.errors.push(AppError {
            inner: err,
            dismiss_at: Instant::now() + Duration::from_secs(5),
        });

        // ensure we always enter normal mode when something does wrong
        // so we don't get stuck in whatever mode we were in previously
        messages.push(Message::EnterNormalMode);
    }

    /// Handles creating an empty commit relative to the current selection.
    fn handle_create_empty_commit(
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

                let full_name = {
                    let repo = ctx.repo.get()?;
                    let reference = repo.find_reference(name)?;
                    reference.name().to_owned()
                };

                let commit_result =
                    commit_insert_blank(ctx, RelativeTo::Reference(full_name), InsertSide::Below)?;

                messages.push(Message::Reload(Some(SelectAfterReload::Commit(
                    commit_result.new_commit,
                ))));
            }
            StatusOutputLineData::Commit { cli_id } => {
                let CliId::Commit { commit_id, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_result =
                    commit_insert_blank(ctx, RelativeTo::Commit(*commit_id), InsertSide::Above)?;

                messages.push(Message::Reload(Some(SelectAfterReload::Commit(
                    commit_result.new_commit,
                ))));
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UnstagedChanges { .. }
            | StatusOutputLineData::UnstagedFile { .. }
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

        let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
        let current_message = commit_details.commit.inner.message.to_string();

        let _suspend_guard = terminal_guard.suspend()?;
        let new_message = get_commit_message_from_editor(
            ctx,
            commit_details,
            current_message.clone(),
            ShowDiffInEditor::Unspecified,
        )?;

        let Some(new_message) = new_message else {
            return Ok(());
        };

        if !should_update_commit_message(&current_message, &new_message) {
            return Ok(());
        }

        let reword_result =
            but_api::commit::reword::commit_reword_only(ctx, commit_id, BString::from(new_message))
                .with_context(|| format!("failed to reword {}", commit_id.to_hex_with_len(7)))?;

        messages.push(Message::Reload(Some(SelectAfterReload::Commit(
            reword_result.new_commit,
        ))));
        Ok(())
    }

    /// Handles entering inline reword mode for single-line commit messages.
    fn handle_start_reword_inline(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if !matches!(self.mode, Mode::Normal) {
            return Ok(());
        }

        let Some(commit_id) = self.selected_commit_id() else {
            return Ok(());
        };

        let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
        let current_message = commit_details.commit.inner.message.to_string();

        if commit_message_has_multiple_lines(&current_message) {
            messages.push(Message::RewordWithEditor);
            return Ok(());
        }

        let first_line = current_message.lines().next().unwrap_or("").to_string();
        let mut textarea = TextArea::from([first_line]);
        textarea.set_cursor_line_style(Style::default());
        textarea.move_cursor(CursorMove::End);

        self.mode = Mode::InlineReword {
            commit_id,
            textarea: Box::new(textarea),
        };

        Ok(())
    }

    /// Handles key input while inline reword mode is active.
    fn handle_reword_inline_input(&mut self, ev: Event) {
        if let Mode::InlineReword { textarea, .. } = &mut self.mode {
            textarea.input(ev);
        }
    }

    /// Handles confirming an inline commit reword.
    fn handle_confirm_inline_reword(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::InlineReword {
            commit_id,
            textarea,
        } = &self.mode
        else {
            messages.push(Message::EnterNormalMode);
            return Ok(());
        };

        let commit_details = but_api::diff::commit_details(ctx, *commit_id, ComputeLineStats::No)?;
        let current_message = commit_details.commit.inner.message.to_string();
        let new_subject = textarea
            .lines()
            .first()
            .map(std::string::String::as_str)
            .unwrap_or("");
        let new_message = normalize_commit_message(new_subject).to_string();

        if !should_update_commit_message(&current_message, &new_message) {
            messages.push(Message::EnterNormalMode);
            return Ok(());
        }

        let reword_result = but_api::commit::reword::commit_reword_only(
            ctx,
            *commit_id,
            BString::from(new_message),
        )
        .with_context(|| format!("failed to reword {}", commit_id.to_hex_with_len(7)))?;

        messages.extend([
            Message::EnterNormalMode,
            Message::Reload(Some(SelectAfterReload::Commit(reword_result.new_commit))),
        ]);

        Ok(())
    }

    fn handle_enter_command_mode(&mut self) {
        if !matches!(self.mode, Mode::Normal) {
            return;
        }

        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.move_cursor(CursorMove::End);

        self.mode = Mode::Command {
            textarea: Box::new(textarea),
        };
    }

    fn handle_command_input(&mut self, ev: Event) {
        if let Mode::Command { textarea } = &mut self.mode {
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
        let Mode::Command { textarea } = &self.mode else {
            messages.push(Message::EnterNormalMode);
            return Ok(());
        };

        let Some(input) = textarea.lines().first() else {
            return Ok(());
        };

        let binary_path = std::env::current_exe().unwrap_or_default();
        let args = shell_words::split(input)?.into_iter().map(OsString::from);

        let mut cmd = Command::new(binary_path);
        cmd.args(args);

        let _suspend_guard = terminal_guard.suspend()?;
        cmd.spawn()?.wait()?;

        let mut input_channel = out
            .prepare_for_terminal_input()
            .context("failed to prepare input")?;

        input_channel.prompt_single_line("\npress enter to continue...")?;

        drop(_suspend_guard);

        messages.extend([Message::EnterNormalMode, Message::Reload(None)]);

        Ok(())
    }

    /// Returns the currently selected commit id when the selected line is a commit.
    fn selected_commit_id(&self) -> Option<gix::ObjectId> {
        let selection = self.cursor.selected_line(&self.status_lines)?;

        let StatusOutputLineData::Commit { cli_id } = &selection.data else {
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

        self.render_status(content_layout[0], frame);
        self.render_hotbar(content_layout[1], frame);
    }

    fn render_status(&self, area: Rect, frame: &mut Frame) {
        let (content_area, debug_area) = if self.debug_enabled {
            let layout =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(area);
            (layout[0], Some(layout[1]))
        } else {
            (area, None)
        };

        let items = self
            .cursor
            .iter_lines(&self.status_lines)
            .map(|(tui_line, is_selected)| self.render_status_list_item(tui_line, is_selected));
        let list = List::new(items);

        frame.render_widget(list, content_area);

        self.render_inline_reword(content_area, frame);

        self.render_errors(content_area, frame);

        if let Some(debug_area) = debug_area {
            self.render_debug(debug_area, frame);
        }
    }

    fn render_status_list_item(
        &self,
        tui_line: &StatusOutputLine,
        is_selected: bool,
    ) -> ListItem<'_> {
        let StatusOutputLine {
            connector,
            content,
            data,
        } = tui_line;

        let mut line = Line::default();

        if let Some(connector) = connector {
            line.extend(connector.clone());
        }

        if is_selected {
            match &self.mode {
                Mode::Normal | Mode::InlineReword { .. } | Mode::Command { .. } => {}
                Mode::Rub {
                    source,
                    available_targets: _,
                } => {
                    if let Some(target) = data.cli_id() {
                        if target == source {
                            line.extend([
                                Span::raw("<< source >>").black().on_green(),
                                Span::raw(" "),
                            ]);
                        }

                        let rub_operation_display =
                            rub_operation_display(source, target).unwrap_or("invalid");
                        line.extend([
                            Span::raw("<< ").black().on_blue(),
                            Span::raw(rub_operation_display).black().on_blue(),
                            Span::raw(" >>").black().on_blue(),
                            Span::raw(" "),
                        ]);
                    }
                }
            }
        } else {
            match &self.mode {
                Mode::Normal | Mode::InlineReword { .. } | Mode::Command { .. } => {}
                Mode::Rub {
                    source,
                    available_targets: _,
                } => {
                    if let Some(cli_id) = data.cli_id()
                        && cli_id == source
                    {
                        line.extend([Span::raw("<< source >>").black().on_green(), Span::raw(" ")]);
                    }
                }
            }
        }

        let content_spans = match content {
            StatusOutputContent::Plain(spans) => spans.clone(),
            StatusOutputContent::Commit(commit_content) => {
                let mut spans = Vec::new();
                spans.extend(commit_content.sha.iter().cloned());
                spans.extend(commit_content.author.iter().cloned());
                spans.extend(commit_content.message.iter().cloned());
                spans.extend(commit_content.suffix.iter().cloned());
                spans
            }
        };

        match &self.mode {
            Mode::Normal | Mode::Command { .. } => {
                line.extend(content_spans);
            }
            Mode::InlineReword { .. } => {
                if is_selected {
                    if let StatusOutputContent::Commit(commit_content) = content {
                        line.extend(commit_content.sha.iter().cloned());
                    }
                } else {
                    line.extend(content_spans);
                }
            }
            Mode::Rub {
                source: _,
                available_targets,
            } => {
                let can_rub_here = if let Some(cli_id) = data.cli_id() {
                    available_targets.contains(cli_id)
                } else {
                    false
                };
                if can_rub_here {
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

        if is_selected && !matches!(self.mode, Mode::Command { .. }) {
            line = line.style(Style::default().bg(CURSOR_BG));
        }

        ListItem::new(line)
    }

    fn render_hotbar(&self, area: Rect, frame: &mut Frame) {
        let mode_span = match self.mode {
            Mode::Normal => Span::styled("  normal  ", Style::default().black().on_green()),
            Mode::Rub { .. } => Span::styled("  rub  ", Style::default().black().on_magenta()),
            Mode::InlineReword { .. } => {
                Span::styled("  reword  ", Style::default().black().on_blue())
            }
            Mode::Command { .. } => {
                Span::styled("  command  ", Style::default().black().on_yellow())
            }
        };

        let layout = Layout::horizontal([
            Constraint::Length(mode_span.width() as _),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

        frame.render_widget(mode_span, layout[0]);

        frame.render_widget(" ", layout[1]);

        match &self.mode {
            Mode::Normal | Mode::Rub { .. } | Mode::InlineReword { .. } => {
                let mut line = Line::default();
                let mut key_binds_iter = self
                    .key_binds
                    .iter_key_binds_available_in_mode(&self.mode)
                    .filter(|key_bind| !key_bind.hide())
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
            Mode::Command { textarea } => {
                let command_layout =
                    Layout::horizontal([Constraint::Length(4), Constraint::Min(1)])
                        .split(layout[2]);

                frame.render_widget("but ", command_layout[0]);
                frame.render_widget(&**textarea, command_layout[1]);
            }
        }
    }

    fn render_errors(&self, area: Rect, frame: &mut Frame) {
        for (idx, err) in self.errors.iter().rev().enumerate() {
            let formatted_err = format!("{:#}", err.inner);
            render_error_popup(
                frame,
                area,
                PopupMargin {
                    right: 1 + idx as u16,
                    bottom: idx as _,
                },
                &formatted_err,
            );
        }
    }

    fn render_inline_reword(&self, area: Rect, frame: &mut Frame) {
        let Mode::InlineReword { textarea, .. } = &self.mode else {
            return;
        };
        let Some((idx, (line, _))) = self
            .cursor
            .iter_lines(&self.status_lines)
            .enumerate()
            .find(|(_, (_, is_selected))| *is_selected)
        else {
            return;
        };
        let StatusOutputLineData::Commit { .. } = &line.data else {
            return;
        };
        let Some(connector) = &line.connector else {
            return;
        };
        let StatusOutputContent::Commit(commit_content) = &line.content else {
            return;
        };
        let connector_and_sha_width = connector
            .iter()
            .chain(&commit_content.sha)
            .map(|span| span.width() as u16)
            .sum::<u16>();
        let padding_between_sha_and_message = 1;

        let start_x = connector_and_sha_width + padding_between_sha_and_message;
        let x = area.x.saturating_add(start_x);
        let width = area.right().saturating_sub(x);
        let area = Rect::new(x, area.y.saturating_add(idx as u16), width, 1);
        frame.render_widget(&**textarea, area);
    }

    fn render_debug(&self, area: Rect, frame: &mut Frame) {
        let list = List::new(
            std::iter::once(ListItem::new(format!("Renders: {}", self.renders))).chain(
                self.cursor
                    .selected_line(&self.status_lines)
                    .map(|selected_line| ListItem::new(format!("{selected_line:#?}"))),
            ),
        );

        frame.render_widget(list, area);
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
                    Mode::InlineReword { .. } => {
                        messages.push(Message::RewordInlineInput(ev));
                    }
                    Mode::Command { .. } => {
                        messages.push(Message::CommandInput(ev));
                    }
                    Mode::Normal | Mode::Rub { .. } => {}
                }
            }
        }
        Event::Resize(_, _) => {
            messages.push(Message::Noop);
        }
        Event::Paste(_) => match mode {
            Mode::InlineReword { .. } => {
                messages.push(Message::RewordInlineInput(ev));
            }
            Mode::Command { .. } => {
                messages.push(Message::CommandInput(ev));
            }
            Mode::Normal | Mode::Rub { .. } => {
                messages.push(Message::Noop);
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
    Noop,
    Quit,
    MoveCursorUp,
    MoveCursorDown,
    MoveCursorPreviousSection,
    MoveCursorNextSection,
    StartRub,
    EnterNormalMode,
    ConfirmRub,
    Reload(Option<SelectAfterReload>),
    ToggleFiles,
    ShowError(Arc<anyhow::Error>),
    CreateEmptyCommit,
    RewordWithEditor,
    StartRewordInline,
    ConfirmInlineReword,
    RewordInlineInput(Event),
    EnterCommandMode,
    CommandInput(Event),
    RunCommand,
}

#[derive(Debug, Default, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::EnumIter, Hash))]
#[strum_discriminants(name(ModeDiscriminant))]
enum Mode {
    #[default]
    Normal,
    Rub {
        source: Arc<CliId>,
        available_targets: Vec<Arc<CliId>>,
    },
    InlineReword {
        commit_id: gix::ObjectId,
        textarea: Box<TextArea<'static>>,
    },
    Command {
        textarea: Box<TextArea<'static>>,
    },
}

/// What to select after reloading
#[derive(Debug, Clone, Copy)]
enum SelectAfterReload {
    /// Select a specific commit
    Commit(gix::ObjectId),
}

fn rub_operation_display(source: &CliId, target: &CliId) -> Option<&'static str> {
    if source == target {
        return Some("no operation");
    }

    Some(match route_operation(source, target)? {
        RubOperation::UnassignUncommitted(..) => "unassign hunks",
        RubOperation::UncommittedToCommit(..) => "amend commit",
        RubOperation::UncommittedToBranch(..) => "assign hunks",
        RubOperation::UncommittedToStack(..) => "assign hunks",
        RubOperation::StackToUnassigned(..) => "unassign hunks",
        RubOperation::StackToStack { .. } => "reassign hunks",
        RubOperation::StackToBranch { .. } => "reassign hunks",
        RubOperation::UnassignedToCommit(..) => "amend commit",
        RubOperation::UnassignedToBranch(..) => "assign hunks",
        RubOperation::UnassignedToStack(..) => "assign hunks",
        RubOperation::UndoCommit(..) => "undo commit",
        RubOperation::SquashCommits { .. } => "squash commits",
        RubOperation::MoveCommitToBranch(..) => "move commit",
        RubOperation::BranchToUnassigned(..) => "unassign hunks",
        RubOperation::BranchToStack { .. } => "reassign hunks",
        RubOperation::BranchToCommit(..) => "amend commit",
        RubOperation::BranchToBranch { .. } => "reassign hunks",
        RubOperation::CommittedFileToBranch(..) => "extract file",
        RubOperation::CommittedFileToCommit(..) => "move file",
        RubOperation::CommittedFileToUnassigned(..) => "extract file",
    })
}

struct PopupMargin {
    right: u16,
    bottom: u16,
}

fn render_error_popup(frame: &mut Frame, area: Rect, margin: PopupMargin, text: &str) {
    let horizontal_padding: u16 = 1;
    let vertical_padding: u16 = 0;

    let height = text.lines().count() as u16 + 2 + (vertical_padding * 2);
    let width = 45;

    let PopupMargin {
        right: right_margin,
        bottom: bottom_margin,
    } = margin;

    let width = width.min(area.width.max(1));
    let height = height.min(area.height.max(1));

    let x = area.x.saturating_add(
        area.width
            .saturating_sub(right_margin)
            .saturating_sub(width),
    );
    let y = area.y.saturating_add(
        area.height
            .saturating_sub(bottom_margin)
            .saturating_sub(height),
    );

    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let widget = Paragraph::new(text)
        .block(
            Block::default()
                .title("⚠️ Error")
                .borders(Borders::ALL)
                .border_style(Style::default().red())
                .padding(Padding::new(
                    horizontal_padding,
                    horizontal_padding,
                    vertical_padding,
                    vertical_padding,
                )),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(widget, popup_area);
}

fn with_noop_output<F, T>(f: F) -> T
where
    F: FnOnce(&mut OutputChannel) -> T,
{
    let mut noop_out = OutputChannel::new_without_pager_non_json(OutputFormat::None);
    f(&mut noop_out)
}

#[derive(Debug)]
pub(super) struct AppError {
    pub(super) inner: Arc<anyhow::Error>,
    pub(super) dismiss_at: Instant,
}
