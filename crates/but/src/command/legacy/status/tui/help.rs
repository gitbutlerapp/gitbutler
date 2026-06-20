use indexmap::IndexMap;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Padding, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use strum::IntoEnumIterator;
use unicode_width::UnicodeWidthStr;

use crate::{
    command::legacy::status::tui::{
        KeyBinds, mode::ModeDiscriminant, popup::Popup, render::SpanExt,
    },
    theme::Theme,
};

#[derive(Debug)]
pub(super) struct Help {
    theme: &'static Theme,
    sections: Vec<HelpSection>,
    scroll_top: usize,
}

impl Help {
    const HEIGHT_PERCENT: u16 = 80;

    pub(super) fn new<'a>(
        key_binds: impl IntoIterator<Item = &'a KeyBinds>,
        theme: &'static Theme,
    ) -> Self {
        let mut mode_to_sections = IndexMap::<ModeDiscriminant, HelpSection>::new();

        for key_binds in key_binds {
            for mode in ModeDiscriminant::iter() {
                match mode {
                    ModeDiscriminant::PickChanges => continue,
                    ModeDiscriminant::Normal
                    | ModeDiscriminant::Rub
                    | ModeDiscriminant::InlineReword
                    | ModeDiscriminant::Command
                    | ModeDiscriminant::Commit
                    | ModeDiscriminant::Move
                    | ModeDiscriminant::Details
                    | ModeDiscriminant::MoveStack
                    | ModeDiscriminant::Stack => {}
                }

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

        Self {
            theme,
            sections,
            scroll_top: 0,
        }
    }

    pub(super) fn render(&self, area: Rect, frame: &mut Frame) {
        let padding = Padding {
            left: 1,
            right: 1,
            top: 0,
            bottom: 0,
        };

        let popup = Popup::new(self.theme, 100, self.height(area))
            .padding(padding)
            .render(area, frame);
        let inner_area = popup.inner;

        let longest_short_description = self
            .sections
            .iter()
            .flat_map(|section| &section.items)
            .map(|item| item.short_description.width())
            .max()
            .unwrap_or(0) as u16;

        let columns_layout = Layout::horizontal([
            Constraint::Length(12),
            Constraint::Length(1),
            Constraint::Length(longest_short_description),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(inner_area);

        let scroll_top = self
            .scroll_top
            .min(self.max_scroll_for_height(inner_area.height));
        let list_entries = || {
            self.list_entries()
                .skip(scroll_top)
                .take(inner_area.height as _)
        };

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
            ScrollbarState::new(self.max_scroll_for_height(inner_area.height)).position(scroll_top);
        let scrollbar_area = Rect {
            x: popup.outer.right().saturating_sub(1),
            y: popup.outer.y.saturating_add(1),
            width: 1,
            height: popup.outer.height.saturating_sub(2),
        };
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
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
        self,
        msg: HelpMessage,
        area: Rect,
    ) -> anyhow::Result<Option<Self>> {
        match msg {
            HelpMessage::Close => Ok(None),
            HelpMessage::ScrollUp(n) => Ok(Some(Self {
                scroll_top: self.scroll_top.saturating_sub(n),
                ..self
            })),
            HelpMessage::ScrollDown(n) => Ok(Some(Self {
                scroll_top: std::cmp::min(
                    self.scroll_top.saturating_add(n),
                    self.max_scroll_for_height(self.height(area).saturating_sub(2)),
                ),
                ..self
            })),
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
}
