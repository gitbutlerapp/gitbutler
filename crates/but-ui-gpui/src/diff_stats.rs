//! A diff's `+N -N` line counts, as but-ui-react's `DiffStats`: in the UI font, each side only
//! when it isn't zero, so nothing at all when nothing changed.

use std::ops::Range;

use gpui::{Div, HighlightStyle, StyledText, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// The count [`diff_stats`] writes in thousands from: six figures leave a file list's name no
/// room.
const THOUSANDS_FROM: u32 = 100_000;

/// The lines a file or a change adds and removes, `solid` when it sits on a solid selection,
/// whose own colours it then takes.
pub fn diff_stats(added: u32, removed: u32, solid: bool, theme: &Theme) -> Div {
    // One run of text, coloured in parts: a list draws this on every row.
    let (text, [added, removed]) = stats_text(added, removed);
    let [safe, danger] = match solid {
        true => [
            theme.derived.inverse_success_text,
            theme.derived.inverse_danger_text,
        ],
        false => [theme.success_text, theme.danger_text],
    };
    let colors = [(added, safe), (removed, danger)]
        .into_iter()
        .filter(|(range, _)| !range.is_empty())
        .map(|(range, color)| (range, HighlightStyle::from(color)));
    div()
        .flex_none()
        .text_12()
        .child(StyledText::new(text).with_highlights(colors))
}

/// `count` lines with their `sign`, as [`diff_stats`] writes them.
fn count_text(sign: char, count: u32) -> String {
    match count {
        0 => String::new(),
        1..THOUSANDS_FROM => format!("{sign}{count}"),
        _ => format!("{sign}{}k", count / 1000),
    }
}

/// What [`diff_stats`] writes, and where in it its added and removed counts are: empty ranges for
/// counts of nothing.
fn stats_text(added: u32, removed: u32) -> (String, [Range<usize>; 2]) {
    let mut text = String::new();
    let mut ranges = [0..0, 0..0];
    let counts = [('+', added), ('-', removed)];
    for ((sign, count), range) in counts.into_iter().zip(&mut ranges) {
        let count = count_text(sign, count);
        if count.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        let start = text.len();
        text.push_str(&count);
        *range = start..text.len();
    }
    (text, ranges)
}

#[cfg(test)]
mod tests {
    use super::stats_text;

    #[test]
    fn each_side_shows_only_when_it_changed() {
        let texts: Vec<String> = [(676, 1140), (0, 72), (139, 0), (0, 0), (120_000, 2)]
            .into_iter()
            .map(|(added, removed)| stats_text(added, removed).0)
            .collect();
        assert_eq!(
            texts,
            ["+676 -1140", "-72", "+139", "", "+120k -2"],
            "as React's DiffStats, a side of nothing takes no room"
        );
        let (text, ranges) = stats_text(0, 72);
        assert_eq!(
            (&text[ranges[1].clone()], ranges[0].is_empty()),
            ("-72", true),
            "only the counts there are get a colour",
        );
    }
}
