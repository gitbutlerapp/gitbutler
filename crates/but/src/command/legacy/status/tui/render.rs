use std::{borrow::Cow, iter::once};

use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use itertools::{Either, Itertools, Position};
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    prelude::*,
    widgets::{Block, BorderType, Borders, List, ListItem},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    command::legacy::status::{
        CommitLineContent, FileLineContent, StatusOutputLine,
        output::{BranchLineContent, StatusOutputContent, StatusOutputLineData},
        tui::{Markable, rub::squash_operation_display},
    },
    theme::Theme,
};

use super::{
    App, CURSOR_CONTEXT_ROWS, Modal, NOOP,
    cursor::is_selectable_in_mode,
    graph_extension::{ExtensionDirection, extend_connector_spans},
    highlight::with_highlight,
    mode::{
        CommandMode, CommandModeKind, CommitMessageComposer, CommitMode, InlineRewordMode, Mode,
        ModeDiscriminant, MoveMode, MoveSource, RubMode, RubSource,
    },
    rub, rub_from_detail_view, toast,
};

pub(super) fn render_app(app: &App, frame: &mut Frame) {
    let content_layout =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(frame.area());
    let main_content_area = content_layout[0];

    let (main_content_area, debug_area) = if app.options.debug {
        let layout = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(main_content_area);
        (layout[0], Some(layout[1]))
    } else {
        (main_content_area, None)
    };

    let hotbar_area = content_layout[1];

    let status_layout = status_layout(app, main_content_area);

    if let Mode::Details(details_mode) = &*app.mode
        && details_mode.full_screen
    {
        let block = pane_block(app, true, Borders::BOTTOM);
        let inner_area = block.inner(status_layout.status_area);
        frame.render_widget(block, status_layout.status_area);
        app.details.render(
            matches!(app.modal, Some(Modal::Help { .. })),
            app.has_focus,
            inner_area,
            frame,
        );
    } else {
        let details_focused = matches!(&*app.mode, Mode::Details(..));
        let status_block = pane_block(app, !details_focused, Borders::BOTTOM);
        let details_block = pane_block(app, details_focused, Borders::BOTTOM);

        {
            let inner_area = status_block.inner(status_layout.status_area);
            frame.render_widget(status_block, status_layout.status_area);
            render_status(app, inner_area, frame);
        }

        if let Some(details_area) = status_layout.details_area {
            let inner_area = details_content_area(app, details_area);
            let details_separator_area = details_block.inner(details_area);
            frame.render_widget(details_block, details_area);
            render_details_separator(app, details_separator_area, frame);
            app.details.render(
                matches!(app.modal, Some(Modal::Help { .. })),
                app.has_focus,
                inner_area,
                frame,
            );
        }
    }

    if let Some(debug_area) = debug_area {
        let outer_block = Block::bordered()
            .border_style(app.theme.border)
            .border_type(BorderType::Thick)
            .borders(Borders::LEFT);
        let inner_area = outer_block.inner(debug_area);
        frame.render_widget(outer_block, debug_area);
        render_debug(app, inner_area, frame);
    }

    render_hotbar(app, hotbar_area, frame);

    render_toasts(
        app,
        status_layout
            .details_area
            .unwrap_or(status_layout.status_area),
        frame,
    );

    match &app.modal {
        Some(Modal::Confirm { confirm, .. }) => confirm.render(app.has_focus, frame.area(), frame),
        Some(Modal::BranchPicker { branch_picker, .. }) => {
            branch_picker.render(app.has_focus, frame.area(), frame);
        }
        Some(Modal::Help { help, .. }) => help.render(frame.area(), frame),
        None => {}
    }
}

fn render_details_separator(app: &App, area: Rect, frame: &mut Frame) {
    frame.render_widget(details_separator(app), area);
}

fn details_content_area(app: &App, details_area: Rect) -> Rect {
    let details_area = pane_block(
        app,
        matches!(&*app.mode, Mode::Details(..)),
        Borders::BOTTOM,
    )
    .inner(details_area);
    details_separator(app).inner(details_area)
}

fn details_separator(app: &App) -> Block<'static> {
    Block::bordered()
        .border_style(app.theme.border)
        .borders(Borders::LEFT)
}

fn pane_block(app: &App, focused: bool, borders: Borders) -> Block<'static> {
    let border_style = if focused {
        app.theme.default.fg(app.mode.bg(app.theme))
    } else {
        app.theme.border
    };
    let border_type = if focused {
        BorderType::Thick
    } else {
        BorderType::Plain
    };

    Block::bordered()
        .border_style(border_style)
        .border_type(border_type)
        .borders(borders)
}

pub(super) fn status_layout(app: &App, area: Rect) -> StatusLayout {
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

fn render_status(app: &App, content_area: Rect, frame: &mut Frame) {
    let visible_height = content_area.height as usize;
    let items = app
        .status_lines
        .iter()
        .enumerate()
        .flat_map(|(idx, tui_line)| {
            render_status_list_item(app, tui_line, app.cursor.index() == idx)
        })
        .skip(app.scroll_top)
        .take(visible_height);
    let list = List::new(items);

    frame.render_widget(list, content_area);

    render_inline_reword(app, content_area, frame);
}

pub(super) fn render_status_list_item(
    app: &App,
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
        if data
            .cli_id()
            .and_then(|id| Markable::try_from_cli_id(id))
            .is_some_and(|markable| app.marks().is_some_and(|marks| marks.contains(&markable)))
        {
            for (idx, span) in connector.iter().enumerate() {
                if idx == 1 {
                    line.push_span(app.theme.sym().mark.span());
                } else if idx == 2 {
                    // after the indicator is a bunch of spaces
                    for (c_idx, c) in span.content.chars().enumerate() {
                        line.push_span(if c_idx == 0 {
                            // color the background of the first space the same as the mark indicator
                            // since the checkmark symbol we use takes up more than one cell
                            Span::raw(c.to_string()).style(app.theme.tui_mark)
                        } else {
                            Span::raw(c.to_string())
                        });
                    }
                } else {
                    line.push_span(span.clone());
                }
            }
        } else {
            line.extend(connector.clone());
        }
    }

    let line_is_to_be_discarded = data.cli_id().is_some_and(|selection| {
        app.to_be_discarded
            .iter()
            .any(|to_be_discarded| to_be_discarded == selection)
    });

    if line_is_to_be_discarded {
        line.extend([Span::raw("<< discard >>").black().on_red(), Span::raw(" ")]);
    } else if is_selected {
        match &*app.mode {
            Mode::Normal(..) | Mode::InlineReword(..) | Mode::Command(..) | Mode::Details(..) => {}
            Mode::Rub(RubMode {
                source,
                how_to_combine_messages,
                available_targets: _,
                _unlock_details: _,
            }) => {
                render_rub_inline_labels_for_selected_line(
                    app,
                    data,
                    source,
                    *how_to_combine_messages,
                    &mut line,
                );
            }
            Mode::Commit(commit_mode) => {
                if data
                    .cli_id()
                    .is_some_and(|target| *commit_mode.source == **target)
                {
                    render_commit_labels_for_selected_line(app, data, commit_mode, &mut line);
                }
            }
            Mode::Move(move_mode) => {
                if data
                    .cli_id()
                    .is_some_and(|target| *move_mode.source == **target)
                    || matches!(data, StatusOutputLineData::MergeBase)
                {
                    render_move_labels_for_selected_line(app, data, move_mode, &mut line);
                }
            }
        }
    } else {
        match &*app.mode {
            Mode::Normal(..) | Mode::InlineReword(..) | Mode::Command(..) | Mode::Details(..) => {}
            Mode::Rub(RubMode {
                source,
                how_to_combine_messages: _,
                available_targets: _,
                _unlock_details: _,
            }) => {
                if let Some(cli_id) = data.cli_id()
                    && source.contains(cli_id)
                {
                    line.extend([source_span(app.theme), Span::raw(" ")]);
                }
            }
            Mode::Commit(CommitMode { source, .. }) => {
                if let Some(cli_id) = data.cli_id()
                    && **source == **cli_id
                {
                    line.extend([source_span(app.theme), Span::raw(" ")]);
                }
            }
            Mode::Move(MoveMode { source, .. }) => {
                if let Some(cli_id) = data.cli_id()
                    && **source == **cli_id
                {
                    line.extend([source_span(app.theme), Span::raw(" ")]);
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
            if data.cli_id().is_some_and(|id| app.highlight.contains(id)) {
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
            if data.cli_id().is_some_and(|id| app.highlight.contains(id)) {
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
            if data.cli_id().is_some_and(|id| app.highlight.contains(id)) {
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

    match &*app.mode {
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
        Mode::Normal(..)
        | Mode::Details(..)
        | Mode::Move(..)
        | Mode::Command(..)
        | Mode::Rub(..)
        | Mode::Commit(..) => {
            if is_selectable_in_mode(tui_line, &app.mode, app.flags.show_files) {
                line.extend(content_spans);
            } else {
                line.extend(
                    content_spans
                        .into_iter()
                        .map(|span| span.style(app.theme.hint)),
                );
            }
        }
    }

    if is_selected {
        match &*app.mode {
            Mode::Commit(commit_mode) => {
                if matches!(data, StatusOutputLineData::Commit { .. })
                    || matches!(data, StatusOutputLineData::Branch { .. })
                {
                    let mut extension_line =
                        highlight_line_if(Line::default(), app.has_focus, app.theme);
                    extend_connector_spans(
                        connector.as_deref().unwrap_or_default(),
                        ExtensionDirection::Below,
                        &mut extension_line,
                    );
                    render_commit_labels_for_selected_line(
                        app,
                        data,
                        commit_mode,
                        &mut extension_line,
                    );
                    return StatusListItem::Double(line, extension_line);
                }
            }
            Mode::Move(move_mode) => {
                if let StatusOutputLineData::Commit { cli_id: target, .. } = data
                    && *move_mode.source != **target
                {
                    let mut extension_line =
                        highlight_line_if(Line::default(), app.has_focus, app.theme);
                    extend_connector_spans(
                        connector.as_deref().unwrap_or_default(),
                        ExtensionDirection::Below,
                        &mut extension_line,
                    );
                    render_move_labels_for_selected_line(app, data, move_mode, &mut extension_line);
                    return StatusListItem::Double(line, extension_line);
                } else if let StatusOutputLineData::Branch { cli_id: target, .. } = data
                    && *move_mode.source != **target
                {
                    if move_mode.source.is_commit() {
                        let mut extension_line =
                            highlight_line_if(Line::default(), app.has_focus, app.theme);
                        extend_connector_spans(
                            connector.as_deref().unwrap_or_default(),
                            ExtensionDirection::Below,
                            &mut extension_line,
                        );
                        render_move_labels_for_selected_line(
                            app,
                            data,
                            move_mode,
                            &mut extension_line,
                        );
                        return StatusListItem::Double(line, extension_line);
                    } else {
                        let mut extension_line =
                            highlight_line_if(Line::default(), app.has_focus, app.theme);
                        extend_connector_spans(
                            connector.as_deref().unwrap_or_default(),
                            ExtensionDirection::Above,
                            &mut extension_line,
                        );
                        render_move_labels_for_selected_line(
                            app,
                            data,
                            move_mode,
                            &mut extension_line,
                        );
                        return StatusListItem::Double(extension_line, line);
                    }
                }
            }
            Mode::Normal(..)
            | Mode::Details(..)
            | Mode::Rub(..)
            | Mode::InlineReword(..)
            | Mode::Command(..) => {}
        }
    }

    line = highlight_line_if(
        line,
        is_selected && !matches!(app.modal, Some(Modal::Help { .. })) && app.has_focus,
        app.theme,
    );

    StatusListItem::Single(line)
}

fn highlight_line_if(line: Line<'static>, highlight: bool, theme: &'static Theme) -> Line<'static> {
    if highlight {
        line.style(theme.selection_highlight)
    } else {
        line
    }
}

fn render_rub_inline_labels_for_selected_line(
    app: &App,
    data: &StatusOutputLineData,
    source: &RubSource,
    how_to_combine_messages: MessageCombinationStrategy,
    line: &mut Line<'static>,
) {
    let Some(target) = data.cli_id() else {
        return;
    };

    if source.contains(target) {
        line.extend([source_span(app.theme), Span::raw(" ")]);
    }

    let display = match source {
        RubSource::CliId(source) => Cow::Borrowed(
            rub::rub_operation_display(NonEmpty::new(source), target, how_to_combine_messages)
                .unwrap_or("invalid"),
        ),
        RubSource::CommittedHunk(hunk) => Cow::Borrowed(
            rub_from_detail_view::rub_operation_display(hunk, target).unwrap_or("invalid"),
        ),
        RubSource::Marks(_) => {
            // squashing is currently the only operation that supports multiple sources
            Cow::Borrowed(squash_operation_display(how_to_combine_messages))
        }
    };
    line.extend([
        Span::raw("<< ").mode_colors(&*app.mode, app.theme),
        Span::raw(display).mode_colors(&*app.mode, app.theme),
        Span::raw(" >>").mode_colors(&*app.mode, app.theme),
        Span::raw(" "),
    ]);
}

fn render_commit_labels_for_selected_line(
    app: &App,
    data: &StatusOutputLineData,
    mode: &CommitMode,
    line: &mut Line<'static>,
) {
    let Some(target) = data.cli_id() else {
        return;
    };

    if *mode.source == **target {
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

fn render_move_labels_for_selected_line(
    app: &App,
    data: &StatusOutputLineData,
    mode: &MoveMode,
    line: &mut Line<'static>,
) {
    if data.cli_id().is_some_and(|target| *mode.source == **target) {
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

fn render_hotbar(app: &App, area: Rect, frame: &mut Frame) {
    let mode_span = Span::raw(format!(
        "  {}  ",
        ModeDiscriminant::from(&*app.mode).hotbar_string()
    ))
    .mode_colors(&*app.mode, app.theme);

    let layout = Layout::horizontal([
        Constraint::Length(mode_span.width() as _),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);

    frame.render_widget(mode_span, layout[0]);

    frame.render_widget(" ", layout[1]);

    match &*app.mode {
        Mode::Normal(..)
        | Mode::Details(..)
        | Mode::Rub(..)
        | Mode::Commit(..)
        | Mode::Move(..)
        | Mode::InlineReword(..) => {
            let separator = Span::styled(" • ", app.theme.hint);
            let area = layout[2];

            let items = app
                .active_key_binds()
                .iter_key_binds_available_in_mode(ModeDiscriminant::from(&*app.mode))
                .filter(|key_bind| !key_bind.hide_from_hotbar())
                .with_position()
                .map(|(pos, key_bind)| {
                    let show_sep = match pos {
                        Position::First | Position::Only => false,
                        Position::Middle | Position::Last => true,
                    };

                    let separator = show_sep.then(|| separator.clone());
                    let chord = Span::styled(key_bind.chord_display(), app.theme.legend);
                    let space = Span::raw(" ");
                    let description = Span::styled(key_bind.short_description(), app.theme.hint);

                    (
                        key_bind,
                        HotBarItem {
                            chord,
                            space,
                            description,
                            separator,
                        },
                    )
                })
                .collect::<Vec<_>>();

            let always_show = items
                .iter()
                .filter(|(key_bind, _)| key_bind.always_show_in_hot_bar())
                .cloned()
                .collect::<Vec<_>>();

            let mut available_width = area.width as usize;
            let mut line = Line::default();

            for (key_bind, item) in items {
                if key_bind.always_show_in_hot_bar() {
                    continue;
                }

                if let Some(remaining_width_after_item) = item.fits_in_hot_bar(available_width)
                    && always_show
                        .iter()
                        .try_fold(remaining_width_after_item, |remaining, (_, item)| {
                            item.fits_in_hot_bar(remaining)
                        })
                        .is_some()
                {
                    available_width = remaining_width_after_item;
                } else {
                    break;
                }

                item.extend_line(&mut line);
            }
            for (_, item) in always_show {
                item.extend_line(&mut line);
            }

            frame.render_widget(line, area);
        }
        Mode::Command(CommandMode { textarea, kind }) => {
            let command_layout = Layout::horizontal([
                match kind {
                    CommandModeKind::But => Constraint::Length(4),
                    CommandModeKind::Shell => Constraint::Length(2),
                },
                Constraint::Min(1),
            ])
            .split(layout[2]);

            match kind {
                CommandModeKind::But => {
                    frame.render_widget("but ", command_layout[0]);
                }
                CommandModeKind::Shell => {
                    frame.render_widget("$ ", command_layout[0]);
                }
            }
            frame.render_widget(&**textarea, command_layout[1]);
        }
    }
}

#[derive(Clone)]
struct HotBarItem<'a> {
    chord: Span<'a>,
    space: Span<'a>,
    description: Span<'a>,
    separator: Option<Span<'a>>,
}

impl<'a> HotBarItem<'a> {
    fn width(&self) -> usize {
        self.chord.width()
            + self.space.width()
            + self.description.width()
            + self.separator.as_ref().map_or(0, |s| s.width())
    }

    fn extend_line(self, line: &mut Line<'a>) {
        let HotBarItem {
            chord,
            space,
            description,
            separator,
        } = self;
        line.extend(separator);
        line.extend([chord, space, description]);
    }

    fn fits_in_hot_bar(&self, available_width: usize) -> Option<usize> {
        available_width.checked_sub(self.width())
    }
}

fn render_toasts(app: &App, area: Rect, frame: &mut Frame) {
    toast::render_toasts(frame, area, &app.toasts, app.theme);
}

fn render_inline_reword(app: &App, area: Rect, frame: &mut Frame) {
    let inline_reword_mode = if let Mode::InlineReword(inline_reword_mode) = &*app.mode {
        inline_reword_mode
    } else {
        return;
    };

    let selected_idx = app.cursor.index();
    let Some(selected_rows) = selected_row_range(app) else {
        return;
    };
    if selected_rows.start < app.scroll_top {
        return;
    }
    let idx = selected_rows.start - app.scroll_top;
    if idx >= area.height as usize {
        return;
    }
    let Some(line) = app.status_lines.get(selected_idx) else {
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

fn render_debug(app: &App, area: Rect, frame: &mut Frame) {
    let renders = once(ListItem::new("FPS").black().on_blue()).chain(once(ListItem::new(format!(
        "{} FPS ({} renders)",
        app.fps.fps(),
        app.renders
    ))));

    let backstack = format!("{:#?}", app.backstack);
    let backstack = once(ListItem::new("Backstack").black().on_blue()).chain(
        backstack
            .lines()
            .take(100)
            .map(|line| ListItem::new(line.to_owned())),
    );

    let details_selection = format!("{:#?}", app.details.selection());
    let details_selection = once(ListItem::new("Details selection").black().on_blue()).chain(
        details_selection
            .lines()
            .take(100)
            .map(|line| ListItem::new(line.to_owned())),
    );

    let status_selection = format!("{:#?}", app.cursor.selected_line(&app.status_lines));
    let status_selection = once(ListItem::new("Status selection").black().on_blue()).chain(
        status_selection
            .lines()
            .take(100)
            .map(|line| ListItem::new(line.to_owned())),
    );

    let list = List::new(
        renders
            .chain(once(ListItem::new("")))
            .chain(backstack)
            .chain(once(ListItem::new("")))
            .chain(details_selection)
            .chain(once(ListItem::new("")))
            .chain(status_selection),
    );

    frame.render_widget(list, area);
}

pub(super) fn commit_operation_display(
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
                Some("insert commit")
            }
        }
        StatusOutputLineData::Commit { stack_id, .. } => {
            if let Some(stack_scope) = mode.scope_to_stack
                && Some(stack_scope) != *stack_id
            {
                // don't allow selecting commits outside the scoped stack
                None
            } else {
                Some("insert commit")
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

pub(super) fn move_operation_display(
    data: &StatusOutputLineData,
    mode: &MoveMode,
) -> Option<&'static str> {
    match &*mode.source {
        MoveSource::Commit { .. } => match data {
            StatusOutputLineData::Commit { .. } | StatusOutputLineData::Branch { .. } => {
                Some("move commit")
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

fn source_span(theme: &'static Theme) -> Span<'static> {
    Span::raw("<< source >>").mode_colors(ModeDiscriminant::Normal, theme)
}

pub(super) trait SpanExt<M> {
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

pub(super) enum StatusListItem {
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

pub(super) struct StatusLayout {
    pub(super) status_area: Rect,
    pub(super) details_area: Option<Rect>,
}

/// Returns the status content area within the terminal.
fn status_content_area(terminal_area: Rect) -> Rect {
    Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(terminal_area)[0]
}

/// Returns the details viewport for the terminal area.
pub(super) fn details_viewport(app: &App, terminal_area: Rect) -> Rect {
    let content_area = status_content_area(terminal_area);
    status_layout(app, content_area)
        .details_area
        .map(|details_area| details_content_area(app, details_area))
        .unwrap_or(content_area)
}

/// Returns the number of terminal rows available for rendering the status list.
pub(super) fn status_viewport_height(app: &App, terminal_area: Rect) -> usize {
    let content_area = status_content_area(terminal_area);
    let status_area = status_layout(app, content_area).status_area;

    // The status pane uses a bottom border, so the inner list viewport is one row shorter
    // than the outer area.
    usize::from(status_area.height.saturating_sub(1)).max(1)
}

/// Returns the rendered height in terminal rows for the given status line.
fn rendered_height_for_status_line(app: &App, line_idx: usize) -> usize {
    app.status_lines
        .get(line_idx)
        .map(|line| {
            render_status_list_item(app, line, app.cursor.index() == line_idx)
                .into_iter()
                .count()
        })
        .unwrap_or(0)
}

/// Returns the total rendered height of the entire status list.
pub(super) fn total_rendered_height(app: &App) -> usize {
    (0..app.status_lines.len())
        .map(|idx| rendered_height_for_status_line(app, idx))
        .sum()
}

/// Returns the rendered row range occupied by the selected line.
pub(super) fn selected_row_range(app: &App) -> Option<std::ops::Range<usize>> {
    let selected_idx = app.cursor.index();
    let selected_line = app.status_lines.get(selected_idx)?;
    let start = (0..selected_idx)
        .map(|idx| rendered_height_for_status_line(app, idx))
        .sum();
    let len = render_status_list_item(app, selected_line, true)
        .into_iter()
        .count();
    Some(start..start.saturating_add(len))
}

/// Clamps the topmost visible rendered row to the available content height.
fn clamp_scroll_top(app: &mut App, visible_height: usize) {
    let max_scroll_top = total_rendered_height(app).saturating_sub(visible_height);
    app.scroll_top = app.scroll_top.min(max_scroll_top);
}

/// Adjusts the viewport so the selected line stays visible with context rows above and below
/// whenever possible.
pub(super) fn ensure_cursor_visible(app: &mut App, visible_height: usize) {
    clamp_scroll_top(app, visible_height);

    let Some(selected_rows) = selected_row_range(app) else {
        return;
    };

    let selected_height = selected_rows.end.saturating_sub(selected_rows.start);
    let context_rows = CURSOR_CONTEXT_ROWS.min(visible_height.saturating_sub(selected_height) / 2);

    let min_scroll_top = selected_rows
        .end
        .saturating_add(context_rows)
        .saturating_sub(visible_height);
    let max_scroll_top = selected_rows.start.saturating_sub(context_rows);

    if app.scroll_top < min_scroll_top {
        app.scroll_top = min_scroll_top;
    } else if app.scroll_top > max_scroll_top {
        app.scroll_top = max_scroll_top;
    }

    clamp_scroll_top(app, visible_height);
}
