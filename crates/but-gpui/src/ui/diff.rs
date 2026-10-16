//! Diffs as a flat list of rows, so a virtualised list only builds what's on screen.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, CursorStyle, HighlightStyle, Hsla, MouseButton, MouseDownEvent, Pixels, Point,
    Role, SharedString, StyledText, Window, combine_highlights, div, prelude::*, px,
};

use super::{Icon, h_flex, small_icon, v_flex};
use crate::backend::{ChangeKind, DiffBody, FileDiff, Hunk, LineKind};
use crate::theme::Theme;

pub const LINE_HEIGHT: f32 = 20.;

/// The height of a file's header row.
pub const FILE_HEADER_HEIGHT: f32 = 38.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffRow {
    File(usize),
    Hunk {
        file: usize,
        hunk: usize,
    },
    Line {
        file: usize,
        hunk: usize,
        line: usize,
        last: bool,
    },
    /// Binary, too large or empty: a one-line explanation instead of lines.
    Note(usize),
    Gap,
}

impl DiffRow {
    pub fn file(self) -> Option<usize> {
        match self {
            DiffRow::File(file) | DiffRow::Note(file) => Some(file),
            DiffRow::Hunk { file, .. } | DiffRow::Line { file, .. } => Some(file),
            DiffRow::Gap => None,
        }
    }
}

/// Rows for `files`, with the files `folded` says are folded reduced to their header and
/// the ones `hidden` says are hidden left out.
pub fn rows(
    files: &[FileDiff],
    folded: impl Fn(&FileDiff) -> bool,
    hidden: impl Fn(&FileDiff) -> bool,
) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for (file_ix, file) in files.iter().enumerate() {
        if hidden(file) {
            continue;
        }
        rows.push(DiffRow::File(file_ix));
        if folded(file) {
            rows.push(DiffRow::Gap);
            continue;
        }
        match &file.body {
            DiffBody::Hunks { hunks, .. } if !hunks.is_empty() => {
                for (hunk_ix, hunk) in hunks.iter().enumerate() {
                    rows.push(DiffRow::Hunk {
                        file: file_ix,
                        hunk: hunk_ix,
                    });
                    let last_hunk = hunk_ix + 1 == hunks.len();
                    for line_ix in 0..hunk.lines.len() {
                        rows.push(DiffRow::Line {
                            file: file_ix,
                            hunk: hunk_ix,
                            line: line_ix,
                            last: last_hunk && line_ix + 1 == hunk.lines.len(),
                        });
                    }
                }
            }
            _ => rows.push(DiffRow::Note(file_ix)),
        }
        rows.push(DiffRow::Gap);
    }
    rows
}

pub fn change_color(kind: ChangeKind, theme: &Theme) -> Hsla {
    match kind {
        ChangeKind::Added => theme.safe,
        ChangeKind::Modified => theme.blue,
        ChangeKind::Deleted => theme.danger,
        ChangeKind::Renamed => theme.purple,
    }
}

/// `src/ui/diff.rs` → (`diff.rs`, `src/ui`).
pub fn split_path(path: &str) -> (&str, &str) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (name, dir),
        None => (path, ""),
    }
}

pub fn stats(added: u32, removed: u32, theme: &Theme) -> impl IntoElement {
    h_flex()
        .flex_none()
        .gap(px(6.))
        .text_size(px(11.))
        .font_family(theme.mono_font.clone())
        .when(added > 0, |el| {
            el.child(div().text_color(theme.safe).child(format!("+{added}")))
        })
        .when(removed > 0, |el| {
            el.child(div().text_color(theme.danger).child(format!("-{removed}")))
        })
}

/// What the diff's owner adds to a row.
#[derive(Default)]
pub struct Decor {
    /// Before a hunk header, like a checkbox.
    pub leading: Option<AnyElement>,
    /// At the end of a file or hunk header, like its controls.
    pub trailing: Option<AnyElement>,
    /// Dims a hunk left out of the next commit.
    pub excluded: bool,
    /// The hunk or file the keyboard is on.
    pub current: bool,
    /// The file shows only its header.
    pub folded: bool,
    /// Clicking a line's numbers puts it in the next commit or takes it out.
    pub toggle: Option<OnToggle>,
    /// Lets the pointer select a line's text.
    pub select: Option<LineSelect>,
    /// The part of a line's text that's selected.
    pub selected: Option<Range<usize>>,
    /// What Find matched in a line's text, each with whether it's the current match.
    pub found: Vec<(Range<usize>, bool)>,
    /// A long line wraps onto more rows instead of running off the edge.
    pub wrap: bool,
}

/// What the pointer does to a line's text, given the byte offset in the line it's at:
/// pressing starts a selection there, dragging across extends it.
pub struct LineSelect {
    pub down: OnLineDown,
    pub drag: OnLineDrag,
}

pub type OnLineDown = Rc<dyn Fn(usize, &MouseDownEvent, &mut Window, &mut App)>;
pub type OnLineDrag = Rc<dyn Fn(usize, &mut Window, &mut App)>;
pub type OnToggle = Rc<dyn Fn(&mut Window, &mut App)>;

pub fn render_row(row: DiffRow, files: &[FileDiff], theme: &Theme, decor: Decor) -> AnyElement {
    div()
        .w_full()
        .px(px(16.))
        .child(render_card_row(row, files, theme, decor))
        .into_any_element()
}

fn render_card_row(row: DiffRow, files: &[FileDiff], theme: &Theme, decor: Decor) -> AnyElement {
    let Decor {
        leading,
        trailing,
        excluded,
        current,
        folded,
        toggle,
        select,
        selected,
        found,
        wrap,
    } = decor;
    let card = |el: gpui::Div| {
        el.w_full()
            .border_l_1()
            .border_r_1()
            .border_color(theme.border_3)
            .bg(theme.bg_1)
    };
    match row {
        DiffRow::Gap => div().h(px(12.)).into_any_element(),
        DiffRow::File(ix) => {
            let file = &files[ix];
            let (name, dir) = split_path(&file.file.path);
            let (added, removed) = file.stats();
            card(h_flex())
                .id(("diff-file", ix))
                .role(Role::Heading)
                .aria_label(SharedString::from(file.file.path.clone()))
                .h(px(FILE_HEADER_HEIGHT))
                .pl(px(12.))
                .pr(px(8.))
                .gap(px(8.))
                .border_t_1()
                .border_b_1()
                .rounded_t(px(6.))
                .when(folded, |el| el.rounded_b(px(6.)))
                .bg(theme.bg_1)
                .when(current, |el| el.border_color(theme.border_2))
                .child(
                    div()
                        .flex_none()
                        .w(px(12.))
                        .text_size(px(11.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(change_color(file.file.kind, theme))
                        .child(file.file.kind.letter()),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_w(px(0.))
                        .gap(px(6.))
                        .overflow_hidden()
                        .child(
                            div()
                                .flex_none()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text_1)
                                .child(name.to_owned()),
                        )
                        .child(
                            div()
                                .min_w(px(0.))
                                .truncate()
                                .text_color(theme.text_2)
                                .child(match &file.file.previous_path {
                                    Some(previous) => format!("{dir}  ← {previous}"),
                                    None => dir.to_owned(),
                                }),
                        ),
                )
                .child(stats(added, removed, theme))
                .children(trailing)
                .into_any_element()
        }
        DiffRow::Hunk { file, hunk } => {
            let DiffBody::Hunks { hunks, .. } = &files[file].body else {
                return div().into_any_element();
            };
            card(h_flex())
                .h(px(LINE_HEIGHT + 6.))
                .pl(px(12.))
                .pr(px(4.))
                .gap(px(8.))
                .bg(if current {
                    theme.pop.opacity(0.1)
                } else {
                    theme.bg_2
                })
                .font_family(theme.mono_font.clone())
                .text_size(px(11.5))
                .text_color(theme.text_2)
                .overflow_hidden()
                .whitespace_nowrap()
                .children(leading)
                .child(hunk_title(&hunks[hunk], theme))
                .when(excluded, |el| {
                    el.child(
                        div()
                            .flex_none()
                            .text_color(theme.text_3)
                            .child("not included"),
                    )
                })
                .children(trailing)
                .into_any_element()
        }
        DiffRow::Note(file) => {
            if let DiffBody::Image { before, after } = &files[file].body {
                return card(image_row(before.clone(), after.clone(), theme)).into_any_element();
            }
            let text = match &files[file].body {
                DiffBody::Binary | DiffBody::Image { .. } => "Binary file".to_owned(),
                DiffBody::TooLarge { bytes } => {
                    format!("Too large to show ({:.1} MB)", *bytes as f64 / 1_000_000.)
                }
                DiffBody::Empty | DiffBody::Hunks { .. } => match files[file].file.kind {
                    ChangeKind::Renamed => "Renamed without changes".to_owned(),
                    _ => "No content changes".to_owned(),
                },
            };
            card(h_flex())
                .h(px(36.))
                .px(px(12.))
                .gap(px(6.))
                .border_b_1()
                .rounded_b(px(6.))
                .text_size(px(12.))
                .text_color(theme.text_2)
                .child(small_icon(Icon::File).text_color(theme.text_3))
                .child(text)
                .into_any_element()
        }
        DiffRow::Line {
            file,
            hunk,
            line,
            last,
        } => {
            let DiffBody::Hunks { hunks, .. } = &files[file].body else {
                return div().into_any_element();
            };
            // Unique per line, for the line's accessibility node.
            let row_id = (file << 40) | (hunk << 20) | line;
            let line = &hunks[hunk].lines[line];
            let (bg, gutter_bg, emph_bg, sign, sign_color) = match line.kind {
                LineKind::Context => (None, None, theme.bg_1, " ", theme.text_3),
                LineKind::Added => (
                    Some(theme.diff_added_bg),
                    Some(theme.diff_added_gutter),
                    theme.diff_added_emph,
                    "+",
                    theme.safe,
                ),
                LineKind::Removed => (
                    Some(theme.diff_removed_bg),
                    Some(theme.diff_removed_gutter),
                    theme.diff_removed_emph,
                    "-",
                    theme.danger,
                ),
            };
            let syntax = line
                .spans
                .iter()
                .map(|(range, token)| (range.clone(), HighlightStyle::from(theme.token(*token))));
            let emph = line
                .emph
                .iter()
                .map(|range| (range.clone(), ground(emph_bg)));
            let number = |n: Option<u32>| {
                div()
                    .flex_none()
                    .w(px(44.))
                    .pr(px(8.))
                    .flex()
                    .items_start()
                    .justify_end()
                    .text_color(theme.text_3)
                    .when_some(gutter_bg, |el, bg| el.bg(bg))
                    .child(n.map(|n| n.to_string()).unwrap_or_default())
            };
            // Blue like the system's text selection, apart from the green and red of the
            // change itself.
            let selected = selected.map(|range| (range, ground(theme.blue.opacity(0.3))));
            let found = found.into_iter().map(|(range, current)| {
                (
                    range,
                    ground(theme.warn.opacity(if current { 0.6 } else { 0.28 })),
                )
            });
            let text = StyledText::new(SharedString::from(line.text.clone())).with_highlights(
                combine_highlights(
                    combine_highlights(combine_highlights(syntax, emph), found),
                    selected,
                ),
            );
            // Where in the text the pointer is, once the text has been laid out: the edge of
            // the character under it nearer the pointer, as text selection goes everywhere.
            let layout = text.layout().clone();
            let chars = line.text.clone();
            let at = move |position: Point<Pixels>| {
                let ix = layout.index_for_position(position).unwrap_or_else(|ix| ix);
                let next = chars[ix..].chars().next().map(|c| ix + c.len_utf8());
                let edges = next.and_then(|next| {
                    Some((
                        layout.position_for_index(ix)?,
                        layout.position_for_index(next)?,
                        next,
                    ))
                });
                match edges {
                    Some((start, end, next)) if position.x > (start.x + end.x) / 2. => next,
                    _ => ix,
                }
            };
            card(h_flex())
                .id(("diff-line", row_id))
                .role(Role::Label)
                .aria_label(SharedString::from(line.text.clone()))
                .aria_description(match line.kind {
                    LineKind::Added => "added",
                    LineKind::Removed => "removed",
                    LineKind::Context => "unchanged",
                })
                .map(|el| {
                    if wrap {
                        el.items_stretch().min_h(px(LINE_HEIGHT))
                    } else {
                        el.h(px(LINE_HEIGHT))
                    }
                })
                .line_height(px(LINE_HEIGHT))
                .font_family(theme.mono_font.clone())
                .text_size(px(12.))
                .when_some(bg, |el, bg| el.bg(bg))
                .when(last, |el| el.border_b_1().rounded_b(px(6.)))
                .overflow_hidden()
                .child(
                    h_flex()
                        .id(("line-numbers", row_id))
                        .flex_none()
                        .child(number(line.old))
                        .child(number(line.new))
                        .when_some(toggle, |el, toggle| {
                            el.cursor(CursorStyle::PointingHand).on_mouse_down(
                                MouseButton::Left,
                                move |_, window, cx| {
                                    cx.stop_propagation();
                                    toggle(window, cx)
                                },
                            )
                        }),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(18.))
                        .pl(px(6.))
                        .text_color(sign_color)
                        .child(sign),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .when(!wrap, |el| el.whitespace_nowrap())
                        .overflow_hidden()
                        .text_color(theme.text_1)
                        .child(text),
                )
                .when_some(select, |el, select| {
                    let at_down = at.clone();
                    el.cursor(CursorStyle::IBeam)
                        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                            (select.down)(at_down(event.position), event, window, cx)
                        })
                        .on_mouse_move(move |event, window, cx| {
                            if event.pressed_button == Some(MouseButton::Left) {
                                (select.drag)(at(event.position), window, cx)
                            }
                        })
                })
                .when(excluded, |el| el.opacity(0.45))
                .into_any_element()
        }
    }
}

/// The scopes a hunk changes, as a breadcrumb that keeps the innermost one readable, or
/// its `@@` header when the language isn't one deltoids parses.
fn hunk_title(hunk: &Hunk, theme: &Theme) -> AnyElement {
    let Some((inner, outer)) = hunk.scope.split_last() else {
        return div()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .child(SharedString::from(hunk.header.clone()))
            .into_any_element();
    };
    h_flex()
        .flex_1()
        .min_w(px(0.))
        .gap(px(6.))
        .overflow_hidden()
        .children(outer.iter().map(|scope| {
            h_flex()
                .flex_shrink(1.)
                .min_w(px(0.))
                .gap(px(6.))
                .text_color(theme.text_3)
                .child(div().min_w(px(0.)).truncate().child(scope.clone()))
                .child("›")
        }))
        .child(
            div()
                .flex_none()
                .text_color(theme.text_1)
                .child(inner.clone()),
        )
        .into_any_element()
}

/// A highlight that only colours the ground behind the text.
fn ground(color: Hsla) -> HighlightStyle {
    HighlightStyle {
        background_color: Some(color),
        ..Default::default()
    }
}

/// An image's two sides next to each other, each at most a few lines tall.
fn image_row(
    before: Option<std::sync::Arc<gpui::Image>>,
    after: Option<std::sync::Arc<gpui::Image>>,
    theme: &Theme,
) -> gpui::Div {
    let side =
        |label: &'static str, image: Option<std::sync::Arc<gpui::Image>>, tint: gpui::Hsla| {
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(6.))
                .items_center()
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(label),
                )
                .child(match image {
                    Some(image) => div()
                        .p(px(6.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(tint)
                        .child(
                            gpui::img(image)
                                .max_w_full()
                                .max_h(px(220.))
                                .object_fit(gpui::ObjectFit::Contain),
                        )
                        .into_any_element(),
                    None => div()
                        .h(px(60.))
                        .flex()
                        .items_center()
                        .text_size(px(12.))
                        .text_color(theme.text_3)
                        .child(if label == "Before" {
                            "New file"
                        } else {
                            "Deleted"
                        })
                        .into_any_element(),
                })
        };
    h_flex()
        .items_start()
        .gap(px(16.))
        .p(px(12.))
        .border_b_1()
        .rounded_b(px(6.))
        .child(side("Before", before, theme.danger.opacity(0.5)))
        .child(side("After", after, theme.safe.opacity(0.6)))
}
