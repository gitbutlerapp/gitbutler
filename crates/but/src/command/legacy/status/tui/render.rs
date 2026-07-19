use std::{
    collections::HashMap,
    iter::once,
    time::{Duration, Instant},
};

use but_core::ref_metadata::StackId;
use but_rebase::graph_rebase::mutate::InsertSide;
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
            BranchLineContent, StatusOutputContent, StatusOutputLineData, UncommittedLineContent,
        },
        tui::app::{
            CommitMessageComposer, CommitMode, JumpMode, MoveMode, MoveSource, MoveStackMode,
            StackMode, find_jump_match,
        },
    },
    theme::Theme,
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
        Some(Modal::CopySelectionPicker { picker, .. }) => {
            picker.render(app.has_focus, frame.area(), frame);
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

    let stack_highlight_rows = stack_highlight_rows(app);

    let mut areas = available_lines_in_area(area);

    for (idx, tui_line) in app
        .status_lines
        .iter()
        .enumerate()
        .skip(app.status_scroll.top())
    {
        let stack_highlight = stack_highlight_rows
            .as_ref()
            .is_some_and(|rows| rows.get(idx).copied().unwrap_or_default());
        if !render_status_list_item(
            app,
            tui_line,
            app.cursor.index() == idx,
            stack_highlight,
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

    app.status_scroll.set_top(scroll_top);
}

fn stack_highlight_rows(app: &App) -> Option<Vec<bool>> {
    let Mode::Stack(..) = &*app.mode else {
        return None;
    };

    let row_stack_ids = row_stack_ids(&app.status_lines);
    let selected_stack_id = row_stack_ids.get(app.cursor.index()).copied().flatten()?;

    Some(
        row_stack_ids
            .into_iter()
            .map(|stack_id| stack_id == Some(selected_stack_id))
            .collect(),
    )
}

fn row_stack_ids(lines: &[StatusOutputLine]) -> Vec<Option<StackId>> {
    let mut current_stack_id = None;
    let mut commit_stack_ids = HashMap::new();

    for line in lines {
        if let StatusOutputLineData::Commit {
            cli_id,
            stack_id: Some(stack_id),
            ..
        } = &line.data
            && let CliId::Commit { commit_id, .. } = &**cli_id
        {
            commit_stack_ids.insert(*commit_id, *stack_id);
        }
    }

    let mut row_stack_ids = lines
        .iter()
        .map(|line| match &line.data {
            StatusOutputLineData::Branch { cli_id, .. } => {
                let stack_id = stack_id_from_cli_id(cli_id.as_ref());
                current_stack_id = stack_id;
                stack_id
            }
            StatusOutputLineData::Commit { stack_id, .. } => {
                current_stack_id = *stack_id;
                *stack_id
            }
            StatusOutputLineData::StagedChanges { cli_id } => {
                let stack_id = stack_id_from_cli_id(cli_id.as_ref());
                current_stack_id = stack_id;
                stack_id
            }
            StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage => current_stack_id,
            StatusOutputLineData::Connector | StatusOutputLineData::BetweenStacks => None,
            StatusOutputLineData::File { cli_id } => match &**cli_id {
                CliId::CommittedFile { commit_id, .. } => {
                    let stack_id = commit_stack_ids
                        .get(commit_id)
                        .copied()
                        .or(current_stack_id);
                    current_stack_id = stack_id;
                    stack_id
                }
                CliId::UncommittedHunkOrFile(..) | CliId::PathPrefix { .. } => current_stack_id,
                CliId::Branch { .. }
                | CliId::Commit { .. }
                | CliId::Uncommitted { .. }
                | CliId::Stack { .. } => None,
            },
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::UncommittedFile { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => {
                current_stack_id = None;
                None
            }
        })
        .collect::<Vec<_>>();

    for idx in 0..lines.len() {
        if !matches!(lines[idx].data, StatusOutputLineData::Connector) {
            continue;
        }

        let stack_id_before = row_stack_ids[..idx]
            .iter()
            .rev()
            .find_map(|stack_id| *stack_id);
        let stack_id_after = row_stack_ids[idx + 1..]
            .iter()
            .find_map(|stack_id| *stack_id);

        if stack_id_before == stack_id_after {
            row_stack_ids[idx] = stack_id_before;
        }
    }

    row_stack_ids
}

fn stack_id_from_cli_id(cli_id: &CliId) -> Option<StackId> {
    match cli_id {
        CliId::Branch { stack_id, .. } => *stack_id,
        CliId::Stack { stack_id, .. } => Some(*stack_id),
        CliId::UncommittedHunkOrFile(..)
        | CliId::PathPrefix { .. }
        | CliId::CommittedFile { .. }
        | CliId::Commit { .. }
        | CliId::Uncommitted { .. } => None,
    }
}

#[must_use]
fn render_status_list_item(
    app: &App,
    tui_line: &StatusOutputLine,
    is_selected: bool,
    stack_highlight: bool,
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
    } = tui_line;

    let operation_extension = if is_selected {
        selected_operation_extension(app, data)
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

    if (is_selected || stack_highlight) && highlight_current_line {
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
            for (idx, span) in connector.iter().enumerate() {
                if idx == 1 {
                    line.render(mark_symbol.span());
                } else if idx == 2 {
                    // after the indicator is a bunch of spaces
                    for (c_idx, c) in span.content.chars().enumerate() {
                        line.render(if c_idx == 0 {
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
            .any(|to_be_discarded| to_be_discarded == selection)
    });

    // ┊●   << source >> 982b7d85c5 my commit
    //      ^^^^^^^^^^^^ render target/source labels
    if line_is_to_be_discarded {
        line.extend([Span::raw("<< discard >>").black().on_red(), Span::raw(" ")]);
    } else if is_selected {
        app.mode
            .as_mode_render()
            .render_operation_target_marker(app, data, &mut line);
    } else {
        app.mode
            .as_mode_render()
            .render_operation_source_marker(app, data, &mut line);
    }

    // Check if the line is the line that will be selected if we confirm the current jump mode
    // search. If so we highlight it so its clear where you'll land.
    let line_is_jump_match = if let Mode::Jump(jump_mode) = &*app.mode {
        find_jump_match(
            app.cursor,
            &app.status_lines,
            jump_mode,
            app.flags.show_files,
        )
        .and_then(|cursor_for_match| {
            if app.cursor == cursor_for_match {
                return None;
            }
            let current_line_id = data.cli_id()?;
            let match_ = cursor_for_match.selected_line(&app.status_lines)?;
            let id = match_.data.cli_id()?;
            Some(id == current_line_id)
        })
        .unwrap_or(false)
    } else {
        false
    };

    // ┊●   982b7d85c5 my commit
    //      ^^^^^^^^^^^^^^^^^^^^ render the main content
    let area_used_by_main_content = line.area_used_by(|line| {
        match content {
            StatusOutputContent::Plain(spans) => {
                line.extend(spans);
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
                    && let Mode::Jump(jump_mode) = &*app.mode
                {
                    line.extend(style_jump_mode_matches(
                        change_id,
                        jump_mode,
                        is_selected || line_is_jump_match,
                    ));
                } else {
                    line.extend(change_id.iter().cloned());
                }

                if line_has_copied_highlight && change_id.is_empty() {
                    line.extend(sha.iter().cloned().map(with_highlight));
                } else if change_id.is_empty()
                    && let Mode::Jump(jump_mode) = &*app.mode
                {
                    line.extend(style_jump_mode_matches(
                        sha,
                        jump_mode,
                        is_selected || line_is_jump_match,
                    ));
                } else {
                    line.extend(sha);
                }
                line.extend(author);

                if let Some(id) = data.cli_id()
                    && let CliId::Commit { commit_id, .. } = &**id
                    && let Mode::InlineReword(InlineRewordMode::Commit {
                        textarea,
                        commit_id: source,
                    }) = &*app.mode
                    && commit_id == source
                {
                    line.render(Span::raw(" "));
                    line.render_textarea(textarea);
                } else {
                    line.extend(message);
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
                } else if let Mode::Jump(jump_mode) = &*app.mode {
                    line.extend(style_jump_mode_matches(
                        id,
                        jump_mode,
                        is_selected || line_is_jump_match,
                    ));
                } else {
                    line.extend(id);
                }
                line.extend(decoration_start);

                if let Some(id) = data.cli_id()
                    && let CliId::Branch { name, .. } = &**id
                    && let Mode::InlineReword(InlineRewordMode::Branch {
                        textarea,
                        name: source,
                        ..
                    }) = &*app.mode
                    && name == source
                {
                    line.render_textarea(textarea);
                } else {
                    if line_has_copied_highlight {
                        line.extend(branch_name.iter().cloned().map(with_highlight));
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
                } else if let Mode::Jump(jump_mode) = &*app.mode {
                    line.extend(style_jump_mode_matches(
                        id,
                        jump_mode,
                        is_selected || line_is_jump_match,
                    ));
                } else {
                    line.extend(id);
                }
                line.extend(status);
                if line_has_copied_highlight {
                    line.extend(path.iter().cloned().map(with_highlight));
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
                } else if let Mode::Jump(jump_mode) = &*app.mode {
                    line.extend(style_jump_mode_matches(
                        id,
                        jump_mode,
                        is_selected || line_is_jump_match,
                    ));
                } else {
                    line.extend(id);
                }
                line.extend(decoration_start);
                line.extend(label);
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

    if !is_selectable_in_mode(tui_line, app.mode.as_ref(), app.flags.show_files) {
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
enum OperationExtension<'a> {
    Commit {
        mode: &'a CommitMode,
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
            Self::Commit { direction, .. } | Self::Move { direction, .. } => direction,
        }
    }
}

fn selected_operation_extension<'a>(
    app: &'a App,
    data: &StatusOutputLineData,
) -> Option<OperationExtension<'a>> {
    match &*app.mode {
        Mode::Commit(mode) => {
            if matches!(data, StatusOutputLineData::Commit { .. }) {
                Some(OperationExtension::Commit {
                    mode,
                    direction: mode.insert_side.into(),
                })
            } else if matches!(data, StatusOutputLineData::Branch { .. }) {
                Some(OperationExtension::Commit {
                    mode,
                    direction: ExtensionDirection::Below,
                })
            } else {
                None
            }
        }
        Mode::Move(mode) => {
            if let StatusOutputLineData::Commit { cli_id: target, .. } = data
                && !mode.source.contains(target)
            {
                Some(OperationExtension::Move {
                    mode,
                    direction: mode.insert_side.into(),
                })
            } else if let StatusOutputLineData::Branch { cli_id: target, .. } = data
                && !mode.source.contains(target)
            {
                let source_is_commit = match &*mode.source {
                    MoveSource::Marks(..) | MoveSource::Commit { .. } => true,
                    MoveSource::Branch { .. } => false,
                };
                Some(OperationExtension::Move {
                    mode,
                    direction: if source_is_commit {
                        ExtensionDirection::Below
                    } else {
                        ExtensionDirection::Above
                    },
                })
            } else {
                None
            }
        }
        Mode::Normal(..)
        | Mode::MoveStack(..)
        | Mode::PickChanges(..)
        | Mode::Details(..)
        | Mode::Rub(..)
        | Mode::Stack(..)
        | Mode::InlineReword(..)
        | Mode::Jump(..)
        | Mode::Command(..) => None,
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
        render_spinner(app, &mut line, started_at);
    }
}

fn render_spinner(app: &App, line: &mut RenderSingleLineSpans<'_, '_>, started_at: Instant) {
    const FRAME_DURATION: Duration = Duration::from_millis(80);

    static STATES: &[&str] = &["⣾", "⣷", "⣯", "⣟", "⡿", "⢿", "⣽", "⣻"];
    line.render(Span::raw(" "));
    let state = STATES
        [(started_at.elapsed().as_millis() / FRAME_DURATION.as_millis()) as usize % STATES.len()];
    line.render(Span::raw(state).style(app.theme.hint));
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
            if let Some(stack_scope) = scope_to_stack
                && let Some(stack_id) = cli_id.stack_id()
                && *stack_scope != stack_id
            {
                // don't allow selecting branches outside the scoped stack
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
                    InsertSide::Above => Some("commit above"),
                    InsertSide::Below => Some("commit below"),
                }
            }
        }
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
    match &**source {
        MoveSource::Commit { .. } => match data {
            StatusOutputLineData::Commit { .. } => match insert_side {
                InsertSide::Above => Some("move commit above"),
                InsertSide::Below => Some("move commit below"),
            },
            StatusOutputLineData::Branch { .. } => Some("move commit to branch"),
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
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => None,
        },
        MoveSource::Marks(marks) => match data {
            StatusOutputLineData::Commit { .. } => match insert_side {
                InsertSide::Above if marks.len() == 1 => Some("move commit above"),
                InsertSide::Above => Some("move commits above"),
                InsertSide::Below if marks.len() == 1 => Some("move commit below"),
                InsertSide::Below => Some("move commits below"),
            },
            StatusOutputLineData::Branch { .. } => {
                if marks.len() == 1 {
                    Some("move commit to branch")
                } else {
                    Some("move commits to branch")
                }
            }
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
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => None,
        },
        MoveSource::Branch { .. } => match data {
            StatusOutputLineData::Branch { .. } => Some("stack branch"),
            StatusOutputLineData::MergeBase => Some("unstack branch"),
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Commit { .. }
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
            let CliId::Branch { name, .. } = &**cli_id else {
                return None;
            };
            if stack_heads.iter().any(|head| head.shorten() == name) {
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

fn style_jump_mode_matches(
    content: &[Span<'static>],
    jump_mode: &JumpMode,
    is_selected: bool,
) -> impl IntoIterator<Item = Span<'static>> {
    use itertools::Either;

    let query = jump_mode.query();

    let Some(first_non_whitespace_index) = content
        .iter()
        .position(|span| !span.content.as_ref().chars().all(char::is_whitespace))
    else {
        return Either::Left(content.iter().cloned());
    };

    let (leading, rest) = content.split_at(first_non_whitespace_index);

    let mut remaining_query = query;
    for span in rest {
        if remaining_query.is_empty() {
            break;
        }

        let span_content = span.content.as_ref();
        if let Some(remaining) = remaining_query.strip_prefix(span_content) {
            remaining_query = remaining;
        } else if span_content.starts_with(remaining_query) {
            remaining_query = "";
        } else {
            return Either::Left(content.iter().cloned());
        }
    }
    if !remaining_query.is_empty() {
        return Either::Left(content.iter().cloned());
    }

    let next_char_style = if is_selected {
        Style::default().black().on_red()
    } else {
        Style::default().black().on_white()
    };

    let mut styled_content = Vec::with_capacity(content.len() + 2);
    styled_content.extend(leading.iter().cloned());

    let mut remaining_query_len = query.len();
    let mut highlighted_next_char = false;
    for span in rest {
        let span_content = span.content.as_ref();
        if highlighted_next_char || remaining_query_len >= span_content.len() {
            remaining_query_len = remaining_query_len.saturating_sub(span_content.len());
            styled_content.push(span.clone());
            continue;
        }

        let (matching, after_match) = span_content.split_at(remaining_query_len);
        let next_char_len = after_match
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or_default();
        let (next_char, remaining) = after_match.split_at(next_char_len);
        if !matching.is_empty() {
            styled_content.push(Span::styled(matching.to_owned(), span.style));
        }
        styled_content.push(Span::styled(next_char.to_owned(), span.style).style(next_char_style));
        if !remaining.is_empty() {
            styled_content.push(Span::styled(remaining.to_owned(), span.style));
        }
        highlighted_next_char = true;
    }

    if !highlighted_next_char && let Some(last_match) = styled_content.last_mut() {
        last_match.style = next_char_style;
    }

    Either::Right(styled_content.into_iter())
}

impl Mode {
    fn as_mode_render(&self) -> &dyn ModeRender {
        match self {
            Mode::Normal(mode) => mode,
            Mode::Rub(mode) => mode,
            Mode::InlineReword(mode) => mode,
            Mode::Command(mode) => mode,
            Mode::Commit(mode) => mode,
            Mode::Move(mode) => mode,
            Mode::Details(mode) => mode,
            Mode::Stack(mode) => mode,
            Mode::MoveStack(mode) => mode,
            Mode::PickChanges(mode) => mode,
            Mode::Jump(mode) => mode,
        }
    }
}

pub trait ModeRender {
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
        let mode = ModeDiscriminant::from(&*app.mode);
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
    pub(super) fn new(frame: &'a mut Frame<'b>, area: Rect) -> Self {
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
