//! Colour and type tokens, taken from `@gitbutler/but-ui` so this app reads like
//! the rest of GitButler, or from a Ghostty colour scheme, which the embedded terminals then
//! draw with too.

use but_tty::{Colors, Ghostty, RgbColor};
use gpui::{
    App, Font, FontFeatures, FontWeight, Global, Hsla, Pixels, Rgba, SharedString,
    WindowAppearance, rgb, rgba,
};
use serde::{Deserialize, Serialize};

use crate::backend::Token;

/// but-ui's tokens as its build resolves them (`packages/but-ui/tokens/tokens.json`), which the
/// build script writes as constants: each colour `[light, dark]` as `0xRRGGBBAA`, and each length
/// in px, named as the token is.
#[expect(
    dead_code,
    reason = "every token is here, and the theme reads only the ones it draws with"
)]
pub(crate) mod tokens {
    include!(concat!(env!("OUT_DIR"), "/color_tokens.rs"));
}

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
    /// What the pointer is over, as but-ui's `--but-hover`.
    pub hover: Hsla,
    /// Selection ground while the list does not have focus, as `--but-selected-inactive`.
    pub selected_unfocused: Hsla,
    /// The ring of a focused field, as `--but-ring`.
    pub focus_border: Hsla,
    /// The border of a control, as `--but-input`.
    pub control_border: Hsla,
    /// A faint gray well, as code blocks and table heads sit in: `--but-secondary`.
    pub secondary: Hsla,
    /// A gray control's ground and that under the pointer, as but-ui-react's Gray button and
    /// inline code: `--but-muted`, and halfway to `--but-muted-nested`.
    pub muted: Hsla,
    pub muted_hover: Hsla,
    /// Secondary text and icons on a solid selection, as `text_2` is on open ground.
    pub muted_on_solid: Hsla,
    /// What the pointer is over on a solid selection, as `--but-inverse-ghost-hover`.
    pub hover_on_solid: Hsla,
    /// A faint ground, rule or track on a solid selection, as `secondary` and `border_3` are on
    /// open ground, and that under the pointer: `--but-inverse-soft(-hover)`.
    pub inverse_soft: Hsla,
    pub inverse_soft_hover: Hsla,
    /// A scrollbar's thumb, and while it's held or pointed at.
    pub scrollbar: Hsla,
    pub scrollbar_hover: Hsla,
    /// What dims the window behind a modal.
    pub overlay: Hsla,
    /// What a floating window casts.
    pub shadow: Hsla,
    pub pop: Hsla,
    pub pop_bg: Hsla,
    /// A Pop button's ground, darker than `pop_bg` so its white label reads at 4.5:1, and
    /// under the pointer.
    pub pop_button: Hsla,
    pub pop_hover: Hsla,
    /// Labels, ticks and dashes on `pop_button` and `pop_bg`.
    pub on_pop: Hsla,
    pub blue: Hsla,
    pub safe: Hsla,
    pub warn: Hsla,
    pub danger: Hsla,
    pub purple: Hsla,
    /// Each hue's soft ground, which its own colour reads on as text, and that under the
    /// pointer, as but-ui-react's Badge and soft buttons draw them: `--but-*-soft(-hover)`.
    /// See-through, for a mark or a control sitting on a ground. [`Theme::soft`] picks one by
    /// [`Hue`].
    pub pop_soft: Hsla,
    pub pop_soft_hover: Hsla,
    pub blue_soft: Hsla,
    pub blue_soft_hover: Hsla,
    pub safe_soft: Hsla,
    pub safe_soft_hover: Hsla,
    pub warn_soft: Hsla,
    pub warn_soft_hover: Hsla,
    pub danger_soft: Hsla,
    pub danger_soft_hover: Hsla,
    pub purple_soft: Hsla,
    pub purple_soft_hover: Hsla,
    /// Each hue's soft ground as an area, opaque, so a banner, a callout or a tile stays clean
    /// on any ground, and that under the pointer: `--but-*-surface(-hover)`.
    /// [`Theme::surface`] picks one by [`Hue`].
    pub pop_surface: Hsla,
    pub pop_surface_hover: Hsla,
    pub blue_surface: Hsla,
    pub blue_surface_hover: Hsla,
    pub safe_surface: Hsla,
    pub safe_surface_hover: Hsla,
    pub warn_surface: Hsla,
    pub warn_surface_hover: Hsla,
    pub danger_surface: Hsla,
    pub danger_surface_hover: Hsla,
    pub purple_surface: Hsla,
    pub purple_surface_hover: Hsla,
    /// Each hue laid over what must still show through it, and stronger for what's matched,
    /// selected or being written now: `--but-*-wash(-strong)`. [`Theme::wash`] picks one by
    /// [`Hue`], [`Theme::highlight`] by what it marks.
    pub pop_wash: Hsla,
    pub pop_wash_strong: Hsla,
    pub blue_wash: Hsla,
    pub blue_wash_strong: Hsla,
    pub safe_wash: Hsla,
    pub safe_wash_strong: Hsla,
    pub warn_wash: Hsla,
    pub warn_wash_strong: Hsla,
    pub danger_wash: Hsla,
    pub danger_wash_strong: Hsla,
    pub purple_wash: Hsla,
    pub purple_wash_strong: Hsla,
    /// Warning as a solid fill, and the text on it: the search match the keys are on.
    pub warn_bg: Hsla,
    pub on_warn: Hsla,
    /// The edge of something marked as a warning or as not sent yet: amber's and blue's
    /// border step.
    pub warn_edge: Hsla,
    pub blue_edge: Hsla,
    /// `safe` and `danger` on a solid selection: each stepped toward `text_inverted` until it
    /// reads on `fill_gray`, so it keeps what hue it can.
    pub safe_on_solid: Hsla,
    pub danger_on_solid: Hsla,
    /// `purple` on a solid selection: the other appearance's, as but.dev's is. For glyphs, which
    /// need 3:1.
    pub purple_on_solid: Hsla,
    /// A snackbar's ground, edge and text, as but-ui's snackbar tokens have them: inverted in
    /// light, so it reads as a passing note over the window, and in dark a step above a popover's
    /// ground, with a popover's edge.
    pub snackbar_bg: Hsla,
    pub snackbar_border: Hsla,
    pub snackbar_text: Hsla,
    /// A warning snackbar's glyph, which says it went but not quite as asked. For a glyph, so
    /// 3:1 on `snackbar_bg`.
    pub snackbar_warn: Hsla,
    /// The line between a snackbar's sentence and its button, and that button under the
    /// pointer.
    pub snackbar_divider: Hsla,
    pub snackbar_hover: Hsla,
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
    /// The letter for an added, modified, deleted and renamed file: but-ui's
    /// change-status colours.
    pub change_status: [Hsla; 4],
    /// The colours a person without a picture can be, as but.dev draws them, under their
    /// initial in `text_1`.
    pub person: [Hsla; 5],
    pub ui_font: SharedString,
    pub mono_font: SharedString,
    /// What code in diffs shows in: `mono_font` unless another was picked.
    pub diff_font: SharedString,
    /// How wide a digit of `diff_font` is, in ems.
    pub diff_digit: f32,
    /// What turns `diff_font`'s ligatures off, if they are.
    pub diff_features: FontFeatures,
    /// What terminals show in: Geist Mono with Nerd Font icons and box drawing, for TUIs, unless
    /// another was picked or Ghostty has one.
    pub terminal_font: Font,
    pub terminal_bold: FontWeight,
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
    /// How round corners are, by what they're on.
    pub shape: Shape,
    /// How tall controls and bars are, and how far their side padding moves, by density.
    pub sizes: Sizes,
    /// Whether these are GitButler's own colours, and dark ones: an accent applies only to
    /// them, as a colour scheme brings its own.
    own_colours: Option<bool>,
}

impl Global for Theme {}

/// A colour something is tinted in by what it is: a move's mark, a pull request's state, a
/// button's tint. [`Theme`] gives each as text, as a soft ground and as a wash over rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hue {
    Pop,
    Blue,
    Safe,
    Warn,
    Danger,
    Purple,
    /// Secondary text, on gray.
    Gray,
}

/// What's marked over text or rows, which must let what's under it show: a diff's green and
/// red lines, a search's tint under a selection. Each is a token's colour at a set strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Highlight {
    /// Selected text in a field with the keys, and in one without.
    Selected,
    SelectedUnfocused,
    /// Selected words in a diff's line, or a line being edited there, as the diff selects.
    Words,
    /// A search's matches, and the one it's on.
    Found,
    FoundHere,
    /// What's new since it was last read.
    Fresh,
    /// What a comment is about, and while that comment is pointed at or written.
    Said,
    Saying,
    /// What a comment not sent yet is about, and while it's pointed at or written.
    Draft,
    SayingDraft,
    /// What blinks over a diff's rows and headers as they're copied: a light wash of pop,
    /// which shows on green and red lines without drowning them out.
    Copy,
    /// A diff's line number picked for a selection.
    Picked,
    /// The words that changed on a commit's side of a conflict.
    Incoming,
}

/// How large code shows in diffs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// Always the light scheme, always the dark one, or whichever macOS is set to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    /// The scheme this choice draws in while the system is `system`.
    /// `GITBUTLER_GPUI_APPEARANCE=dark|light` overrides it, handy for screenshots.
    pub fn shade(self, system: WindowAppearance) -> Shade {
        match (std::env::var("GITBUTLER_GPUI_APPEARANCE").as_deref(), self) {
            (Ok("dark"), _) => Shade::Dark,
            (Ok("light"), _) => Shade::Light,
            (_, Appearance::Dark) => Shade::Dark,
            (_, Appearance::Light) => Shade::Light,
            (_, Appearance::System) => match system {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => Shade::Dark,
                WindowAppearance::Light | WindowAppearance::VibrantLight => Shade::Light,
            },
        }
    }
}

/// Which of the two colour schemes Settings keeps: the one for light, or the one for dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shade {
    Light,
    Dark,
}

/// How round corners are, by role, in px as designed, from but-ui's radius tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    /// The smallest marks: a checkbox, a keycap.
    pub xs: f32,
    /// A control or a row: a button, a field, a segment, a tag.
    pub control: f32,
    /// A card: a tile, a file's or a hunk's header, a menu's rows.
    pub card: f32,
    /// What floats over the window: a popover, a modal, a snackbar.
    pub popup: f32,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            xs: tokens::RADIUS_XS,
            control: tokens::RADIUS_CONTROL,
            card: tokens::RADIUS_CARD,
            popup: tokens::RADIUS_POPUP,
        }
    }
}

/// How tall controls and bars are, in px as designed, from but-ui's size tokens, and how far a
/// density moves their side padding: nothing at Balanced, Figma's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    /// A button, a select, a segmented control or a toggle group.
    pub control: f32,
    /// A small one; a small button or segment is 4px taller than this, for its line.
    pub control_sm: f32,
    /// A bar inside a popup: its search row or its footer.
    pub bar: f32,
    /// Added to a control's side padding.
    pub padding: f32,
}

impl Default for Sizes {
    fn default() -> Self {
        Sizes {
            control: tokens::SIZE_CONTROL,
            control_sm: tokens::SIZE_CONTROL_SM,
            bar: tokens::SIZE_BAR,
            padding: 0.,
        }
    }
}

/// The colour GitButler's own theme picks things out in: its primary buttons, ticks and the
/// accent's text. One of but-ui's accents, or any colour, drawn so it still reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accent {
    #[default]
    Teal,
    Blue,
    Purple,
    /// The colour in `Prefs::accent_color`.
    Custom,
}

impl Accent {
    /// Its name in but-ui's settings.
    fn name(self) -> &'static str {
        match self {
            Accent::Teal => "teal",
            Accent::Blue => "blue",
            Accent::Purple => "purple",
            Accent::Custom => "custom",
        }
    }
}

/// The gray GitButler's own colours are built on: gray, or one of but-ui's tinted grays, at
/// gray's lightness step for step, so everything reads as it does on gray.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ground {
    #[default]
    Gray,
    Slate,
    Sage,
    Sand,
}

impl Ground {
    /// Its name in but-ui's settings.
    fn name(self) -> &'static str {
        match self {
            Ground::Gray => "gray",
            Ground::Slate => "slate",
            Ground::Sage => "sage",
            Ground::Sand => "sand",
        }
    }
}

/// How round corners are: but-ui's scale moved sharper or rounder, Soft its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Corners {
    Sharp,
    #[default]
    Soft,
    Round,
}

impl Corners {
    /// Its name in but-ui's settings.
    pub fn name(self) -> &'static str {
        match self {
            Corners::Sharp => "sharp",
            Corners::Soft => "soft",
            Corners::Round => "round",
        }
    }
}

/// How tall controls are: but-ui's sizes a step shorter or taller, Balanced its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Density {
    Compact,
    #[default]
    Balanced,
    Spacious,
}

impl Density {
    /// Its name in but-ui's settings.
    pub fn name(self) -> &'static str {
        match self {
            Density::Compact => "compact",
            Density::Balanced => "balanced",
            Density::Spacious => "spacious",
        }
    }
}

/// but-ui's presets, each a corner setting and a density by one name.
pub fn presets() -> impl Iterator<Item = (&'static str, Corners, Density)> {
    (tokens::PRESETS.iter()).filter_map(|&(_, name, corners, density)| {
        let corners = [Corners::Sharp, Corners::Soft, Corners::Round]
            .into_iter()
            .find(|c| c.name() == corners)?;
        let density = [Density::Compact, Density::Balanced, Density::Spacious]
            .into_iter()
            .find(|d| d.name() == density)?;
        Some((name, corners, density))
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorScheme {
    /// GitButler's own, light as the light scheme and dark as the dark one.
    #[default]
    GitButler,
    /// The light or the dark theme the user's Ghostty config sets, as the scheme is light's
    /// or dark's.
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

    /// This theme with `diff_font`'s ligatures on or off. Fonts keep them under `liga` or `calt`.
    pub fn with_ligatures(mut self, on: bool) -> Self {
        let off = [("liga".to_owned(), 0), ("calt".to_owned(), 0)];
        self.diff_features = FontFeatures(if on { vec![] } else { off.to_vec() }.into());
        self
    }

    /// This theme with terminals in `family`, or with none picked, in the first family of
    /// `ghostty`'s that `installed` says the machine has, with `ghostty`'s font features and
    /// bold.
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
            if ghostty.bold_disabled {
                self.terminal_bold = self.terminal_font.weight;
            }
        }
        self
    }

    /// This theme picking things out in `accent`, `custom` (`#rrggbb` or `#rgb`) for a custom
    /// one, when it's in GitButler's own colours; a colour scheme keeps its own.
    pub fn with_accent(mut self, accent: Accent, custom: &str) -> Self {
        let Some(dark) = self.own_colours else {
            return self;
        };
        let pick = |[light, dark_value]: [u32; 2]| if dark { dark_value } else { light };
        let family: Vec<(&str, u32)> = match accent {
            Accent::Custom => custom_accent(custom, dark),
            named => (tokens::ACCENTS.iter())
                .find(|(name, _)| *name == named.name())
                .map(|(_, family)| (family.iter()).map(|(token, modes)| (*token, pick(*modes))))
                .into_iter()
                .flatten()
                .collect(),
        };
        // As `0xRRGGBBAA`: the soft fills are see-through.
        let token = |name: &str| {
            family
                .iter()
                .find(|(token, _)| *token == name)
                .map(|&(_, c)| Hsla::from(rgba(c)))
        };
        if let Some(primary) = token("primary") {
            (self.pop_bg, self.pop_button) = (primary, primary);
            // The washes are the accent's primary at but-ui's strengths, as CSS mixes them.
            let [wash, strong] = WASHES;
            (self.pop_wash, self.pop_wash_strong) =
                (primary.opacity(wash), primary.opacity(strong));
        }
        if let Some(text) = token("primary-text") {
            self.pop = text;
        }
        if let Some(hover) = token("primary-hover") {
            self.pop_hover = hover;
        }
        if let Some(soft) = token("primary-soft") {
            self.pop_soft = soft;
        }
        if let Some(soft) = token("primary-soft-hover") {
            self.pop_soft_hover = soft;
        }
        if let Some(surface) = token("primary-surface") {
            self.pop_surface = surface;
        }
        if let Some(surface) = token("primary-surface-hover") {
            self.pop_surface_hover = surface;
        }
        self
    }

    /// This theme on `ground`, when it's in GitButler's own colours; a colour scheme keeps its
    /// own. Called before anything else changes it, as it draws GitButler's colours again.
    pub fn with_ground(self, ground: Ground) -> Self {
        let Some(dark) = self.own_colours else {
            return self;
        };
        Theme {
            code_text: self.code_text,
            code_line: self.code_line,
            ..Self::gitbutler_on(dark, CodeSize::default(), ground)
        }
    }

    pub fn with_corners(mut self, corners: Corners) -> Self {
        if let Some((_, [xs, control, card, popup])) =
            (tokens::CORNERS.iter()).find(|(name, _)| *name == corners.name())
        {
            self.shape = Shape {
                xs: *xs,
                control: *control,
                card: *card,
                popup: *popup,
            };
        }
        self
    }

    pub fn with_density(mut self, density: Density) -> Self {
        if let Some((_, [control, control_sm, bar, padding])) =
            (tokens::DENSITIES.iter()).find(|(name, _)| *name == density.name())
        {
            self.sizes = Sizes {
                control: *control,
                control_sm: *control_sm,
                bar: *bar,
                padding: *padding,
            };
        }
        self
    }

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

    /// The theme `light` or `dark` draws in, as `choice` picks while the system is `system`,
    /// with code in diffs at `code`. `ghostty` is the user's Ghostty config, which colours the
    /// app and its terminals for [`ColorScheme::Ghostty`]. The terminals always draw in the
    /// scheme the app does. A scheme without a background and foreground, as a named theme
    /// that's gone, draws as GitButler's.
    pub fn new(
        choice: Appearance,
        [light, dark]: [&ColorScheme; 2],
        system: WindowAppearance,
        code: CodeSize,
        ghostty: &Ghostty,
    ) -> Self {
        let shade = choice.shade(system);
        let scheme = match shade {
            Shade::Light => light,
            Shade::Dark => dark,
        };
        let dark = shade == Shade::Dark;
        // `GITBUTLER_GPUI_COLOR_SCHEME=gitbutler|ghostty|<theme name>` overrides the settings,
        // handy for screenshots.
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
    pub(crate) fn gitbutler(dark: bool, code: CodeSize) -> Self {
        Self::gitbutler_on(dark, code, Ground::Gray)
    }

    /// GitButler's own colours on `ground`, light or dark, the terminals' too.
    fn gitbutler_on(dark: bool, code: CodeSize, ground: Ground) -> Self {
        // The colours that come out differently on the ground than on gray.
        let on_ground = (tokens::GROUNDS.iter())
            .find(|(name, _)| *name == ground.name())
            .map_or(&[][..], |(_, colors)| *colors);
        let grounded = |name: &str, gray: [u32; 2]| {
            (on_ground.iter())
                .find(|(token, _)| *token == name)
                .map_or(gray, |(_, colors)| *colors)
        };
        // A but-ui token, as it comes out on the ground.
        macro_rules! on_ground {
            ($token:ident) => {
                grounded(stringify!($token), tokens::$token)
            };
        }
        let pick = |light: u32, dark_value: u32| -> Hsla {
            rgb(if dark { dark_value } else { light }).into()
        };
        // A but-ui token in this appearance.
        let token = |[light, dark_value]: [u32; 2]| -> Hsla {
            rgba(if dark { dark_value } else { light }).into()
        };
        // Halfway from one opaque but-ui token to another, as CSS's `color-mix` at 50%.
        let halfway = |a: [u32; 2], b: [u32; 2]| -> Hsla {
            let pick = |[light, dark_value]: [u32; 2]| (if dark { dark_value } else { light }) >> 8;
            color(mix(pick(a), pick(b), 0.5))
        };
        let (code_text, code_line) = code.metrics();
        // GitHub's light and dark, which but.dev's diffs take their colours from.
        let added = pick(0x28a745, 0x34d058);
        let removed = pick(0xd73a49, 0xea4a5a);
        // Coloured text is a hue's step 11, which reads at 4.5:1 on the gray grounds.
        let [safe, danger] = [on_ground!(SUCCESS_TEXT), on_ground!(DANGER_TEXT)].map(token);
        let [fill_gray, text_inverted] =
            [on_ground!(INVERSE), on_ground!(INVERSE_FOREGROUND)].map(token);
        let [done_light, done_dark] = on_ground!(DONE_TEXT);
        let on_solid = |accent| {
            color(readable_on(
                rgb_hex(accent),
                rgb_hex(text_inverted),
                rgb_hex(fill_gray),
            ))
        };
        Theme {
            code_text,
            code_line,
            scale: 1.,
            // The cards and panes, and the window's ground.
            bg_1: token(on_ground!(BACKGROUND)),
            bg_2: token(on_ground!(CANVAS)),
            // Secondary text meets WCAG AA (4.5:1), faint text the 3:1 for de-emphasised
            // details, on both grounds.
            text_1: token(on_ground!(FOREGROUND)),
            text_2: token(on_ground!(MUTED_FOREGROUND)),
            text_3: token(on_ground!(SUBTLE_FOREGROUND)),
            border_1: token(on_ground!(BORDER_HOVER)),
            border_2: token(on_ground!(BORDER)),
            border_3: token(on_ground!(BORDER_SUBTLE)),
            fill_gray,
            text_inverted,
            hover: token(on_ground!(HOVER)),
            selected_unfocused: token(on_ground!(SELECTED_INACTIVE)),
            focus_border: token(on_ground!(RING)),
            control_border: token(on_ground!(INPUT)),
            secondary: token(on_ground!(SECONDARY)),
            muted: token(on_ground!(MUTED)),
            muted_hover: halfway(on_ground!(MUTED), on_ground!(MUTED_NESTED)),
            muted_on_solid: token(on_ground!(INVERSE_MUTED_FOREGROUND)),
            hover_on_solid: token(on_ground!(INVERSE_GHOST_HOVER)),
            inverse_soft: token(on_ground!(INVERSE_SOFT)),
            inverse_soft_hover: token(on_ground!(INVERSE_SOFT_HOVER)),
            scrollbar: token(on_ground!(SCROLLBAR_THUMB)),
            scrollbar_hover: token(on_ground!(SCROLLBAR_THUMB_HOVER)),
            overlay: token(on_ground!(OVERLAY)),
            shadow: token(on_ground!(SHADOW)),
            // The accent as text, and as the ground its label and tick read on at 4.5:1, in
            // both themes.
            pop: token(on_ground!(PRIMARY_TEXT)),
            pop_bg: token(on_ground!(PRIMARY)),
            pop_button: token(on_ground!(PRIMARY)),
            pop_hover: token(on_ground!(PRIMARY_HOVER)),
            on_pop: token(on_ground!(PRIMARY_FOREGROUND)),
            blue: token(on_ground!(INFO_TEXT)),
            safe,
            warn: token(on_ground!(WARNING_TEXT)),
            danger,
            purple: token([done_light, done_dark]),
            pop_soft: token(on_ground!(PRIMARY_SOFT)),
            pop_soft_hover: token(on_ground!(PRIMARY_SOFT_HOVER)),
            blue_soft: token(on_ground!(INFO_SOFT)),
            blue_soft_hover: token(on_ground!(INFO_SOFT_HOVER)),
            safe_soft: token(on_ground!(SUCCESS_SOFT)),
            safe_soft_hover: token(on_ground!(SUCCESS_SOFT_HOVER)),
            warn_soft: token(on_ground!(WARNING_SOFT)),
            warn_soft_hover: token(on_ground!(WARNING_SOFT_HOVER)),
            danger_soft: token(on_ground!(DANGER_SOFT)),
            danger_soft_hover: token(on_ground!(DANGER_SOFT_HOVER)),
            purple_soft: token(on_ground!(DONE_SOFT)),
            purple_soft_hover: token(on_ground!(DONE_SOFT_HOVER)),
            pop_surface: token(on_ground!(PRIMARY_SURFACE)),
            pop_surface_hover: token(on_ground!(PRIMARY_SURFACE_HOVER)),
            blue_surface: token(on_ground!(INFO_SURFACE)),
            blue_surface_hover: token(on_ground!(INFO_SURFACE_HOVER)),
            safe_surface: token(on_ground!(SUCCESS_SURFACE)),
            safe_surface_hover: token(on_ground!(SUCCESS_SURFACE_HOVER)),
            warn_surface: token(on_ground!(WARNING_SURFACE)),
            warn_surface_hover: token(on_ground!(WARNING_SURFACE_HOVER)),
            danger_surface: token(on_ground!(DANGER_SURFACE)),
            danger_surface_hover: token(on_ground!(DANGER_SURFACE_HOVER)),
            purple_surface: token(on_ground!(DONE_SURFACE)),
            purple_surface_hover: token(on_ground!(DONE_SURFACE_HOVER)),
            pop_wash: token(on_ground!(PRIMARY_WASH)),
            pop_wash_strong: token(on_ground!(PRIMARY_WASH_STRONG)),
            blue_wash: token(on_ground!(INFO_WASH)),
            blue_wash_strong: token(on_ground!(INFO_WASH_STRONG)),
            safe_wash: token(on_ground!(SUCCESS_WASH)),
            safe_wash_strong: token(on_ground!(SUCCESS_WASH_STRONG)),
            warn_wash: token(on_ground!(WARNING_WASH)),
            warn_wash_strong: token(on_ground!(WARNING_WASH_STRONG)),
            danger_wash: token(on_ground!(DANGER_WASH)),
            danger_wash_strong: token(on_ground!(DANGER_WASH_STRONG)),
            purple_wash: token(on_ground!(DONE_WASH)),
            purple_wash_strong: token(on_ground!(DONE_WASH_STRONG)),
            warn_bg: token(on_ground!(WARNING)),
            on_warn: token(on_ground!(WARNING_FOREGROUND)),
            // Amber's and blue's step 7, the scales' border.
            warn_edge: token(on_ground!(AMBER_7)),
            blue_edge: token(on_ground!(BLUE_7)),
            safe_on_solid: on_solid(safe),
            danger_on_solid: on_solid(danger),
            purple_on_solid: token([done_dark, done_light]),
            snackbar_bg: token(on_ground!(SNACKBAR)),
            snackbar_border: token(on_ground!(SNACKBAR_BORDER)),
            snackbar_text: token(on_ground!(SNACKBAR_FOREGROUND)),
            snackbar_warn: token(on_ground!(WARNING)),
            snackbar_divider: token(on_ground!(SNACKBAR_DIVIDER)),
            snackbar_hover: token(on_ground!(SNACKBAR_HOVER)),
            diff_added: added,
            diff_removed: removed,
            diff_added_bg: pick(0xeef7ee, 0x2c3f35),
            diff_removed_bg: pick(0xffeeed, 0x413134),
            diff_added_emph: added.opacity(if dark { 0.2 } else { 0.15 }),
            diff_removed_emph: removed.opacity(if dark { 0.2 } else { 0.15 }),
            diff_muted: pick(0x696c70, 0x999da1),
            // Each hue's step 9.
            change_status: [
                on_ground!(CHANGE_STATUS_ADDITION),
                on_ground!(CHANGE_STATUS_MODIFICATION),
                on_ground!(CHANGE_STATUS_DELETION),
                on_ground!(CHANGE_STATUS_RENAME),
            ]
            .map(token),
            // Teal, purple, green, amber and red at step 7.
            person: [
                on_ground!(CATEGORY_1),
                on_ground!(CATEGORY_2),
                on_ground!(CATEGORY_3),
                on_ground!(CATEGORY_4),
                on_ground!(CATEGORY_5),
            ]
            .map(token),
            // Embedded by `ui::fonts`.
            ui_font: "Inter".into(),
            mono_font: "GeistMono Nerd Font Mono".into(),
            diff_font: "GeistMono Nerd Font Mono".into(),
            diff_digit: 0.6,
            diff_features: FontFeatures::default(),
            terminal_font: gpui::font("GeistMono Nerd Font Mono"),
            terminal_bold: FontWeight::SEMIBOLD,
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
            own_colours: Some(dark),
            shape: Shape::default(),
            sizes: Sizes::default(),
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
    /// on both grounds and on a gray control's: secondary text and colours used as text at
    /// 4.5:1, faint text and marks at 3:1.
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
        // A step of the foreground into the ground, as but-ui's opaque grays: muted, and
        // halfway on to muted-nested.
        let gray = |share: f32| mix(bg, fg, share);
        let muted = gray(if dark { 0.09 } else { 0.06 });
        let muted_hover = gray(if dark { 0.105 } else { 0.077 });
        let grounds = [bg, bg_2, muted, muted_hover];
        let ink = if dark { 0xffffff } else { 0x000000 };
        // An ANSI colour's soft ground and that under the pointer, a step further toward it.
        let soft = |index: u8| mix(bg, ansi(index), if dark { 0.18 } else { 0.12 });
        let soft_hover = |index: u8| mix(bg, ansi(index), if dark { 0.22 } else { 0.17 });
        let text_on = |from: u32, ratio: f32, also: &[u32]| {
            let grounds: Vec<u32> = grounds.iter().chain(also).copied().collect();
            (reaching(from, fg, &grounds, ratio))
                .or_else(|| reaching(fg, ink, &grounds, ratio))
                .unwrap_or(ink)
        };
        let text = |from: u32, ratio: f32| text_on(from, ratio, &[]);
        // An ANSI colour as text, which reads on its soft grounds too.
        let hue_text = |index: u8| text_on(ansi(index), 4.5, &[soft(index), soft_hover(index)]);
        // A snackbar's ground is the foreground, so its glyph steps toward whichever of black
        // and white reads there.
        let snackbar_ink = match contrast(fg, 0xffffff) > contrast(fg, 0x000000) {
            true => 0xffffff,
            false => 0x000000,
        };
        let pop = ansi(6);
        // What reads at 4.5:1 on a solid fill of `fill`: white, the ground, or black.
        let ink_on = |fill: u32| {
            [0xffffff, bg, 0x000000]
                .into_iter()
                .find(|&ink| contrast(ink, fill) >= 4.5)
                .unwrap_or(0x000000)
        };
        let (on_pop, on_warn) = (ink_on(pop), ink_on(ansi(3)));
        // An ANSI colour laid over what shows through, at but-ui's wash strengths.
        let washed = |index: u8, strength: usize| {
            let share = WASHES.get(strength).copied().unwrap_or(0.);
            color(ansi(index)).opacity(share)
        };
        let text_3 = text(mix(bg, fg, 0.45), 3.);
        let [safe, blue, danger, purple] = [2, 4, 1, 5].map(hue_text);
        let [added, removed] = [2, 1].map(|index| text(ansi(index), 3.));
        // `fg` laid over the ground at `share`, as but-ui's grays are the foreground's.
        let veil = |share: f32| color(fg).opacity(share);
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
            hover: veil(0.05),
            selected_unfocused: veil(0.1),
            focus_border: veil(0.7),
            control_border: veil(0.26),
            secondary: veil(0.08),
            muted: color(muted),
            muted_hover: color(muted_hover),
            muted_on_solid: color(bg).opacity(0.6),
            hover_on_solid: color(bg).opacity(0.05),
            inverse_soft: color(bg).opacity(0.15),
            inverse_soft_hover: color(bg).opacity(0.22),
            scrollbar: veil(0.15),
            scrollbar_hover: veil(0.25),
            overlay: match dark {
                true => color(0x000000).opacity(0.5),
                false => color(gray(0.15)).opacity(0.45),
            },
            shadow: color(0x000000).opacity(if dark { 0.5 } else { 0.1 }),
            pop: color(hue_text(6)),
            pop_bg: color(pop),
            pop_button: color(pop),
            pop_hover: color(mix(pop, 0x000000, 0.08)),
            on_pop: color(on_pop),
            blue: color(blue),
            safe: color(safe),
            warn: color(hue_text(3)),
            danger: color(danger),
            purple: color(purple),
            pop_soft: color(soft(6)),
            pop_soft_hover: color(soft_hover(6)),
            blue_soft: color(soft(4)),
            blue_soft_hover: color(soft_hover(4)),
            safe_soft: color(soft(2)),
            safe_soft_hover: color(soft_hover(2)),
            warn_soft: color(soft(3)),
            warn_soft_hover: color(soft_hover(3)),
            danger_soft: color(soft(1)),
            danger_soft_hover: color(soft_hover(1)),
            purple_soft: color(soft(5)),
            purple_soft_hover: color(soft_hover(5)),
            // A scheme's soft grounds are opaque already, so they serve as its areas too.
            pop_surface: color(soft(6)),
            pop_surface_hover: color(soft_hover(6)),
            blue_surface: color(soft(4)),
            blue_surface_hover: color(soft_hover(4)),
            safe_surface: color(soft(2)),
            safe_surface_hover: color(soft_hover(2)),
            warn_surface: color(soft(3)),
            warn_surface_hover: color(soft_hover(3)),
            danger_surface: color(soft(1)),
            danger_surface_hover: color(soft_hover(1)),
            purple_surface: color(soft(5)),
            purple_surface_hover: color(soft_hover(5)),
            pop_wash: washed(6, 0),
            pop_wash_strong: washed(6, 1),
            blue_wash: washed(4, 0),
            blue_wash_strong: washed(4, 1),
            safe_wash: washed(2, 0),
            safe_wash_strong: washed(2, 1),
            warn_wash: washed(3, 0),
            warn_wash_strong: washed(3, 1),
            danger_wash: washed(1, 0),
            danger_wash_strong: washed(1, 1),
            purple_wash: washed(5, 0),
            purple_wash_strong: washed(5, 1),
            warn_bg: color(ansi(3)),
            on_warn: color(on_warn),
            warn_edge: color(mix(bg, ansi(3), 0.5)),
            blue_edge: color(mix(bg, ansi(4), 0.5)),
            safe_on_solid: color(readable_on(safe, bg, fg)),
            danger_on_solid: color(readable_on(danger, bg, fg)),
            purple_on_solid: color(readable_on(purple, bg, fg)),
            snackbar_bg: color(fg),
            snackbar_border: gpui::transparent_black(),
            snackbar_text: color(bg),
            snackbar_warn: color(
                reaching(ansi(3), snackbar_ink, &[fg], 3.).unwrap_or(snackbar_ink),
            ),
            snackbar_divider: color(mix(bg, fg, 0.18)).opacity(0.3),
            snackbar_hover: color(bg).opacity(0.05),
            diff_added: color(added),
            diff_removed: color(removed),
            diff_added_bg: color(soft(2)),
            diff_removed_bg: color(soft(1)),
            diff_added_emph: color(added).opacity(if dark { 0.2 } else { 0.15 }),
            diff_removed_emph: color(removed).opacity(if dark { 0.2 } else { 0.15 }),
            diff_muted: color(text_3),
            change_status: [safe, blue, danger, purple].map(color),
            person: [6, 5, 2, 3, 1].map(|index| color(mix(bg, ansi(index), 0.55))),
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
            own_colours: None,
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

    /// The edge of the card with the keys, the tile's or the navigator's: a focused field's
    /// ring, as it reads on the field's ground, the cards'. Opaque, so it's the same gray drawn
    /// over a card's own edge as in its place, or twice over.
    pub fn focus_edge(&self) -> Hsla {
        self.bg_1.blend(self.focus_border)
    }

    /// The unfocused selection as it shows on a card, opaque, for what's drawn over the
    /// card's content, as a sticky header.
    pub fn selected_on_card(&self) -> Hsla {
        self.bg_1.blend(self.selected_unfocused)
    }

    /// Halfway from the page to the first gray step: a passage set off from the page, not a
    /// panel.
    pub fn passage(&self) -> Hsla {
        self.bg_1.blend(self.bg_2.opacity(0.5))
    }

    /// `hue` as text and marks.
    pub fn hue(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.pop,
            Hue::Blue => self.blue,
            Hue::Safe => self.safe,
            Hue::Warn => self.warn,
            Hue::Danger => self.danger,
            Hue::Purple => self.purple,
            Hue::Gray => self.text_2,
        }
    }

    /// `hue`'s soft ground, which it reads on as text.
    pub fn soft(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.pop_soft,
            Hue::Blue => self.blue_soft,
            Hue::Safe => self.safe_soft,
            Hue::Warn => self.warn_soft,
            Hue::Danger => self.danger_soft,
            Hue::Purple => self.purple_soft,
            Hue::Gray => self.muted,
        }
    }

    /// `hue`'s soft ground under the pointer.
    pub fn soft_hover(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.pop_soft_hover,
            Hue::Blue => self.blue_soft_hover,
            Hue::Safe => self.safe_soft_hover,
            Hue::Warn => self.warn_soft_hover,
            Hue::Danger => self.danger_soft_hover,
            Hue::Purple => self.purple_soft_hover,
            Hue::Gray => self.muted_hover,
        }
    }

    /// `hue`'s ground as an area, opaque: a banner, a callout, a tile.
    pub fn surface(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.pop_surface,
            Hue::Blue => self.blue_surface,
            Hue::Safe => self.safe_surface,
            Hue::Warn => self.warn_surface,
            Hue::Danger => self.danger_surface,
            Hue::Purple => self.purple_surface,
            Hue::Gray => self.muted,
        }
    }

    /// `hue`'s area under the pointer.
    pub fn surface_hover(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.pop_surface_hover,
            Hue::Blue => self.blue_surface_hover,
            Hue::Safe => self.safe_surface_hover,
            Hue::Warn => self.warn_surface_hover,
            Hue::Danger => self.danger_surface_hover,
            Hue::Purple => self.purple_surface_hover,
            Hue::Gray => self.muted_hover,
        }
    }

    /// `hue` washed over rows, which still show through: a move's mark, a drop target, a mark
    /// under the pointer.
    pub fn wash(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.pop_wash,
            Hue::Blue => self.blue_wash,
            Hue::Safe => self.safe_wash,
            Hue::Warn => self.warn_wash,
            Hue::Danger => self.danger_wash,
            Hue::Purple => self.purple_wash,
            Hue::Gray => self.hover,
        }
    }

    /// A stronger [`Theme::wash`], for the one the pointer is over.
    pub fn wash_strong(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.pop_wash_strong,
            Hue::Blue => self.blue_wash_strong,
            Hue::Safe => self.safe_wash_strong,
            Hue::Warn => self.warn_wash_strong,
            Hue::Danger => self.danger_wash_strong,
            Hue::Purple => self.purple_wash_strong,
            Hue::Gray => self.selected_unfocused,
        }
    }

    /// What marks `highlight`, over what's under it. The search match the keys are on is solid,
    /// with [`Theme::on_warn`] as its text.
    pub fn highlight(&self, highlight: Highlight) -> Hsla {
        match highlight {
            Highlight::Selected | Highlight::Fresh | Highlight::Copy => self.pop_wash_strong,
            Highlight::SelectedUnfocused | Highlight::Picked => self.pop_wash,
            Highlight::Words | Highlight::SayingDraft | Highlight::Incoming => {
                self.blue_wash_strong
            }
            Highlight::Draft => self.blue_wash,
            Highlight::Found | Highlight::Saying => self.warn_wash_strong,
            Highlight::Said => self.warn_wash,
            Highlight::FoundHere => self.warn_bg,
        }
    }
}

/// but-ui's wash strengths, `--but-opacity-wash(-strong)`, read from the alpha of its primary
/// washes, for what mixes them itself: another accent, a colour scheme.
const WASHES: [f32; 2] = [
    alpha(tokens::PRIMARY_WASH),
    alpha(tokens::PRIMARY_WASH_STRONG),
];

/// The alpha of a `[light, dark]` token, as a share, from its light value.
const fn alpha([light, _]: [u32; 2]) -> f32 {
    (light & 0xff) as f32 / 255.
}

/// Ghostty's own 16 colours, for the ones a theme leaves unset.
const GHOSTTY_ANSI: [u32; 16] = [
    0x1d1f21, 0xcc6666, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xc5c8c6, 0x666666,
    0xd54e53, 0xb9ca4a, 0xe7c547, 0x7aa6da, 0xc397d8, 0x70c0b1, 0xeaeaea,
];

/// The custom accent's family drawn from `pick`, `#rrggbb` or `#rgb`, as but-ui's tokens.css draws
/// it: each token is the pick's hue, its chroma capped and taken at a share, at a lightness, in
/// OKLCH, then clipped into sRGB as a browser does, and see-through where the step is, as
/// `0xRRGGBBAA`. A pick that isn't a colour draws as teal.
fn custom_accent(pick: &str, dark: bool) -> Vec<(&'static str, u32)> {
    let [light, dark_value] = tokens::CUSTOM_ACCENT_FALLBACK;
    let fallback = if dark { dark_value } else { light } >> 8;
    let (_, chroma, hue) = oklch(parse_hex(pick).unwrap_or(fallback));
    let chroma = chroma.min(tokens::CUSTOM_ACCENT_CHROMA_CAP);
    (tokens::CUSTOM_ACCENT.iter())
        .map(|&(token, [light, dark_modes])| {
            let (lightness, share, alpha) = if dark { dark_modes } else { light };
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "clamped to 0–255 first"
            )]
            let alpha = (alpha.clamp(0., 1.) * 255.).round() as u32;
            (
                token,
                from_oklch(lightness, chroma * share, hue) << 8 | alpha,
            )
        })
        .collect()
}

/// `#rrggbb` or `#rgb` as `0xRRGGBB`, the `#` left out or not, as a colour pasted from
/// elsewhere may come without it.
pub(crate) fn parse_hex(text: &str) -> Option<u32> {
    let text = text.trim();
    let digits = text.strip_prefix('#').unwrap_or(text);
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(value),
        3 => {
            let [r, g, b] = [value >> 8, value >> 4 & 0xf, value & 0xf].map(|c| c * 0x11);
            Some(r << 16 | g << 8 | b)
        }
        _ => None,
    }
}

/// `hex` in OKLCH: its lightness, chroma and hue in degrees.
fn oklch(hex: u32) -> (f32, f32, f32) {
    let [r, g, b] = channels(hex).map(|c| {
        let c = c / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let lightness = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    (lightness, a.hypot(b), b.atan2(a).to_degrees())
}

/// The colour at `lightness`, `chroma` and `hue` (degrees) in OKLCH, each channel clipped into
/// sRGB, as `0xRRGGBB`.
fn from_oklch(lightness: f32, chroma: f32, hue: f32) -> u32 {
    let (a, b) = (
        chroma * hue.to_radians().cos(),
        chroma * hue.to_radians().sin(),
    );
    let l = (lightness + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (lightness - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (lightness - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let linear = [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ];
    let [r, g, b] = linear.map(|c| {
        let c = if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1. / 2.4) - 0.055
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 0–255 first"
        )]
        let byte = (c.clamp(0., 1.) * 255.).round() as u32;
        byte
    });
    r << 16 | g << 8 | b
}

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

/// `accent` stepped toward `ink` until it reads as text on `ground`, or `ink` if it never does.
fn readable_on(accent: u32, ink: u32, ground: u32) -> u32 {
    reaching(accent, ink, &[ground], 4.5).unwrap_or(ink)
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
    use gpui::{FontWeight, WindowAppearance};

    use super::{
        Accent, Appearance, CodeSize, ColorScheme, Hue, Theme, WASHES, channels, contrast,
        custom_accent, rgb_hex,
    };

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
            [&ColorScheme::Ghostty; 2],
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
            let glyph = contrast(rgb_hex(theme.snackbar_warn), rgb_hex(theme.snackbar_bg));
            assert!(
                glyph >= 3.,
                "{name}: a warning snackbar's glyph reads at {glyph}"
            );
        }
    }

    #[test]
    fn a_custom_accent_draws_as_the_web_draws_it() {
        // What but-ui's build resolves the custom accent to, for 25 picks, as a browser would.
        let samples: serde_json::Value = serde_json::from_str(include_str!(
            "../../../packages/but-ui/tokens/accent-samples.json"
        ))
        .unwrap();
        let mut off = Vec::new();
        for sample in samples["samples"].as_array().unwrap() {
            let pick = sample["color"].as_str().unwrap();
            for (mode, dark) in [("light", false), ("dark", true)] {
                for (token, drawn) in custom_accent(pick, dark) {
                    // `#rrggbb`, or `#rrggbbaa` for a see-through step.
                    let web = sample["tokens"][token][mode].as_str().unwrap();
                    let digits = web.trim_start_matches('#');
                    let web = u32::from_str_radix(digits, 16).unwrap();
                    let web = if digits.len() == 6 {
                        web << 8 | 0xff
                    } else {
                        web
                    };
                    let alpha = |c: u32| (c & 0xff) as f32;
                    let apart = (channels(drawn >> 8).into_iter().zip(channels(web >> 8)))
                        .map(|(a, b)| (a - b).abs())
                        .fold((alpha(drawn) - alpha(web)).abs(), f32::max);
                    if apart > 1. {
                        off.push(format!("{pick} {mode} {token}: {drawn:08x}, web {web:08x}"));
                    }
                }
            }
        }
        assert!(
            off.is_empty(),
            "each channel within 1 of the web's:\n{}",
            off.join("\n")
        );
    }

    #[test]
    fn a_warning_snackbars_glyph_reads_in_gitbutlers_colours() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = Theme::new(
                appearance,
                [&ColorScheme::GitButler; 2],
                WindowAppearance::Light,
                CodeSize::Medium,
                &Ghostty::default(),
            );
            let glyph = contrast(rgb_hex(theme.snackbar_warn), rgb_hex(theme.snackbar_bg));
            assert!(
                glyph >= 3.,
                "{appearance:?}: a warning snackbar's glyph reads at {glyph}, under 3:1"
            );
        }
    }

    #[test]
    fn each_hue_reads_on_its_soft_ground() {
        let catppuccin = palette(
            0x1e1e2e,
            0xcdd6f4,
            [
                0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de,
            ],
        );
        let solarized = palette(
            0xfdf6e3,
            0x657b83,
            [
                0x073642, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x2aa198, 0xeee8d5,
            ],
        );
        let themes = [
            ("gitbutler light", Theme::gitbutler(false, CodeSize::Medium)),
            ("gitbutler dark", Theme::gitbutler(true, CodeSize::Medium)),
            (
                "gitbutler purple",
                Theme::gitbutler(true, CodeSize::Medium).with_accent(Accent::Purple, ""),
            ),
            ("catppuccin", ghostty_theme(catppuccin, Appearance::Dark)),
            ("solarized", ghostty_theme(solarized, Appearance::Light)),
        ];
        for (name, theme) in themes {
            for hue in [
                Hue::Pop,
                Hue::Blue,
                Hue::Safe,
                Hue::Warn,
                Hue::Danger,
                Hue::Purple,
                Hue::Gray,
            ] {
                let ink = rgb_hex(theme.hue(hue));
                // The soft fills are see-through, so each is laid on what it sits on: the
                // content, and for the fill at rest the recessed ground too, as but-ui checks.
                for (what, ground, unders) in [
                    ("soft", theme.soft(hue), vec![theme.bg_1, theme.bg_2]),
                    (
                        "soft under the pointer",
                        theme.soft_hover(hue),
                        vec![theme.bg_1],
                    ),
                    // Opaque, so the ground under it doesn't change it.
                    ("surface", theme.surface(hue), vec![theme.bg_1]),
                    (
                        "surface under the pointer",
                        theme.surface_hover(hue),
                        vec![theme.bg_1],
                    ),
                ] {
                    for under in unders {
                        let reads = contrast(ink, rgb_hex(under.blend(ground)));
                        assert!(
                            reads >= 4.5,
                            "{name}: {hue:?} reads at {reads} on its {what} ground"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn an_accents_washes_follow_it() {
        let teal = Theme::gitbutler(false, CodeSize::Medium);
        let purple = teal.clone().with_accent(Accent::Purple, "");
        for (what, wash, strong) in [
            ("teal", teal.pop_wash, teal.pop_wash_strong),
            ("purple", purple.pop_wash, purple.pop_wash_strong),
        ] {
            let ground = if what == "teal" {
                teal.pop_bg
            } else {
                purple.pop_bg
            };
            for (name, wash, share) in [("wash", wash, WASHES[0]), ("strong", strong, WASHES[1])] {
                assert_eq!(
                    (wash.h, wash.s, wash.l),
                    (ground.h, ground.s, ground.l),
                    "{what}: its {name} is its primary"
                );
                assert!(
                    (wash.a - share).abs() < 0.01,
                    "{what}: its {name} is at but-ui's strength, {} not {share}",
                    wash.a
                );
            }
        }
        assert_ne!(
            teal.pop_wash, purple.pop_wash,
            "another accent washes in its own colour"
        );
    }

    #[test]
    fn line_counts_read_on_a_solid_selection() {
        let catppuccin = palette(
            0x1e1e2e,
            0xcdd6f4,
            [
                0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de,
            ],
        );
        let solarized = palette(
            0xfdf6e3,
            0x657b83,
            [
                0x073642, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x2aa198, 0xeee8d5,
            ],
        );
        // Solarized's own selected text reads under 4.5:1, so its counts can only match it.
        let themes = [
            (
                "gitbutler light",
                Theme::gitbutler(false, CodeSize::Medium),
                true,
            ),
            (
                "gitbutler dark",
                Theme::gitbutler(true, CodeSize::Medium),
                true,
            ),
            (
                "catppuccin",
                ghostty_theme(catppuccin, Appearance::Dark),
                true,
            ),
            (
                "solarized",
                ghostty_theme(solarized, Appearance::Light),
                false,
            ),
        ];
        for (name, theme, keeps_hue) in themes {
            let ground = rgb_hex(theme.fill_gray);
            let label = contrast(rgb_hex(theme.text_inverted), ground);
            for (what, ink) in [
                ("added", theme.safe_on_solid),
                ("removed", theme.danger_on_solid),
            ] {
                let reads = contrast(rgb_hex(ink), ground);
                assert!(
                    reads >= label.min(4.5),
                    "{name}: {what} reads at {reads} on the selection"
                );
                assert_eq!(
                    ink != theme.text_inverted,
                    keeps_hue,
                    "{name}: {what} keeps its hue only where it can read"
                );
            }
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
                [&ColorScheme::GitButler; 2],
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
            [&ColorScheme::GitButler; 2],
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
    fn terminals_draw_bold_in_the_regular_weight_when_ghostty_turns_it_off() {
        let ghostty = Ghostty {
            font_families: vec!["Menlo".into()],
            bold_disabled: true,
            ..Ghostty::default()
        };
        let installed = |family: &str| family == "Menlo";

        let followed = gitbutler_theme().with_terminal_font(None, &ghostty, installed);
        assert_eq!(
            followed.terminal_bold, followed.terminal_font.weight,
            "bold text draws as regular text does"
        );

        let picked = gitbutler_theme().with_terminal_font(Some("Menlo"), &ghostty, installed);
        assert_eq!(
            picked.terminal_bold,
            FontWeight::SEMIBOLD,
            "a picked font draws bold as the app does, as with Ghostty's features"
        );
    }

    #[test]
    fn the_appearance_picks_the_light_or_the_dark_scheme() {
        let solarized = palette(0xfdf6e3, 0x657b83, [0x073642; 8]);
        let ghostty = Ghostty {
            light: solarized,
            ..Ghostty::default()
        };
        let schemes = [&ColorScheme::Ghostty, &ColorScheme::GitButler];
        let draws = |choice, system| {
            let theme = Theme::new(choice, schemes, system, CodeSize::Medium, &ghostty);
            rgb_hex(theme.bg_1)
        };
        let dark = rgb_hex(gitbutler_theme().bg_1);
        for (choice, system, bg, why) in [
            (
                Appearance::System,
                WindowAppearance::Light,
                0xfdf6e3,
                "a light system draws light's",
            ),
            (
                Appearance::System,
                WindowAppearance::Dark,
                dark,
                "a dark system draws dark's",
            ),
            (
                Appearance::Light,
                WindowAppearance::Dark,
                0xfdf6e3,
                "Light draws light's always",
            ),
            (
                Appearance::Dark,
                WindowAppearance::Light,
                dark,
                "Dark draws dark's always",
            ),
        ] {
            assert_eq!(draws(choice, system), bg, "{why}");
        }
    }

    #[test]
    fn a_scheme_without_colours_draws_as_gitbutler() {
        let ghostty = Ghostty::default();
        let system = WindowAppearance::Light;
        let gitbutler = Theme::new(
            Appearance::Dark,
            [&ColorScheme::GitButler; 2],
            system,
            CodeSize::Medium,
            &ghostty,
        );
        let missing = ColorScheme::Named("No theme is called this".into());
        let fallen_back = Theme::new(
            Appearance::Dark,
            [&missing; 2],
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
