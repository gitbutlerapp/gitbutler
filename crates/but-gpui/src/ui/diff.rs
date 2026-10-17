//! Diffs as a flat list of rows, so a virtualised list only builds what's on screen.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, CursorStyle, ElementId, HighlightStyle, Hsla, MouseButton, MouseDownEvent,
    Pixels, Point, Role, SharedString, StyledText, Window, combine_highlights, div, prelude::*, px,
};

use super::{Icon, h_flex, small_icon, v_flex};
use crate::backend::{ChangeKind, DiffBody, DiffLine, FileDiff, Hunk, LineKind};
use crate::theme::Theme;

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
    /// An unchanged line between or around the hunks of a file shown whole, by its numbers
    /// on either side.
    Context {
        file: usize,
        old: u32,
        new: u32,
        last: bool,
    },
    /// Side by side: the hunk's line on the old side and the one on the new, either missing
    /// where the other side has more lines. An unchanged line is on both.
    Pair {
        file: usize,
        hunk: usize,
        left: Option<usize>,
        right: Option<usize>,
        last: bool,
    },
    /// Binary, too large or empty: a one-line explanation instead of lines.
    Note(usize),
    Gap,
}

impl DiffRow {
    /// Whether `self` shows what `other` showed, as after rows were built again: the same
    /// row whether or not it ends its card now, or the same line in the other layout.
    pub fn same_place(self, other: DiffRow) -> bool {
        use DiffRow::{Line, Pair};
        match (self.settled(), other.settled()) {
            (
                Line {
                    file, hunk, line, ..
                },
                Pair {
                    file: f,
                    hunk: h,
                    left,
                    right,
                    ..
                },
            )
            | (
                Pair {
                    file: f,
                    hunk: h,
                    left,
                    right,
                    ..
                },
                Line {
                    file, hunk, line, ..
                },
            ) => (file, hunk) == (f, h) && (left == Some(line) || right == Some(line)),
            (row, other) => row == other,
        }
    }

    /// The row without what a rebuild can change around it: whether it ends its card, and
    /// an unchanged line's old-side number, which moves with the hunks above it.
    fn settled(mut self) -> DiffRow {
        match &mut self {
            DiffRow::Line { last, .. } | DiffRow::Pair { last, .. } => *last = false,
            DiffRow::Context { old, last, .. } => (*old, *last) = (0, false),
            DiffRow::File(_) | DiffRow::Hunk { .. } | DiffRow::Note(_) | DiffRow::Gap => {}
        }
        self
    }

    /// The same row in file `to`, as when the file list changed around it.
    pub fn in_file(mut self, to: usize) -> DiffRow {
        match &mut self {
            DiffRow::File(file)
            | DiffRow::Note(file)
            | DiffRow::Hunk { file, .. }
            | DiffRow::Line { file, .. }
            | DiffRow::Pair { file, .. }
            | DiffRow::Context { file, .. } => *file = to,
            DiffRow::Gap => {}
        }
        self
    }

    pub fn file(self) -> Option<usize> {
        match self {
            DiffRow::File(file)
            | DiffRow::Note(file)
            | DiffRow::Hunk { file, .. }
            | DiffRow::Line { file, .. }
            | DiffRow::Pair { file, .. }
            | DiffRow::Context { file, .. } => Some(file),
            DiffRow::Gap => None,
        }
    }
}

/// Rows for `files`, with the files `folded` says are folded reduced to their header, the
/// files and hunks `hidden` and `hidden_hunk` say are hidden left out, and the files `whole`
/// gives the length of shown with every unchanged line. With `split`, a hunk's lines pair up
/// side by side.
pub fn rows(
    files: &[FileDiff],
    folded: impl Fn(&FileDiff) -> bool,
    hidden: impl Fn(&FileDiff) -> bool,
    hidden_hunk: impl Fn(&FileDiff, &Hunk) -> bool,
    whole: impl Fn(&FileDiff) -> Option<u32>,
    split: bool,
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
                let total = whole(file);
                // The first new-side line not shown yet.
                let mut next = 1;
                for (hunk_ix, hunk) in hunks.iter().enumerate() {
                    let header = hunk.key.0;
                    if total.is_some() {
                        let ahead = i64::from(header.new_start) - i64::from(header.old_start);
                        context_rows(&mut rows, file_ix, next..header.new_start, ahead);
                    }
                    next = header.new_start + header.new_lines;
                    if hidden_hunk(file, hunk) {
                        continue;
                    }
                    rows.push(DiffRow::Hunk {
                        file: file_ix,
                        hunk: hunk_ix,
                    });
                    if split {
                        rows.extend(pair_lines(&hunk.lines).into_iter().map(|(left, right)| {
                            DiffRow::Pair {
                                file: file_ix,
                                hunk: hunk_ix,
                                left,
                                right,
                                last: false,
                            }
                        }));
                    } else {
                        rows.extend((0..hunk.lines.len()).map(|line| DiffRow::Line {
                            file: file_ix,
                            hunk: hunk_ix,
                            line,
                            last: false,
                        }));
                    }
                }
                if let Some(total) = total.filter(|total| next <= *total) {
                    let end = hunks[hunks.len() - 1].key.0;
                    let ahead = i64::from(end.new_start + end.new_lines)
                        - i64::from(end.old_start + end.old_lines);
                    context_rows(&mut rows, file_ix, next..total + 1, ahead);
                }
            }
            _ => rows.push(DiffRow::Note(file_ix)),
        }
        // The line before the gap ends the file's card.
        if let Some(
            DiffRow::Line { last, .. } | DiffRow::Pair { last, .. } | DiffRow::Context { last, .. },
        ) = rows.last_mut()
        {
            *last = true;
        }
        rows.push(DiffRow::Gap);
    }
    rows
}

/// A hunk's lines side by side: unchanged ones on both sides, and each run of removals
/// beside the additions that follow it, one for one.
fn pair_lines(lines: &[DiffLine]) -> Vec<(Option<usize>, Option<usize>)> {
    let mut pairs = Vec::new();
    let mut ix = 0;
    while ix < lines.len() {
        if lines[ix].kind == LineKind::Context {
            pairs.push((Some(ix), Some(ix)));
            ix += 1;
            continue;
        }
        let run = |from: usize, kind| {
            (from..lines.len())
                .take_while(|&i| lines[i].kind == kind)
                .count()
        };
        let removed = run(ix, LineKind::Removed);
        let added = run(ix + removed, LineKind::Added);
        for k in 0..removed.max(added) {
            pairs.push((
                (k < removed).then_some(ix + k),
                (k < added).then_some(ix + removed + k),
            ));
        }
        ix += removed + added;
    }
    pairs
}

/// The unchanged new-side lines `lines`, numbered `ahead` lines further on than the old side.
fn context_rows(rows: &mut Vec<DiffRow>, file: usize, lines: Range<u32>, ahead: i64) {
    rows.extend(lines.map(|new| DiffRow::Context {
        file,
        old: u32::try_from(i64::from(new) - ahead).unwrap_or_default(),
        new,
        last: false,
    }));
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

/// What the diff's owner adds to a file or hunk header.
#[derive(Default)]
pub struct HeaderDecor {
    /// Before a hunk header, like a checkbox.
    pub leading: Option<AnyElement>,
    /// At the end of the header, like its controls.
    pub trailing: Option<AnyElement>,
    /// Marks a hunk left out of the next commit.
    pub excluded: bool,
    /// The hunk or file the keyboard is on.
    pub current: bool,
    /// The file shows only its header.
    pub folded: bool,
}

/// What the diff's owner adds to a line.
#[derive(Default)]
pub struct LineDecor {
    /// Dims a line left out of the next commit.
    pub excluded: bool,
    /// Clicking the line's numbers puts it in the next commit or takes it out.
    pub toggle: Option<OnToggle>,
    /// Lets the pointer select the line's text.
    pub select: Option<LineSelect>,
    /// The part of the line's text that's selected.
    pub selected: Option<Range<usize>>,
    /// What Find matched in the line's text, each with whether it's the current match.
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

/// A line's row by its file, hunk and index in the hunk, as a number unique to each row
/// for its element ids.
fn row_id((file, hunk, line): (usize, usize, usize)) -> usize {
    (file << 40) | (hunk << 20) | line
}

/// The sides and ground of a file's card, of which each of its rows draws a slice.
fn card(el: gpui::Div, theme: &Theme) -> gpui::Div {
    el.w_full()
        .border_l_1()
        .border_r_1()
        .border_color(theme.border_3)
        .bg(theme.bg_1)
}

/// The header at the top of the card of file `ix`.
pub fn file_header(ix: usize, file: &FileDiff, theme: &Theme, decor: HeaderDecor) -> AnyElement {
    let (name, dir) = split_path(&file.file.path);
    let (added, removed) = file.stats();
    card(h_flex(), theme)
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
        .when(decor.folded, |el| el.rounded_b(px(6.)))
        .when(decor.current, |el| el.border_color(theme.border_2))
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
        .children(decor.trailing)
        .into_any_element()
}

/// The header above a hunk's lines.
pub fn hunk_header(hunk: &Hunk, theme: &Theme, decor: HeaderDecor) -> AnyElement {
    card(h_flex(), theme)
        .h(px(theme.code_line + 6.))
        .pl(px(12.))
        .pr(px(4.))
        .gap(px(8.))
        .bg(if decor.current {
            theme.pop.opacity(0.1)
        } else {
            theme.bg_2
        })
        .font_family(theme.mono_font.clone())
        .text_size(px(11.5))
        .text_color(theme.text_2)
        .overflow_hidden()
        .whitespace_nowrap()
        .children(decor.leading)
        .child(hunk_title(hunk, theme))
        .when(decor.excluded, |el| {
            el.child(
                div()
                    .flex_none()
                    .text_color(theme.text_3)
                    .child("not included"),
            )
        })
        .children(decor.trailing)
        .into_any_element()
}

/// In place of the lines of a file without any to show: an image's two sides, or why
/// there's nothing.
pub fn note(file: &FileDiff, theme: &Theme) -> AnyElement {
    let text = match &file.body {
        DiffBody::Image { before, after } => {
            let images = image_row(before.clone(), after.clone(), theme);
            return card(images, theme).into_any_element();
        }
        DiffBody::Binary => "Binary file".to_owned(),
        DiffBody::TooLarge { bytes } => {
            format!("Too large to show ({:.1} MB)", *bytes as f64 / 1_000_000.)
        }
        DiffBody::Empty | DiffBody::Hunks { .. } => match file.file.kind {
            ChangeKind::Renamed => "Renamed without changes".to_owned(),
            _ => "No content changes".to_owned(),
        },
    };
    card(h_flex(), theme)
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

/// How a line of `kind` looks: its ground, its gutter's, its changed words', and its sign.
struct Look {
    bg: Option<Hsla>,
    gutter: Option<Hsla>,
    emph: Hsla,
    sign: &'static str,
    sign_color: Hsla,
}

fn look(kind: LineKind, theme: &Theme) -> Look {
    match kind {
        LineKind::Context => Look {
            bg: None,
            gutter: None,
            emph: theme.bg_1,
            sign: " ",
            sign_color: theme.text_3,
        },
        LineKind::Added => Look {
            bg: Some(theme.diff_added_bg),
            gutter: Some(theme.diff_added_gutter),
            emph: theme.diff_added_emph,
            sign: "+",
            sign_color: theme.safe,
        },
        LineKind::Removed => Look {
            bg: Some(theme.diff_removed_bg),
            gutter: Some(theme.diff_removed_gutter),
            emph: theme.diff_removed_emph,
            sign: "-",
            sign_color: theme.danger,
        },
    }
}

fn number_cell(n: Option<u32>, gutter: Option<Hsla>, theme: &Theme) -> gpui::Div {
    div()
        .flex_none()
        // Five digits and the padding, at the size code shows.
        .w(px((theme.code_text * 11. / 3.).round()))
        .pr(px(8.))
        .flex()
        .items_start()
        .justify_end()
        .text_color(theme.text_3)
        .when_some(gutter, |el, bg| el.bg(bg))
        .child(n.map(|n| n.to_string()).unwrap_or_default())
}

/// A line's text with its syntax, changed words, Find's matches and the selection marked.
fn line_text(
    line: &DiffLine,
    emph_bg: Hsla,
    found: Vec<(Range<usize>, bool)>,
    selected: Option<Range<usize>>,
    theme: &Theme,
) -> StyledText {
    let syntax = line
        .spans
        .iter()
        .map(|(range, token)| (range.clone(), HighlightStyle::from(theme.token(*token))));
    let emph = line
        .emph
        .iter()
        .map(|range| (range.clone(), ground(emph_bg)));
    // Blue like the system's text selection, apart from the green and red of the
    // change itself.
    let selected = selected.map(|range| (range, ground(theme.blue.opacity(0.3))));
    let found = found.into_iter().map(|(range, current)| {
        (
            range,
            ground(theme.warn.opacity(if current { 0.6 } else { 0.28 })),
        )
    });
    StyledText::new(SharedString::from(line.text.clone())).with_highlights(combine_highlights(
        combine_highlights(combine_highlights(syntax, emph), found),
        selected,
    ))
}

/// The sign and the text of a line, after its numbers.
fn sign_and_text(look: &Look, text: StyledText, wrap: bool, theme: &Theme) -> [gpui::Div; 2] {
    [
        div()
            .flex_none()
            .w(px(18.))
            .pl(px(6.))
            .text_color(look.sign_color)
            .child(look.sign),
        div()
            .flex_1()
            .min_w(px(0.))
            .when(!wrap, |el| el.whitespace_nowrap())
            .overflow_hidden()
            .text_color(theme.text_1)
            .child(text),
    ]
}

/// A row of a file's card, a line tall or taller when wrapping.
fn line_card(id: ElementId, last: bool, wrap: bool, theme: &Theme) -> gpui::Stateful<gpui::Div> {
    card(h_flex(), theme)
        .id(id)
        .map(|el| {
            if wrap {
                el.items_stretch().min_h(px(theme.code_line))
            } else {
                el.h(px(theme.code_line))
            }
        })
        .line_height(px(theme.code_line))
        .font_family(theme.mono_font.clone())
        .text_size(px(theme.code_text))
        .when(last, |el| el.border_b_1().rounded_b(px(6.)))
        .overflow_hidden()
}

/// Line numbers that put the line in the next commit or take it out when clicked.
fn numbers(id: ElementId, cells: Vec<gpui::Div>, toggle: Option<OnToggle>) -> AnyElement {
    h_flex()
        .id(id)
        .flex_none()
        .children(cells)
        .when_some(toggle, |el, toggle| {
            el.cursor(CursorStyle::PointingHand).on_mouse_down(
                MouseButton::Left,
                move |_, window, cx| {
                    cx.stop_propagation();
                    toggle(window, cx)
                },
            )
        })
        .into_any_element()
}

/// A line of the diff inside its file's card, numbered `old` and `new`. `place` is the
/// line's file, hunk and index in the hunk, which tell rows apart.
pub fn line_row(
    line: &DiffLine,
    (old, new): (Option<u32>, Option<u32>),
    place: (usize, usize, usize),
    last: bool,
    theme: &Theme,
    decor: LineDecor,
) -> AnyElement {
    let LineDecor {
        excluded,
        toggle,
        select,
        selected,
        found,
        wrap,
    } = decor;
    let id = row_id(place);
    let look = look(line.kind, theme);
    let text = line_text(line, look.emph, found, selected, theme);
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
    line_card(("diff-line", id).into(), last, wrap, theme)
        .role(Role::Label)
        .aria_label(SharedString::from(line.text.clone()))
        .aria_description(match line.kind {
            LineKind::Added => "added",
            LineKind::Removed => "removed",
            LineKind::Context => "unchanged",
        })
        .when_some(look.bg, |el, bg| el.bg(bg))
        .child(numbers(
            ("line-numbers", id).into(),
            vec![
                number_cell(old, look.gutter, theme),
                number_cell(new, look.gutter, theme),
            ],
            toggle,
        ))
        .children(sign_and_text(&look, text, wrap, theme))
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

/// An unchanged line of a file shown whole, from the file's new side, at `old` and `new`;
/// on both sides with `split`, where its text can't be selected.
pub fn context_row(
    line: &DiffLine,
    file: usize,
    (old, new): (u32, u32),
    last: bool,
    theme: &Theme,
    decor: LineDecor,
    split: bool,
) -> AnyElement {
    // Apart from the hunks' lines, which number their hunk below 2^20.
    let place = (file, 0xfffff, new as usize);
    if split {
        let sides = [Some((line, Some(old))), Some((line, Some(new)))];
        return pair_row(sides, Default::default(), place, last, theme, decor.wrap);
    }
    line_row(line, (Some(old), Some(new)), place, last, theme, decor)
}

/// What one side of a side-by-side row adds to its line.
#[derive(Default)]
pub struct Half {
    pub excluded: bool,
    pub toggle: Option<OnToggle>,
    pub found: Vec<(Range<usize>, bool)>,
}

/// A side-by-side row: a line from the old side on the left and one from the new on the
/// right, each with its number on that side. `place` tells rows apart, as for [`line_row`].
pub fn pair_row(
    [left, right]: [Option<(&DiffLine, Option<u32>)>; 2],
    [left_half, right_half]: [Half; 2],
    place: (usize, usize, usize),
    last: bool,
    theme: &Theme,
    wrap: bool,
) -> AnyElement {
    let id = row_id(place);
    let side = |line: Option<(&DiffLine, Option<u32>)>, half: Half, name: &'static str| {
        let Some((line, number)) = line else {
            // Nothing on this side: a quiet gap the other side's lines line up against.
            return div().flex_1().min_w(px(0.)).bg(theme.bg_2);
        };
        let look = look(line.kind, theme);
        let text = line_text(line, look.emph, half.found, None, theme);
        h_flex()
            .flex_1()
            .min_w(px(0.))
            .when(wrap, |el| el.items_stretch())
            .when_some(look.bg, |el, bg| el.bg(bg))
            .child(numbers(
                (name, id).into(),
                vec![number_cell(number, look.gutter, theme)],
                half.toggle,
            ))
            .children(sign_and_text(&look, text, wrap, theme))
            .when(half.excluded, |el| el.opacity(0.45))
    };
    line_card(("diff-pair", id).into(), last, wrap, theme)
        .child(side(left, left_half, "pair-left"))
        .child(div().flex_none().w(px(1.)).h_full().bg(theme.border_3))
        .child(side(right, right_half, "pair-right"))
        .into_any_element()
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
    let side = |label: &'static str,
                missing: &'static str,
                image: Option<std::sync::Arc<gpui::Image>>,
                tint: gpui::Hsla| {
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
                    .child(missing)
                    .into_any_element(),
            })
    };
    h_flex()
        .items_start()
        .gap(px(16.))
        .p(px(12.))
        .border_b_1()
        .rounded_b(px(6.))
        .child(side(
            "Before",
            "New file",
            before,
            theme.danger.opacity(0.5),
        ))
        .child(side("After", "Deleted", after, theme.safe.opacity(0.6)))
}

#[cfg(test)]
mod tests {
    use super::DiffRow::{Context, File, Gap, Line, Note, Pair};
    use super::pair_lines;
    use crate::backend::{DiffLine, LineKind};

    #[test]
    fn same_place_finds_a_row_again_after_a_rebuild_or_in_the_other_layout() {
        let line = |line, last| Line {
            file: 1,
            hunk: 2,
            line,
            last,
        };
        let pair = |left, right, last| Pair {
            file: 1,
            hunk: 2,
            left,
            right,
            last,
        };
        let context = |old, new, last| Context {
            file: 1,
            old,
            new,
            last,
        };
        let same = [
            // Whether or not they end their card.
            (line(4, false), line(4, true)),
            (pair(Some(4), None, true), pair(Some(4), None, false)),
            // A removed line on the left of its pair, an added one on the right, either way round.
            (line(4, false), pair(Some(4), Some(6), true)),
            (line(6, true), pair(Some(4), Some(6), false)),
            (pair(Some(4), Some(6), false), line(6, false)),
            // An unchanged line by its new-side number, as the hunks above move its old one.
            (context(9, 12, false), context(7, 12, true)),
            (File(1), File(1)),
            (Note(1), Note(1)),
        ];
        for (a, b) in same {
            assert!(a.same_place(b), "{a:?} shows what {b:?} showed");
        }
        let in_other_hunk = Pair {
            file: 1,
            hunk: 3,
            left: Some(4),
            right: None,
            last: false,
        };
        let different = [
            (line(4, false), line(5, false)),
            (pair(Some(4), Some(6), false), pair(Some(4), None, false)),
            (pair(Some(4), Some(6), false), line(5, false)),
            (line(4, false), in_other_hunk),
            (context(9, 12, false), context(9, 13, false)),
            // An unchanged line of a file shown whole is no hunk's line.
            (context(4, 4, false), line(4, false)),
            (File(1), File(2)),
            (Gap, File(1)),
        ];
        for (a, b) in different {
            assert!(!a.same_place(b), "{a:?} shows something other than {b:?}");
        }
    }

    #[test]
    fn pairs_removals_with_the_additions_after_them() {
        let lines: Vec<DiffLine> = [
            LineKind::Context,
            LineKind::Removed,
            LineKind::Removed,
            LineKind::Added,
            LineKind::Context,
            LineKind::Added,
        ]
        .into_iter()
        .map(|kind| DiffLine {
            kind,
            old: None,
            new: None,
            text: "".into(),
            spans: Vec::new(),
            emph: Vec::new(),
        })
        .collect();
        assert_eq!(
            pair_lines(&lines),
            vec![
                (Some(0), Some(0)),
                (Some(1), Some(3)),
                (Some(2), None),
                (Some(4), Some(4)),
                (None, Some(5)),
            ],
            "the first removal faces the addition, the second has nothing opposite, and a \
             lone addition faces a gap"
        );
    }
}
