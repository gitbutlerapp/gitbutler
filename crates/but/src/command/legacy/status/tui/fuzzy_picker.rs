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
    state: FuzzyPickerState<T>,
    on_item_selected: DebugAsType<
        Box<dyn FnOnce(T, &mut Context, &mut Vec<Message>) -> anyhow::Result<()> + Send>,
    >,
    theme: &'static Theme,
}

pub trait FuzzyPickerItem: Clone {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>>;

    fn style(&self, theme: &Theme) -> Style;

    /// Styling for non-searchable metadata columns.
    fn secondary_style(&self, _theme: &Theme) -> Style {
        Style::default()
    }
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

#[derive(Debug)]
pub struct FuzzyPickerState<T> {
    items: NonEmpty<T>,
    items_to_show: Vec<ItemToShow>,
    textarea: TextArea<'static>,
    cursor: usize,
    scroll_top: Cell<usize>,
    matcher: DebugAsType<SkimMatcherV2>,
}

impl<T: FuzzyPickerItem> FuzzyPickerState<T> {
    /// Creates a picker with an empty query and the first item selected.
    pub fn new(
        items: NonEmpty<T>,
        theme: &Theme,
        placeholder_text: Option<&str>,
    ) -> FuzzyPickerState<T> {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(theme.default);
        if let Some(placeholder_text) = placeholder_text {
            textarea.set_placeholder_text(placeholder_text);
        }
        let mut state = FuzzyPickerState {
            items,
            items_to_show: Default::default(),
            textarea,
            cursor: 0,
            scroll_top: Cell::new(0),
            matcher: DebugAsType(SkimMatcherV2::default()),
        };
        state.filter_items();
        state
    }

    /// Updates the search input and refreshes cached matches.
    pub fn input(&mut self, event: Event) {
        if self.textarea.input(event) {
            self.filter_items();
        }
    }

    /// Moves the selection down without going past the last match.
    pub fn move_cursor_down(&mut self) {
        self.cursor = if self.items_to_show.is_empty() {
            0
        } else {
            std::cmp::min(self.cursor.saturating_add(1), self.items_to_show.len() - 1)
        };
    }

    /// Moves the selection up without going past the first match.
    pub fn move_cursor_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Returns the selected domain item, or none when the query has no matches.
    pub fn selected_item(&self) -> Option<&T> {
        let item_idx = match self.items_to_show.get(self.cursor)? {
            ItemToShow::Plain { item_idx } | ItemToShow::FuzzyMatch { item_idx, .. } => *item_idx,
        };
        Some(&self.items[item_idx])
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
}

const COLUMN_SPACING: u16 = 2;

fn column_widths<T: FuzzyPickerItem>(items: &NonEmpty<T>) -> Vec<Vec<usize>> {
    items
        .iter()
        .map(|item| item.columns(SearchableToken(())))
        .map(|cols| cols.into_iter().map(|col| col.text.width()).collect())
        .collect()
}

/// Renders picker content directly into `area`, without a popup or border.
pub fn render_fuzzy_picker<T: FuzzyPickerItem>(
    state: &FuzzyPickerState<T>,
    theme: &Theme,
    has_focus: bool,
    area: Rect,
    frame: &mut Frame,
) {
    let input_height: u16 = 1;
    let col_widths = column_widths(&state.items);

    // assume the searchable column is at the same index on every row
    let searchable_col_idx = state.items[0]
        .columns(SearchableToken(()))
        .into_iter()
        .position(|col| col.searchable.is_some())
        .expect("no searchable columns");

    // assume each row contains the same number of columns
    let num_cols = col_widths[0].len();

    let column_spacing = COLUMN_SPACING;

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

    let content_layout =
        Layout::vertical([Constraint::Length(input_height), Constraint::Min(1)]).split(area);

    {
        let input_layout = Layout::horizontal([Constraint::Length(2), Constraint::Min(1)])
            .split(content_layout[0]);
        frame.render_widget("> ", input_layout[0]);
        frame.render_widget(&state.textarea, input_layout[1]);
    }

    let visible_rows = content_layout[1].height as usize;
    let mut scroll_top = state.scroll_top.get();

    if visible_rows == 0 {
        scroll_top = 0;
    } else {
        if state.cursor < scroll_top {
            scroll_top = state.cursor;
        } else if state.cursor >= scroll_top + visible_rows {
            scroll_top = state.cursor + 1 - visible_rows;
        }

        let max_scroll = state.items_to_show.len().saturating_sub(visible_rows);
        scroll_top = scroll_top.min(max_scroll);
    }

    state.scroll_top.set(scroll_top);

    let rows = state
        .items_to_show
        .iter()
        .enumerate()
        .skip(scroll_top)
        .take(visible_rows)
        .map(|(idx, items_to_show_idx)| {
            let row = match items_to_show_idx {
                ItemToShow::Plain { item_idx } => {
                    let item = &state.items[*item_idx];
                    let cols = item.columns(SearchableToken(()));
                    Row::new(cols.into_iter().map(|col| {
                        let line = Line::from(col.text);
                        if col.searchable.is_some() {
                            line.style(item.style(theme))
                        } else {
                            line.style(item.secondary_style(theme))
                        }
                    }))
                }
                ItemToShow::FuzzyMatch {
                    item_idx,
                    char_indices,
                } => {
                    let item = &state.items[*item_idx];
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
                            Line::from_iter(spans).style(item.style(theme))
                        } else {
                            Line::from(col.text).style(item.secondary_style(theme))
                        }
                    }))
                }
            };
            if has_focus && idx == state.cursor {
                row.patch_style(theme.selection_highlight)
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

impl<T: FuzzyPickerItem> FuzzyPicker<T> {
    pub fn new<F>(items: NonEmpty<T>, theme: &'static Theme, on_item_selected: F) -> FuzzyPicker<T>
    where
        F: FnOnce(T, &mut Context, &mut Vec<Message>) -> anyhow::Result<()> + Send + 'static,
    {
        FuzzyPicker {
            state: FuzzyPickerState::new(items, theme, None),
            on_item_selected: DebugAsType(Box::new(on_item_selected)),
            theme,
        }
    }

    pub fn render(&self, has_focus: bool, area: Rect, frame: &mut Frame) {
        let padding = Padding::ZERO;
        let horizontal_padding = padding.left + padding.right;
        let space_taken_up_by_border: u16 = 2;
        let col_widths = column_widths(&self.state.items);
        let num_cols = col_widths[0].len();
        let column_spacing = COLUMN_SPACING;
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

        render_fuzzy_picker(&self.state, self.theme, has_focus, inner_area, frame);
    }

    pub fn handle_message(
        mut self,
        msg: FuzzyPickerMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<Option<FuzzyPicker<T>>> {
        match msg {
            FuzzyPickerMessage::Close => Ok(None),
            FuzzyPickerMessage::MoveCursorDown => {
                self.state.move_cursor_down();
                Ok(Some(self))
            }
            FuzzyPickerMessage::MoveCursorUp => {
                self.state.move_cursor_up();
                Ok(Some(self))
            }
            FuzzyPickerMessage::Confirm => {
                let Some(item) = self.state.selected_item().cloned() else {
                    return Ok(Some(self));
                };
                (self.on_item_selected.0)(item, ctx, messages)?;
                Ok(None)
            }
            FuzzyPickerMessage::Input(event) => {
                self.state.input(event);
                Ok(Some(self))
            }
        }
    }
}

#[derive(Debug)]
pub enum FuzzyPickerMessage {
    MoveCursorDown,
    MoveCursorUp,
    Input(Event),
    Confirm,
    Close,
}
