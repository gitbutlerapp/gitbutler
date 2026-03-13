use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Context as _;
use bstr::BString;
use but_api::{
    commit::{commit_insert_blank, ui::RelativeTo},
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
        reword::get_commit_message_from_editor,
        rub::{RubOperation, route_operation},
        status::{
            StatusFlags, StatusOutput, StatusOutputLine, build_status_context, build_status_output,
            tui::{
                cursor::Cursor,
                key_bind::{KEY_BINDS, KeyBind},
            },
        },
    },
    tui::TerminalGuard,
    utils::OutputChannel,
};

use super::output::{StatusOutputContent, StatusOutputLineData};

mod cursor;
mod key_bind;

const CURSOR_BG: Color = Color::Rgb(69, 71, 90);

pub(super) async fn render_tui(
    ctx: &mut Context,
    out: &mut OutputChannel,
    mode: &OperatingMode,
    flags: StatusFlags,
    status_lines: Vec<StatusOutputLine>,
    debug: bool,
) -> anyhow::Result<Vec<StatusOutputLine>> {
    let mut guard = TerminalGuard::new(true)?;

    let mut app = App::new(status_lines, flags, debug);

    let mut messages = Vec::new();

    // second buffer so we can send messages from `App::handle_message`
    let mut other_messages = Vec::new();

    loop {
        if std::mem::take(&mut app.should_render) {
            guard.terminal_mut().draw(|frame| {
                app.renders += 1;
                app.render(frame)
            })?;
        }

        // poll for events
        if event::poll(Duration::from_millis(30))? {
            let ev = event::read()?;
            event_to_messages(ev, app.key_binds, &app.mode, &mut messages);
        }

        // handle messages
        loop {
            if messages.is_empty() {
                break;
            }

            for msg in messages.drain(..) {
                app.handle_message(ctx, out, mode, &mut guard, &mut other_messages, msg)
                    .await;
            }
            std::mem::swap(&mut messages, &mut other_messages);
        }

        // dismiss errors
        let now = Instant::now();
        let errors_before = app.errors.len();
        app.errors.retain(|err| err.dismiss_at > now);
        if app.errors.len() != errors_before {
            app.should_render = true;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(app.status_lines)
}

#[derive(Debug)]
struct App {
    status_lines: Vec<StatusOutputLine>,
    flags: StatusFlags,
    should_quit: bool,
    should_render: bool,
    cursor: Cursor,
    mode: Mode,
    key_binds: &'static [KeyBind],
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
            key_binds: KEY_BINDS,
            debug_enabled: debug,
            errors: Vec::new(),
            renders: 0,
        }
    }

    async fn handle_message(
        &mut self,
        ctx: &mut Context,
        out: &mut OutputChannel,
        mode: &OperatingMode,
        guard: &mut TerminalGuard,
        messages: &mut Vec<Message>,
        msg: Message,
    ) {
        if let Err(err) = self
            .try_handle_message(ctx, out, mode, guard, messages, msg)
            .await
        {
            messages.push(Message::ShowError(Arc::new(err)));
        }
    }

    async fn try_handle_message(
        &mut self,
        ctx: &mut Context,
        out: &mut OutputChannel,
        mode: &OperatingMode,
        guard: &mut TerminalGuard,
        messages: &mut Vec<Message>,
        msg: Message,
    ) -> anyhow::Result<()> {
        self.should_render = true;

        match msg {
            Message::Quit => {
                self.should_quit = true;
            }
            Message::Noop => {}
            Message::MoveCursorUp => self.cursor.move_up(&self.status_lines, &self.mode),
            Message::MoveCursorDown => self.cursor.move_down(&self.status_lines, &self.mode),
            Message::StartRub => {
                let Some(selected_line) = self.cursor.selected_line(&self.status_lines) else {
                    return Ok(());
                };

                let Some(cli_id) = selected_line.data.cli_id() else {
                    return Ok(());
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
                    return Ok(());
                }

                let previous_cursor = self.cursor;
                self.cursor.move_down(&self.status_lines, &self.mode);
                if self.cursor == previous_cursor {
                    self.cursor.move_up(&self.status_lines, &self.mode);
                }
            }
            Message::EnterNormalMode => {
                self.mode = Mode::Normal;
            }
            Message::ToggleFiles => {
                self.flags.show_files = !self.flags.show_files;
                messages.push(Message::Reload(None));
            }
            Message::ConfirmRub => {
                if let Mode::Rub { source, .. } = &self.mode
                    && let Some(selected_line) = self.cursor.selected_line(&self.status_lines)
                    && let Some(target) = selected_line.data.cli_id()
                    && let Some(operation) = route_operation(source, target)
                {
                    with_noop_output(|out| operation.execute(ctx, out))?;
                }

                messages.extend([Message::EnterNormalMode, Message::Reload(None)]);
            }
            Message::Reload(select_after_reload) => {
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
                        SelectAfterReload::Commit(commit_id) => {
                            Cursor::select(commit_id, &new_lines)
                        }
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
            }
            Message::ShowError(err) => {
                self.errors.push(AppError {
                    inner: err,
                    dismiss_at: Instant::now() + Duration::from_secs(5),
                });
            }
            Message::CreateEmptyCommit => {
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

                        let commit_result = commit_insert_blank(
                            ctx,
                            RelativeTo::Reference(full_name),
                            InsertSide::Below,
                        )?;

                        messages.push(Message::Reload(Some(SelectAfterReload::Commit(
                            commit_result.new_commit,
                        ))));
                    }
                    StatusOutputLineData::Commit { cli_id } => {
                        let CliId::Commit { commit_id, .. } = &**cli_id else {
                            return Ok(());
                        };

                        let commit_result = commit_insert_blank(
                            ctx,
                            RelativeTo::Commit(*commit_id),
                            InsertSide::Above,
                        )?;

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
            }
            Message::RewordWithEditor => {
                if !matches!(self.mode, Mode::Normal) {
                    return Ok(());
                }
                let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
                    return Ok(());
                };

                let cli_id = match &selection.data {
                    StatusOutputLineData::Commit { cli_id } => cli_id,
                    StatusOutputLineData::UpdateNotice
                    | StatusOutputLineData::Connector
                    | StatusOutputLineData::StagedChanges { .. }
                    | StatusOutputLineData::StagedFile { .. }
                    | StatusOutputLineData::UnstagedChanges { .. }
                    | StatusOutputLineData::UnstagedFile { .. }
                    | StatusOutputLineData::Branch { .. }
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

                let CliId::Commit { commit_id, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_details =
                    but_api::diff::commit_details(ctx, *commit_id, ComputeLineStats::No)?;
                let current_message = commit_details.commit.inner.message.to_string();

                let _suspend_guard = guard.suspend()?;
                let new_message = get_commit_message_from_editor(
                    ctx,
                    commit_details,
                    current_message.clone(),
                    ShowDiffInEditor::Unspecified,
                )?;

                let Some(new_message) = new_message else {
                    return Ok(());
                };

                if new_message == current_message {
                    return Ok(());
                }

                let reword_result = but_api::commit::commit_reword_only(
                    ctx,
                    *commit_id,
                    BString::from(new_message),
                )
                .with_context(|| format!("failed to reword {}", commit_id.to_hex_with_len(7)))?;

                messages.push(Message::Reload(Some(SelectAfterReload::Commit(
                    reword_result.new_commit,
                ))));
            }
            Message::StartRewordInline => {
                if !matches!(self.mode, Mode::Normal) {
                    return Ok(());
                }
                let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
                    return Ok(());
                };

                let cli_id = match &selection.data {
                    StatusOutputLineData::Commit { cli_id } => cli_id,
                    StatusOutputLineData::UpdateNotice
                    | StatusOutputLineData::Connector
                    | StatusOutputLineData::StagedChanges { .. }
                    | StatusOutputLineData::StagedFile { .. }
                    | StatusOutputLineData::UnstagedChanges { .. }
                    | StatusOutputLineData::UnstagedFile { .. }
                    | StatusOutputLineData::Branch { .. }
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

                let CliId::Commit { commit_id, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_details =
                    but_api::diff::commit_details(ctx, *commit_id, ComputeLineStats::No)?;
                let current_message = commit_details.commit.inner.message.to_string();

                if current_message.split_once('\n').is_some() {
                    messages.push(Message::RewordWithEditor);
                    return Ok(());
                }

                let first_line = current_message.lines().next().unwrap_or("").to_string();

                let mut textarea = TextArea::from([first_line]);
                textarea.set_cursor_line_style(Style::default()); // remove underline
                textarea.move_cursor(CursorMove::End);

                self.mode = Mode::InlineReword {
                    commit_id: *commit_id,
                    textarea: Box::new(textarea),
                };
            }
            Message::RewordInlineInput(ev) => {
                if let Mode::InlineReword { textarea, .. } = &mut self.mode {
                    textarea.input(ev);
                }
            }
            Message::ConfirmInlineReword => {
                let Mode::InlineReword {
                    commit_id,
                    textarea,
                } = &self.mode
                else {
                    return Ok(());
                };

                let commit_details =
                    but_api::diff::commit_details(ctx, *commit_id, ComputeLineStats::No)?;
                let current_message = commit_details.commit.inner.message.to_string();
                let new_subject = textarea
                    .lines()
                    .first()
                    .map(std::string::String::as_str)
                    .unwrap_or("");

                let new_message = new_subject.to_string();

                if new_message == current_message {
                    messages.push(Message::EnterNormalMode);
                    return Ok(());
                }

                let reword_result = but_api::commit::commit_reword_only(
                    ctx,
                    *commit_id,
                    BString::from(new_message),
                )
                .with_context(|| format!("failed to reword {}", commit_id.to_hex_with_len(7)))?;

                messages.extend([
                    Message::EnterNormalMode,
                    Message::Reload(Some(SelectAfterReload::Commit(reword_result.new_commit))),
                ]);
            }
        }

        Ok(())
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
                Mode::Normal | Mode::InlineReword { .. } => {}
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
                Mode::Normal | Mode::InlineReword { .. } => {}
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
            Mode::Normal => {
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

        if is_selected {
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
        };

        let padding = Span::raw(" ");

        let mut line = Line::from_iter([mode_span, padding]);

        let mut key_binds_iter = self
            .key_binds
            .iter()
            .copied()
            .filter(|key_bind| key_bind.available_in_mode(&self.mode))
            .filter(|key_bind| !key_bind.hidden)
            .peekable();
        while let Some(key_bind) = key_binds_iter.next() {
            line.extend([
                Span::styled(key_bind.code_display, Style::default().blue()),
                Span::raw(" "),
                Span::styled(key_bind.short_description, Style::default().dim()),
            ]);

            if key_binds_iter.peek().is_some() {
                line.push_span(Span::styled(" • ", Style::default().dim()));
            }
        }

        frame.render_widget(line, area);
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

fn event_to_messages(ev: Event, key_binds: &[KeyBind], mode: &Mode, messages: &mut Vec<Message>) {
    match ev {
        Event::Key(key) => {
            let mut handled = false;
            for key_bind in key_binds.iter().copied() {
                if key_bind.matches(&key, mode) {
                    messages.push(key_bind.message.clone());
                    handled = true;
                }
            }

            if !handled {
                match mode {
                    Mode::InlineReword { .. } => {
                        messages.push(Message::RewordInlineInput(ev));
                    }
                    Mode::Normal | Mode::Rub { .. } => {}
                }
            }
        }
        Event::Resize(_, _) | Event::Paste(_) => {
            messages.push(Message::Noop); // trigger a render
        }
        Event::FocusGained | Event::FocusLost | Event::Mouse(_) => {}
    }
}

#[derive(Debug, Clone)]
enum Message {
    Noop,
    Quit,
    MoveCursorUp,
    MoveCursorDown,
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
}

#[derive(Debug, Default, strum::EnumDiscriminants)]
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
