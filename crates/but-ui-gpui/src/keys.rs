//! Keys, written as the platform writes them and drawn as but-ui-react's `Kbd`.

use gpui::{App, Div, Hsla, SharedString, Window, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// The keys bound to `action`, as macOS writes them, like `⌘⇧P`: [`keycap`] and [`keys_text`]
/// show them as the platform does.
pub fn shortcut(action: &dyn gpui::Action, cx: &App) -> Option<SharedString> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    Some(keys_of(keymap.bindings_for_action(action).next_back()?))
}

/// Every key that runs `action`, as [`shortcut`] writes keys, with the context it works in.
pub fn bindings_of(action: &dyn gpui::Action, cx: &App) -> Vec<(SharedString, Option<String>)> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    (keymap.bindings_for_action(action))
        .map(|binding| (keys_of(binding), binding.predicate().map(|p| p.to_string())))
        .collect()
}

/// The keys that run `action` where the keys go now, unless the same keys do something else
/// there, as [`shortcut`] writes them. Keys bound to several actions there run the first one
/// handled, so one with no handler here doesn't take them: r is Reword's on a commit and Rename
/// branch's on a branch.
pub fn shortcut_here(action: &dyn gpui::Action, window: &Window, cx: &App) -> Option<SharedString> {
    window.focused(cx)?;
    let context = window.context_stack();
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let runs = |binding: &gpui::KeyBinding| {
        let (bound, _) = keymap.bindings_for_input(binding.keystrokes(), &context);
        (bound.iter())
            .find(|b| b.action().partial_eq(action) || window.is_action_available(b.action(), cx))
            .is_some_and(|b| b.action().partial_eq(action))
    };
    let binding = keymap.bindings_for_action(action).rev().find(|b| runs(b))?;
    Some(keys_of(binding))
}

pub fn keys_of(binding: &gpui::KeyBinding) -> SharedString {
    let keys: Vec<String> = binding
        .keystrokes()
        .iter()
        .map(|keystroke| key_text(keystroke.modifiers(), keystroke.key()))
        .collect();
    keys.join(" ").into()
}

/// `keys`, as a keymap writes them, as [`shortcut`] writes keys.
pub fn keys_written(keys: &str) -> SharedString {
    let strokes: Vec<String> = (keys.split_whitespace())
        .map(|stroke| match gpui::Keystroke::parse(stroke) {
            Ok(keystroke) => key_text(&keystroke.modifiers, &keystroke.key),
            Err(_) => stroke.to_owned(),
        })
        .collect();
    strokes.join(" ").into()
}

fn key_text(m: &gpui::Modifiers, key: &str) -> String {
    let mut text = String::new();
    for (on, symbol) in [
        (m.control, "⌃"),
        (m.alt, "⌥"),
        (m.shift, "⇧"),
        (m.platform, "⌘"),
    ] {
        if on {
            text.push_str(symbol);
        }
    }
    text.push_str(match key {
        "enter" => "↩",
        "backspace" => "⌫",
        "delete" => "⌦",
        "escape" => "esc",
        "space" => "space",
        "tab" => "⇥",
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        key if key.len() == 1
            || key
                .strip_prefix('f')
                .is_some_and(|n| n.parse::<u8>().is_ok()) =>
        {
            return text + &key.to_uppercase();
        }
        key => key,
    });
    text
}

/// What Windows calls the keys macOS writes as symbols. The app's ⌘ keys are ctrl there, and a
/// modifier's name ends in `+`, as it's written before the key.
#[cfg(not(target_os = "macos"))]
const KEY_NAMES: [(char, &str); 8] = [
    ('⌘', "Ctrl+"),
    ('⌃', "Ctrl+"),
    ('⌥', "Alt+"),
    ('⇧', "Shift+"),
    ('↩', "Enter"),
    ('⌫', "Backspace"),
    ('⌦', "Delete"),
    ('⇥', "Tab"),
];

/// `text` with the keys in it, written as macOS writes them (`⌘⇧P`), as the platform writes
/// them: unchanged on macOS, `Ctrl+Shift+P` on Windows.
pub fn keys_text(text: &str) -> String {
    #[cfg(target_os = "macos")]
    return text.to_owned();
    #[cfg(not(target_os = "macos"))]
    text.chars()
        .map(
            |c| match KEY_NAMES.iter().find(|(symbol, _)| *symbol == c) {
                Some((_, name)) => (*name).to_owned(),
                None => c.to_string(),
            },
        )
        .collect()
}

/// Keys as but-ui-react's `Kbd` draws them: a small box per key, like `⌘` `⇧` `P`, or `Ctrl` `Shift`
/// `P` on Windows. `text` writes them as macOS does.
pub fn keycap(text: &str, theme: &Theme) -> Div {
    keycaps(text, 16., 11., theme.text_2, theme)
}

/// `keycap` for a table of keys, where they're what's being read.
pub fn keycap_large(text: &str, theme: &Theme) -> Div {
    keycaps(text, 20., 13., theme.text_1, theme)
}

fn keycaps(text: &str, height: f32, size: f32, color: Hsla, theme: &Theme) -> Div {
    let mut keys: Vec<String> = Vec::new();
    for token in text.split(' ') {
        let mut rest = token;
        while let Some(symbol) = rest.chars().next().filter(|c| "⌃⌥⇧⌘".contains(*c)) {
            keys.push(
                keys_text(&symbol.to_string())
                    .trim_end_matches('+')
                    .to_owned(),
            );
            rest = rest.strip_prefix(symbol).unwrap_or_default();
        }
        if !rest.is_empty() {
            keys.push(keys_text(rest));
        }
    }
    h_flex()
        .flex_none()
        .gap(dp(3.))
        .children(keys.into_iter().map(|key| {
            div()
                .flex_none()
                .min_w(dp(height))
                .h(dp(height))
                .px(dp(3.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(dp(theme.shape.xs))
                .bg(theme.hover)
                .border_1()
                .border_color(theme.border_3)
                .text_size(dp(size))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(color)
                .child(key)
        }))
}
