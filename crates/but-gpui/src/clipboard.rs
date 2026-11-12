//! The clipboard. Everything the app copies or pastes goes through here, as `clippy.toml` holds
//! the rest of the app to, so that every copy says what it took the same way.
//!
//! A copy someone asks for says so in the snackbar, "Copied …", as [`copied`] words it. A text
//! field or a terminal copies without a word, as editors do, and so does a copy that something
//! on screen already mentions, as GitHub's sign-in code.

use gpui::{Action, App, ClipboardItem, Div, Entity, InteractiveElement, SharedString, Stateful};

use crate::state::{Store, snack};

/// The most of a line the snackbar names in full.
const NAMED: usize = 64;

/// Put `text` on the clipboard, and say what was copied.
pub fn copy(store: &Entity<Store>, text: String, cx: &mut App) {
    let message = copied(&text);
    copy_saying(store, text, message, cx);
}

/// Put `text` on the clipboard, and say `message`, for what [`copied`] can't name from the
/// text alone.
pub fn copy_saying(
    store: &Entity<Store>,
    text: String,
    message: impl Into<SharedString>,
    cx: &mut App,
) {
    copy_silently(text, cx);
    snack(store, message, cx);
}

/// Put an error message on the clipboard, and say so, as every Copy error message does.
pub fn copy_error(store: &Entity<Store>, text: String, cx: &mut App) {
    copy_saying(store, text, "Copied the error message", cx);
}

/// Register `A` on `el` as copying `text`, and saying what was copied.
pub fn copy_on<A: Action>(el: Stateful<Div>, store: &Entity<Store>, text: String) -> Stateful<Div> {
    let store = store.clone();
    el.on_action(move |_: &A, _, cx| copy(&store, text.clone(), cx))
}

/// Put `image` on the clipboard, and say `message`. Only a screenshot copies one, as only
/// macOS takes them.
#[cfg(target_os = "macos")]
pub fn copy_image(
    store: &Entity<Store>,
    image: &gpui::Image,
    message: impl Into<SharedString>,
    cx: &mut App,
) {
    write(ClipboardItem::new_image(image), cx);
    snack(store, message, cx);
}

/// Put `text` on the clipboard without a word: a text field's or a terminal's copy, or one
/// that something on screen already mentions.
pub fn copy_silently(text: String, cx: &mut App) {
    write(ClipboardItem::new_string(text), cx);
}

/// The text on the clipboard, to paste.
#[expect(
    clippy::disallowed_methods,
    reason = "the one place the app reads the clipboard"
)]
pub fn text(cx: &App) -> Option<String> {
    cx.read_from_clipboard().and_then(|item| item.text())
}

#[expect(
    clippy::disallowed_methods,
    reason = "the one place the app writes the clipboard"
)]
fn write(item: ClipboardItem, cx: &mut App) {
    cx.write_to_clipboard(item);
}

/// What the snackbar says after copying `text`: the text itself, or how many lines it has.
pub fn copied(text: &str) -> String {
    took("Copied", text)
}

/// What the snackbar says after `verb`, as "Copied" or "Cut", took `text`: the text itself,
/// its middle left out when it's long, or how many lines it has.
pub fn took(verb: &str, text: &str) -> String {
    match text.lines().count() {
        0 | 1 => format!("{verb} {}", shortened(text)),
        lines => format!("{verb} {lines} lines"),
    }
}

/// `text`, or when it's longer than [`NAMED`], its start and end around a `…`, so a path keeps
/// its file name.
fn shortened(text: &str) -> String {
    let count = text.chars().count();
    if count <= NAMED {
        return text.to_owned();
    }
    let start = (NAMED - 1) / 2;
    let end = NAMED - 1 - start;
    let start: String = text.chars().take(start).collect();
    let end: String = text.chars().skip(count - end).collect();
    format!("{start}…{end}")
}

#[cfg(test)]
mod tests {
    use super::{NAMED, copied, took};

    #[test]
    fn a_copy_says_what_it_took() {
        assert_eq!(copied("main"), "Copied main", "short text is named");
        let most = "x".repeat(NAMED);
        assert_eq!(
            copied(&most),
            format!("Copied {most}"),
            "as long as it may be"
        );
        let path = "/Users/someone/code/gitbutler/crates/but-gpui/src/detail/brief/actions.rs";
        assert_eq!(
            copied(path),
            "Copied /Users/someone/code/gitbutler/c…gpui/src/detail/brief/actions.rs",
            "a long line keeps its start and end"
        );
        let wide = "é".repeat(NAMED + 1);
        assert_eq!(
            copied(&wide).chars().count(),
            "Copied ".len() + NAMED,
            "it counts characters, not bytes"
        );
        assert_eq!(
            copied("a\nb\nc"),
            "Copied 3 lines",
            "several lines are counted"
        );
        assert_eq!(took("Cut", "word"), "Cut word", "as cutting says it");
    }
}
