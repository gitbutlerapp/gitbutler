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
        use DiffRow::{Context, Line, Pair};
        match (self, other) {
            (
                Line {
                    file, hunk, line, ..
                },
                Line {
                    file: f,
                    hunk: h,
                    line: l,
                    ..
                },
            ) => (file, hunk, line) == (f, h, l),
            (
                Pair {
                    file,
                    hunk,
                    left,
                    right,
                    ..
                },
                Pair {
                    file: f,
                    hunk: h,
                    left: l,
                    right: r,
                    ..
                },
            ) => (file, hunk, left, right) == (f, h, l, r),
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
            (
                Context { file, new, .. },
                Context {
                    file: f, new: n, ..
                },
            ) => (file, new) == (f, n),
            _ => self == other,
        }
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
/// ones `hidden` says are hidden left out, and the ones `whole` gives the length of shown
/// with every unchanged line. With `split`, a hunk's lines pair up side by side.
pub fn rows(
    files: &[FileDiff],
    folded: impl Fn(&FileDiff) -> bool,
    hidden: impl Fn(&FileDiff) -> bool,
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
                    rows.push(DiffRow::Hunk {
                        file: file_ix,
                        hunk: hunk_ix,
                    });
                    let last_hunk =
                        hunk_ix + 1 == hunks.len() && total.is_none_or(|total| next > total);
                    if split {
                        let pairs = pair_lines(&hunk.lines);
                        let count = pairs.len();
                        rows.extend(pairs.into_iter().enumerate().map(|(ix, (left, right))| {
                            DiffRow::Pair {
                                file: file_ix,
                                hunk: hunk_ix,
                                left,
                                right,
                                last: last_hunk && ix + 1 == count,
                            }
                        }));
                    } else {
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
                if let Some(total) = total.filter(|total| next <= *total) {
                    let end = hunks[hunks.len() - 1].key.0;
                    let ahead = i64::from(end.new_start + end.new_lines)
                        - i64::from(end.old_start + end.old_lines);
                    context_rows(&mut rows, file_ix, next..total + 1, ahead);
                    if let Some(DiffRow::Context { last, .. }) = rows.last_mut() {
                        *last = true;
                    }
                }
            }
            _ => rows.push(DiffRow::Note(file_ix)),
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

/// An unchanged line of a file shown whole, from the file's new side, at `old` and `new`;
/// on both sides with `split`, where its text can't be selected.
pub fn render_context_row(
    line: &DiffLine,
    file: usize,
    (old, new): (u32, u32),
    last: bool,
    theme: &Theme,
    decor: Decor,
    split: bool,
) -> AnyElement {
    let line = DiffLine {
        old: Some(old),
        new: Some(new),
        ..line.clone()
    };
    // Apart from the hunks' lines, which number their hunk below 2^20.
    let row_id = (file << 40) | (0xfffff << 20) | new as usize;
    if split {
        let halves = [Half::default(), Half::default()];
        return render_pair_row(
            (Some(&line), Some(&line)),
            halves,
            row_id,
            last,
            theme,
            decor.wrap,
        );
    }
    div()
        .w_full()
        .px(px(16.))
        .child(line_row(&line, row_id, last, theme, decor))
        .into_any_element()
}

fn render_card_row(row: DiffRow, files: &[FileDiff], theme: &Theme, decor: Decor) -> AnyElement {
    if let DiffRow::Line {
        file,
        hunk,
        line,
        last,
    } = row
    {
        let DiffBody::Hunks { hunks, .. } = &files[file].body else {
            return div().into_any_element();
        };
        // Unique per line, for the line's accessibility node.
        let row_id = (file << 40) | (hunk << 20) | line;
        return line_row(&hunks[hunk].lines[line], row_id, last, theme, decor);
    }
    let Decor {
        leading,
        trailing,
        excluded,
        current,
        folded,
        ..
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
                .h(px(theme.code_line + 6.))
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
        // Drawn by `line_row`, which these need their line for.
        DiffRow::Line { .. } | DiffRow::Pair { .. } | DiffRow::Context { .. } => {
            div().into_any_element()
        }
    }
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
    h_flex()
        .id(id)
        .w_full()
        .border_l_1()
        .border_r_1()
        .border_color(theme.border_3)
        .bg(theme.bg_1)
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

/// A line of the diff, inside its file's card.
fn line_row(line: &DiffLine, row_id: usize, last: bool, theme: &Theme, decor: Decor) -> AnyElement {
    let Decor {
        excluded,
        toggle,
        select,
        selected,
        found,
        wrap,
        ..
    } = decor;
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
    let cells = vec![
        number_cell(line.old, look.gutter, theme),
        number_cell(line.new, look.gutter, theme),
    ];
    line_card(("diff-line", row_id).into(), last, wrap, theme)
        .role(Role::Label)
        .aria_label(SharedString::from(line.text.clone()))
        .aria_description(match line.kind {
            LineKind::Added => "added",
            LineKind::Removed => "removed",
            LineKind::Context => "unchanged",
        })
        .when_some(look.bg, |el, bg| el.bg(bg))
        .child(numbers(("line-numbers", row_id).into(), cells, toggle))
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

/// What one side of a side-by-side row adds to its line.
#[derive(Default)]
pub struct Half {
    pub excluded: bool,
    pub toggle: Option<OnToggle>,
    pub found: Vec<(Range<usize>, bool)>,
}

/// A side-by-side row: `left` from the old side, numbered there, and `right` from the new.
pub fn render_pair_row(
    (left, right): (Option<&DiffLine>, Option<&DiffLine>),
    [left_half, right_half]: [Half; 2],
    row_id: usize,
    last: bool,
    theme: &Theme,
    wrap: bool,
) -> AnyElement {
    let side = |line: Option<&DiffLine>, number: Option<u32>, half: Half, id: &'static str| {
        let Some(line) = line else {
            // Nothing on this side: a quiet gap the other side's lines line up against.
            return div().flex_1().min_w(px(0.)).bg(theme.bg_2);
        };
        let look = look(line.kind, theme);
        let text = line_text(line, look.emph, half.found, None, theme);
        let cells = vec![number_cell(number, look.gutter, theme)];
        h_flex()
            .flex_1()
            .min_w(px(0.))
            .when(wrap, |el| el.items_stretch())
            .when_some(look.bg, |el, bg| el.bg(bg))
            .child(numbers((id, row_id).into(), cells, half.toggle))
            .children(sign_and_text(&look, text, wrap, theme))
            .when(half.excluded, |el| el.opacity(0.45))
    };
    div()
        .w_full()
        .px(px(16.))
        .child(
            line_card(("diff-pair", row_id).into(), last, wrap, theme)
                .child(side(left, left.and_then(|l| l.old), left_half, "pair-left"))
                .child(div().flex_none().w(px(1.)).h_full().bg(theme.border_3))
                .child(side(
                    right,
                    right.and_then(|l| l.new),
                    right_half,
                    "pair-right",
                )),
        )
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

#[cfg(test)]
mod tests {
    use super::pair_lines;
    use crate::backend::{DiffLine, LineKind};

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
