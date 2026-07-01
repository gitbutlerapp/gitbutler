use std::{borrow::Cow, cell::Cell};

use but_ctx::Context;
use crossterm::event::Event;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Padding, Row, Table},
};
use ratatui_textarea::TextArea;
use unicode_width::UnicodeWidthStr;

use crate::{
    command::legacy::status::tui::{Message, popup::Popup},
    theme::{PatchStyle as _, Theme},
    utils::DebugAsType,
};

#[derive(Debug)]
pub struct FuzzyPicker<T> {
    items: NonEmpty<T>,
    items_to_show: Vec<ItemToShow>,
    textarea: TextArea<'static>,
    cursor: usize,
    scroll_top: Cell<usize>,
    matcher: DebugAsType<SkimMatcherV2>,
    on_item_selected:
        DebugAsType<Box<dyn FnOnce(T, &mut Context, &mut Vec<Message>) -> anyhow::Result<()>>>,
    theme: &'static Theme,
}

pub trait FuzzyPickerItem: Clone {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>>;

    fn style(&self, theme: &'static Theme) -> Style;
}

pub struct Col<'a> {
    pub text: Cow<'a, str>,
    pub searchable: Option<SearchableToken>,
}

// intentionally not constructable by parent modules, that way we're guaranteed that
// `FuzzyPickerItem::columns` only has one searchable row
pub struct SearchableToken(());

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
    pub fn new<F>(items: NonEmpty<T>, theme: &'static Theme, on_item_selected: F) -> Self
    where
        F: FnOnce(T, &mut Context, &mut Vec<Message>) -> anyhow::Result<()> + 'static,
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

    pub fn render(&self, has_focus: bool, area: Rect, frame: &mut Frame) {
        let padding = Padding {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
        };
        let horizontal_padding = padding.left + padding.right;

        let space_taken_up_by_border: u16 = 2;
        let input_height: u16 = 1;

        let col_widths = self
            .items
            .iter()
            .map(|item| item.columns(SearchableToken(())))
            .map(|cols| {
                cols.into_iter()
                    .map(|col| col.text.width())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        // assume the searchable column is at the same index on every row
        let searchable_col_idx = self.items[0]
            .columns(SearchableToken(()))
            .into_iter()
            .position(|col| col.searchable.is_some())
            .expect("no searchable columns");

        // assume each row contains the same number of columns
        let num_cols = col_widths[0].len();

        let column_spacing = 2;

        let mut col_constraints = (0..num_cols)
            .map(|n| col_widths.iter().map(|c| &c[n]).collect::<Vec<_>>())
            .map(|col| col.iter().copied().max().unwrap())
            .map(|&width| Constraint::Length(width as u16))
            .collect::<Vec<_>>();
        for (i, constraint) in col_constraints.iter_mut().enumerate() {
            if i == searchable_col_idx {
                *constraint = Constraint::Min(1);
                break;
            }
        }

        let longest_item_width: usize = col_widths
            .iter()
            .map(|widths| widths.iter().sum())
            .max()
            .unwrap();

        let popup_width = std::cmp::max(
            (longest_item_width as u16)
                + (column_spacing * (num_cols - 1) as u16)
                + space_taken_up_by_border
                + horizontal_padding,
            65,
        );
        let popup_height = 20.min(area.height);

        let inner_area = Popup::new(self.theme, popup_width, popup_height)
            .padding(padding)
            .render(area, frame)
            .inner;

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

        let rows = self
            .items_to_show
            .iter()
            .enumerate()
            .skip(scroll_top)
            .take(visible_rows)
            .map(|(idx, items_to_show_idx)| {
                let row = match items_to_show_idx {
                    ItemToShow::Plain { item_idx } => {
                        let item = &self.items[*item_idx];
                        let cols = item.columns(SearchableToken(()));
                        Row::new(cols.into_iter().map(|col| {
                            let line = Line::from(col.text);
                            if col.searchable.is_some() {
                                line.style(item.style(self.theme))
                            } else {
                                line
                            }
                        }))
                    }
                    ItemToShow::FuzzyMatch {
                        item_idx,
                        char_indices,
                    } => {
                        let item = &self.items[*item_idx];
                        let cols = item.columns(SearchableToken(())).into_iter();
                        Row::new(cols.map(|col| {
                            if col.searchable.is_some() {
                                let spans = col.text.chars().enumerate().map(|(idx, c)| {
                                    let span = Span::raw(c.to_string());
                                    if char_indices.contains(&idx) {
                                        span.underlined()
                                    } else {
                                        span
                                    }
                                });
                                Line::from_iter(spans).style(item.style(self.theme))
                            } else {
                                Line::from(col.text)
                            }
                        }))
                    }
                };
                if has_focus && idx == self.cursor {
                    row.patch_style(self.theme.selection_highlight)
                } else {
                    row
                }
            });
        let table = rows
            .collect::<Table>()
            .widths(col_constraints)
            .column_spacing(column_spacing);
        frame.render_widget(table, content_layout[1]);
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
                    let col = item
                        .columns(SearchableToken(()))
                        .into_iter()
                        .find(|col| col.searchable.is_some())
                        .expect("FuzzyPickerItem::columns must return a searchable column");
                    let (score, indices) = self.matcher.fuzzy_indices(&col.text, query)?;
                    Some((item_idx, col, score, indices))
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

    pub fn handle_message(
        mut self,
        msg: FuzzyPickerMessage,
        ctx: &mut Context,
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
                (self.on_item_selected.0)(item, ctx, messages)?;
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
pub enum FuzzyPickerMessage {
    MoveCursorDown,
    MoveCursorUp,
    Input(Event),
    Confirm,
    Close,
}
