use std::{
    iter::once,
    time::{Duration, Instant},
};

use but_rebase::graph_rebase::mutate::InsertSide;
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    prelude::*,
    widgets::{Block, BorderType, Borders},
};
use ratatui_textarea::TextArea;
use unicode_width::UnicodeWidthStr;

use crate::{
    CliId,
    command::legacy::status::{
        CommitLineContent, FileLineContent, StatusOutputLine,
        output::{
            BranchLineContent, MergeBaseLineContent, StatusOutputContent, StatusOutputLineData,
            UncommittedLineContent,
        },
        tui::app::{
            BranchMode, CherryPickMode, CommitMessageComposer, CommitMode, JumpPart, MoveMarks,
            MoveMode, MoveSource, MoveStackMode, StackMode, WorktreeMode,
            lines_part_of_current_stack,
            mark::{CommittedMark, committed_changes_to_act_on},
        },
    },
    id::{CommitId, LaneId},
    theme::Theme,
    utils::targeting::Side,
};

use super::{
    App, CURSOR_CONTEXT_ROWS, InlineRewordMode, Modal, NOOP,
    cursor::is_selectable_in_mode,
    graph_extension::{ExtensionDirection, extend_connector_spans},
    highlight::with_highlight,
    key_bind::KeyBind,
    mode::{Mode, ModeDiscriminant},
    toast,
};

pub fn render_app(app: &App, frame: &mut Frame) {
    app.status_line_areas.borrow_mut().clear();
    let layout = app_layout(app, frame.area());

    match layout.details {
        Some(DetailsPaneLayout::FullScreen {
            content_area,
            file_browser_area,
            details_pane_area,
        }) => {
            let status_block = pane_block(app, Borders::NONE);
            frame.render_widget(status_block, layout.status_area);

            if let Some(file_browser_area) = file_browser_area
                && let Some(file_browser) = &app.file_browser
            {
                file_browser.render(file_browser_area, frame);
            }

            if let Some(details_pane_area) = details_pane_area {
                let details_block = file_browser_details_block(app);
                frame.render_widget(details_block, details_pane_area);
            }

            render_details_pane(app, content_area, frame);
        }
        Some(DetailsPaneLayout::Split {
            pane_area,
            content_area,
        }) => {
            let status_block = pane_block(app, Borders::NONE);
            let details_block = pane_block(app, Borders::NONE);

            let status_inner_area = status_block.inner(layout.status_area);
            frame.render_widget(status_block, layout.status_area);
            render_status(app, status_inner_area, frame);

            let details_separator_area = details_block.inner(pane_area);
            frame.render_widget(details_block, pane_area);
            render_details_separator(app, details_separator_area, frame);
            render_details_pane(app, content_area, frame);
        }
        None => {
            let status_block = pane_block(app, Borders::NONE);
            let status_inner_area = status_block.inner(layout.status_area);
            frame.render_widget(status_block, layout.status_area);
            render_status(app, status_inner_area, frame);
        }
    }

    if let Some(debug_area) = layout.debug_area {
        let block = debug_block(app);
        let inner_area = block.inner(debug_area);
        frame.render_widget(block, debug_area);
        render_debug(app, inner_area, frame);
    }

    render_hot_bar(app, layout.hotbar_area, frame);
    render_toasts(app, layout.toast_area(), frame);

    match &app.modal {
        Some(Modal::Confirm { confirm, .. }) => confirm.render(app.has_focus, frame.area(), frame),
        Some(Modal::GotoBranchPicker { picker, .. }) => {
            picker.render(app.has_focus, frame.area(), frame);
        }
        Some(Modal::ApplyStackPicker { picker, .. }) => {
            picker.render(app.has_focus, frame.area(), frame);
        }
        Some(Modal::UnarchiveWorktreePicker { picker, .. }) => {
            picker.render(app.has_focus, frame.area(), frame);
        }
        Some(Modal::SwitchBranchPicker { picker, .. }) => {
            picker.render(app.has_focus, frame.area(), frame);
        }
        Some(Modal::CopySelectionPicker { picker, .. }) => {
            picker.render(app.has_focus, frame.area(), frame);
        }
        Some(Modal::ProgramPicker { picker, .. }) => {
            picker.render(app.has_focus, frame.area(), frame)
        }
        Some(Modal::Help { help, .. }) => help.render(frame.area(), frame),
        None => {}
    }
}

fn render_details_pane(app: &App, area: Rect, frame: &mut Frame) {
    let marks = app.mode.marks_ref();
    app.details.render(
        matches!(app.modal, Some(Modal::Help { .. })),
        app.has_focus,
        marks,
        area,
        frame,
    );
}

fn render_details_separator(app: &App, area: Rect, frame: &mut Frame) {
    frame.render_widget(details_separator(app), area);
}

pub(crate) fn details_content_area(app: &App, details_area: Rect) -> Rect {
    let details_area = pane_block(app, Borders::NONE).inner(details_area);
    details_separator(app).inner(details_area)
}

fn details_separator(app: &App) -> Block<'static> {
    Block::bordered()
        .border_style(app.theme.border)
        .borders(Borders::LEFT)
}

fn file_browser_details_block(app: &App) -> Block<'static> {
    Block::bordered()
        .border_style(app.theme.border)
        .border_type(BorderType::Plain)
        .borders(Borders::LEFT)
}

fn debug_block(app: &App) -> Block<'static> {
    Block::bordered()
        .border_style(app.theme.border)
        .border_type(BorderType::Thick)
        .borders(Borders::LEFT)
}

fn pane_block(app: &App, borders: Borders) -> Block<'static> {
    let border_style = app.theme.border;
    let border_type = BorderType::Plain;

    Block::bordered()
        .border_style(border_style)
        .border_type(border_type)
        .borders(borders)
}

#[derive(Debug)]
struct AppLayout {
    status_area: Rect,
    hotbar_area: Rect,
    debug_area: Option<Rect>,
    details: Option<DetailsPaneLayout>,
}

impl AppLayout {
    fn details_content_area(&self) -> Option<Rect> {
        match self.details {
            Some(DetailsPaneLayout::FullScreen { content_area, .. })
            | Some(DetailsPaneLayout::Split { content_area, .. }) => Some(content_area),
            None => None,
        }
    }

    fn toast_area(&self) -> Rect {
        match self.details {
            Some(DetailsPaneLayout::Split { pane_area, .. }) => pane_area,
            Some(DetailsPaneLayout::FullScreen { .. }) | None => self.status_area,
        }
    }
}

#[derive(Debug)]
enum DetailsPaneLayout {
    FullScreen {
        content_area: Rect,
        file_browser_area: Option<Rect>,
        details_pane_area: Option<Rect>,
    },
    Split {
        pane_area: Rect,
        content_area: Rect,
    },
}

fn app_layout(app: &App, terminal_area: Rect) -> AppLayout {
    let content_layout =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(terminal_area);
    let main_content_area = content_layout[0];

    let (main_content_area, debug_area) = if app.launch_options.debug {
        let layout = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(main_content_area);
        (layout[0], Some(layout[1]))
    } else {
        (main_content_area, None)
    };

    let status_layout = status_layout(app, main_content_area);
    let details = if let Mode::Details(details_mode) = &*app.mode
        && details_mode.full_screen
    {
        if app.file_browser.is_some() {
            let status_block_area = pane_block(app, Borders::NONE).inner(status_layout.status_area);
            let details_layout = Layout::horizontal([Constraint::Length(50), Constraint::Min(1)])
                .split(status_block_area);
            let details_pane_area = details_layout[1];
            let content_area = file_browser_details_block(app).inner(details_pane_area);
            Some(DetailsPaneLayout::FullScreen {
                content_area,
                file_browser_area: Some(details_layout[0]),
                details_pane_area: Some(details_pane_area),
            })
        } else {
            let content_area = pane_block(app, Borders::NONE).inner(status_layout.status_area);
            let content_area = Rect {
                x: content_area.x.saturating_add(1),
                width: content_area.width.saturating_sub(1),
                ..content_area
            };
            Some(DetailsPaneLayout::FullScreen {
                content_area,
                file_browser_area: None,
                details_pane_area: None,
            })
        }
    } else {
        status_layout.details_area.map(|pane_area| {
            let content_area = details_content_area(app, pane_area);
            DetailsPaneLayout::Split {
                pane_area,
                content_area,
            }
        })
    };

    AppLayout {
        status_area: status_layout.status_area,
        hotbar_area: content_layout[1],
        debug_area,
        details,
    }
}

pub(crate) fn status_area_for_app(app: &App, terminal_area: Rect) -> Option<Rect> {
    if matches!(&*app.mode, Mode::Details(mode) if mode.full_screen) {
        return None;
    }
    Some(app_layout(app, terminal_area).status_area)
}

pub(crate) fn details_content_area_for_app(app: &App, terminal_area: Rect) -> Option<Rect> {
    app_layout(app, terminal_area).details_content_area()
}

pub(crate) fn debug_content_area_for_app(app: &App, terminal_area: Rect) -> Option<Rect> {
    if !app.launch_options.debug {
        return None;
    }

    app_layout(app, terminal_area)
        .debug_area
        .map(|area| debug_block(app).inner(area))
}

pub fn status_layout(app: &App, area: Rect) -> StatusLayout {
    if let Mode::Details(details_mode) = &*app.mode
        && details_mode.full_screen
    {
        return StatusLayout {
            status_area: area,
            details_area: None,
        };
    }

    let (status_area, details_area) = if app.is_details_visible {
        let layout = Layout::horizontal([
            Constraint::Percentage(app.status_width_percentage),
            Constraint::Percentage(100 - app.status_width_percentage),
        ])
        .split(area);
        (layout[0], Some(layout[1]))
    } else {
        (area, None)
    };

    StatusLayout {
        status_area,
        details_area,
    }
}

fn render_status(app: &App, area: Rect, frame: &mut Frame) {
    update_status_scroll(app, area);

    let lines_part_of_current_stack = lines_part_of_current_stack(app);
    let lines_part_of_current_branch = app
        .cursor
        .lines_part_of_current_branch(&app.mode, app.status_lines.iter().map(|line| &line.data));

    let mut areas = available_lines_in_area(area);

    for (idx, status_line) in app
        .status_lines
        .iter()
        .enumerate()
        .skip(app.status_scroll.top())
    {
        let stack_highlight = lines_part_of_current_stack
            .as_ref()
            .is_some_and(|rows| rows.get(idx).copied().unwrap_or_default());

        let branch_highlight = lines_part_of_current_branch
            .as_ref()
            .is_some_and(|rows| rows.get(idx).copied().unwrap_or_default());

        let mode_highlight = stack_highlight || branch_highlight;

        if !render_status_list_item(
            app,
            status_line,
            app.cursor.index() == idx,
            mode_highlight,
            idx,
            lines_part_of_current_branch.as_deref(),
            &mut areas,
            frame,
        ) {
            break;
        }
    }
}

fn update_status_scroll(app: &App, area: Rect) {
    let viewport_height = area.height as usize;
    let max_scroll_top = app.status_lines.len().saturating_sub(viewport_height);
    let mut scroll_top = app.status_scroll.top().min(max_scroll_top);

    if app.status_scroll.take_pending_cursor() {
        scroll_top = app.cursor.scroll_top_for_viewport(
            scroll_top,
            app.status_lines.len(),
            viewport_height,
            CURSOR_CONTEXT_ROWS,
        );
    }

    // A click doesn't restore cursor context, but both the selected item and its
    // preview must fit. Reserve the extra row with only the minimum scroll needed
    // to show both (when both rows can fit).
    if viewport_height > 1
        && app
            .cursor
            .selected_line(&app.status_lines)
            .and_then(|line| app.mode.as_mode_render().operation_extension(&line.data))
            .is_some()
    {
        scroll_top = scroll_top.max(
            app.cursor
                .index()
                .saturating_add(2)
                .saturating_sub(viewport_height),
        );
    }

    app.status_scroll.set_top(scroll_top);
}

#[must_use]
fn render_status_list_item(
    app: &App,
    status_line: &StatusOutputLine,
    is_selected: bool,
    mode_highlight: bool,
    status_line_idx: usize,
    lines_part_of_current_branch: Option<&[bool]>,
    areas: &mut dyn Iterator<Item = Rect>,
    frame: &mut Frame,
) -> bool {
    let Some(area) = areas.next() else {
        return false;
    };

    let highlight_current_line = !matches!(app.modal, Some(Modal::Help { .. })) && app.has_focus;

    let StatusOutputLine {
        connector,
        content,
        data,
    } = status_line;

    let operation_extension = if is_selected {
        app.mode.as_mode_render().operation_extension(data)
    } else {
        None
    };

    let (area, operation_extension_area) = match operation_extension {
        Some(extension) => match extension.direction() {
            ExtensionDirection::Above => {
                let Some(next_area) = areas.next() else {
                    render_operation_extension_line(
                        app,
                        data,
                        connector.as_deref(),
                        area,
                        extension,
                        frame,
                    );
                    return true;
                };
                (next_area, Some((area, extension)))
            }
            ExtensionDirection::Below => (area, areas.next().map(|area| (area, extension))),
        },
        None => (area, None),
    };

    if let Some((area, extension)) = operation_extension_area {
        render_operation_extension_line(app, data, connector.as_deref(), area, extension, frame);
    }
    app.status_line_areas
        .borrow_mut()
        .push((area, status_line_idx));

    if (is_selected || mode_highlight) && highlight_current_line {
        frame
            .buffer_mut()
            .set_style(area, app.selection_highlight_color());
    }

    let mut line = RenderSingleLineSpans::new(frame, area);

    // ┊╭┄dp [dp-branch-1]
    // ^^^ render the connector, optionally with mark symbol
    if let Some(connector) = connector {
        let mark_symbol = data.cli_id().and_then(|id| {
            let marks = app.marks_ref();
            if marks.contains_cli_id(id) {
                Some(&app.theme.sym().mark)
            } else if marks.contains_child_of(id) {
                Some(&app.theme.sym().partial_mark)
            } else {
                None
            }
        });

        if let Some(mark_symbol) = mark_symbol {
            let mut mark_padding = 0;
            for (idx, span) in connector.iter().enumerate() {
                // Linked worktrees prepend graph lanes; the indicator and its spacing
                // remain the final two spans regardless of nesting depth.
                if idx + 2 == connector.len() {
                    let mark_span = mark_symbol.span();
                    mark_padding = span.width().saturating_sub(mark_span.width());
                    line.render(mark_span);
                    for _ in 0..mark_padding {
                        line.render(Span::raw(" ").style(app.theme.tui_mark));
                    }
                } else if idx + 1 == connector.len() {
                    // after the indicator is a bunch of spaces
                    for (c_idx, c) in span.content.chars().enumerate() {
                        line.render(if c_idx == 0 && mark_padding == 0 {
                            // color the background of the first space the same as the mark indicator
                            // since the checkmark symbol we use takes up more than one cell
                            Span::raw(c.to_string()).style(app.theme.tui_mark)
                        } else {
                            Span::raw(c.to_string())
                        });
                    }
                } else {
                    line.render_ref(span);
                }
            }
        } else {
            for span in connector {
                line.render_ref(span);
            }
        }
    }

    let line_has_copied_highlight = data.cli_id().is_some_and(|id| app.highlight.contains(id));
    let line_is_to_be_discarded = data.cli_id().is_some_and(|selection| {
        app.to_be_discarded
            .iter()
            .any(|to_be_discarded| to_be_discarded == &**selection)
    });

    // ┊●   << source >> 982b7d85c5 my commit
    //      ^^^^^^^^^^^^ render target/source labels
    if line_is_to_be_discarded {
        line.extend([Span::raw("<< discard >>").black().on_red(), Span::raw(" ")]);
    } else if let Mode::Branch(branch_mode) = &*app.mode {
        branch_mode.render_insert_branch_marker(
            app,
            data,
            is_selected,
            status_line_idx,
            lines_part_of_current_branch,
            &mut line,
        );
    } else if is_selected {
        app.mode
            .as_mode_render()
            .render_operation_target_marker(app, data, &mut line);
    } else {
        app.mode
            .as_mode_render()
            .render_operation_source_marker(app, data, &mut line);
    }

    let jump_match = if let Mode::Jump(mode) = &*app.mode {
        mode.match_line(status_line, app.flags.show_files)
    } else {
        None
    };

    // ┊●   982b7d85c5 my commit
    //      ^^^^^^^^^^^^^^^^^^^^ render the main content
    let area_used_by_main_content = line.area_used_by(|line| {
        match content {
            StatusOutputContent::Plain(spans) => {
                line.extend(spans);
            }
            StatusOutputContent::MergeBase(MergeBaseLineContent { id, suffix }) => {
                if let Some(jump_match) = &jump_match {
                    line.extend(underline_jump_matches(
                        id,
                        jump_match.char_indices(JumpPart::Id),
                    ));
                } else {
                    line.extend(id);
                }
                line.extend(suffix);
            }
            StatusOutputContent::Commit(CommitLineContent {
                sha,
                change_id,
                author,
                message,
                suffix,
            }) => {
                if line_has_copied_highlight && !change_id.is_empty() {
                    line.extend(change_id.iter().cloned().map(|span| {
                        if span.content.trim().is_empty() {
                            span
                        } else {
                            with_highlight(span)
                        }
                    }));
                } else if !change_id.is_empty()
                    && let Some(jump_match) = &jump_match
                {
                    line.extend(underline_jump_matches(
                        change_id,
                        jump_match.char_indices(JumpPart::Id),
                    ));
                } else {
                    line.extend(change_id.iter().cloned());
                }

                if line_has_copied_highlight && change_id.is_empty() {
                    line.extend(sha.iter().cloned().map(with_highlight));
                } else if change_id.is_empty()
                    && let Some(jump_match) = &jump_match
                {
                    line.extend(underline_jump_matches(
                        sha,
                        jump_match.char_indices(JumpPart::Id),
                    ));
                } else {
                    line.extend(sha);
                }
                line.extend(author);

                if let Some(id) = data.cli_id()
                    && let CliId::Commit {
                        commit: CommitId { commit_id, .. },
                        id: _,
                    } = &**id
                    && let Mode::InlineReword(InlineRewordMode::Commit {
                        textarea,
                        commit_id: source,
                    }) = &*app.mode
                    && *commit_id == source.commit_id
                {
                    line.render(Span::raw(" "));
                    line.render_textarea(textarea);
                } else {
                    if let Some(jump_match) = &jump_match {
                        line.extend(underline_jump_matches(
                            message,
                            jump_match.char_indices(JumpPart::Text),
                        ));
                    } else {
                        line.extend(message);
                    }
                    line.extend(suffix);
                }
            }
            StatusOutputContent::Branch(BranchLineContent {
                id,
                decoration_start,
                branch_name,
                decoration_end,
                suffix,
            }) => {
                if line_has_copied_highlight {
                    line.extend(id);
                } else if let Some(jump_match) = &jump_match {
                    line.extend(underline_jump_matches(
                        id,
                        jump_match.char_indices(JumpPart::Id),
                    ));
                } else {
                    line.extend(id);
                }
                line.extend(decoration_start);

                if let Some(id) = data.cli_id()
                    && let CliId::Branch(branch) = &**id
                    && let Mode::InlineReword(InlineRewordMode::Branch {
                        textarea,
                        name: source,
                        ..
                    }) = &*app.mode
                    && branch.name == source.shorten()
                {
                    line.render_textarea(textarea);
                } else {
                    if line_has_copied_highlight {
                        line.extend(branch_name.iter().cloned().map(with_highlight));
                    } else if let Some(jump_match) = &jump_match {
                        line.extend(underline_jump_matches(
                            branch_name,
                            jump_match.char_indices(JumpPart::Text),
                        ));
                    } else {
                        line.extend(branch_name);
                    }
                }

                line.extend(decoration_end);
                line.extend(suffix);
            }
            StatusOutputContent::File(FileLineContent { id, status, path }) => {
                if line_has_copied_highlight {
                    line.extend(id);
                } else if let Some(jump_match) = &jump_match {
                    line.extend(underline_jump_matches(
                        id,
                        jump_match.char_indices(JumpPart::Id),
                    ));
                } else {
                    line.extend(id);
                }
                line.extend(status);
                if line_has_copied_highlight {
                    line.extend(path.iter().cloned().map(with_highlight));
                } else if let Some(jump_match) = &jump_match {
                    line.extend(underline_jump_matches(
                        path,
                        jump_match.char_indices(JumpPart::Text),
                    ));
                } else {
                    line.extend(path);
                }
            }
            StatusOutputContent::Uncommitted(UncommittedLineContent {
                id,
                decoration_start,
                label,
                decoration_end,
                suffix,
            }) => {
                if line_has_copied_highlight {
                    line.extend(id.iter().cloned().map(with_highlight));
                } else if let Some(jump_match) = &jump_match {
                    line.extend(underline_jump_matches(
                        id,
                        jump_match.char_indices(JumpPart::Id),
                    ));
                } else {
                    line.extend(id);
                }
                line.extend(decoration_start);
                if let Some(jump_match) = &jump_match {
                    line.extend(underline_jump_matches(
                        label,
                        jump_match.char_indices(JumpPart::Text),
                    ));
                } else {
                    line.extend(label);
                }
                line.extend(decoration_end);
                line.extend(suffix);
            }
        };
    });

    // Style the main content section when the line is queued for discard.
    if line_is_to_be_discarded {
        line.frame
            .buffer_mut()
            .set_style(area_used_by_main_content, Style::default().crossed_out());
    }

    if !is_selectable_in_mode(status_line, app.mode.as_ref(), app.flags.show_files) {
        line.frame
            .buffer_mut()
            .set_style(area_used_by_main_content, app.theme.hint);
    }

    if is_selected && let Mode::MoveStack(move_mode) = &*app.mode {
        // ┊<< move stack >>
        //  ^^^^^^^^^^^^^^^^ render move stack label for in between rows
        if !data.cli_id().is_some_and(|id| move_mode.source.matches(id)) {
            render_move_stack_operation_target_marker(app, data, move_mode, &mut line);
        }
    }

    true
}

#[derive(Clone, Copy)]
pub(crate) enum OperationExtension<'a> {
    Commit {
        mode: &'a CommitMode,
        direction: ExtensionDirection,
    },
    CherryPick {
        mode: &'a CherryPickMode,
        direction: ExtensionDirection,
    },
    Move {
        mode: &'a MoveMode,
        direction: ExtensionDirection,
    },
}

impl OperationExtension<'_> {
    const fn direction(self) -> ExtensionDirection {
        match self {
            Self::Commit { direction, .. }
            | Self::Move { direction, .. }
            | Self::CherryPick { direction, .. } => direction,
        }
    }
}

fn render_operation_extension_line(
    app: &App,
    data: &StatusOutputLineData,
    connector: Option<&[Span<'_>]>,
    area: Rect,
    extension: OperationExtension<'_>,
    frame: &mut Frame,
) {
    if app.has_focus {
        frame
            .buffer_mut()
            .set_style(area, app.selection_highlight_color());
    }

    let mut line = RenderSingleLineSpans::new(frame, area);
    extend_connector_spans(
        connector.unwrap_or_default(),
        extension.direction(),
        &mut line,
    );

    match extension {
        OperationExtension::Commit { mode, .. } => {
            render_commit_operation_target_marker(app, data, mode, &mut line);
        }
        OperationExtension::CherryPick { mode, .. } => {
            render_cherry_pick_operation_target_marker(app, data, mode, &mut line);
        }
        OperationExtension::Move { mode, .. } => {
            render_move_operation_target_marker(app, data, mode, &mut line);
        }
    }
}

pub(crate) fn render_commit_operation_target_marker(
    app: &App,
    data: &StatusOutputLineData,
    mode: &CommitMode,
    line: &mut RenderSingleLineSpans<'_, '_>,
) {
    let Some(target) = data.cli_id() else {
        return;
    };

    if mode.source.contains(target) {
        line.extend([source_span(app.theme), Span::raw(" ")]);
        line.extend(
            [
                Span::raw("<< ").mode_colors(&*app.mode, app.theme),
                Span::raw(NOOP).mode_colors(&*app.mode, app.theme),
            ]
            .into_iter()
            .chain(match mode.message_composer {
                CommitMessageComposer::Editor => None,
                CommitMessageComposer::Empty => {
                    Some(Span::raw(" (empty message)").mode_colors(&*app.mode, app.theme))
                }
                CommitMessageComposer::Inline => {
                    Some(Span::raw(" (reword inline)").mode_colors(&*app.mode, app.theme))
                }
            })
            .chain([
                Span::raw(" >>").mode_colors(&*app.mode, app.theme),
                Span::raw(" "),
            ]),
        );
    } else if let Some(display) = commit_operation_display(data, mode) {
        line.extend(
            [
                Span::raw("<< ").mode_colors(&*app.mode, app.theme),
                Span::raw(display).mode_colors(&*app.mode, app.theme),
            ]
            .into_iter()
            .chain(match mode.message_composer {
                CommitMessageComposer::Editor => None,
                CommitMessageComposer::Empty => {
                    Some(Span::raw(" (empty message)").mode_colors(&*app.mode, app.theme))
                }
                CommitMessageComposer::Inline => {
                    Some(Span::raw(" (reword inline)").mode_colors(&*app.mode, app.theme))
                }
            })
            .chain([
                Span::raw(" >>").mode_colors(&*app.mode, app.theme),
                Span::raw(" "),
            ]),
        );
    }
}

pub(crate) fn render_move_operation_target_marker(
    app: &App,
    data: &StatusOutputLineData,
    mode: &MoveMode,
    line: &mut RenderSingleLineSpans<'_, '_>,
) {
    if data
        .cli_id()
        .is_some_and(|target| mode.source.contains(target))
    {
        line.extend([source_span(app.theme), Span::raw(" ")]);
        line.extend([
            Span::raw("<< ").mode_colors(&*app.mode, app.theme),
            Span::raw(NOOP).mode_colors(&*app.mode, app.theme),
            Span::raw(" >>").mode_colors(&*app.mode, app.theme),
            Span::raw(" "),
        ]);
    } else if let Some(display) = move_operation_display(data, mode) {
        line.extend([
            Span::raw("<< ").mode_colors(&*app.mode, app.theme),
            Span::raw(display).mode_colors(&*app.mode, app.theme),
            Span::raw(" >>").mode_colors(&*app.mode, app.theme),
            Span::raw(" "),
        ]);
    }
}

pub(crate) fn render_cherry_pick_operation_target_marker(
    app: &App,
    data: &StatusOutputLineData,
    mode: &CherryPickMode,
    line: &mut RenderSingleLineSpans<'_, '_>,
) {
    if data
        .cli_id()
        .is_some_and(|target| mode.source.contains(target))
    {
        line.extend([source_span(app.theme), Span::raw(" ")]);
        line.extend([
            Span::raw("<< ").mode_colors(&*app.mode, app.theme),
            Span::raw(NOOP).mode_colors(&*app.mode, app.theme),
            Span::raw(" >>").mode_colors(&*app.mode, app.theme),
            Span::raw(" "),
        ]);
    } else if let Some(display) = cherry_pick_operation_display(data, mode) {
        line.extend([
            Span::raw("<< ").mode_colors(&*app.mode, app.theme),
            Span::raw(display).mode_colors(&*app.mode, app.theme),
            Span::raw(" >>").mode_colors(&*app.mode, app.theme),
            Span::raw(" "),
        ]);
    }
}

pub(crate) fn render_move_stack_operation_target_marker(
    app: &App,
    data: &StatusOutputLineData,
    mode: &MoveStackMode,
    line: &mut RenderSingleLineSpans<'_, '_>,
) {
    if data
        .cli_id()
        .is_some_and(|target| mode.source.matches(target))
    {
        line.extend([source_span(app.theme), Span::raw(" ")]);
        line.extend([
            Span::raw("<< ").mode_colors(&*app.mode, app.theme),
            Span::raw(NOOP).mode_colors(&*app.mode, app.theme),
            Span::raw(" >>").mode_colors(&*app.mode, app.theme),
            Span::raw(" "),
        ]);
    } else if let Some(display) = reorder_operation_display(data, mode) {
        line.extend([
            Span::raw("<< ").mode_colors(&*app.mode, app.theme),
            Span::raw(display).mode_colors(&*app.mode, app.theme),
            Span::raw(" >>").mode_colors(&*app.mode, app.theme),
            Span::raw(" "),
        ]);
    }
}

fn render_hot_bar(app: &App, area: Rect, frame: &mut Frame) {
    let mode_span = Span::raw(ModeDiscriminant::from(&*app.mode).hotbar_str())
        .mode_colors(&*app.mode, app.theme);

    let loading_spinner_started_at = app
        .details
        .started_polling_thread_at()
        .filter(|started_at| started_at.elapsed() > Duration::from_secs_f32(1.0));

    let layout = if loading_spinner_started_at.is_some() {
        Layout::horizontal([
            Constraint::Length(mode_span.width() as _),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area)
    } else {
        Layout::horizontal([
            Constraint::Length(mode_span.width() as _),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area)
    };

    frame.render_widget(mode_span, layout[0]);

    frame.render_widget(" ", layout[1]);

    app.mode
        .as_mode_render()
        .render_hot_bar_content(app, layout[2], frame);

    if let Some(started_at) = loading_spinner_started_at {
        let mut line = RenderSingleLineSpans::new(frame, layout[3]);
        render_spinner(app.theme, &mut line, started_at);
    }
}

pub(crate) fn render_spinner(
    theme: &Theme,
    line: &mut RenderSingleLineSpans<'_, '_>,
    started_at: Instant,
) {
    const FRAME_DURATION: Duration = Duration::from_millis(80);

    static STATES: &[&str] = &["⣾", "⣷", "⣯", "⣟", "⡿", "⢿", "⣽", "⣻"];
    line.render(Span::raw(" "));
    let state = STATES
        [(started_at.elapsed().as_millis() / FRAME_DURATION.as_millis()) as usize % STATES.len()];
    line.render(Span::raw(state).style(theme.hint));
    line.render(Span::raw(" "));
}

const HOT_BAR_ITEM_SEPARATOR: &str = " • ";
const HOT_BAR_ITEM_SPACE: &str = " ";

fn hot_bar_item_width(key_bind: &KeyBind, include_separator: bool) -> usize {
    usize::from(include_separator) * HOT_BAR_ITEM_SEPARATOR.width()
        + key_bind.chord_display().width()
        + HOT_BAR_ITEM_SPACE.width()
        + key_bind.short_description().width()
}

fn render_hot_bar_item(
    line: &mut RenderSingleLineSpans<'_, '_>,
    key_bind: &KeyBind,
    include_separator: bool,
    theme: &'static Theme,
) {
    if include_separator {
        line.render(Span::styled(HOT_BAR_ITEM_SEPARATOR, theme.hint));
    }
    line.render(Span::styled(key_bind.chord_display(), theme.legend));
    line.render(Span::raw(HOT_BAR_ITEM_SPACE));
    line.render(Span::styled(key_bind.short_description(), theme.hint));
}

fn render_toasts(app: &App, area: Rect, frame: &mut Frame) {
    toast::render_toasts(frame, area, &app.toasts, app.theme);
}

fn render_debug(app: &App, area: Rect, frame: &mut Frame) {
    let renders = once(Span::raw("FPS").black().on_blue()).chain(once(Span::raw(format!(
        "{} FPS ({} renders)",
        app.fps.fps(),
        app.renders
    ))));
    let renders_line_count = 2;

    let backstack = format!("{:#?}", app.backstack);
    let backstack_line_count = 1 + backstack.lines().count();
    let backstack =
        once(Span::raw("Backstack").black().on_blue()).chain(backstack.lines().map(Span::raw));

    let marks = format!("{:#?}", app.marks_ref());
    let marks_line_count = 1 + marks.lines().count();
    let marks = once(Span::raw("Marks").black().on_blue()).chain(marks.lines().map(Span::raw));

    let details_worker_busy = format!("Worker busy: {}", app.details.worker_is_busy());
    let details_cache_size = format!("Cache size: {} lines", app.details.cache_size());
    let details_line_count =
        1 + details_worker_busy.lines().count() + details_cache_size.lines().count();
    let details = once(Span::raw("Details").black().on_blue()).chain(
        details_worker_busy
            .lines()
            .chain(details_cache_size.lines())
            .map(Span::raw),
    );

    let details_selection = app
        .details
        .selected_section_cli_id()
        .map(|id| format!("{id:#?}"));
    let details_selection_line_count = 1 + details_selection
        .as_deref()
        .map_or(0, |selection| selection.lines().count());
    let details_selection = once(Span::raw("Details selection").black().on_blue()).chain(
        details_selection
            .as_deref()
            .into_iter()
            .flat_map(str::lines)
            .map(Span::raw),
    );

    let status_selection = format!("{:#?}", app.cursor.selected_line(&app.status_lines));
    let status_selection_line_count = 1 + status_selection.lines().count();
    let status_selection = once(Span::raw("Status selection").black().on_blue())
        .chain(status_selection.lines().map(Span::raw));

    let total_line_count = renders_line_count
        + backstack_line_count
        + marks_line_count
        + details_line_count
        + details_selection_line_count
        + status_selection_line_count
        + 5;
    let max_scroll_top = total_line_count.saturating_sub(area.height as usize);
    let scroll_top = app.debug_scroll.top().min(max_scroll_top);
    app.debug_scroll.set_top(scroll_top);

    let spans = renders
        .chain(once(Span::raw("")))
        .chain(backstack)
        .chain(once(Span::raw("")))
        .chain(marks)
        .chain(once(Span::raw("")))
        .chain(details)
        .chain(once(Span::raw("")))
        .chain(details_selection)
        .chain(once(Span::raw("")))
        .chain(status_selection);

    for (span, line_area) in spans.skip(scroll_top).zip(available_lines_in_area(area)) {
        if span.style.bg.is_some() {
            frame.buffer_mut().set_style(line_area, span.style);
        }
        RenderSingleLineSpans::new(frame, line_area).render(span);
    }
}

pub fn commit_operation_display(
    data: &StatusOutputLineData,
    mode: &CommitMode,
) -> Option<&'static str> {
    let CommitMode {
        insert_side,
        scope_to_stack,
        source: _,
        message_composer: _,
    } = mode;

    match data {
        StatusOutputLineData::Branch { cli_id, .. } => {
            let outside_scope = match (scope_to_stack, cli_id.lane()) {
                (Some(_), Some(LaneId::Worktree(_))) => true,
                (Some(stack_scope), Some(LaneId::Stack(Some(stack_id)))) => stack_scope != stack_id,
                _ => false,
            };
            if outside_scope {
                None
            } else {
                Some("commit to branch")
            }
        }
        StatusOutputLineData::Commit { stack_id, .. } => {
            if let Some(stack_scope) = scope_to_stack
                && Some(*stack_scope) != *stack_id
            {
                // don't allow selecting commits outside the scoped stack
                None
            } else {
                match insert_side {
                    Side::Above => Some("commit above"),
                    Side::Below => Some("commit below"),
                }
            }
        }
        StatusOutputLineData::WorktreeUncommitted { .. } => None,
        StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::UncommittedFile { .. }
        | StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
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

pub fn move_operation_display(
    data: &StatusOutputLineData,
    mode: &MoveMode,
) -> Option<&'static str> {
    let MoveMode {
        source,
        insert_side,
    } = mode;
    match source {
        MoveSource::Commit { .. } => match data {
            StatusOutputLineData::Commit { .. } => match insert_side {
                InsertSide::Above => Some("move commit above"),
                InsertSide::Below => Some("move commit below"),
            },
            StatusOutputLineData::Branch { .. } => Some("move commit to branch"),
            StatusOutputLineData::WorktreeUncommitted { .. } => None,
            StatusOutputLineData::MergeBase => Some("move commit to new branch"),
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::UncommittedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => None,
        },
        MoveSource::CommittedChange(change) => {
            move_committed_changes_display(data, *insert_side, &NonEmpty::new(change))
        }
        MoveSource::Marks(MoveMarks::Commits(commits)) => match data {
            StatusOutputLineData::Commit { .. } => match (insert_side, commits.len() == 1) {
                (InsertSide::Above, true) => Some("move commit above"),
                (InsertSide::Above, false) => Some("move commits above"),
                (InsertSide::Below, true) => Some("move commit below"),
                (InsertSide::Below, false) => Some("move commits below"),
            },
            StatusOutputLineData::Branch { .. } => match commits.len() == 1 {
                true => Some("move commit to branch"),
                false => Some("move commits to branch"),
            },
            StatusOutputLineData::WorktreeUncommitted { .. } => None,
            StatusOutputLineData::MergeBase => match commits.len() == 1 {
                true => Some("move commit to new branch"),
                false => Some("move commits to new branch"),
            },
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::UncommittedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => None,
        },
        MoveSource::Marks(MoveMarks::Committed(marks)) => {
            move_committed_changes_display(data, *insert_side, &committed_changes_to_act_on(marks))
        }
        MoveSource::Branch(..) => match data {
            StatusOutputLineData::Branch { .. } => Some("stack branch"),
            StatusOutputLineData::MergeBase => Some("unstack branch"),
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::WorktreeUncommitted { .. }
            | StatusOutputLineData::UncommittedFile { .. }
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

/// The move target marker for committed `changes`, named after whether they're files, hunks or
/// both.
fn move_committed_changes_display(
    data: &StatusOutputLineData,
    insert_side: InsertSide,
    changes: &NonEmpty<&CommittedMark>,
) -> Option<&'static str> {
    let file_count = changes
        .iter()
        .filter(|change| matches!(change, CommittedMark::File(..)))
        .count();
    let [above, below, to_branch, to_new_branch] = match (file_count, changes.len()) {
        (1, 1) => [
            "move file above",
            "move file below",
            "move file to branch",
            "move file to new branch",
        ],
        (0, 1) => [
            "move hunk above",
            "move hunk below",
            "move hunk to branch",
            "move hunk to new branch",
        ],
        (file_count, change_count) if file_count == change_count => [
            "move files above",
            "move files below",
            "move files to branch",
            "move files to new branch",
        ],
        (0, _) => [
            "move hunks above",
            "move hunks below",
            "move hunks to branch",
            "move hunks to new branch",
        ],
        _ => [
            "move changes above",
            "move changes below",
            "move changes to branch",
            "move changes to new branch",
        ],
    };
    match data {
        StatusOutputLineData::Commit { .. } => match insert_side {
            InsertSide::Above => Some(above),
            InsertSide::Below => Some(below),
        },
        StatusOutputLineData::Branch { .. } => Some(to_branch),
        StatusOutputLineData::WorktreeUncommitted { .. } => None,
        StatusOutputLineData::MergeBase => Some(to_new_branch),
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::UncommittedFile { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    }
}

pub fn reorder_operation_display(
    data: &StatusOutputLineData,
    _mode: &MoveStackMode,
) -> Option<&'static str> {
    match data {
        StatusOutputLineData::BetweenStacks => Some("move stack"),
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::WorktreeUncommitted { .. }
        | StatusOutputLineData::UncommittedFile { .. }
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
    }
}

pub fn stack_operation_display(
    data: &StatusOutputLineData,
    mode: &StackMode,
) -> Option<&'static str> {
    let StackMode { stack_heads } = mode;
    match data {
        StatusOutputLineData::Branch { cli_id, .. } => {
            let CliId::Branch(branch) = &**cli_id else {
                return None;
            };
            if stack_heads.iter().any(|head| head.shorten() == branch.name) {
                Some("stack")
            } else {
                None
            }
        }
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::WorktreeUncommitted { .. }
        | StatusOutputLineData::UncommittedFile { .. }
        | StatusOutputLineData::Commit { .. }
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

pub fn cherry_pick_operation_display(
    data: &StatusOutputLineData,
    mode: &CherryPickMode,
) -> Option<&'static str> {
    let CherryPickMode { insert_side, .. } = mode;
    match data {
        StatusOutputLineData::Branch { .. } => Some("pick to branch"),
        StatusOutputLineData::Commit { .. } => match insert_side {
            InsertSide::Above => Some("pick above"),
            InsertSide::Below => Some("pick below"),
        },
        StatusOutputLineData::MergeBase => Some("pick to new unstacked branch"),
        StatusOutputLineData::WorktreeUncommitted { .. } => None,
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedFile { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    }
}

pub fn branch_operation_display(
    data: &StatusOutputLineData,
    _mode: &BranchMode,
) -> Option<&'static str> {
    match data {
        StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::Branch { .. }
        | StatusOutputLineData::MergeBase => Some("branch"),
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::WorktreeUncommitted { .. }
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedFile { .. }
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

pub fn worktree_operation_display(
    data: &StatusOutputLineData,
    _mode: &WorktreeMode,
) -> Option<&'static str> {
    match data {
        StatusOutputLineData::Branch { cli_id, .. }
            if cli_id.lane().and_then(LaneId::worktree_name).is_some() =>
        {
            Some("worktree")
        }
        StatusOutputLineData::Commit { .. } | StatusOutputLineData::MergeBase => Some("worktree"),
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::Branch { .. }
        | StatusOutputLineData::WorktreeUncommitted { .. }
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedFile { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    }
}

pub(crate) fn source_span(theme: &'static Theme) -> Span<'static> {
    Span::raw("<< source >>").mode_colors(ModeDiscriminant::Normal, theme)
}

pub trait SpanExt<M> {
    fn mode_colors(self, mode: M, theme: &'static Theme) -> Self;
}

impl SpanExt<&Mode> for Span<'_> {
    fn mode_colors(self, mode: &Mode, theme: &'static Theme) -> Self {
        self.mode_colors(ModeDiscriminant::from(mode), theme)
    }
}

impl SpanExt<ModeDiscriminant> for Span<'_> {
    fn mode_colors(self, mode: ModeDiscriminant, theme: &'static Theme) -> Self {
        self.fg(mode.fg(theme)).bg(mode.bg(theme))
    }
}

pub struct StatusLayout {
    pub status_area: Rect,
    pub details_area: Option<Rect>,
}

fn cursor_at_end(textarea: &TextArea<'_>) -> bool {
    let (_, col) = textarea.cursor();
    col == textarea.lines()[0].chars().count()
}

/// Underlines the characters at `char_indices`, counted across all of `content`.
fn underline_jump_matches(content: &[Span<'static>], char_indices: &[usize]) -> Vec<Span<'static>> {
    let mut styled_content = Vec::with_capacity(content.len());
    let mut char_idx = 0;
    for span in content {
        let mut run = String::new();
        let mut run_is_match = false;
        for c in span.content.chars() {
            let is_match = char_indices.contains(&char_idx);
            if is_match != run_is_match && !run.is_empty() {
                styled_content.push(jump_match_run(
                    std::mem::take(&mut run),
                    span.style,
                    run_is_match,
                ));
            }
            run_is_match = is_match;
            run.push(c);
            char_idx += 1;
        }
        if !run.is_empty() {
            styled_content.push(jump_match_run(run, span.style, run_is_match));
        }
    }
    styled_content
}

fn jump_match_run(run: String, style: Style, is_match: bool) -> Span<'static> {
    let span = Span::styled(run, style);
    if is_match { span.underlined() } else { span }
}

impl Mode {
    fn as_mode_render(&self) -> &dyn ModeRender {
        match self {
            Mode::Normal(mode) => mode,
            Mode::Squash(mode) => mode,
            Mode::InlineReword(mode) => mode,
            Mode::Command(mode) => mode,
            Mode::Commit(mode) => mode,
            Mode::Move(mode) => mode,
            Mode::Details(mode) => mode,
            Mode::Stack(mode) => mode,
            Mode::MoveStack(mode) => mode,
            Mode::PickChanges(mode) => mode,
            Mode::Jump(mode) => mode,
            Mode::CherryPick(mode) => mode,
            Mode::Branch(mode) => mode,
            Mode::Worktree(mode) => mode,
        }
    }
}

pub(crate) trait ModeRender {
    // ┊│   << target >>
    // ┊●   982b7d85c5 my commit
    // ^^^^^^^^^^^^^^^^^^^^^^^^^ configure an operation extension line above or below the selected row
    #[allow(unused_variables)]
    fn operation_extension(&self, data: &StatusOutputLineData) -> Option<OperationExtension<'_>> {
        None
    }

    // ┊●   << source >> 982b7d85c5 my commit
    //      ^^^^^^^^^^^^ render source labels
    #[allow(unused_variables)]
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
    }

    // ┊●   << target >> 982b7d85c5 my commit
    //      ^^^^^^^^^^^^ render target labels
    #[allow(unused_variables)]
    fn render_operation_source_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
    }

    // Renders the mode specific content in the hot bar.
    //
    // For most modes that is key binds but some modes, such as command mode, override that.
    fn render_hot_bar_content(&self, app: &App, area: Rect, frame: &mut Frame) {
        let mode = &*app.mode;
        let key_binds = app.active_key_binds();
        let selection = app
            .cursor
            .selected_line(&app.status_lines)
            .and_then(|line| Some(&**line.data.cli_id()?));
        let always_show_count = key_binds
            .iter_key_binds_available_in_mode(mode, selection)
            .filter(|key_bind| !key_bind.hide_from_hotbar())
            .filter(|key_bind| key_bind.always_show_in_hot_bar())
            .count();
        let always_show_width_without_separators = key_binds
            .iter_key_binds_available_in_mode(mode, selection)
            .filter(|key_bind| !key_bind.hide_from_hotbar())
            .filter(|key_bind| key_bind.always_show_in_hot_bar())
            .map(|key_bind| hot_bar_item_width(key_bind, false))
            .sum::<usize>();

        let always_show_width = |rendered_before: bool| {
            let separator_count = match (always_show_count, rendered_before) {
                (0, _) => 0,
                (count, true) => count,
                (count, false) => count - 1,
            };
            always_show_width_without_separators + separator_count * HOT_BAR_ITEM_SEPARATOR.width()
        };

        let mut available_width = area.width as usize;
        let mut rendered_any = false;
        let mut line = RenderSingleLineSpans::new(frame, area);

        for key_bind in key_binds
            .iter_key_binds_available_in_mode(mode, selection)
            .filter(|key_bind| !key_bind.hide_from_hotbar())
            .filter(|key_bind| !key_bind.always_show_in_hot_bar())
        {
            let width = hot_bar_item_width(key_bind, rendered_any);
            let Some(remaining_width_after_item) = available_width.checked_sub(width) else {
                break;
            };
            if remaining_width_after_item < always_show_width(true) {
                break;
            }

            render_hot_bar_item(&mut line, key_bind, rendered_any, app.theme);
            rendered_any = true;
            available_width = remaining_width_after_item;
        }

        for key_bind in key_binds
            .iter_key_binds_available_in_mode(mode, selection)
            .filter(|key_bind| !key_bind.hide_from_hotbar())
            .filter(|key_bind| key_bind.always_show_in_hot_bar())
        {
            render_hot_bar_item(&mut line, key_bind, rendered_any, app.theme);
            rendered_any = true;
        }
    }
}

pub fn available_lines_in_area(area: Rect) -> impl Iterator<Item = Rect> {
    (0..area.height).map(move |i| {
        let y = area.y + i;
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        }
    })
}

/// Render `Span`s onto a single line without allocating a `Line`. Just render one `Span` after the
/// next.
pub struct RenderSingleLineSpans<'a, 'b> {
    frame: &'a mut Frame<'b>,
    area: Rect,
}

impl<'a, 'b> RenderSingleLineSpans<'a, 'b> {
    pub fn new(frame: &'a mut Frame<'b>, area: Rect) -> Self {
        Self { frame, area }
    }

    pub fn render(&mut self, span: Span<'_>) {
        self.render_ref(&span);
    }

    pub fn render_ref(&mut self, span: &Span<'_>) {
        if self.area.width == 0 {
            return;
        }

        let width = span.width().min(self.area.width as usize) as u16;
        let area = Rect { width, ..self.area };
        self.frame.render_widget(span, area);
        self.area = Rect {
            x: self.area.x.saturating_add(width),
            width: self.area.width.saturating_sub(width),
            ..self.area
        };
    }

    pub fn render_textarea(&mut self, textarea: &TextArea<'_>) {
        let content_width = textarea
            .lines()
            .first()
            .map(|line| line.width())
            .unwrap_or_default();
        let cursor_padding = usize::from(cursor_at_end(textarea));
        let width = content_width
            .saturating_add(cursor_padding)
            .max(1)
            .min(self.area.width as usize) as u16;
        let area = Rect { width, ..self.area };

        self.frame.render_widget(textarea, area);
        self.area = Rect {
            x: self.area.x.saturating_add(width),
            width: self.area.width.saturating_sub(width),
            ..self.area
        };
    }

    pub fn area_used_by<F>(&mut self, f: F) -> Rect
    where
        F: FnOnce(&mut Self),
    {
        let area_before = self.area;
        f(self);
        let area_after = self.area;
        Rect {
            width: area_before.width.saturating_sub(area_after.width),
            ..area_before
        }
    }
}

impl<'a> Extend<Span<'a>> for RenderSingleLineSpans<'_, '_> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Span<'a>>,
    {
        for span in iter {
            self.render(span);
        }
    }
}

impl<'a, 'b> Extend<&'a Span<'b>> for RenderSingleLineSpans<'_, '_> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = &'a Span<'b>>,
    {
        for span in iter {
            self.render_ref(span);
        }
    }
}
