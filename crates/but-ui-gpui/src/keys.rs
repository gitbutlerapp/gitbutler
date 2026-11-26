//! Keys, written as the platform writes them and drawn as but-ui-react's `Kbd`.

use gpui::{App, Div, Font, Hsla, SharedString, TextRun, Window, div, prelude::*};

use crate::theme::Theme;
use crate::*;

/// The keys bound to `action` as macOS writes them (`⌘⇧P`) on every platform: [`keycap`] and
/// [`keys_text`] show them as the platform does.
pub fn shortcut(action: &dyn gpui::Action, cx: &App) -> Option<SharedString> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    Some(keys_of(keymap.bindings_for_action(action).next_back()?))
}

/// Every binding of `action`, with the context it works in.
pub fn bindings_of(action: &dyn gpui::Action, cx: &App) -> Vec<(SharedString, Option<String>)> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    (keymap.bindings_for_action(action))
        .map(|binding| (keys_of(binding), binding.predicate().map(|p| p.to_string())))
        .collect()
}

/// The keys that run `action` where focus is now, unless they do something else there. Keys
/// bound to several actions run the first one handled: r is Reword's on a commit and Rename
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

/// `keys` in keymap syntax (`cmd-shift-p`), as [`shortcut`] writes keys.
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

/// Off macOS, the app's ⌘ keys are ctrl.
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

/// `text`'s macOS key symbols (`⌘⇧P`) as the platform writes them: `Ctrl+Shift+P` on Windows.
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

/// A box per key, as but-ui-react's `Kbd`; `text` writes them as macOS does.
pub fn keycap(text: &str, theme: &Theme) -> Div {
    keycaps(text, 16., 11., theme.muted_foreground, theme)
}

/// For a table of keys, where they're what's being read.
pub fn keycap_large(text: &str, theme: &Theme) -> Div {
    keycaps(text, 20., 13., theme.foreground, theme)
}

/// How wide [`keycap`] draws `text`, in design pixels: for a column that lines several up, as a
/// menu's keys do.
pub fn keycap_width(text: &str, window: &Window, theme: &Theme) -> f32 {
    let font = Font {
        weight: weight(FONT_WEIGHT_SEMIBOLD),
        ..gpui::font(theme.ui_font.clone())
    };
    let keys = key_caps(text);
    let caps: f32 = (keys.iter())
        .map(|key| {
            let run = TextRun {
                len: key.len(),
                font: font.clone(),
                color: theme.muted_foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = (window.text_system()).shape_line(
                SharedString::from(key.clone()),
                theme.scaled(11.),
                &[run],
                None,
            );
            // Its padding and its border, which keeps its pixel at every scale.
            (f32::from(line.width) / theme.scale + 6. + 2. / theme.scale).max(16.)
        })
        .sum();
    caps + 3. * keys.len().saturating_sub(1) as f32
}

/// The caps `text` is drawn as: each modifier on its own, then the rest of each chord.
fn key_caps(text: &str) -> Vec<String> {
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
    keys
}

fn keycaps(text: &str, height: f32, size: f32, color: Hsla, theme: &Theme) -> Div {
    let keys = key_caps(text);
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
                .border_color(theme.border_subtle)
                .text_off_scale(size)
                .semibold()
                .text_color(color)
                .child(key)
        }))
}
