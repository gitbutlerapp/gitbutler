use crossterm::event::Event;
use indexmap::IndexMap;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{
        Block, BorderType, Clear, List, ListItem, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};
use ratatui_textarea::TextArea;
use strum::IntoEnumIterator;
use unicode_width::UnicodeWidthStr;

use crate::{
    command::legacy::status::tui::{KeyBinds, mode::ModeDiscriminant, render::SpanExt},
    theme::Theme,
};

#[derive(Debug)]
pub(super) struct Help {
    theme: &'static Theme,
    sections: Vec<HelpSection>,
    scroll_top: usize,
    textarea: TextArea<'static>,
    default_cursor_style: Style,
    pub is_search_focused: bool,
}

struct HelpLayout {
    centered_area: Rect,
    fixed_header_area: Rect,
    key_binds_area: Rect,
    scrollbar_area: Rect,
}

impl Help {
    const HEIGHT_PERCENT: u16 = 80;
    const FIXED_HEADER_HEIGHT: u16 = 1;

    pub(super) fn new<'a>(
        key_binds: impl IntoIterator<Item = &'a KeyBinds>,
        theme: &'static Theme,
    ) -> Self {
        let mut mode_to_sections = IndexMap::<ModeDiscriminant, HelpSection>::new();

        for key_binds in key_binds {
            for mode in ModeDiscriminant::iter() {
                let section = mode_to_sections.entry(mode).or_insert_with(|| HelpSection {
                    mode: Some(mode),
                    items: Vec::new(),
                });

                for key_bind in key_binds.iter_key_binds_available_in_mode(mode) {
                    if key_bind.show_only_in_normal_mode_help_section()
                        && mode != ModeDiscriminant::Normal
                    {
                        continue;
                    }

                    let help_item = HelpItem {
                        chord_display: key_bind.chord_display().to_owned(),
                        short_description: key_bind.short_description().to_owned(),
                        long_description: key_bind
                            .long_description()
                            .map(|s| s.to_owned())
                            .unwrap_or_default(),
                    };
                    section.items.push(help_item);
                }
            }
        }

        let sections = mode_to_sections
            .into_values()
            .filter(|section| !section.items.is_empty())
            .collect();

        let textarea = TextArea::new(Default::default());
        let default_cursor_style = textarea.cursor_style();

        let mut help = Self {
            theme,
            sections,
            scroll_top: 0,
            textarea,
            is_search_focused: false,
            default_cursor_style,
        };

        help.update_textarea_style();

        help
    }

    pub(super) fn render(&self, area: Rect, frame: &mut Frame) {
        let layout = self.layout(area);

        frame.render_widget(Clear, layout.centered_area);

        let outer_block = self.outer_block();
        frame.render_widget(outer_block, layout.centered_area);

        self.render_search(layout.fixed_header_area, frame);
        self.render_key_binds_help(layout.key_binds_area, layout.scrollbar_area, frame);
    }

    fn render_search(&self, area: Rect, frame: &mut Frame) {
        let showing_placeholder = self
            .textarea
            .lines()
            .iter()
            .map(|line| line.len())
            .sum::<usize>()
            > 0;

        let layout = Layout::horizontal([
            Constraint::Length(if self.is_search_focused || showing_placeholder {
                2
            } else {
                1
            }),
            Constraint::Min(1),
        ])
        .split(area);

        frame.render_widget(Span::styled(">", self.theme.hint), layout[0]);
        frame.render_widget(&self.textarea, layout[1]);
    }

    fn render_key_binds_help(&self, area: Rect, scrollbar_area: Rect, frame: &mut Frame) {
        let longest_short_description = self
            .sections
            .iter()
            .flat_map(|section| &section.items)
            .map(|item| item.short_description.width())
            .max()
            .unwrap_or(0) as u16;

        let columns_layout = Layout::horizontal([
            Constraint::Length(11),
            Constraint::Length(1),
            Constraint::Length(longest_short_description),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

        let scroll_top = self.scroll_top.min(self.max_scroll_for_height(area.height));
        let list_entries = || self.list_entries().skip(scroll_top).take(area.height as _);

        // key bind
        frame.render_widget(
            List::new(list_entries().map(|entry| {
                match entry {
                    HelpLine::Section { mode } => ListItem::new(
                        Span::raw(center(mode.hotbar_string(), columns_layout[0].width as _))
                            .mode_colors(mode, self.theme),
                    ),
                    HelpLine::Item(help_item) => ListItem::new(
                        Line::from_iter([Span::styled(
                            &help_item.chord_display,
                            self.theme.legend,
                        )])
                        .right_aligned(),
                    ),
                    HelpLine::Empty => ListItem::new(""),
                }
            })),
            columns_layout[0],
        );

        // space between key bind and short description
        frame.render_widget(Clear, columns_layout[1]);

        // short description
        frame.render_widget(
            List::new(list_entries().map(|entry| match entry {
                HelpLine::Item(help_item) => ListItem::new(Span::raw(&help_item.short_description)),
                HelpLine::Section { .. } | HelpLine::Empty => ListItem::new(""),
            })),
            columns_layout[2],
        );

        // space between short description and description
        frame.render_widget(Clear, columns_layout[3]);

        // description
        frame.render_widget(
            List::new(list_entries().map(|entry| match entry {
                HelpLine::Item(help_item) => {
                    ListItem::new(Span::styled(&help_item.long_description, self.theme.hint))
                }
                HelpLine::Section { .. } | HelpLine::Empty => ListItem::new(""),
            })),
            columns_layout[4],
        );

        // scrollbar
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .style(self.theme.border);
        let mut scrollbar_state =
            ScrollbarState::new(self.max_scroll_for_height(area.height)).position(scroll_top);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }

    fn layout(&self, area: Rect) -> HelpLayout {
        let width = 100;

        let horizontal_layout = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .split(area);

        let centered_layout = Layout::vertical([Constraint::Length(self.height(area))])
            .flex(Flex::Center)
            .split(horizontal_layout[0]);
        let centered_area = centered_layout[0];

        let inner_area = self.outer_block().inner(centered_area);
        let content_layout = Layout::vertical([
            Constraint::Length(Self::FIXED_HEADER_HEIGHT),
            Constraint::Min(1),
        ])
        .split(inner_area);
        let fixed_header_area = content_layout[0];
        let key_binds_area = content_layout[1];
        let scrollbar_area = Rect {
            x: centered_area.right().saturating_sub(1),
            y: key_binds_area.y,
            width: 1,
            height: key_binds_area.height,
        };

        HelpLayout {
            centered_area,
            fixed_header_area,
            key_binds_area,
            scrollbar_area,
        }
    }

    fn outer_block(&self) -> Block<'static> {
        Block::bordered()
            .padding(Padding {
                left: 1,
                right: 1,
                top: 0,
                bottom: 0,
            })
            .border_type(BorderType::Rounded)
            .border_style(self.theme.border)
    }

    fn list_entries(&self) -> impl Iterator<Item = HelpLine<'_>> {
        let section_count = self.sections.len();
        self.sections
            .iter()
            .enumerate()
            .flat_map(move |(section_index, section)| {
                // let section_entry = std::iter::once(HelpLine::Section { mode: section.mode });
                let section_entry = section
                    .mode
                    .map(|mode| HelpLine::Section { mode })
                    .into_iter();

                let item_entries = section.items.iter().map(HelpLine::Item);

                let separator = (section_index + 1 < section_count)
                    .then_some(HelpLine::Empty)
                    .into_iter();

                section_entry.chain(item_entries).chain(separator)
            })
    }

    /// Returns the popup height for the given available area.
    fn height(&self, area: Rect) -> u16 {
        area.height.saturating_mul(Self::HEIGHT_PERCENT) / 100
    }

    /// Returns the maximum scroll offset for the given list viewport height.
    fn max_scroll_for_height(&self, viewport_height: u16) -> usize {
        self.list_entries()
            .count()
            .saturating_sub(viewport_height as _)
    }

    pub(super) fn handle_message(
        mut self,
        msg: HelpMessage,
        area: Rect,
    ) -> anyhow::Result<Option<Self>> {
        let mut result = match msg {
            HelpMessage::Close => {
                if self.is_search_focused {
                    Ok(Some(Self {
                        is_search_focused: false,
                        ..self
                    }))
                } else {
                    Ok(None)
                }
            }
            HelpMessage::ScrollUp(n) => {
                if self.is_search_focused {
                    Ok(Some(self))
                } else {
                    Ok(Some(Self {
                        scroll_top: self.scroll_top.saturating_sub(n),
                        ..self
                    }))
                }
            }
            HelpMessage::ScrollDown(n) => {
                if self.is_search_focused {
                    Ok(Some(self))
                } else {
                    let layout = self.layout(area);
                    Ok(Some(Self {
                        scroll_top: std::cmp::min(
                            self.scroll_top.saturating_add(n),
                            self.max_scroll_for_height(layout.key_binds_area.height),
                        ),
                        ..self
                    }))
                }
            }
            HelpMessage::FocusSearch => Ok(Some(Self {
                is_search_focused: true,
                ..self
            })),
            HelpMessage::SearchInput(event) => {
                if self.is_search_focused {
                    self.textarea.input(event);
                }
                Ok(Some(self))
            }
        };

        if let Ok(Some(help)) = &mut result {
            help.update_textarea_style();
        }

        result
    }

    fn update_textarea_style(&mut self) {
        if self.is_search_focused {
            self.textarea.set_cursor_style(self.default_cursor_style);
            self.textarea.set_placeholder_text("");
        } else {
            self.textarea.set_placeholder_text("Press / to search");
            self.textarea.set_cursor_style(Style::default());
            self.textarea.set_placeholder_style(self.theme.hint);
            self.textarea.set_cursor_line_style(self.theme.default);
        }
    }
}

/// Center `s` inside a string of a given width. Will place the necessary spaces on either side.
fn center(s: &str, width: usize) -> String {
    let text_width = s.width();
    let padding = width.saturating_sub(text_width);
    let left_padding = padding / 2;
    let right_padding = padding - left_padding;

    format!(
        "{}{}{}",
        " ".repeat(left_padding),
        s,
        " ".repeat(right_padding)
    )
}

enum HelpLine<'a> {
    Section { mode: ModeDiscriminant },
    Item(&'a HelpItem),
    Empty,
}

#[derive(Debug)]
struct HelpSection {
    mode: Option<ModeDiscriminant>,
    items: Vec<HelpItem>,
}

#[derive(Debug)]
struct HelpItem {
    chord_display: String,
    short_description: String,
    long_description: String,
}

#[derive(Debug, Clone)]
pub(super) enum HelpMessage {
    Close,
    ScrollUp(usize),
    ScrollDown(usize),
    FocusSearch,
    SearchInput(Event),
}
