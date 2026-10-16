//! Colour and type tokens, taken from `@gitbutler/design-core` so this app reads like
//! the rest of GitButler.

use gpui::{App, Global, Hsla, SharedString, WindowAppearance, rgb};

use crate::backend::Token;

#[derive(Clone)]
pub struct Theme {
    /// Surfaces: content and recessed (navigator, diff ground).
    pub bg_1: Hsla,
    pub bg_2: Hsla,
    pub text_1: Hsla,
    pub text_2: Hsla,
    pub text_3: Hsla,
    /// Stronger and softer rules.
    pub border_2: Hsla,
    pub border_3: Hsla,
    /// The solid gray of a focused selection; hover and unfocused selection are tints of it.
    pub fill_gray: Hsla,
    pub text_inverted: Hsla,
    pub pop: Hsla,
    pub pop_bg: Hsla,
    pub blue: Hsla,
    pub safe: Hsla,
    pub warn: Hsla,
    pub danger: Hsla,
    pub purple: Hsla,
    /// The dark ground of a snackbar, in both themes.
    pub snackbar_bg: Hsla,
    pub snackbar_text: Hsla,
    pub diff_added_bg: Hsla,
    pub diff_removed_bg: Hsla,
    pub diff_added_gutter: Hsla,
    pub diff_removed_gutter: Hsla,
    /// Behind the words that changed within a line.
    pub diff_added_emph: Hsla,
    pub diff_removed_emph: Hsla,
    pub ui_font: SharedString,
    pub mono_font: SharedString,
    syntax: [Hsla; 7],
}

impl Global for Theme {}

impl Theme {
    pub fn get(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        // `GITBUTLER_GPUI_APPEARANCE=dark|light` overrides the system, handy for screenshots.
        let dark = match std::env::var("GITBUTLER_GPUI_APPEARANCE").as_deref() {
            Ok("dark") => true,
            Ok("light") => false,
            _ => matches!(
                appearance,
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        let pick = |light: u32, dark_value: u32| -> Hsla {
            rgb(if dark { dark_value } else { light }).into()
        };
        // Embedded by `ui::fonts`.
        let ui_font = "Inter";
        let mono_font = "Geist Mono";

        let safe = pick(0x259466, 0x2eb87f);
        let danger = pick(0xd64029, 0xe28576);
        Theme {
            bg_1: pick(0xffffff, 0x292929),
            bg_2: pick(0xf2f2f2, 0x1e1e1e),
            text_1: pick(0x1e1e1e, 0xf8f8f8),
            text_2: pick(0x888888, 0xb2b2b2),
            text_3: pick(0xb2b2b2, 0x6d6d6d),
            border_2: pick(0xcccccc, 0x565656),
            border_3: pick(0xdddddc, 0x434343),
            fill_gray: pick(0x434343, 0xf2f2f2),
            text_inverted: pick(0xf8f8f8, 0x1e1e1e),
            pop: pick(0x1e8e8e, 0x25b1b1),
            pop_bg: pick(0x25b1b1, 0x1e8e8e),
            blue: pick(0x0b61eb, 0x84aff5),
            safe,
            warn: pick(0xc87705, 0xf99406),
            danger,
            purple: pick(0x6e2cb1, 0xb287de),
            snackbar_bg: pick(0x1e1e1e, 0x434343),
            snackbar_text: rgb(0xf8f8f8).into(),
            diff_added_bg: safe.opacity(if dark { 0.13 } else { 0.10 }),
            diff_removed_bg: danger.opacity(if dark { 0.13 } else { 0.10 }),
            diff_added_gutter: safe.opacity(if dark { 0.22 } else { 0.20 }),
            diff_removed_gutter: danger.opacity(if dark { 0.22 } else { 0.20 }),
            diff_added_emph: safe.opacity(if dark { 0.34 } else { 0.30 }),
            diff_removed_emph: danger.opacity(if dark { 0.34 } else { 0.28 }),
            ui_font: ui_font.into(),
            mono_font: mono_font.into(),
            // In `Token` order: comment, string, constant, keyword, function, type, tag.
            syntax: [
                pick(0x6e7781, 0x8b949e),
                pick(0x0a3069, 0xa5d6ff),
                pick(0x0550ae, 0x79c0ff),
                pick(0xcf222e, 0xff7b72),
                pick(0x8250df, 0xd2a8ff),
                pick(0x953800, 0xffa657),
                pick(0x116329, 0x7ee787),
            ],
        }
    }

    pub fn token(&self, token: Token) -> Hsla {
        self.syntax[token as usize]
    }

    /// Row hover ground.
    pub fn hover(&self) -> Hsla {
        self.fill_gray.opacity(0.06)
    }

    /// Selection ground while the list does not have focus.
    pub fn selected_unfocused(&self) -> Hsla {
        self.fill_gray.opacity(0.11)
    }
}
