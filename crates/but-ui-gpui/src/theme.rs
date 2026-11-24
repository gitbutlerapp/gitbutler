//! Colour and type tokens from `@gitbutler/but-ui`, or from a Ghostty colour scheme that the
//! embedded terminals draw with too.

use but_tty::{Colors, Ghostty, RgbColor};
use gpui::{
    App, Font, FontFeatures, FontWeight, Global, Hsla, Pixels, Rgba, SharedString,
    WindowAppearance, rgb, rgba,
};
use serde::{Deserialize, Serialize};

/// but-ui's tokens as its build resolves them (`packages/but-ui/tokens/tokens.json`): each
/// colour `[light, dark]` as `0xRRGGBBAA`, each length in px.
pub mod tokens {
    include!(concat!(env!("OUT_DIR"), "/color_tokens.rs"));
}

/// A syntax-highlighting class in diffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Comment,
    String,
    Constant,
    Keyword,
    Function,
    Type,
    Tag,
}

include!(concat!(env!("OUT_DIR"), "/color_fields.rs"));

#[derive(Clone)]
pub struct Theme {
    /// but-ui's semantic colours, which `Theme` derefs to: `theme.muted_foreground`.
    pub colors: ColorTokens,
    /// The colours but-ui has no token for, each worked out in one place.
    pub derived: Derived,
    /// Every token in the other shade, for what a row selected on the solid fill (`inverse`)
    /// draws, as but-ui-react's rows flip `color-scheme` there: its counts, statuses and
    /// commits' glyphs stay legible and keep their meaning.
    pub on_solid: ColorTokens,
    pub ui_font: SharedString,
    pub mono_font: SharedString,
    pub diff_font: SharedString,
    /// A digit's width in `diff_font`, in ems.
    pub diff_digit: f32,
    pub diff_features: FontFeatures,
    pub terminal_font: Font,
    pub terminal_bold: FontWeight,
    pub code_text: f32,
    pub code_line: f32,
    /// The interface scale, 1 at 100%.
    pub scale: f32,
    pub terminal: Colors,
    pub shape: Shape,
    pub sizes: Sizes,
    /// `Some(dark)` while these are GitButler's own colours, which an accent applies to.
    own_colours: Option<bool>,
}

impl Global for Theme {}

impl std::ops::Deref for Theme {
    type Target = ColorTokens;

    fn deref(&self) -> &ColorTokens {
        &self.colors
    }
}

/// The colours but-ui has no token for. Each one here is either worked out from tokens, the
/// same way in every scheme, or picked by the scheme itself; any of them could become a token.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Derived {
    /// `muted` halfway to `muted-nested`: a gray button or a muted row under the pointer.
    pub muted_hover: Hsla,
    /// `primary` with 14% of `foreground` in it: a checked box or radio under the pointer.
    pub primary_checked_hover: Hsla,
    /// `inverse-foreground` at 35% on a dark solid selection and 26% on a light one: an
    /// outline's edge there, as React's `outline-inverted`. Stronger than `input`'s 26% on the
    /// dark fill only, as a faint light edge on it reads fainter than a dark edge on a light one;
    /// 35% reads as the coloured outlines' edges do there.
    pub inverse_input: Hsla,
    /// `inverse-muted-foreground` at 40%: a tree's guide on a solid selection, as
    /// but-ui-react's `--guide-color` there.
    pub inverse_guide: Hsla,
    /// `inverse-foreground` at 8%: the line between two rows of a run of selected ones on a
    /// solid selection, as but-ui-react's `--joined-line` there.
    pub joined_line_on_solid: Hsla,
    /// `success-text` stepped toward `inverse-foreground` until it reads at 4.5:1 on `inverse`:
    /// an added line count on a solid selection.
    pub inverse_success_text: Hsla,
    /// `danger-text`, as `inverse_success_text`: a removed line count on a solid selection.
    pub inverse_danger_text: Hsla,
    /// `warning` stepped toward black or white until it reads at 3:1 on `snackbar`: a warning
    /// snackbar's glyph, beside `snackbar-danger` and `snackbar-success`.
    pub snackbar_warning: Hsla,
    /// A proposal's edge and a suggestion's: GitButler's `amber-7` ramp step, a scheme's
    /// yellow halfway to its background.
    pub warning_edge: Hsla,
    /// A quote's edge and a draft comment's: `blue-7`, a scheme's blue halfway to its
    /// background.
    pub info_edge: Hsla,
    /// An unread row's dot: GitButler's `gray-8`, between the border and subtle text, which no
    /// role names.
    pub unread_dot: Hsla,
    /// `done-text` from the other shade, for glyphs on a solid selection only: it reads at 3:1,
    /// not 4.5:1.
    pub inverse_done_mark: Hsla,
    /// GitHub's added green and removed red, as but.dev's diffs take them: line numbers, a
    /// line's bar, the change marks. A scheme's green and red at 3:1.
    pub diff_added: Hsla,
    pub diff_removed: Hsla,
    /// An added or removed line's ground, opaque.
    pub diff_added_line: Hsla,
    pub diff_removed_line: Hsla,
    /// The words that changed in an added or removed line: `diff_added` and `diff_removed` at
    /// 15%, 20% in dark.
    pub diff_added_words: Hsla,
    pub diff_removed_words: Hsla,
    /// An unchanged line's number in a diff.
    pub diff_line_number: Hsla,
    /// Shiki's GitHub light and dark, as but.dev highlights diffs, in `Token` order.
    syntax: [Hsla; 7],
}

/// What a scheme picks for itself in [`Derived`].
struct Picked {
    warning_edge: Hsla,
    info_edge: Hsla,
    unread_dot: Hsla,
    inverse_done_mark: Hsla,
    diff_added: Hsla,
    diff_removed: Hsla,
    diff_added_line: Hsla,
    diff_removed_line: Hsla,
    diff_line_number: Hsla,
    syntax: [Hsla; 7],
}

impl Derived {
    fn new(c: &ColorTokens, dark: bool, picked: Picked) -> Self {
        let words = if dark { 0.2 } else { 0.15 };
        let on_inverse = |text: Hsla| {
            let [text, ink, ground] = [text, c.inverse_foreground, c.inverse].map(rgb_hex);
            color(readable_on(text, ink, ground))
        };
        let snackbar = rgb_hex(c.snackbar);
        let snackbar_ink = match contrast(snackbar, 0xffffff) > contrast(snackbar, 0x000000) {
            true => 0xffffff,
            false => 0x000000,
        };
        let snackbar_warning = reaching(rgb_hex(c.warning), snackbar_ink, &[snackbar], 3.);
        let Picked {
            warning_edge,
            info_edge,
            unread_dot,
            inverse_done_mark,
            diff_added,
            diff_removed,
            diff_added_line,
            diff_removed_line,
            diff_line_number,
            syntax,
        } = picked;
        Derived {
            muted_hover: color(mix(rgb_hex(c.muted), rgb_hex(c.muted_nested), 0.5)),
            primary_checked_hover: primary_checked_hover(c),
            inverse_input: c.inverse_foreground.opacity(match c.inverse.l < 0.5 {
                true => 0.35,
                false => 0.26,
            }),
            inverse_guide: c.inverse_muted_foreground.opacity(0.4),
            joined_line_on_solid: c.inverse_foreground.opacity(0.08),
            inverse_success_text: on_inverse(c.success_text),
            inverse_danger_text: on_inverse(c.danger_text),
            snackbar_warning: color(snackbar_warning.unwrap_or(snackbar_ink)),
            warning_edge,
            info_edge,
            unread_dot,
            inverse_done_mark,
            diff_added,
            diff_removed,
            diff_added_line,
            diff_removed_line,
            diff_added_words: diff_added.opacity(words),
            diff_removed_words: diff_removed.opacity(words),
            diff_line_number,
            syntax,
        }
    }
}

fn primary_checked_hover(c: &ColorTokens) -> Hsla {
    color(mix(rgb_hex(c.primary), rgb_hex(c.foreground), 0.14))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hue {
    Pop,
    Blue,
    Safe,
    Warn,
    Danger,
    Purple,
    Gray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Highlight {
    Selected,
    SelectedUnfocused,
    /// Selected words in a diff's line, or a line being edited there.
    Words,
    /// A search's matches, and the current one.
    Found,
    FoundHere,
    /// New since it was last read.
    Fresh,
    /// What a comment is about, and while it's pointed at or written.
    Said,
    Saying,
    /// What an unsent comment is about, and while it's pointed at or written.
    Draft,
    SayingDraft,
    /// Blinks over a diff's rows and headers as they're copied.
    Copy,
    /// A diff's line number picked for a selection.
    Picked,
    /// The words that changed on a commit's side of a conflict.
    Incoming,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeSize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    /// `GITBUTLER_GPUI_APPEARANCE=dark|light` overrides the choice, handy for screenshots.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shade {
    Light,
    Dark,
}

/// Corner radii in px as designed, before the interface scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    pub xs: f32,
    pub sm: f32,
    pub control: f32,
    pub card: f32,
    pub popup: f32,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            xs: tokens::RADIUS_XS,
            sm: tokens::RADIUS_SM,
            control: tokens::RADIUS_CONTROL,
            card: tokens::RADIUS_CARD,
            popup: tokens::RADIUS_POPUP,
        }
    }
}

/// Control and bar heights in px as designed, before the interface scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    pub control: f32,
    /// A medium button, a row of them in a dense header.
    pub control_md: f32,
    /// A small button or segment is 4px taller than this.
    pub control_sm: f32,
    /// A bar inside a popup, as its search row or footer.
    pub bar: f32,
    /// Added to a control's side padding; 0 at Balanced.
    pub padding: f32,
}

impl Default for Sizes {
    fn default() -> Self {
        Sizes {
            control: tokens::SIZE_CONTROL,
            control_md: tokens::SIZE_CONTROL_MD,
            control_sm: tokens::SIZE_CONTROL_SM,
            bar: tokens::SIZE_BAR,
            padding: 0.,
        }
    }
}

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

/// but-ui's presets: each one's name, corners and density.
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
    #[default]
    GitButler,
    /// The light or dark theme the user's Ghostty config sets, by the shade.
    Ghostty,
    /// One Ghostty theme by name, light or dark by its background.
    Named(String),
}

impl ColorScheme {
    pub fn label(&self) -> &str {
        match self {
            ColorScheme::GitButler => "GitButler",
            ColorScheme::Ghostty => "Ghostty",
            ColorScheme::Named(name) => name,
        }
    }
}

/// The interface zoom, in percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UiScale(u16);

impl Default for UiScale {
    fn default() -> Self {
        Self(100)
    }
}

impl UiScale {
    /// The ⌘+ and ⌘− steps, as Chrome's.
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

    fn factor(self) -> f32 {
        f32::from(self.clamp(Self::STEPS[0], Self::STEPS[8]).0) / 100.
    }

    pub fn larger(self) -> Self {
        (Self::STEPS.into_iter())
            .find(|&step| step > self)
            .unwrap_or(Self::STEPS[8])
    }

    pub fn smaller(self) -> Self {
        (Self::STEPS.into_iter().rev())
            .find(|&step| step < self)
            .unwrap_or(Self::STEPS[0])
    }
}

impl CodeSize {
    /// The code size and line height, in px.
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

    /// `digit` gives a family's digit width in ems, or `None` when the machine hasn't got it,
    /// which keeps the current font.
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

    pub fn with_ligatures(mut self, on: bool) -> Self {
        let off = [("liga".to_owned(), 0), ("calt".to_owned(), 0)];
        self.diff_features = FontFeatures(if on { vec![] } else { off.to_vec() }.into());
        self
    }

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

    /// `custom` is `#rrggbb` or `#rgb`, for [`Accent::Custom`].
    pub fn with_accent(mut self, accent: Accent, custom: &str) -> Self {
        let Some(dark) = self.own_colours else {
            return self;
        };
        for (colors, dark) in [(&mut self.colors, dark), (&mut self.on_solid, !dark)] {
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
            // The soft fills are see-through, so these carry alpha.
            for (token, value) in family {
                if let Some(field) = colors.named(token) {
                    *field = rgba(value).into();
                }
            }
            // The washes are the primary at but-ui's strengths, whichever it is.
            let [wash, strong] = WASHES;
            let primary = colors.primary;
            (colors.primary_wash, colors.primary_wash_strong) =
                (primary.opacity(wash), primary.opacity(strong));
        }
        self.derived.primary_checked_hover = primary_checked_hover(&self.colors);
        self
    }

    /// Draws GitButler's colours again, so call it before any other `with_*`.
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
        if let Some((_, [xs, sm, control, card, popup])) =
            (tokens::CORNERS.iter()).find(|(name, _)| *name == corners.name())
        {
            self.shape = Shape {
                xs: *xs,
                sm: *sm,
                control: *control,
                card: *card,
                popup: *popup,
            };
        }
        self
    }

    pub fn with_density(mut self, density: Density) -> Self {
        if let Some((_, [control, control_md, control_sm, bar, padding])) =
            (tokens::DENSITIES.iter()).find(|(name, _)| *name == density.name())
        {
            self.sizes = Sizes {
                control: *control,
                control_md: *control_md,
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

    pub fn scaled(&self, n: f32) -> Pixels {
        Pixels::from(n * self.scale)
    }

    /// Whether these are GitButler's own colours, which an accent and ground apply to.
    pub fn own_colours(&self) -> bool {
        self.own_colours.is_some()
    }

    /// This theme as a row selected on the solid fill draws its marks, as but-ui-react's rows
    /// flip their colour scheme there: each mark colour, and each badge's fill and text, in
    /// its token's other value (`on_solid`). A component built with it on such a row needs
    /// nothing else to know.
    pub fn for_solid(&self) -> Theme {
        let (mut theme, flip) = (self.clone(), &self.on_solid);
        let c = &mut theme.colors;
        c.foreground = flip.foreground;
        c.neutral_soft = flip.neutral_soft;
        c.primary_soft = flip.primary_soft;
        c.primary_soft_foreground = flip.primary_soft_foreground;
        c.info_soft = flip.info_soft;
        c.info_soft_foreground = flip.info_soft_foreground;
        c.success_soft = flip.success_soft;
        c.success_soft_foreground = flip.success_soft_foreground;
        c.warning_soft = flip.warning_soft;
        c.warning_soft_foreground = flip.warning_soft_foreground;
        c.danger_soft = flip.danger_soft;
        c.danger_soft_foreground = flip.danger_soft_foreground;
        c.done_soft = flip.done_soft;
        c.done_soft_foreground = flip.done_soft_foreground;
        c.muted_foreground = flip.muted_foreground;
        c.subtle_foreground = flip.subtle_foreground;
        c.info_text = flip.info_text;
        c.warning_text = flip.warning_text;
        c.success_text = flip.success_text;
        c.danger_text = flip.danger_text;
        c.done_text = flip.done_text;
        c.info_mark = flip.info_mark;
        c.warning_mark = flip.warning_mark;
        c.success_mark = flip.success_mark;
        c.danger_mark = flip.danger_mark;
        c.done_mark = flip.done_mark;
        c.commit_local = flip.commit_local;
        c.commit_remote = flip.commit_remote;
        c.commit_upstream = flip.commit_upstream;
        c.commit_integrated = flip.commit_integrated;
        theme
    }

    pub fn get(cx: &App) -> &Theme {
        cx.global::<Theme>()
    }

    /// A scheme without a background and foreground, as a named theme that's gone, draws as
    /// GitButler's.
    pub fn new(shade: Shade, scheme: &ColorScheme, code: CodeSize, ghostty: &Ghostty) -> Self {
        let dark = shade == Shade::Dark;
        // The environment overrides the settings, handy for screenshots.
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

    pub fn gitbutler(dark: bool, code: CodeSize) -> Self {
        Self::gitbutler_on(dark, code, Ground::Gray)
    }

    fn gitbutler_on(dark: bool, code: CodeSize, ground: Ground) -> Self {
        // Only the tokens that differ from gray's.
        let on_ground = (tokens::GROUNDS.iter())
            .find(|(name, _)| *name == ground.name())
            .map_or(&[][..], |(_, colors)| *colors);
        let shade = |[light, dark_value]: [u32; 2]| if dark { dark_value } else { light };
        let tokens_in = |dark: bool| {
            ColorTokens::from_tokens(|name, gray| {
                let [light, dark_value] = (on_ground.iter())
                    .find(|(token, _)| *token == name)
                    .map_or(gray, |(_, colors)| *colors);
                rgba(if dark { dark_value } else { light }).into()
            })
        };
        let (colors, on_solid) = (tokens_in(dark), tokens_in(!dark));
        let pick = |light: u32, dark_value: u32| color(shade([light, dark_value]));
        let (code_text, code_line) = code.metrics();
        let [done_light, done_dark] = tokens::DONE_TEXT;
        let derived = Derived::new(
            &colors,
            dark,
            Picked {
                warning_edge: rgba(shade(tokens::AMBER_7)).into(),
                info_edge: rgba(shade(tokens::BLUE_7)).into(),
                unread_dot: rgba(shade(tokens::GRAY_8)).into(),
                inverse_done_mark: rgba(shade([done_dark, done_light])).into(),
                // GitHub's light and dark, which but.dev's diffs take their colours from.
                diff_added: pick(0x28a745, 0x34d058),
                diff_removed: pick(0xd73a49, 0xea4a5a),
                diff_added_line: pick(0xeef7ee, 0x2c3f35),
                diff_removed_line: pick(0xffeeed, 0x413134),
                diff_line_number: pick(0x696c70, 0x999da1),
                syntax: [
                    pick(0x6a737d, 0x6a737d),
                    pick(0x032f62, 0x9ecbff),
                    pick(0x005cc5, 0x79b8ff),
                    pick(0xd73a49, 0xf97583),
                    pick(0x6f42c1, 0xb392f0),
                    pick(0x6f42c1, 0xb392f0),
                    pick(0x22863a, 0x85e89d),
                ],
            },
        );
        Theme {
            code_text,
            code_line,
            scale: 1.,
            colors,
            derived,
            on_solid,
            // Embedded by `ui::fonts`.
            ui_font: "Inter".into(),
            mono_font: "GeistMono Nerd Font Mono".into(),
            diff_font: "GeistMono Nerd Font Mono".into(),
            diff_digit: 0.6,
            diff_features: FontFeatures::default(),
            terminal_font: gpui::font("GeistMono Nerd Font Mono"),
            terminal_bold: FontWeight::SEMIBOLD,
            terminal: Colors::default(),
            own_colours: Some(dark),
            shape: Shape::default(),
            sizes: Sizes::default(),
        }
        .with_own_terminal(dark)
    }

    /// ANSI black and white are its grays, so both read on the ground.
    fn with_own_terminal(mut self, dark: bool) -> Self {
        let c = self.colors;
        let [bg, fg] = [c.background, c.foreground].map(rgb_hex);
        let black = rgb_hex(if dark { c.border_subtle } else { c.foreground });
        let accents = [
            c.danger_text,
            c.success_text,
            c.warning_text,
            c.info_text,
            c.done_text,
            c.primary_text,
        ]
        .map(rgb_hex);
        let normal = (std::iter::once(black))
            .chain(accents)
            .chain([rgb_hex(c.muted_foreground)]);
        let bright = (std::iter::once(rgb_hex(c.subtle_foreground)))
            .chain(accents.map(|accent| mix(accent, fg, 0.25)))
            .chain([fg]);
        self.terminal = Colors {
            background: Some(rgb_color(bg)),
            foreground: Some(rgb_color(fg)),
            cursor: Some(rgb_color(fg)),
            selection_background: Some(rgb_color(mix(bg, rgb_hex(c.info_text), 0.3))),
            selection_foreground: None,
            palette: (0..).zip(normal.chain(bright).map(rgb_color)).collect(),
        };
        self
    }

    fn painted(self, palette: Colors) -> Self {
        let (Some(bg), Some(fg)) = (palette.background, palette.foreground) else {
            return self;
        };
        let (bg, fg) = (hex(bg), hex(fg));
        let (colors, derived) = palette_colors(bg, fg, &palette);
        // The solid fill is the foreground, so on it the palette reads with the two swapped.
        let (on_solid, _) = palette_colors(fg, bg, &palette);
        Theme {
            colors,
            derived,
            on_solid,
            terminal: palette,
            own_colours: None,
            ..self
        }
    }

    pub fn token(&self, token: Token) -> Hsla {
        let [comment, string, constant, keyword, function, ty, tag] = self.derived.syntax;
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

    /// The focused field's ring made opaque, so it looks the same drawn over a card's edge.
    pub fn focus_edge(&self) -> Hsla {
        self.background.blend(self.ring)
    }

    /// `selected-inactive` made opaque on a card, for a sticky header over its content.
    pub fn selected_on_card(&self) -> Hsla {
        self.background.blend(self.selected_inactive)
    }

    /// `top` at half strength over `ground`, made solid: how a disabled control looks once
    /// it's faded as one picture, as CSS's `opacity` on the control draws it. GPUI fades each
    /// layer on its own instead, so a mark over a fill would let the fill show through it.
    pub fn disabled_on(top: Hsla, ground: Hsla) -> Hsla {
        ground.blend(top.opacity(0.5))
    }

    /// A passage set off from the page, not a panel.
    pub fn passage(&self) -> Hsla {
        self.background.blend(self.canvas.opacity(0.5))
    }

    pub fn hue(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_text,
            Hue::Blue => self.info_text,
            Hue::Safe => self.success_text,
            Hue::Warn => self.warning_text,
            Hue::Danger => self.danger_text,
            Hue::Purple => self.done_text,
            Hue::Gray => self.muted_foreground,
        }
    }

    /// The hue as a mark: an icon, a dot or a light with no words beside it.
    pub fn mark(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_mark,
            Hue::Blue => self.info_mark,
            Hue::Safe => self.success_mark,
            Hue::Warn => self.warning_mark,
            Hue::Danger => self.danger_mark,
            Hue::Purple => self.done_mark,
            Hue::Gray => self.muted_foreground,
        }
    }

    /// The hue's text and icons on its own soft or surface fill.
    pub fn on_soft(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_soft_foreground,
            Hue::Blue => self.info_soft_foreground,
            Hue::Safe => self.success_soft_foreground,
            Hue::Warn => self.warning_soft_foreground,
            Hue::Danger => self.danger_soft_foreground,
            Hue::Purple => self.done_soft_foreground,
            Hue::Gray => self.muted_foreground,
        }
    }

    pub fn soft(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_soft,
            Hue::Blue => self.info_soft,
            Hue::Safe => self.success_soft,
            Hue::Warn => self.warning_soft,
            Hue::Danger => self.danger_soft,
            Hue::Purple => self.done_soft,
            Hue::Gray => self.muted,
        }
    }

    pub fn soft_hover(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_soft_hover,
            Hue::Blue => self.info_soft_hover,
            Hue::Safe => self.success_soft_hover,
            Hue::Warn => self.warning_soft_hover,
            Hue::Danger => self.danger_soft_hover,
            Hue::Purple => self.done_soft_hover,
            Hue::Gray => self.derived.muted_hover,
        }
    }

    /// A coloured outline button's edge: its hue's text at 40%, a touch stronger than
    /// `input`'s neutral edge, as a coloured line reads fainter than a gray one.
    pub fn outline_edge(&self, hue: Hue) -> Hsla {
        self.hue(hue).opacity(0.4)
    }

    /// A coloured outline button's text, edge and hover on a solid selection: its hue in the
    /// other scheme's values, as but-ui-react's Button flips its scheme there. On a dark fill
    /// the edge is 55% rather than 40%, as `inverse_input` is stronger there, and blue's 70%,
    /// as it reads darker than amber and green and so matches theirs. A light fill keeps 40%.
    /// Only info, success and warning are coloured; anything else takes the inverted ghost's.
    pub fn outline_on_solid(&self, hue: Option<Hue>) -> (Hsla, Hsla, Hsla) {
        let flip = &self.on_solid;
        let (text, hover) = match hue {
            Some(Hue::Blue) => (flip.info_text, flip.info_outline_hover),
            Some(Hue::Warn) => (flip.warning_text, flip.warning_outline_hover),
            Some(Hue::Safe) => (flip.success_text, flip.success_outline_hover),
            _ => (self.inverse_foreground, self.inverse_ghost_hover),
        };
        let edge = match (hue, self.inverse.l < 0.5) {
            (_, false) => 0.4,
            (Some(Hue::Blue), true) => 0.7,
            (_, true) => 0.55,
        };
        (text, text.opacity(edge), hover)
    }

    /// Under the pointer, a coloured outline button's ground: a faint see-through tint of its
    /// hue, as Radix's outline buttons hover. Only info, success and warning draw one.
    pub fn outline_hover(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Blue => self.info_outline_hover,
            Hue::Safe => self.success_outline_hover,
            Hue::Warn => self.warning_outline_hover,
            _ => self.hover,
        }
    }

    pub fn surface(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_surface,
            Hue::Blue => self.info_surface,
            Hue::Safe => self.success_surface,
            Hue::Warn => self.warning_surface,
            Hue::Danger => self.danger_surface,
            Hue::Purple => self.done_surface,
            Hue::Gray => self.muted,
        }
    }

    pub fn surface_hover(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_surface_hover,
            Hue::Blue => self.info_surface_hover,
            Hue::Safe => self.success_surface_hover,
            Hue::Warn => self.warning_surface_hover,
            Hue::Danger => self.danger_surface_hover,
            Hue::Purple => self.done_surface_hover,
            Hue::Gray => self.derived.muted_hover,
        }
    }

    pub fn wash(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_wash,
            Hue::Blue => self.info_wash,
            Hue::Safe => self.success_wash,
            Hue::Warn => self.warning_wash,
            Hue::Danger => self.danger_wash,
            Hue::Purple => self.done_wash,
            Hue::Gray => self.hover,
        }
    }

    pub fn wash_strong(&self, hue: Hue) -> Hsla {
        match hue {
            Hue::Pop => self.primary_wash_strong,
            Hue::Blue => self.info_wash_strong,
            Hue::Safe => self.success_wash_strong,
            Hue::Warn => self.warning_wash_strong,
            Hue::Danger => self.danger_wash_strong,
            Hue::Purple => self.done_wash_strong,
            Hue::Gray => self.selected_inactive,
        }
    }

    /// `FoundHere` is solid, with [`ColorTokens::warning_foreground`] as its text.
    pub fn highlight(&self, highlight: Highlight) -> Hsla {
        match highlight {
            Highlight::Selected | Highlight::Fresh | Highlight::Copy => self.primary_wash_strong,
            Highlight::SelectedUnfocused | Highlight::Picked => self.primary_wash,
            Highlight::Words | Highlight::SayingDraft | Highlight::Incoming => {
                self.info_wash_strong
            }
            Highlight::Draft => self.info_wash,
            Highlight::Found | Highlight::Saying => self.warning_wash_strong,
            Highlight::Said => self.warning_wash,
            Highlight::FoundHere => self.warning,
        }
    }
}

/// Every token worked out from a terminal palette, drawn on `bg` in `fg`, with its ANSI colours
/// for the hues.
fn palette_colors(bg: u32, fg: u32, palette: &Colors) -> (ColorTokens, Derived) {
    let dark = contrast(bg, 0xffffff) > contrast(bg, 0x000000);
    let given = |index: u8| {
        (palette.palette.iter())
            .find(|(at, _)| *at == index)
            .map(|(_, color)| hex(*color))
    };
    let ansi = |index: u8| {
        given(index)
            .or_else(|| GHOSTTY_ANSI.get(usize::from(index)).copied())
            .unwrap_or(fg)
    };
    let canvas = mix(bg, 0x000000, if dark { 0.25 } else { 0.05 });
    // As but-ui's opaque grays: muted, halfway on to muted-nested under the pointer.
    let gray = |share: f32| mix(bg, fg, share);
    let muted = gray(if dark { 0.09 } else { 0.06 });
    let muted_hover = gray(if dark { 0.105 } else { 0.077 });
    let muted_nested = mix(muted, muted_hover, 2.);
    let grounds = [bg, canvas, muted, muted_hover];
    let ink = if dark { 0xffffff } else { 0x000000 };
    let soft = |hue: u32| mix(bg, hue, if dark { 0.18 } else { 0.12 });
    let soft_hover = |hue: u32| mix(bg, hue, if dark { 0.22 } else { 0.17 });
    let text_on = |from: u32, ratio: f32, also: &[u32]| {
        let grounds: Vec<u32> = grounds.iter().chain(also).copied().collect();
        (reaching(from, fg, &grounds, ratio))
            .or_else(|| reaching(fg, ink, &grounds, ratio))
            .unwrap_or(ink)
    };
    let text = |from: u32, ratio: f32| text_on(from, ratio, &[]);
    let hue_text = |hue: u32| text_on(hue, 4.5, &[soft(hue), soft_hover(hue)]);
    // A snackbar's ground is the foreground.
    let snackbar_ink = match contrast(fg, 0xffffff) > contrast(fg, 0x000000) {
        true => 0xffffff,
        false => 0x000000,
    };
    // The first of cyan, blue, magenta and their bright ones that isn't gray, so the accent
    // stays a colour. Not the cursor, which is often yellow or red, as warn and danger are.
    let pop = [6, 4, 5, 14, 12, 13]
        .map(given)
        .into_iter()
        .flatten()
        .find(|&hue| chroma(hue) >= 40.)
        .unwrap_or_else(|| ansi(6));
    // At its own hue, as stepping toward a gray foreground would leave a pale one gray.
    let also = [soft(pop), soft_hover(pop)];
    let pop_text = toned(pop, ink, &[&grounds[..], &also].concat(), 4.5).unwrap_or(ink);
    let ink_on = |fill: u32| {
        [0xffffff, bg, 0x000000]
            .into_iter()
            .find(|&ink| contrast(ink, fill) >= 4.5)
            .unwrap_or(0x000000)
    };
    let pressed = |fill: u32| mix(fill, 0x000000, 0.08);
    let washed = |hue: u32, strength: usize| {
        let share = WASHES.get(strength).copied().unwrap_or(0.);
        color(hue).opacity(share)
    };
    let subtle = text(mix(bg, fg, 0.45), 3.);
    let [red, green, yellow, blue, magenta] = [1, 2, 3, 4, 5].map(ansi);
    let [success, info, danger, done] = [green, blue, red, magenta].map(hue_text);
    let warning = hue_text(yellow);
    let [added, removed] = [green, red].map(|hue| text(hue, 3.));
    let syntax = [
        subtle,
        text(green, 4.5),
        text(ansi(6), 4.5),
        text(magenta, 4.5),
        text(blue, 4.5),
        text(yellow, 4.5),
        text(red, 4.5),
    ]
    .map(color);
    let category = |hue: u32| color(mix(bg, hue, 0.55));
    let veil = |share: f32| color(fg).opacity(share);
    let border = mix(bg, fg, 0.3);
    // A scheme's text already reads on the soft fills too, and its soft grounds are opaque
    // already, so they serve as its surfaces as well; its marks only need 3:1.
    let colors = ColorTokens {
        background: color(bg),
        canvas: color(canvas),
        card: color(bg),
        card_foreground: color(fg),
        popover: color(if dark { muted } else { bg }),
        popover_foreground: color(fg),
        popover_border: color(ink).opacity(if dark { 0.16 } else { 0.12 }),
        foreground: color(fg),
        muted_foreground: color(text(mix(bg, fg, 0.65), 4.5)),
        subtle_foreground: color(subtle),
        secondary: veil(0.08),
        secondary_foreground: color(fg),
        muted: color(muted),
        muted_nested: color(muted_nested),
        disabled: color(muted),
        border_hover: color(mix(bg, fg, 0.45)),
        border: color(border),
        border_subtle: color(mix(bg, fg, 0.18)),
        hover: veil(0.05),
        selected: color(fg),
        selected_foreground: color(bg),
        selected_inactive: veil(0.1),
        ring: veil(0.7),
        input: veil(0.26),
        scrollbar_thumb: veil(0.15),
        scrollbar_thumb_hover: veil(0.25),
        overlay: match dark {
            true => color(0x000000).opacity(0.5),
            false => color(gray(0.15)).opacity(0.45),
        },
        shadow: color(0x000000).opacity(if dark { 0.5 } else { 0.1 }),
        inverse: color(fg),
        inverse_hover: color(mix(bg, fg, 0.89)),
        inverse_foreground: color(bg),
        inverse_muted_foreground: color(bg).opacity(0.6),
        inverse_subtle_foreground: color(bg).opacity(0.5),
        inverse_ghost_hover: color(bg).opacity(0.05),
        inverse_soft: color(bg).opacity(0.15),
        inverse_soft_hover: color(bg).opacity(0.22),
        neutral_soft: veil(if dark { 0.14 } else { 0.1 }),
        primary: color(pop),
        primary_hover: color(pressed(pop)),
        primary_foreground: color(ink_on(pop)),
        primary_text: color(pop_text),
        primary_mark: color(toned(pop, ink, &grounds, 3.).unwrap_or(pop_text)),
        primary_soft: color(soft(pop)),
        primary_soft_hover: color(soft_hover(pop)),
        primary_soft_foreground: color(pop_text),
        primary_surface: color(soft(pop)),
        primary_surface_hover: color(soft_hover(pop)),
        primary_wash: washed(pop, 0),
        primary_wash_strong: washed(pop, 1),
        info: color(blue),
        info_hover: color(pressed(blue)),
        info_foreground: color(ink_on(blue)),
        info_text: color(info),
        info_mark: color(text(blue, 3.)),
        info_soft: color(soft(blue)),
        info_soft_hover: color(soft_hover(blue)),
        info_soft_foreground: color(info),
        info_surface: color(soft(blue)),
        info_surface_hover: color(soft_hover(blue)),
        info_outline_hover: washed(blue, 0),
        info_wash: washed(blue, 0),
        info_wash_strong: washed(blue, 1),
        success: color(green),
        success_hover: color(pressed(green)),
        success_foreground: color(ink_on(green)),
        success_text: color(success),
        success_mark: color(text(green, 3.)),
        success_soft: color(soft(green)),
        success_soft_hover: color(soft_hover(green)),
        success_soft_foreground: color(success),
        success_surface: color(soft(green)),
        success_surface_hover: color(soft_hover(green)),
        success_outline_hover: washed(green, 0),
        success_wash: washed(green, 0),
        success_wash_strong: washed(green, 1),
        warning: color(yellow),
        warning_hover: color(pressed(yellow)),
        warning_foreground: color(ink_on(yellow)),
        warning_text: color(warning),
        warning_mark: color(text(yellow, 3.)),
        warning_soft: color(soft(yellow)),
        warning_soft_hover: color(soft_hover(yellow)),
        warning_soft_foreground: color(warning),
        warning_surface: color(soft(yellow)),
        warning_surface_hover: color(soft_hover(yellow)),
        warning_outline_hover: washed(yellow, 0),
        warning_wash: washed(yellow, 0),
        warning_wash_strong: washed(yellow, 1),
        danger: color(red),
        danger_hover: color(pressed(red)),
        danger_foreground: color(ink_on(red)),
        danger_text: color(danger),
        danger_mark: color(text(red, 3.)),
        danger_soft: color(soft(red)),
        danger_soft_hover: color(soft_hover(red)),
        danger_soft_foreground: color(danger),
        danger_surface: color(soft(red)),
        danger_surface_hover: color(soft_hover(red)),
        danger_wash: washed(red, 0),
        danger_wash_strong: washed(red, 1),
        done: color(magenta),
        done_hover: color(pressed(magenta)),
        done_foreground: color(ink_on(magenta)),
        done_text: color(done),
        done_mark: color(text(magenta, 3.)),
        done_soft: color(soft(magenta)),
        done_soft_hover: color(soft_hover(magenta)),
        done_soft_foreground: color(done),
        done_surface: color(soft(magenta)),
        done_surface_hover: color(soft_hover(magenta)),
        done_wash: washed(magenta, 0),
        done_wash_strong: washed(magenta, 1),
        snackbar: color(fg),
        snackbar_border: gpui::transparent_black(),
        snackbar_foreground: color(bg),
        snackbar_muted_foreground: color(bg).opacity(0.6),
        snackbar_danger: color(reaching(red, snackbar_ink, &[fg], 3.).unwrap_or(snackbar_ink)),
        snackbar_success: color(reaching(green, snackbar_ink, &[fg], 3.).unwrap_or(snackbar_ink)),
        snackbar_divider: color(mix(bg, fg, 0.18)).opacity(0.3),
        snackbar_hover: color(bg).opacity(0.05),
        change_status_addition: color(success),
        change_status_modification: color(info),
        change_status_deletion: color(danger),
        change_status_rename: color(done),
        commit_local: color(subtle),
        commit_remote: color(pop_text),
        commit_upstream: color(warning),
        commit_integrated: color(done),
        diff_added: color(added),
        diff_removed: color(removed),
        diff_added_muted: color(green).opacity(0.12),
        diff_added_emphasis: color(green).opacity(0.28),
        diff_removed_muted: color(red).opacity(0.12),
        diff_removed_emphasis: color(red).opacity(0.28),
        syntax_comment: syntax[0],
        syntax_string: syntax[1],
        syntax_constant: syntax[2],
        syntax_keyword: syntax[3],
        syntax_function: syntax[4],
        syntax_type: syntax[5],
        syntax_tag: syntax[6],
        category_1: category(ansi(6)),
        category_2: category(magenta),
        category_3: category(green),
        category_4: category(yellow),
        category_5: category(red),
        category_6: category(blue),
    };
    let derived = Derived::new(
        &colors,
        dark,
        Picked {
            warning_edge: color(mix(bg, yellow, 0.5)),
            info_edge: color(mix(bg, blue, 0.5)),
            unread_dot: color(mix(bg, fg, 0.4)),
            inverse_done_mark: color(readable_on(done, bg, fg)),
            diff_added: color(added),
            diff_removed: color(removed),
            diff_added_line: color(soft(green)),
            diff_removed_line: color(soft(red)),
            diff_line_number: color(subtle),
            syntax,
        },
    );
    (colors, derived)
}

/// but-ui's `--but-opacity-wash(-strong)`, read from its primary washes' alpha.
const WASHES: [f32; 2] = [
    alpha(tokens::PRIMARY_WASH),
    alpha(tokens::PRIMARY_WASH_STRONG),
];

const fn alpha([light, _]: [u32; 2]) -> f32 {
    (light & 0xff) as f32 / 255.
}

/// Ghostty's default 16 colours, for those a theme leaves unset.
const GHOSTTY_ANSI: [u32; 16] = [
    0x1d1f21, 0xcc6666, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xc5c8c6, 0x666666,
    0xd54e53, 0xb9ca4a, 0xe7c547, 0x7aa6da, 0xc397d8, 0x70c0b1, 0xeaeaea,
];

/// The custom accent's tokens as `0xRRGGBBAA`, computed from `pick` as but-ui's tokens.css
/// does: in OKLCH, clipped into sRGB as a browser does.
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

/// `#rrggbb` or `#rgb`, the `#` optional, as `0xRRGGBB`.
pub fn parse_hex(text: &str) -> Option<u32> {
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

/// Lightness, chroma and hue in degrees.
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

/// Hue in degrees; each channel is clipped into sRGB.
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

/// The first of 20 steps from `from` toward `toward` that reads at `ratio` on every ground.
fn reaching(from: u32, toward: u32, grounds: &[u32], ratio: f32) -> Option<u32> {
    (0..=20)
        .map(|step| mix(from, toward, step as f32 / 20.))
        .find(|&color| {
            grounds
                .iter()
                .all(|&ground| contrast(color, ground) >= ratio)
        })
}

/// Like [`reaching`], but moving only the lightness, toward `ink`'s.
fn toned(from: u32, ink: u32, grounds: &[u32], ratio: f32) -> Option<u32> {
    let (from, target) = (color(from), color(ink).l);
    (0..=20)
        .map(|step| {
            let l = from.l + (target - from.l) * step as f32 / 20.;
            rgb_hex(Hsla { l, ..from })
        })
        .find(|&color| {
            grounds
                .iter()
                .all(|&ground| contrast(color, ground) >= ratio)
        })
}

/// How far `hex` is from a gray, 0 to 255.
fn chroma(hex: u32) -> f32 {
    let [r, g, b] = channels(hex);
    r.max(g).max(b) - r.min(g).min(b)
}

fn readable_on(accent: u32, ink: u32, ground: u32) -> u32 {
    reaching(accent, ink, &[ground], 4.5).unwrap_or(ink)
}

fn rgb_color(hex: u32) -> RgbColor {
    let [_, r, g, b] = hex.to_be_bytes();
    RgbColor { r, g, b }
}

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
        Accent, Appearance, CodeSize, ColorScheme, Hue, Shade, Theme, WASHES, channels, chroma,
        contrast, custom_accent, rgb_hex,
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

    fn ghostty_theme(colors: Colors, choice: Shade) -> Theme {
        let ghostty = Ghostty {
            light: colors.clone(),
            dark: colors,
            ..Ghostty::default()
        };
        Theme::new(choice, &ColorScheme::Ghostty, CodeSize::Medium, &ghostty)
    }

    #[test]
    fn a_pale_accent_keeps_its_hue() {
        // 3024 Day, whose cyan reads at under 2:1 on its ground.
        let colors = palette(
            0xf7f7f7,
            0x4a4543,
            [
                0x090300, 0xdb2d20, 0x01a252, 0xcaba00, 0x01a0e4, 0xa16a94, 0x8fbece, 0xa5a2a2,
            ],
        );
        let theme = ghostty_theme(colors, Shade::Light);
        let cyan = gpui::Hsla::from(gpui::rgb(0x8fbece));
        assert!(
            chroma(rgb_hex(theme.primary_text)) > 60.,
            "the cyan is darkened, not grayed: {:06x}",
            rgb_hex(theme.primary_text)
        );
        assert!(
            (theme.primary_text.h - cyan.h).abs() < 0.02,
            "the accent is still the scheme's cyan: {:06x}",
            rgb_hex(theme.primary_text)
        );
    }

    #[test]
    fn a_gray_cyan_gives_way_to_a_colour() {
        let colors = palette(
            0xfdf6e3,
            0x657b83,
            [
                0x073642, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x888888, 0xeee8d5,
            ],
        );
        let theme = ghostty_theme(colors, Shade::Light);
        let blue = gpui::Hsla::from(gpui::rgb(0x268bd2));
        assert!(
            (theme.primary_text.h - blue.h).abs() < 0.02,
            "the blue stands in for the gray cyan: {:06x}",
            rgb_hex(theme.primary_text)
        );
        assert_eq!(
            rgb_hex(theme.primary),
            0x268bd2,
            "buttons fill with the blue as the scheme gives it"
        );
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
            let theme = ghostty_theme(colors, Shade::Dark);
            for ground in [theme.background, theme.canvas].map(rgb_hex) {
                for (what, ink, ratio) in [
                    ("secondary text", theme.muted_foreground, 4.5),
                    ("faint text", theme.subtle_foreground, 3.),
                    ("danger", theme.danger_text, 4.5),
                    ("pop", theme.primary_text, 4.5),
                ] {
                    let reads = contrast(rgb_hex(ink), ground);
                    assert!(
                        reads >= ratio,
                        "{name}: {what} reads at {reads}, under {ratio}"
                    );
                }
            }
            let label = contrast(rgb_hex(theme.primary_foreground), rgb_hex(theme.primary));
            assert!(
                label >= 4.5,
                "{name}: a pop button's label reads at {label}"
            );
            let glyph = contrast(
                rgb_hex(theme.derived.snackbar_warning),
                rgb_hex(theme.snackbar),
            );
            assert!(
                glyph >= 3.,
                "{name}: a warning snackbar's glyph reads at {glyph}"
            );
        }
    }

    #[test]
    fn a_custom_accent_draws_as_the_web_draws_it() {
        // What but-ui's build resolves the custom accent to, as a browser would.
        let samples: serde_json::Value = serde_json::from_str(include_str!(
            "../../../packages/but-ui/tokens/accent-samples.json"
        ))
        .unwrap();
        let mut off = Vec::new();
        for sample in samples["samples"].as_array().unwrap() {
            let pick = sample["color"].as_str().unwrap();
            for (mode, dark) in [("light", false), ("dark", true)] {
                for (token, drawn) in custom_accent(pick, dark) {
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
        for appearance in [Shade::Light, Shade::Dark] {
            let theme = Theme::new(
                appearance,
                &ColorScheme::GitButler,
                CodeSize::Medium,
                &Ghostty::default(),
            );
            let glyph = contrast(
                rgb_hex(theme.derived.snackbar_warning),
                rgb_hex(theme.snackbar),
            );
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
            ("catppuccin", ghostty_theme(catppuccin, Shade::Dark)),
            ("solarized", ghostty_theme(solarized, Shade::Light)),
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
                // Text on a hue's own fill takes its on-soft colour; `hue` is for the plain grounds.
                let ink = rgb_hex(theme.on_soft(hue));
                // Soft fills are see-through, so each is checked on what it sits on, as
                // but-ui checks.
                for (what, ground, unders) in [
                    (
                        "soft",
                        theme.soft(hue),
                        vec![theme.background, theme.canvas],
                    ),
                    (
                        "soft under the pointer",
                        theme.soft_hover(hue),
                        vec![theme.background],
                    ),
                    // Opaque, so the ground under it doesn't change it.
                    ("surface", theme.surface(hue), vec![theme.background]),
                    (
                        "surface under the pointer",
                        theme.surface_hover(hue),
                        vec![theme.background],
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
            ("teal", teal.primary_wash, teal.primary_wash_strong),
            ("purple", purple.primary_wash, purple.primary_wash_strong),
        ] {
            let ground = if what == "teal" {
                teal.primary
            } else {
                purple.primary
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
            teal.primary_wash, purple.primary_wash,
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
            ("catppuccin", ghostty_theme(catppuccin, Shade::Dark), true),
            ("solarized", ghostty_theme(solarized, Shade::Light), false),
        ];
        for (name, theme, keeps_hue) in themes {
            let ground = rgb_hex(theme.inverse);
            let label = contrast(rgb_hex(theme.inverse_foreground), ground);
            for (what, ink) in [
                ("added", theme.derived.inverse_success_text),
                ("removed", theme.derived.inverse_danger_text),
            ] {
                let reads = contrast(rgb_hex(ink), ground);
                assert!(
                    reads >= label.min(4.5),
                    "{name}: {what} reads at {reads} on the selection"
                );
                assert_eq!(
                    ink != theme.inverse_foreground,
                    keeps_hue,
                    "{name}: {what} keeps its hue only where it can read"
                );
            }
        }
    }

    #[test]
    fn a_rows_marks_read_on_a_solid_selection() {
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
                Theme::gitbutler(false, CodeSize::Medium).with_accent(Accent::Purple, ""),
            ),
            ("catppuccin", ghostty_theme(catppuccin, Shade::Dark)),
            ("solarized", ghostty_theme(solarized, Shade::Light)),
        ];
        for (name, theme) in themes {
            let fill = rgb_hex(theme.inverse);
            let on = &theme.on_solid;
            for (what, ink, ratio) in [
                ("muted text", on.muted_foreground, 4.5),
                ("subtle text", on.subtle_foreground, 3.),
                ("info", on.info_text, 3.),
                ("warning", on.warning_text, 3.),
                ("success", on.success_text, 3.),
                ("danger", on.danger_text, 3.),
                ("done", on.done_text, 3.),
                ("primary", on.primary_text, 3.),
            ] {
                let reads = contrast(rgb_hex(ink), fill);
                assert!(
                    reads >= ratio,
                    "{name}: {what} reads at {reads} on a solid selection"
                );
            }
        }
    }

    #[test]
    fn a_scheme_colours_the_app_and_the_terminals_whatever_the_appearance() {
        let solarized = palette(0xfdf6e3, 0x657b83, [0x073642; 8]);
        let theme = ghostty_theme(solarized.clone(), Shade::Dark);
        assert_eq!(
            [theme.background, theme.foreground].map(rgb_hex),
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
        for choice in [Shade::Light, Shade::Dark] {
            let theme = Theme::new(choice, &ColorScheme::GitButler, CodeSize::Medium, &ghostty);
            let terminal = &theme.terminal;
            assert_eq!(
                [terminal.background, terminal.foreground].map(|c| c.map(super::hex)),
                [theme.background, theme.foreground].map(|c| Some(rgb_hex(c))),
                "the terminal's ground and text are the app's"
            );
            let red = terminal.palette.iter().find(|(index, _)| *index == 1);
            assert_eq!(
                red.map(|(_, color)| super::hex(*color)),
                Some(rgb_hex(theme.danger_text)),
                "red is the app's danger, not Ghostty's"
            );
            assert_eq!(terminal.palette.len(), 16, "every ANSI colour is set");
        }
    }

    fn gitbutler_theme() -> Theme {
        Theme::new(
            Shade::Dark,
            &ColorScheme::GitButler,
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
        for (choice, system, shade, why) in [
            (
                Appearance::System,
                WindowAppearance::Light,
                Shade::Light,
                "a light system draws light's",
            ),
            (
                Appearance::System,
                WindowAppearance::Dark,
                Shade::Dark,
                "a dark system draws dark's",
            ),
            (
                Appearance::Light,
                WindowAppearance::Dark,
                Shade::Light,
                "Light draws light's always",
            ),
            (
                Appearance::Dark,
                WindowAppearance::Light,
                Shade::Dark,
                "Dark draws dark's always",
            ),
        ] {
            assert_eq!(choice.shade(system), shade, "{why}");
        }
    }

    #[test]
    fn a_scheme_without_colours_draws_as_gitbutler() {
        let ghostty = Ghostty::default();
        let gitbutler = Theme::new(
            Shade::Dark,
            &ColorScheme::GitButler,
            CodeSize::Medium,
            &ghostty,
        );
        let missing = ColorScheme::Named("No theme is called this".into());
        let fallen_back = Theme::new(Shade::Dark, &missing, CodeSize::Medium, &ghostty);
        let unset = ghostty_theme(Colors::default(), Shade::Dark);
        for theme in [fallen_back, unset] {
            assert_eq!(
                [theme.background, theme.foreground, theme.primary_text].map(rgb_hex),
                [
                    gitbutler.background,
                    gitbutler.foreground,
                    gitbutler.primary_text
                ]
                .map(rgb_hex),
                "with nothing to draw in, GitButler's own colours stay"
            );
        }
    }
}
