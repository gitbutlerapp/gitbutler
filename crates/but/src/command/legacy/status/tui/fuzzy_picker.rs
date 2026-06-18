use std::{borrow::Cow, cell::Cell};

use bstr::ByteSlice;
use crossterm::event::Event;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use gix::refs::FullName;
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, Padding},
};
use ratatui_textarea::TextArea;
use unicode_width::UnicodeWidthStr;

use crate::{
    command::legacy::status::tui::Message,
    theme::{PatchStyle as _, Theme},
    utils::DebugAsType,
};

#[derive(Debug)]
pub(super) struct FuzzyPicker<T> {
    items: NonEmpty<T>,
    items_to_show: Vec<ItemToShow>,
    textarea: TextArea<'static>,
    cursor: usize,
    scroll_top: Cell<usize>,
    matcher: DebugAsType<SkimMatcherV2>,
    on_item_selected: DebugAsType<Box<dyn FnOnce(T, &mut Vec<Message>) -> anyhow::Result<()>>>,
    theme: &'static Theme,
}

pub(super) trait FuzzyPickerItem: Clone {
    fn to_str(&self) -> Cow<'_, str>;

    fn style(&self, theme: &'static Theme) -> Style;
}

#[derive(Debug, Clone)]
pub(super) enum BranchItem {
    Branch(FullName),
    Unassigned,
}

impl FuzzyPickerItem for BranchItem {
    fn to_str(&self) -> Cow<'_, str> {
        match self {
            Self::Branch(full_name) => full_name.shorten().to_str_lossy(),
            Self::Unassigned => Cow::Borrowed("unassigned changes"),
        }
    }

    fn style(&self, theme: &'static Theme) -> Style {
        match self {
            Self::Branch(..) => theme.local_branch,
            Self::Unassigned => theme.info,
        }
    }
}

#[derive(Debug)]
enum ItemToShow {
    Plain {
        item_idx: usize,
    },
    FuzzyMatch {
        item_idx: usize,
        char_indices: Vec<usize>,
    },
}

impl<T> FuzzyPicker<T>
where
    T: FuzzyPickerItem,
{
    pub(super) fn new<F>(items: NonEmpty<T>, theme: &'static Theme, on_item_selected: F) -> Self
    where
        F: FnOnce(T, &mut Vec<Message>) -> anyhow::Result<()> + 'static,
    {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(theme.default);

        let mut this = Self {
            items,
            items_to_show: Default::default(),
            textarea,
            cursor: 0,
            scroll_top: Cell::new(0),
            on_item_selected: DebugAsType(Box::new(on_item_selected)),
            matcher: DebugAsType(SkimMatcherV2::default()),
            theme,
        };

        this.filter_items();

        this
    }

    pub(super) fn render(&self, has_focus: bool, area: Rect, frame: &mut Frame) {
        let padding = Padding {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
        };
        let horizontal_padding = padding.left + padding.right;

        let space_taken_up_by_border: u16 = 2;
        let input_height: u16 = 1;

        let longest_item_width = self
            .items
            .iter()
            .map(|item| item.to_str().width())
            .max()
            .unwrap();

        let horizontal_layout = Layout::horizontal([Constraint::Length(std::cmp::max(
            (longest_item_width as u16) + space_taken_up_by_border + horizontal_padding,
            65,
        ))])
        .flex(Flex::Center)
        .split(area);

        let popup_height = 15.min(area.height);

        let centered_layout = Layout::vertical([Constraint::Length(popup_height)])
            .flex(Flex::Center)
            .split(horizontal_layout[0]);

        frame.render_widget(Clear, centered_layout[0]);

        let outer_block = Block::bordered()
            .padding(padding)
            .border_type(BorderType::Rounded)
            .border_style(self.theme.border);
        let inner_area = outer_block.inner(centered_layout[0]);
        frame.render_widget(outer_block, centered_layout[0]);

        let content_layout =
            Layout::vertical([Constraint::Length(input_height), Constraint::Min(1)])
                .split(inner_area);

        {
            let input_layout = Layout::horizontal([Constraint::Length(2), Constraint::Min(1)])
                .split(content_layout[0]);
            frame.render_widget("> ", input_layout[0]);
            frame.render_widget(&self.textarea, input_layout[1]);
        }

        let visible_rows = content_layout[1].height as usize;
        let mut scroll_top = self.scroll_top.get();

        if visible_rows == 0 {
            scroll_top = 0;
        } else {
            if self.cursor < scroll_top {
                scroll_top = self.cursor;
            } else if self.cursor >= scroll_top + visible_rows {
                scroll_top = self.cursor + 1 - visible_rows;
            }

            let max_scroll = self.items_to_show.len().saturating_sub(visible_rows);
            scroll_top = scroll_top.min(max_scroll);
        }

        self.scroll_top.set(scroll_top);

        let items = self
            .items_to_show
            .iter()
            .enumerate()
            .skip(scroll_top)
            .take(visible_rows)
            .map(|(idx, items_to_show_idx)| {
                let item = match items_to_show_idx {
                    ItemToShow::Plain { item_idx } => {
                        let item = &self.items[*item_idx];
                        ListItem::new(item.to_str()).style(item.style(self.theme))
                    }
                    ItemToShow::FuzzyMatch {
                        item_idx,
                        char_indices,
                    } => {
                        let item = &self.items[*item_idx];
                        let item_name = item.to_str();
                        let spans = item_name.chars().enumerate().map(|(idx, c)| {
                            let span = Span::raw(c.to_string());
                            if char_indices.contains(&idx) {
                                span.underlined()
                            } else {
                                span
                            }
                        });
                        ListItem::new(Line::from_iter(spans)).style(item.style(self.theme))
                    }
                };
                if has_focus && idx == self.cursor {
                    item.patch_style(self.theme.selection_highlight)
                } else {
                    item
                }
            });

        frame.render_widget(List::new(items), content_layout[1]);
    }

    fn filter_items(&mut self) {
        let query = self
            .textarea
            .lines()
            .first()
            .map(|q| &**q)
            .unwrap_or_default();

        self.items_to_show.clear();
        self.cursor = 0;
        self.scroll_top.set(0);

        if query.is_empty() {
            self.items_to_show.extend(
                self.items
                    .iter()
                    .enumerate()
                    .map(|(item_idx, _)| ItemToShow::Plain { item_idx }),
            );
        } else {
            let mut fuzzy_matches = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(item_idx, item)| {
                    let item_name = item.to_str();
                    let (score, indices) = self.matcher.fuzzy_indices(&item_name, query)?;
                    Some((item_idx, item_name, score, indices))
                })
                .collect::<Vec<_>>();
            fuzzy_matches.sort_unstable_by(|(_, _, score_a, _), (_, _, score_b, _)| {
                score_a.cmp(score_b).reverse()
            });
            self.items_to_show.extend(fuzzy_matches.into_iter().map(
                |(item_idx, _, _, indices)| ItemToShow::FuzzyMatch {
                    item_idx,
                    char_indices: indices,
                },
            ));
        }
    }

    pub(super) fn handle_message(
        mut self,
        msg: FuzzyPickerMessage,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<Option<Self>> {
        match msg {
            FuzzyPickerMessage::Close => Ok(None),
            FuzzyPickerMessage::MoveCursorDown => {
                let cursor = if self.items_to_show.is_empty() {
                    0
                } else {
                    std::cmp::min(self.cursor.saturating_add(1), self.items_to_show.len() - 1)
                };
                Ok(Some(Self { cursor, ..self }))
            }
            FuzzyPickerMessage::MoveCursorUp => Ok(Some(Self {
                cursor: self.cursor.saturating_sub(1),
                ..self
            })),
            FuzzyPickerMessage::Confirm => {
                let Some(item) = self
                    .items_to_show
                    .get(self.cursor)
                    .map(|idx| match idx {
                        ItemToShow::Plain { item_idx }
                        | ItemToShow::FuzzyMatch { item_idx, .. } => *item_idx,
                    })
                    .map(|item_idx| &self.items[item_idx])
                    .cloned()
                else {
                    return Ok(Some(self));
                };
                (self.on_item_selected.0)(item, messages)?;
                Ok(None)
            }
            FuzzyPickerMessage::Input(event) => {
                self.textarea.input(event);
                self.filter_items();
                Ok(Some(self))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum FuzzyPickerMessage {
    MoveCursorDown,
    MoveCursorUp,
    Input(Event),
    Confirm,
    Close,
}
