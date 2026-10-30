//! Colour and type tokens, taken from `@gitbutler/design-core` so this app reads like
//! the rest of GitButler, or from a Ghostty colour scheme, which the embedded terminals then
//! draw with too.

use but_tty::{Colors, Ghostty, RgbColor};
use gpui::{
    App, Font, FontFeatures, Global, Hsla, Pixels, Rgba, SharedString, WindowAppearance, rgb,
};
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
    /// A Pop button's ground, darker than `pop_bg` so its white label reads at 4.5:1.
    pub pop_button: Hsla,
    /// Labels, ticks and dashes on `pop_button` and `pop_bg`.
    pub on_pop: Hsla,
    pub blue: Hsla,
    pub safe: Hsla,
    pub warn: Hsla,
    pub danger: Hsla,
    pub purple: Hsla,
    /// The dark ground of a snackbar, in both themes.
    pub snackbar_bg: Hsla,
    pub snackbar_text: Hsla,
    /// A diff's changed lines as @pierre/diffs colours them in but.dev: the line numbers and
    /// the bar beside them, the ground of the line and its numbers, and behind the words that
    /// changed within it.
    pub diff_added: Hsla,
    pub diff_removed: Hsla,
    pub diff_added_bg: Hsla,
    pub diff_removed_bg: Hsla,
    pub diff_added_emph: Hsla,
    pub diff_removed_emph: Hsla,
    /// The text of the box that stands for the unchanged lines between hunks, which an
    /// unchanged line's number shares.
    pub diff_muted: Hsla,
    /// The letter for an added, modified, deleted and renamed file: design-core's
    /// change-status colours.
    pub change_status: [Hsla; 4],
    /// The colours a person without a picture can be, as but.dev draws them: a ground and the
    /// pattern on it.
    pub person: [[Hsla; 2]; 5],
    pub ui_font: SharedString,
    pub mono_font: SharedString,
    /// What code in diffs shows in: `mono_font` unless another was picked.
    pub diff_font: SharedString,
    /// How wide a digit of `diff_font` is, in ems.
    pub diff_digit: f32,
    /// What terminals show in: Geist Mono with Nerd Font icons and box drawing, for TUIs, unless
    /// another was picked or Ghostty has one.
    pub terminal_font: Font,
    /// Code in diffs: its size and the height of a line of it.
    pub code_text: f32,
    pub code_line: f32,
    /// The interface scale, 1 at 100%: the window's rem follows it, and [`Theme::scaled`]
    /// gives sizes the layout doesn't, as for scroll and pointer maths.
    pub scale: f32,
    /// What the embedded terminals draw with: the scheme's palette, or for GitButler's, its
    /// own grounds, text and accents. A part unset takes `bg_1` or `text_1`, or the
    /// terminal's own default.
    pub terminal: Colors,
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

/// Which colours the app draws in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorScheme {
    /// GitButler's own, in light or dark as `Appearance` says.
    #[default]
    GitButler,
    /// The light and dark themes the user's Ghostty config sets, picked by `Appearance`.
    Ghostty,
    /// One Ghostty theme, by name, which is light or dark by its background.
    Named(String),
}

impl ColorScheme {
    /// What the picker shows it as.
    pub fn label(&self) -> &str {
        match self {
            ColorScheme::GitButler => "GitButler",
            ColorScheme::Ghostty => "Ghostty",
            ColorScheme::Named(name) => name,
        }
    }
}

/// How large the whole interface draws, in percent, as a browser zooms a page: every size
/// the app gives in [`crate::ui::dp`] grows or shrinks with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UiScale(u16);

impl Default for UiScale {
    fn default() -> Self {
        Self(100)
    }
}

impl UiScale {
    /// The steps ⌘+ and ⌘− go through, as Chrome's.
    pub const STEPS: [UiScale; 9] = [
        Self(75),
        Self(80),
        Self(90),
        Self(100),
        Self(110),
        Self(125),
        Self(150),
        Self(175),
        Self(200),
    ];

    pub fn percent(self) -> u16 {
        self.0
    }

    /// How many times 100% it is, kept within the steps.
    fn factor(self) -> f32 {
        f32::from(self.clamp(Self::STEPS[0], Self::STEPS[8]).0) / 100.
    }

    /// The next step up, or the largest.
    pub fn larger(self) -> Self {
        (Self::STEPS.into_iter())
            .find(|&step| step > self)
            .unwrap_or(Self::STEPS[8])
    }

    /// The next step down, or the smallest.
    pub fn smaller(self) -> Self {
        (Self::STEPS.into_iter().rev())
            .find(|&step| step < self)
            .unwrap_or(Self::STEPS[0])
    }
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

    /// This theme with code in diffs in `family`, unless `digit`, which gives how wide a family's
    /// digits are in ems, finds the machine hasn't got it.
    pub fn with_diff_font(
        mut self,
        family: Option<&str>,
        digit: impl Fn(&str) -> Option<f32>,
    ) -> Self {
        if let Some((family, width)) = family.and_then(|family| Some((family, digit(family)?))) {
            self.diff_font = family.to_owned().into();
            self.diff_digit = width;
        }
        self
    }

    /// This theme with terminals in `family`, or with none picked, in the first family of
    /// `ghostty`'s that `installed` says the machine has, with `ghostty`'s font features.
    pub fn with_terminal_font(
        mut self,
        family: Option<&str>,
        ghostty: &Ghostty,
        installed: impl Fn(&str) -> bool,
    ) -> Self {
        let configured = (ghostty.font_families.iter()).map(String::as_str);
        if let Some(found) = family.into_iter().chain(configured).find(|f| installed(f)) {
            self.terminal_font = gpui::font(found.to_owned());
        }
        if family.is_none() {
            self.terminal_font.features = FontFeatures(ghostty.font_features.clone().into());
        }
        self
    }

    /// This theme at interface scale `scale`.
    pub fn with_scale(mut self, scale: UiScale) -> Self {
        self.scale = scale.factor();
        self
    }

    /// `n` pixels as designed, at the interface scale: what [`crate::ui::dp`] lays out as.
    pub fn scaled(&self, n: f32) -> Pixels {
        Pixels::from(n * self.scale)
    }

    pub fn get(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }

    /// The theme `scheme` draws in, light or dark as `choice` says or, when the choice is to
    /// follow it, as `system` is, with code in diffs at `code`. `ghostty` is the user's
    /// Ghostty config, which colours the app and its terminals for [`ColorScheme::Ghostty`].
    /// The terminals always draw in the scheme the app does. A scheme without a background and foreground, as a named theme that's gone, draws as
    /// GitButler's.
    pub fn new(
        choice: Appearance,
        scheme: &ColorScheme,
        system: WindowAppearance,
        code: CodeSize,
        ghostty: &Ghostty,
    ) -> Self {
        // `GITBUTLER_GPUI_APPEARANCE=dark|light` and `GITBUTLER_GPUI_COLOR_SCHEME=gitbutler|
        // ghostty|<theme name>` override the settings, handy for screenshots.
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
        let forced = std::env::var("GITBUTLER_GPUI_COLOR_SCHEME").ok();
        let forced = forced.map(|name| match name.as_str() {
            "gitbutler" => ColorScheme::GitButler,
            "ghostty" => ColorScheme::Ghostty,
            _ => ColorScheme::Named(name),
        });
        let palette = match forced.as_ref().unwrap_or(scheme) {
            ColorScheme::GitButler => None,
            ColorScheme::Ghostty => Some(ghostty.colors(dark).clone()),
            ColorScheme::Named(name) => Ghostty::theme(name),
        };
        let gitbutler = Self::gitbutler(dark, code);
        match palette {
            Some(palette) => gitbutler.painted(palette),
            None => gitbutler,
        }
    }

    /// GitButler's own colours, light or dark, the terminals' too.
    fn gitbutler(dark: bool, code: CodeSize) -> Self {
        let pick = |light: u32, dark_value: u32| -> Hsla {
            rgb(if dark { dark_value } else { light }).into()
        };
        let (code_text, code_line) = code.metrics();
        // GitHub's light and dark, which but.dev's diffs take their colours from.
        let added = pick(0x28a745, 0x34d058);
        let removed = pick(0xd73a49, 0xea4a5a);
        Theme {
            code_text,
            code_line,
            scale: 1.,
            bg_1: pick(0xffffff, 0x292929),
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
            // Design-core's pop 40 and pop 30, darker than its fill-pop. A white tick on pop 40
            // reads at 3:1, and a white label needs pop 30 for 4.5:1.
            pop_bg: rgb(0x1e8e8e).into(),
            pop_button: rgb(0x177070).into(),
            on_pop: gpui::white(),
            blue: pick(0x0b61eb, 0x84aff5),
            // In light, design-core's safe 30 and danger 40, a step darker than its text-safe
            // and text-danger, which read under 4.5:1 on `bg_2`.
            safe: pick(0x1d744e, 0x2eb87f),
            warn: pick(0xa05f04, 0xf99406),
            danger: pick(0xac3421, 0xe28576),
            purple: pick(0x6e2cb1, 0xb287de),
            snackbar_bg: pick(0x1e1e1e, 0x434343),
            snackbar_text: rgb(0xf8f8f8).into(),
            diff_added: added,
            diff_removed: removed,
            diff_added_bg: pick(0xeef7ee, 0x2c3f35),
            diff_removed_bg: pick(0xffeeed, 0x413134),
            diff_added_emph: added.opacity(if dark { 0.2 } else { 0.15 }),
            diff_removed_emph: removed.opacity(if dark { 0.2 } else { 0.15 }),
            diff_muted: pick(0x696c70, 0x999da1),
            change_status: [
                pick(0x6fdaad, 0x1d744e),
                pick(0x84aff5, 0x0b61eb),
                pick(0xe28576, 0xac3421),
                pick(0xb287de, 0x8943d0),
            ],
            // Design-core's pop, purple, safe, warn and danger: the 80 step under the 70.
            person: [
                [(0xd0f2f2, 0x115151), (0x9ee7e7, 0x177070)],
                [(0xe7dbf3, 0x3e1964), (0xcfb5e9, 0x57238b)],
                [(0xd4f1e5, 0x15543a), (0xa6e5cb, 0x1d744e)],
                [(0xfae9d0, 0x724403), (0xf8d29e, 0x9d5d04)],
                [(0xf4dad6, 0x621d13), (0xebb4ab, 0x87291a)],
            ]
            .map(|steps| steps.map(|(light, dark_value)| pick(light, dark_value))),
            // Embedded by `ui::fonts`.
            ui_font: "Inter".into(),
            mono_font: "Geist Mono".into(),
            diff_font: "Geist Mono".into(),
            diff_digit: 0.6,
            terminal_font: gpui::font("GeistMono Nerd Font Mono"),
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
            terminal: Colors::default(),
        }
        .with_own_terminal(dark)
    }

    /// This theme with terminals in its own colours: its ground and text, and its accents for
    /// the ANSI colours, the bright ones a step toward the text. Black and white are its
    /// greys, so both read on the ground, as programs write bright white as strong text.
    fn with_own_terminal(mut self, dark: bool) -> Self {
        let [bg, fg] = [self.bg_1, self.text_1].map(rgb_hex);
        let black = rgb_hex(if dark { self.border_3 } else { self.text_1 });
        let accents = [
            self.danger,
            self.safe,
            self.warn,
            self.blue,
            self.purple,
            self.pop,
        ]
        .map(rgb_hex);
        let normal = (std::iter::once(black))
            .chain(accents)
            .chain([rgb_hex(self.text_2)]);
        let bright = (std::iter::once(rgb_hex(self.text_3)))
            .chain(accents.map(|accent| mix(accent, fg, 0.25)))
            .chain([fg]);
        self.terminal = Colors {
            background: Some(rgb_color(bg)),
            foreground: Some(rgb_color(fg)),
            cursor: Some(rgb_color(fg)),
            selection_background: Some(rgb_color(mix(bg, rgb_hex(self.blue), 0.3))),
            selection_foreground: None,
            palette: (0..).zip(normal.chain(bright).map(rgb_color)).collect(),
        };
        self
    }

    /// This theme in `palette`'s colours, its sizes and fonts kept, or as it is when the
    /// palette has no background or foreground. Text steps toward the foreground until it reads
    /// on both grounds: secondary text and colours used as text at 4.5:1, faint text and
    /// marks at 3:1.
    fn painted(self, palette: Colors) -> Self {
        let (Some(bg), Some(fg)) = (palette.background, palette.foreground) else {
            return self;
        };
        let (bg, fg) = (hex(bg), hex(fg));
        let dark = contrast(bg, 0xffffff) > contrast(bg, 0x000000);
        let ansi = |index: u8| {
            (palette.palette.iter())
                .find(|(at, _)| *at == index)
                .map(|(_, color)| hex(*color))
                .or_else(|| GHOSTTY_ANSI.get(usize::from(index)).copied())
                .unwrap_or(fg)
        };
        let bg_2 = mix(bg, 0x000000, if dark { 0.25 } else { 0.05 });
        let grounds = [bg, bg_2];
        let ink = if dark { 0xffffff } else { 0x000000 };
        let text = |from: u32, ratio: f32| {
            (reaching(from, fg, &grounds, ratio))
                .or_else(|| reaching(fg, ink, &grounds, ratio))
                .unwrap_or(ink)
        };
        let pop = ansi(6);
        let on_pop = [0xffffff, bg, 0x000000]
            .into_iter()
            .find(|&ink| contrast(ink, pop) >= 4.5)
            .unwrap_or(0x000000);
        let text_3 = text(mix(bg, fg, 0.45), 3.);
        let [safe, blue, danger, purple] = [2, 4, 1, 5].map(|index| text(ansi(index), 4.5));
        let [added, removed] = [2, 1].map(|index| text(ansi(index), 3.));
        let ground = |index: u8| mix(bg, ansi(index), if dark { 0.18 } else { 0.12 });
        Theme {
            bg_1: color(bg),
            bg_2: color(bg_2),
            text_1: color(fg),
            text_2: color(text(mix(bg, fg, 0.65), 4.5)),
            text_3: color(text_3),
            border_1: color(mix(bg, fg, 0.45)),
            border_2: color(mix(bg, fg, 0.3)),
            border_3: color(mix(bg, fg, 0.18)),
            fill_gray: color(fg),
            text_inverted: color(bg),
            pop: color(text(pop, 4.5)),
            pop_bg: color(pop),
            pop_button: color(pop),
            on_pop: color(on_pop),
            blue: color(blue),
            safe: color(safe),
            warn: color(text(ansi(3), 4.5)),
            danger: color(danger),
            purple: color(purple),
            snackbar_bg: color(fg),
            snackbar_text: color(bg),
            diff_added: color(added),
            diff_removed: color(removed),
            diff_added_bg: color(ground(2)),
            diff_removed_bg: color(ground(1)),
            diff_added_emph: color(added).opacity(if dark { 0.2 } else { 0.15 }),
            diff_removed_emph: color(removed).opacity(if dark { 0.2 } else { 0.15 }),
            diff_muted: color(text_3),
            change_status: [safe, blue, danger, purple].map(color),
            person: [6, 5, 2, 3, 1]
                .map(|index| [mix(bg, ansi(index), 0.25), mix(bg, ansi(index), 0.55)].map(color)),
            syntax: [
                text_3,
                text(ansi(2), 4.5),
                text(ansi(6), 4.5),
                text(ansi(5), 4.5),
                text(ansi(4), 4.5),
                text(ansi(3), 4.5),
                text(ansi(1), 4.5),
            ]
            .map(color),
            terminal: palette,
            ..self
        }
    }

    pub fn token(&self, token: Token) -> Hsla {
        let [comment, string, constant, keyword, function, ty, tag] = self.syntax;
        match token {
            Token::Comment => comment,
            Token::String => string,
            Token::Constant => constant,
            Token::Keyword => keyword,
            Token::Function => function,
            Token::Type => ty,
            Token::Tag => tag,
        }
    }

    pub fn hover(&self) -> Hsla {
        self.fill_gray.opacity(0.05)
    }

    /// The ring of a focused field, as design-core's `--border-focus`.
    pub fn focus_border(&self) -> Hsla {
        self.fill_gray.opacity(0.7)
    }

    /// The border of a control, as design-core's `--border-control`.
    pub fn control_border(&self) -> Hsla {
        self.fill_gray.opacity(0.26)
    }

    /// Selection ground while the list does not have focus.
    pub fn selected_unfocused(&self) -> Hsla {
        self.fill_gray.opacity(0.1)
    }

    /// Secondary text and icons on a solid selection, as `text_2` is on open ground.
    pub fn muted_on_solid(&self) -> Hsla {
        self.text_inverted.opacity(0.7)
    }
}

/// Ghostty's own 16 colours, for the ones a theme leaves unset.
const GHOSTTY_ANSI: [u32; 16] = [
    0x1d1f21, 0xcc6666, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xc5c8c6, 0x666666,
    0xd54e53, 0xb9ca4a, 0xe7c547, 0x7aa6da, 0xc397d8, 0x70c0b1, 0xeaeaea,
];

fn hex(color: RgbColor) -> u32 {
    u32::from(color.r) << 16 | u32::from(color.g) << 8 | u32::from(color.b)
}

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn channels(hex: u32) -> [f32; 3] {
    [16, 8, 0].map(|shift| ((hex >> shift) & 0xff) as f32)
}

/// `a` taken `t` of the way to `b`.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let [a, b] = [channels(a), channels(b)];
    (a.iter().zip(b))
        .map(|(a, b)| (a + (b - a) * t).round() as u32)
        .fold(0, |hex, channel| hex << 8 | channel)
}

/// WCAG's relative luminance.
fn luminance(hex: u32) -> f32 {
    let [r, g, b] = channels(hex).map(|channel| {
        let c = channel / 255.;
        match c <= 0.04045 {
            true => c / 12.92,
            false => ((c + 0.055) / 1.055).powf(2.4),
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG's contrast ratio, from 1 to 21.
fn contrast(a: u32, b: u32) -> f32 {
    let [a, b] = [luminance(a), luminance(b)];
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `from`, or the first step from it toward `toward` that reads at `ratio` on every one of
/// `grounds`, if one does.
fn reaching(from: u32, toward: u32, grounds: &[u32], ratio: f32) -> Option<u32> {
    (0..=20)
        .map(|step| mix(from, toward, step as f32 / 20.))
        .find(|&color| {
            grounds
                .iter()
                .all(|&ground| contrast(color, ground) >= ratio)
        })
}

fn rgb_color(hex: u32) -> RgbColor {
    let [_, r, g, b] = hex.to_be_bytes();
    RgbColor { r, g, b }
}

/// A colour as the `#rrggbb` it was made from.
fn rgb_hex(color: Hsla) -> u32 {
    let Rgba { r, g, b, .. } = color.into();
    [r, g, b]
        .map(|channel| (channel * 255.).round() as u32)
        .into_iter()
        .fold(0, |hex, channel| hex << 8 | channel)
}

#[cfg(test)]
mod tests {
    use but_tty::{Colors, Ghostty, RgbColor};
    use gpui::WindowAppearance;

    use super::{Appearance, CodeSize, ColorScheme, Theme, contrast, rgb_hex};

    fn rgb(hex: u32) -> RgbColor {
        let [_, r, g, b] = hex.to_be_bytes();
        RgbColor { r, g, b }
    }

    fn palette(background: u32, foreground: u32, ansi: [u32; 8]) -> Colors {
        Colors {
            background: Some(rgb(background)),
            foreground: Some(rgb(foreground)),
            palette: (0..).zip(ansi.map(rgb)).collect(),
            ..Colors::default()
        }
    }

    /// The theme the Ghostty scheme gives when Ghostty's config colours both appearances as
    /// `colors`, asked for in `choice`.
    fn ghostty_theme(colors: Colors, choice: Appearance) -> Theme {
        let ghostty = Ghostty {
            light: colors.clone(),
            dark: colors,
            ..Ghostty::default()
        };
        let system = WindowAppearance::Light;
        Theme::new(
            choice,
            &ColorScheme::Ghostty,
            system,
            CodeSize::Medium,
            &ghostty,
        )
    }

    #[test]
    fn derived_text_reads_on_both_grounds() {
        let schemes = [
            (
                "dark",
                palette(
                    0x1e1e2e,
                    0xcdd6f4,
                    [
                        0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5,
                        0xbac2de,
                    ],
                ),
            ),
            (
                "light",
                palette(
                    0xfdf6e3,
                    0x657b83,
                    [
                        0x073642, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x2aa198,
                        0xeee8d5,
                    ],
                ),
            ),
            ("murky", palette(0x404040, 0x707070, [0x505050; 8])),
        ];
        for (name, colors) in schemes {
            let theme = ghostty_theme(colors, Appearance::Dark);
            for ground in [theme.bg_1, theme.bg_2].map(rgb_hex) {
                for (what, ink, ratio) in [
                    ("secondary text", theme.text_2, 4.5),
                    ("faint text", theme.text_3, 3.),
                    ("danger", theme.danger, 4.5),
                    ("pop", theme.pop, 4.5),
                ] {
                    let reads = contrast(rgb_hex(ink), ground);
                    assert!(
                        reads >= ratio,
                        "{name}: {what} reads at {reads}, under {ratio}"
                    );
                }
            }
            let label = contrast(rgb_hex(theme.on_pop), rgb_hex(theme.pop_button));
            assert!(
                label >= 4.5,
                "{name}: a pop button's label reads at {label}"
            );
        }
    }

    #[test]
    fn a_scheme_colours_the_app_and_the_terminals_whatever_the_appearance() {
        let solarized = palette(0xfdf6e3, 0x657b83, [0x073642; 8]);
        let theme = ghostty_theme(solarized.clone(), Appearance::Dark);
        assert_eq!(
            [theme.bg_1, theme.text_1].map(rgb_hex),
            [0xfdf6e3, 0x657b83],
            "a light scheme draws light though the appearance asks for dark"
        );
        assert_eq!(
            theme.terminal, solarized,
            "the terminals draw in the scheme too"
        );
    }

    #[test]
    fn gitbutlers_terminals_draw_in_its_colours_whatever_ghostty_sets() {
        let ghostty = Ghostty {
            light: palette(0x000000, 0xffffff, [0x123456; 8]),
            dark: palette(0x000000, 0xffffff, [0x123456; 8]),
            ..Ghostty::default()
        };
        let system = WindowAppearance::Light;
        for choice in [Appearance::Light, Appearance::Dark] {
            let theme = Theme::new(
                choice,
                &ColorScheme::GitButler,
                system,
                CodeSize::Medium,
                &ghostty,
            );
            let terminal = &theme.terminal;
            assert_eq!(
                [terminal.background, terminal.foreground].map(|c| c.map(super::hex)),
                [theme.bg_1, theme.text_1].map(|c| Some(rgb_hex(c))),
                "the terminal's ground and text are the app's"
            );
            let red = terminal.palette.iter().find(|(index, _)| *index == 1);
            assert_eq!(
                red.map(|(_, color)| super::hex(*color)),
                Some(rgb_hex(theme.danger)),
                "red is the app's danger, not Ghostty's"
            );
            assert_eq!(terminal.palette.len(), 16, "every ANSI colour is set");
        }
    }

    fn gitbutler_theme() -> Theme {
        Theme::new(
            Appearance::Dark,
            &ColorScheme::GitButler,
            WindowAppearance::Light,
            CodeSize::Medium,
            &Ghostty::default(),
        )
    }

    #[test]
    fn diffs_show_a_picked_font_only_when_the_machine_has_it() {
        let digit = |family: &str| (family == "Menlo").then_some(0.5);
        let picked = gitbutler_theme().with_diff_font(Some("Menlo"), digit);
        assert_eq!(
            (picked.diff_font.as_ref(), picked.diff_digit),
            ("Menlo", 0.5),
            "the picked font draws, with its own digit width"
        );
        let gone = gitbutler_theme().with_diff_font(Some("Gone"), digit);
        assert_eq!(
            (gone.diff_font, gone.diff_digit),
            (gone.mono_font, 0.6),
            "a font that's gone keeps the app's own, and its width"
        );
    }

    #[test]
    fn terminals_follow_ghostty_until_a_font_is_picked() {
        let ghostty = Ghostty {
            font_families: vec!["Gone".into(), "Menlo".into()],
            font_features: vec![("calt".into(), 0)],
            ..Ghostty::default()
        };
        let installed = |family: &str| family != "Gone";

        let followed = gitbutler_theme().with_terminal_font(None, &ghostty, installed);
        assert_eq!(
            (
                followed.terminal_font.family.as_ref(),
                followed.terminal_font.features.tag_value_list()
            ),
            ("Menlo", &[("calt".to_owned(), 0)][..]),
            "the first family the machine has, with Ghostty's features"
        );

        let picked = gitbutler_theme().with_terminal_font(Some("Monaco"), &ghostty, installed);
        assert_eq!(
            (
                picked.terminal_font.family.as_ref(),
                picked.terminal_font.features.tag_value_list()
            ),
            ("Monaco", &[][..]),
            "a picked font wins, and Ghostty's features were written for Ghostty's font"
        );

        let nothing = Ghostty {
            font_families: vec!["Gone".into()],
            ..Ghostty::default()
        };
        let fallen_back = gitbutler_theme().with_terminal_font(None, &nothing, installed);
        assert_eq!(
            fallen_back.terminal_font.family,
            gitbutler_theme().terminal_font.family,
            "with none of Ghostty's fonts here, the app's own"
        );
    }

    #[test]
    fn a_scheme_without_colours_draws_as_gitbutler() {
        let ghostty = Ghostty::default();
        let system = WindowAppearance::Light;
        let gitbutler = Theme::new(
            Appearance::Dark,
            &ColorScheme::GitButler,
            system,
            CodeSize::Medium,
            &ghostty,
        );
        let missing = ColorScheme::Named("No theme is called this".into());
        let fallen_back = Theme::new(
            Appearance::Dark,
            &missing,
            system,
            CodeSize::Medium,
            &ghostty,
        );
        let unset = ghostty_theme(Colors::default(), Appearance::Dark);
        for theme in [fallen_back, unset] {
            assert_eq!(
                [theme.bg_1, theme.text_1, theme.pop].map(rgb_hex),
                [gitbutler.bg_1, gitbutler.text_1, gitbutler.pop].map(rgb_hex),
                "with nothing to draw in, GitButler's own colours stay"
            );
        }
    }
}
