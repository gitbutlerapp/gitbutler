//! Colour and type tokens, taken from `@gitbutler/design-core` so this app reads like
//! the rest of GitButler.

use gpui::{App, Global, Hsla, SharedString, WindowAppearance, rgb};
use serde::{Deserialize, Serialize};

use crate::backend::Token;

#[derive(Clone)]
pub struct Theme {
    /// Surfaces: content and recessed (navigator, diff ground).
    pub bg_1: Hsla,
    pub bg_2: Hsla,
    pub text_1: Hsla,
    pub text_2: Hsla,
    pub text_3: Hsla,
    /// The strongest rule, then stronger and softer ones.
    pub border_1: Hsla,
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
    /// A diff's changed lines as @pierre/diffs colours them in but.dev: from a base, for the
    /// line numbers and the bar beside them, the ground of the line and of its numbers, and
    /// behind the words that changed within it.
    pub diff_added: Hsla,
    pub diff_removed: Hsla,
    pub diff_added_bg: Hsla,
    pub diff_removed_bg: Hsla,
    pub diff_added_gutter: Hsla,
    pub diff_removed_gutter: Hsla,
    pub diff_added_emph: Hsla,
    pub diff_removed_emph: Hsla,
    /// The box that stands for the unchanged lines between hunks.
    pub diff_separator: Hsla,
    pub ui_font: SharedString,
    pub mono_font: SharedString,
    /// Code in diffs: its size and the height of a line of it.
    pub code_text: f32,
    pub code_line: f32,
    syntax: [Hsla; 7],
}

impl Global for Theme {}

/// How large code shows in diffs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// Light, dark, or whichever macOS is set to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl CodeSize {
    /// The size of code in diffs and the height of a line of it.
    fn metrics(self) -> (f32, f32) {
        match self {
            CodeSize::Small => (11., 18.),
            CodeSize::Medium => (12., 20.),
            CodeSize::Large => (14., 23.),
        }
    }
}

impl Theme {
    /// This theme with code in diffs at `size`.
    pub fn with_code(mut self, size: CodeSize) -> Self {
        (self.code_text, self.code_line) = size.metrics();
        self
    }

    pub fn get(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }

    /// The theme for `choice`, or for `system` when the choice is to follow it, with code
    /// in diffs at `code`.
    pub fn for_appearance(choice: Appearance, system: WindowAppearance, code: CodeSize) -> Self {
        // `GITBUTLER_GPUI_APPEARANCE=dark|light` overrides both, handy for screenshots.
        let dark = match (
            std::env::var("GITBUTLER_GPUI_APPEARANCE").as_deref(),
            choice,
        ) {
            (Ok("dark"), _) => true,
            (Ok("light"), _) => false,
            (_, Appearance::Dark) => true,
            (_, Appearance::Light) => false,
            (_, Appearance::System) => matches!(
                system,
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
        let (code_text, code_line) = code.metrics();
        let bg_1 = pick(0xffffff, 0x292929);
        let added = pick(0x0dbe4e, 0x5ecc71);
        let removed = pick(0xff2e3f, 0xff6762);
        // How much of the base each ground takes, in light and dark: pierre mixes in lab,
        // this in sRGB, which comes out close.
        let tint = |base: Hsla, light: f32, dark_amount: f32| {
            bg_1.blend(base.opacity(if dark { dark_amount } else { light }))
        };
        Theme {
            code_text,
            code_line,
            bg_1,
            bg_2: pick(0xf2f2f2, 0x1e1e1e),
            text_1: pick(0x1e1e1e, 0xf8f8f8),
            // Secondary text meets WCAG AA (4.5:1), faint text the 3:1 for de-emphasised
            // details, on both grounds.
            text_2: pick(0x6e6e6e, 0xb2b2b2),
            text_3: pick(0x8a8a8a, 0x7d7d7d),
            border_1: pick(0xb2b2b2, 0x6d6d6d),
            border_2: pick(0xcccccc, 0x565656),
            border_3: pick(0xdddddc, 0x434343),
            fill_gray: pick(0x434343, 0xf2f2f2),
            text_inverted: pick(0xf8f8f8, 0x1e1e1e),
            pop: pick(0x177a7a, 0x25b1b1),
            pop_bg: pick(0x25b1b1, 0x1e8e8e),
            blue: pick(0x0b61eb, 0x84aff5),
            safe,
            warn: pick(0xa05f04, 0xf99406),
            danger,
            purple: pick(0x6e2cb1, 0xb287de),
            snackbar_bg: pick(0x1e1e1e, 0x434343),
            snackbar_text: rgb(0xf8f8f8).into(),
            diff_added: added,
            diff_removed: removed,
            diff_added_bg: tint(added, 0.12, 0.2),
            diff_removed_bg: tint(removed, 0.12, 0.2),
            diff_added_gutter: tint(added, 0.09, 0.15),
            diff_removed_gutter: tint(removed, 0.09, 0.15),
            diff_added_emph: added.opacity(if dark { 0.2 } else { 0.15 }),
            diff_removed_emph: removed.opacity(if dark { 0.2 } else { 0.15 }),
            diff_separator: tint(pick(0x000000, 0xffffff), 0.04, 0.1),
            ui_font: ui_font.into(),
            mono_font: mono_font.into(),
            // Shiki's GitHub light and dark, as but.dev highlights diffs. In `Token` order:
            // comment, string, constant, keyword, function, type, tag.
            syntax: [
                pick(0x6a737d, 0x6a737d),
                pick(0x032f62, 0x9ecbff),
                pick(0x005cc5, 0x79b8ff),
                pick(0xd73a49, 0xf97583),
                pick(0x6f42c1, 0xb392f0),
                pick(0x6f42c1, 0xb392f0),
                pick(0x22863a, 0x85e89d),
            ],
        }
    }

    pub fn token(&self, token: Token) -> Hsla {
        self.syntax[token as usize]
    }

    pub fn hover(&self) -> Hsla {
        self.fill_gray.opacity(0.06)
    }

    /// Selection ground while the list does not have focus.
    pub fn selected_unfocused(&self) -> Hsla {
        self.fill_gray.opacity(0.11)
    }
}
