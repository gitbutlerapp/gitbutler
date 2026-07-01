//! Shared utilities for terminal-width detection and text truncation.
//!
//! Every function here is Unicode-width-aware (CJK, emoji, combining marks)
//! and ANSI-escape-aware so that colored strings are measured and truncated
//! correctly.

use std::borrow::Cow;
use terminal_size::Width;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Returns the current terminal width in columns, defaulting to 80
/// when detection fails (e.g. when stdout is not a TTY).
pub fn terminal_width() -> usize {
    if cfg!(test) {
        80
    } else {
        terminal_size::terminal_size().map_or(80, |(Width(w), _)| w as usize)
    }
}

/// Truncate `text` to fit within `max_width` display columns.
///
/// Uses [`unicode_width`] so that CJK / emoji characters (which occupy
/// two terminal columns each) are measured correctly. ANSI escape
/// sequences (e.g. color codes) are passed through without counting
/// toward the width.
///
/// When truncation occurs an `…` character (1 column wide) is appended
/// and the total result is guaranteed to be ≤ `max_width` columns.
pub fn truncate_text<'a>(text: impl Into<Cow<'a, str>>, max_width: usize) -> Cow<'a, str> {
    let text = text.into();

    if max_width == 0 {
        return Cow::Borrowed("");
    }

    let visible_width = strip_ansi_codes(text.as_ref()).width();
    if visible_width <= max_width {
        return text;
    }

    if max_width == 1 {
        return Cow::Borrowed("…");
    }

    // We know truncation is needed; reserve one display column for ellipsis.
    let target_width = max_width.saturating_sub(1);
    let mut width = 0;
    let mut out = String::new();
    let mut in_ansi = false;
    let mut ansi_buffer = String::new();

    for ch in text.chars() {
        // Start of an ANSI escape sequence — buffer it.
        if ch == '\x1b' {
            in_ansi = true;
            ansi_buffer.push(ch);
            continue;
        }

        // Inside an escape sequence — keep buffering until the terminating 'm'.
        if in_ansi {
            ansi_buffer.push(ch);
            if ch == 'm' {
                // Flush the whole escape sequence into the result
                // without counting toward display width.
                out.extend(ansi_buffer.drain(..));
                in_ansi = false;
            }
            continue;
        }

        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > target_width {
            out.push('…');
            return out.into();
        }
        out.push(ch);
        width += ch_width;
    }

    out.into()
}

/// Remove all ANSI escape sequences from `s`, returning plaintext.
///
/// Useful when you need to measure the *display* width of a string
/// that may contain color / style codes.
pub fn strip_ansi_codes(s: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(s);
    let mut needs_mutation = false;
    let mut in_escape = false;

    for (idx, ch) in s.char_indices() {
        if ch == '\x1b' {
            if !needs_mutation {
                out.to_mut().truncate(idx);
                needs_mutation = true;
            }
            in_escape = true;
            continue;
        }

        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }

        if needs_mutation {
            out.to_mut().push(ch);
        }
    }

    out
}

#[cfg(test)]
mod truncate_text_tests {
    use unicode_width::UnicodeWidthStr;

    use super::truncate_text;

    #[test]
    fn text_at_exact_limit_is_not_truncated() {
        assert_eq!(truncate_text("hello", 5), "hello");
    }

    #[test]
    fn text_exceeding_limit_is_truncated_with_ellipsis() {
        assert_eq!(truncate_text("hello world", 5), "hell…");
    }

    #[test]
    fn truncation_does_not_drop_plain_text_when_boundary_ends_with_m() {
        assert_eq!(truncate_text("lorem ipsum", 5), "lore…");
    }

    #[test]
    fn max_width_of_two_keeps_one_character_plus_ellipsis() {
        assert_eq!(truncate_text("hello", 2), "h…");
    }

    #[test]
    fn empty_text_stays_empty() {
        assert_eq!(truncate_text("", 10), "");
    }

    #[test]
    fn max_width_of_zero_gives_empty_string() {
        assert_eq!(truncate_text("hello", 0), "");
    }

    #[test]
    fn max_width_of_one_gives_ellipsis_only() {
        assert_eq!(truncate_text("hello", 1), "…");
    }

    #[test]
    fn unicode_single_width_characters() {
        assert_eq!(
            truncate_text("über-cool", 5),
            "über…",
            "ü is a single-width character (1 display column"
        );
    }

    #[test]
    fn cjk_double_width_characters() {
        // Each CJK character occupies 2 display columns.
        assert_eq!(
            truncate_text("你好世界", 5),
            "你好…",
            "你(2) + 好(2) = 4 cols, + …(1) = 5 cols total."
        );
        assert_eq!(truncate_text("你好世界", 5).width(), 5);
    }

    #[test]
    fn cjk_does_not_exceed_max_width() {
        // With max_width 4, a second CJK char (2 cols) leaves no room
        // for the ellipsis alongside it, so only the first char + … fits.
        let result = truncate_text("你好世界", 4);
        assert_eq!(result.width(), 3, "你(2) + …(1) = 3 cols ≤ 4");
        assert_eq!(result, "你…");
    }

    #[test]
    fn truncation_preserves_exact_boundary() {
        let msg = "this is a overly long commit message to demonstrate truncation";
        let result = truncate_text(msg, 60);
        assert!(result.ends_with('…'));
        assert_eq!(
            result.width(),
            60,
            "For ASCII text, display width == char count"
        );
    }

    #[test]
    fn emoji_characters() {
        // Many emoji are wide characters; ensure we respect their display width.
        let single = "🙂";
        let single_width = single.width();
        assert!(single_width >= 1);
        // A single emoji that fits within max_width should not be truncated.
        assert_eq!(truncate_text(single, single_width), single);
        // Repeated emoji should be truncated without exceeding max_width.
        let repeated = "🙂🙂🙂";
        let result = truncate_text(repeated, single_width * 2 + 1);
        assert_eq!(result, "🙂🙂…");

        let result = truncate_text(repeated, single_width * 2);
        assert_eq!(
            result, "🙂…",
            "4 columns aren't enough, so we use less than the allowed width"
        );
    }

    #[test]
    fn zero_width_combining_characters_are_handled() {
        let text = "a\u{0301}";
        assert_eq!(
            text.width(),
            1,
            "'a' + COMBINING ACUTE ACCENT; display width should be 1."
        );
        assert_eq!(
            truncate_text(text, 1),
            text,
            "With max_width equal to the display width, no truncation should occur."
        );
    }

    #[test]
    fn ansi_codes_only_produce_no_visible_output_within_width() {
        let just_color = "\x1b[31m\x1b[0m";
        let max_width = 5;
        assert!(
            max_width < just_color.width(),
            "Naive counting would want to truncate this string"
        );
        assert_eq!(
            truncate_text(just_color, max_width),
            just_color,
            "nothing changes as non-printing characters don't participate"
        );
    }

    #[test]
    fn ansi_colored_text_is_truncated_without_counting_escapes() {
        // "\x1b[31m" = red, "\x1b[0m" = reset — both zero-width.
        let colored = "\x1b[31mhello world\x1b[0m";
        let result = truncate_text(colored, 5);
        assert_eq!(
                result, "\x1b[31mhell…",
                "The ANSI prefix should be preserved and the visible text truncated to 4 chars + ellipsis.
                This also means we will remove relevant ansi codes."
            );
    }
}

#[cfg(test)]
mod strip_ansi_codes_tests {
    use super::strip_ansi_codes;

    #[test]
    fn broken_ansi_sequence_is_handled_without_panic() {
        let broken = "hello world\x1b[31broken";
        assert_eq!(
            strip_ansi_codes(broken),
            "hello world",
            "Missing the terminating 'm' after the CSI sequence is fine,
            but it hides everything after"
        );
    }

    #[test]
    fn plain_text_is_unchanged_by_strip_ansi() {
        assert_eq!(strip_ansi_codes("hello world"), "hello world");
    }

    #[test]
    fn strip_ansi_removes_color_codes() {
        let colored = "\x1b[31mhello\x1b[0m world";
        assert_eq!(strip_ansi_codes(colored), "hello world");
    }
}
