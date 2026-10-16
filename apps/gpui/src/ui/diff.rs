//! Diffs as a flat list of rows, so a virtualised list only builds what's on screen.

use gpui::{AnyElement, HighlightStyle, Hsla, SharedString, StyledText, div, prelude::*, px};

use super::{Icon, h_flex, small_icon};
use crate::backend::{ChangeKind, DiffBody, FileDiff, LineKind};
use crate::theme::Theme;

pub const LINE_HEIGHT: f32 = 20.;

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

/// Rows for `files`, with the files `folded` says are folded reduced to their header.
pub fn rows(files: &[FileDiff], folded: impl Fn(&FileDiff) -> bool) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for (file_ix, file) in files.iter().enumerate() {
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
}

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
                .h(px(38.))
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
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .child(SharedString::from(hunks[hunk].header.clone())),
                )
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
            let text = match &files[file].body {
                DiffBody::Binary => "Binary file".to_owned(),
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
            let line = &hunks[hunk].lines[line];
            let (bg, gutter_bg, sign, sign_color) = match line.kind {
                LineKind::Context => (None, None, " ", theme.text_3),
                LineKind::Added => (
                    Some(theme.diff_added_bg),
                    Some(theme.diff_added_gutter),
                    "+",
                    theme.safe,
                ),
                LineKind::Removed => (
                    Some(theme.diff_removed_bg),
                    Some(theme.diff_removed_gutter),
                    "-",
                    theme.danger,
                ),
            };
            let number = |n: Option<u32>| {
                div()
                    .flex_none()
                    .w(px(44.))
                    .h_full()
                    .pr(px(8.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .text_color(theme.text_3)
                    .when_some(gutter_bg, |el, bg| el.bg(bg))
                    .child(n.map(|n| n.to_string()).unwrap_or_default())
            };
            card(h_flex())
                .h(px(LINE_HEIGHT))
                .font_family(theme.mono_font.clone())
                .text_size(px(12.))
                .when_some(bg, |el, bg| el.bg(bg))
                .when(last, |el| el.border_b_1().rounded_b(px(6.)))
                .overflow_hidden()
                .child(number(line.old))
                .child(number(line.new))
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
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_color(theme.text_1)
                        .child(
                            StyledText::new(SharedString::from(line.text.clone())).with_highlights(
                                line.spans.iter().map(|(range, token)| {
                                    (
                                        range.clone(),
                                        HighlightStyle {
                                            color: Some(theme.token(*token)),
                                            ..Default::default()
                                        },
                                    )
                                }),
                            ),
                        ),
                )
                .when(excluded, |el| el.opacity(0.45))
                .into_any_element()
        }
    }
}
