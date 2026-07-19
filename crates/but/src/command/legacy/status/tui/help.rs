use crossterm::event::{Event, KeyCode, KeyModifiers};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use indexmap::IndexMap;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::Span,
    widgets::{Padding, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use ratatui_textarea::TextArea;
use strum::IntoEnumIterator;
use unicode_width::UnicodeWidthStr;

use crate::{
    command::legacy::status::tui::{
        KeyBinds,
        mode::ModeDiscriminant,
        popup::Popup,
        render::{RenderSingleLineSpans, SpanExt, available_lines_in_area},
    },
    theme::Theme,
    utils::DebugAsType,
};

#[derive(Debug)]
pub struct Help {
    theme: &'static Theme,
    sections: Vec<HelpSection>,
    visible_sections: Vec<VisibleHelpSection>,
    textarea: TextArea<'static>,
    search_focused: bool,
    matcher: DebugAsType<SkimMatcherV2>,
    scroll_top: usize,
}

impl Help {
    const HEIGHT_PERCENT: u16 = 80;
    const KEY_BIND_COLUMN_WIDTH: u16 = 12;
    const KEY_BIND_COLUMN_PADDING: &str = "            ";

    pub fn new<'a>(
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
                    | ModeDiscriminant::Jump
                    | ModeDiscriminant::Stack => {}
                }

                let section = mode_to_sections.entry(mode).or_insert_with(|| HelpSection {
                    mode: Some(mode),
                    items: Vec::new(),
                });

                for key_bind in
                    key_binds.iter_key_binds_available_in_mode_regardless_of_conditions(mode)
                {
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
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(theme.default);

        let mut this = Self {
            theme,
            sections,
            visible_sections: Vec::new(),
            textarea,
            search_focused: false,
            matcher: DebugAsType(SkimMatcherV2::default()),
            scroll_top: 0,
        };
        this.filter_items();
        this
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let padding = Padding {
            left: 1,
            right: 1,
            top: 0,
            bottom: 0,
        };

        let popup = Popup::new(self.theme, 100, self.height(area))
            .padding(padding)
            .render(area, frame);
        let content_layout =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(popup.inner);
        let input_area = content_layout[0];
        let list_area = content_layout[1];

        if self.search_focused {
            frame.render_widget(&self.textarea, input_area);
        } else {
            let query = self
                .textarea
                .lines()
                .first()
                .map(String::as_str)
                .unwrap_or_default();
            let text = if query.is_empty() {
                "Press / to search"
            } else {
                query
            };
            frame.render_widget(Span::styled(text, self.theme.hint), input_area);
        }

        let longest_short_description = self
            .visible_sections
            .iter()
            .flat_map(|visible_section| {
                let section = &self.sections[visible_section.section_idx];
                visible_section.items.iter().map(|visible_item| {
                    section.items[visible_item.item_idx]
                        .short_description
                        .width()
                })
            })
            .max()
            .unwrap_or(0) as u16;

        let columns_layout = Layout::horizontal([
            Constraint::Length(Self::KEY_BIND_COLUMN_WIDTH),
            Constraint::Length(1),
            Constraint::Length(longest_short_description),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(list_area);

        let scroll_top = self
            .scroll_top
            .min(self.max_scroll_for_height(list_area.height));
        for (entry, row_area) in self
            .list_entries()
            .skip(scroll_top)
            .zip(available_lines_in_area(list_area))
        {
            match entry {
                HelpLine::Section { mode } => {
                    let mut line =
                        RenderSingleLineSpans::new(frame, row_of(columns_layout[0], row_area.y));
                    let mode_name = mode.hotbar_str();
                    let padding =
                        (columns_layout[0].width as usize).saturating_sub(mode_name.width());
                    let left_padding = padding / 2;
                    let right_padding = padding - left_padding;
                    line.render(
                        Span::raw(&Self::KEY_BIND_COLUMN_PADDING[..left_padding])
                            .mode_colors(mode, self.theme),
                    );
                    line.render(Span::raw(mode_name).mode_colors(mode, self.theme));
                    line.render(
                        Span::raw(&Self::KEY_BIND_COLUMN_PADDING[..right_padding])
                            .mode_colors(mode, self.theme),
                    );
                }
                HelpLine::Item {
                    help_item,
                    short_match_indices,
                    long_match_indices,
                } => {
                    let mut key_bind =
                        RenderSingleLineSpans::new(frame, row_of(columns_layout[0], row_area.y));
                    let padding = (columns_layout[0].width as usize)
                        .saturating_sub(help_item.chord_display.width());
                    key_bind.render(Span::raw(&Self::KEY_BIND_COLUMN_PADDING[..padding]));
                    key_bind.render(Span::styled(&help_item.chord_display, self.theme.legend));

                    let mut short_description =
                        RenderSingleLineSpans::new(frame, row_of(columns_layout[2], row_area.y));
                    short_description.extend(highlight_matches(
                        &help_item.short_description,
                        short_match_indices,
                    ));

                    let mut long_description =
                        RenderSingleLineSpans::new(frame, row_of(columns_layout[4], row_area.y));
                    long_description.extend(
                        highlight_matches(&help_item.long_description, long_match_indices)
                            .map(|span| span.patch_style(self.theme.hint)),
                    );
                }
                HelpLine::Empty => {}
            }
        }

        // scrollbar
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .style(self.theme.border);
        let mut scrollbar_state =
            ScrollbarState::new(self.max_scroll_for_height(list_area.height)).position(scroll_top);
        let scrollbar_area = Rect {
            x: popup.outer.right().saturating_sub(1),
            y: list_area.y,
            width: 1,
            height: list_area.height,
        };
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }

    fn filter_items(&mut self) {
        let query = self
            .textarea
            .lines()
            .first()
            .map(String::as_str)
            .unwrap_or_default();

        self.visible_sections.clear();
        self.scroll_top = 0;

        for (section_idx, section) in self.sections.iter().enumerate() {
            let mut items = if query.is_empty() {
                section
                    .items
                    .iter()
                    .enumerate()
                    .map(|(item_idx, _)| VisibleHelpItem {
                        item_idx,
                        score: 0,
                        short_match_indices: Vec::new(),
                        long_match_indices: Vec::new(),
                    })
                    .collect::<Vec<_>>()
            } else {
                section
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(item_idx, item)| {
                        let short_match =
                            self.matcher.fuzzy_indices(&item.short_description, query);
                        let long_match = self.matcher.fuzzy_indices(&item.long_description, query);
                        let score = short_match
                            .as_ref()
                            .map(|(score, _)| *score)
                            .into_iter()
                            .chain(long_match.as_ref().map(|(score, _)| *score))
                            .max()?;

                        Some(VisibleHelpItem {
                            item_idx,
                            score,
                            short_match_indices: short_match
                                .map(|(_, indices)| indices)
                                .unwrap_or_default(),
                            long_match_indices: long_match
                                .map(|(_, indices)| indices)
                                .unwrap_or_default(),
                        })
                    })
                    .collect::<Vec<_>>()
            };

            if !query.is_empty() {
                items.sort_by_key(|a| a.score);
            }
            if !items.is_empty() {
                self.visible_sections
                    .push(VisibleHelpSection { section_idx, items });
            }
        }
    }

    fn list_entries(&self) -> impl Iterator<Item = HelpLine<'_>> {
        let section_count = self.visible_sections.len();
        self.visible_sections.iter().enumerate().flat_map(
            move |(section_index, visible_section)| {
                let section = &self.sections[visible_section.section_idx];
                let section_entry = section
                    .mode
                    .map(|mode| HelpLine::Section { mode })
                    .into_iter();

                let item_entries =
                    visible_section
                        .items
                        .iter()
                        .map(|visible_item| HelpLine::Item {
                            help_item: &section.items[visible_item.item_idx],
                            short_match_indices: &visible_item.short_match_indices,
                            long_match_indices: &visible_item.long_match_indices,
                        });

                let separator = (section_index + 1 < section_count)
                    .then_some(HelpLine::Empty)
                    .into_iter();

                section_entry.chain(item_entries).chain(separator)
            },
        )
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

    pub fn is_search_focused(&self) -> bool {
        self.search_focused
    }

    pub fn handle_message(mut self, msg: HelpMessage, area: Rect) -> anyhow::Result<Option<Self>> {
        match msg {
            HelpMessage::Close => Ok(None),
            HelpMessage::Escape if self.search_focused => {
                self.clear_search();
                Ok(Some(self))
            }
            HelpMessage::Escape => Ok(None),
            HelpMessage::ToggleSearch if self.search_focused => {
                self.clear_search();
                Ok(Some(self))
            }
            HelpMessage::ToggleSearch => {
                self.search_focused = true;
                Ok(Some(self))
            }
            HelpMessage::Input(Event::Key(key)) if is_newline_key(key.code, key.modifiers) => {
                Ok(Some(self))
            }
            HelpMessage::Input(event) => {
                self.textarea.input(event);
                self.filter_items();
                Ok(Some(self))
            }
            HelpMessage::ScrollUp(n) => Ok(Some(Self {
                scroll_top: self.scroll_top.saturating_sub(n),
                ..self
            })),
            HelpMessage::ScrollDown(n) => Ok(Some(Self {
                scroll_top: std::cmp::min(
                    self.scroll_top.saturating_add(n),
                    self.max_scroll_for_height(self.height(area).saturating_sub(3)),
                ),
                ..self
            })),
        }
    }

    fn clear_search(&mut self) {
        self.textarea = TextArea::default();
        self.textarea.set_cursor_line_style(self.theme.default);
        self.search_focused = false;
        self.filter_items();
    }
}

fn is_newline_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::Enter | KeyCode::Char('\n' | '\r'))
        || code == KeyCode::Char('m') && modifiers.contains(KeyModifiers::CONTROL)
}

fn highlight_matches<'a>(
    text: &'a str,
    char_indices: &'a [usize],
) -> impl Iterator<Item = Span<'a>> + 'a {
    let mut chars = text.char_indices().peekable();
    let mut char_idx = 0;

    std::iter::from_fn(move || {
        let (start_byte, _) = chars.next()?;
        let matched = char_indices.contains(&char_idx);

        while chars.peek().is_some() && char_indices.contains(&(char_idx + 1)) == matched {
            chars.next();
            char_idx += 1;
        }

        let end_byte = chars
            .peek()
            .map(|(byte_idx, _)| *byte_idx)
            .unwrap_or(text.len());
        char_idx += 1;

        let span = Span::raw(&text[start_byte..end_byte]);
        Some(if matched { span.underlined() } else { span })
    })
}

fn row_of(area: Rect, y: u16) -> Rect {
    Rect {
        y,
        height: 1,
        ..area
    }
}

enum HelpLine<'a> {
    Section {
        mode: ModeDiscriminant,
    },
    Item {
        help_item: &'a HelpItem,
        short_match_indices: &'a [usize],
        long_match_indices: &'a [usize],
    },
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

#[derive(Debug)]
struct VisibleHelpSection {
    section_idx: usize,
    items: Vec<VisibleHelpItem>,
}

#[derive(Debug)]
struct VisibleHelpItem {
    item_idx: usize,
    score: i64,
    short_match_indices: Vec<usize>,
    long_match_indices: Vec<usize>,
}

#[derive(Debug)]
pub enum HelpMessage {
    Close,
    Escape,
    ToggleSearch,
    Input(Event),
    ScrollUp(usize),
    ScrollDown(usize),
}
