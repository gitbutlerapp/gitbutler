//! A global, serializable color theme for CLI output.
//!
//! Styled output *must* begin from one of the global theme's styles. Using [`colored`] or [`Style`]
//! independently of this theme is prohibited, as that breaks user-defined theming.
//!
//! The theme controls the styling of semantic elements (branch names, commit IDs, file statuses,
//! etc.) for human-readable output modes. It can be loaded from a JSON file so users can customize
//! colors, or left at its defaults which reproduce the original hard-coded palette.
//!
//! # Startup
//!
//! Call [`init`] exactly once before any output is produced (typically in [`crate::handle_args`]).
//! After that, [`get`] returns a `&'static Theme`.
//!
//! Note that unit tests **do not need to initializes** the theme as they will automatically fall
//! back on the default if not initialized.
//!
//!
//! # Serialization
//!
//! Style fields are [`ratatui::style::Style`] values which serialize to JSON like:
//!
//! ```json
//! { "fg": "Green", "add_modifier": "BOLD" }
//! ```
//!
//! Missing fields in a user-supplied file fall back to the built-in defaults thanks to
//! `#[serde(default)]`.

use std::{fmt::Display, path::Path, str::FromStr, sync::OnceLock};

use colored::{ColoredString, Colorize as _};
use ratatui::{
    palette::Hsl,
    style::{Color, Modifier, Style, Styled},
    text::Span,
};
use serde::{Deserialize, Serialize};
use syntect::highlighting::{self, ThemeSet};

const MONOKAI_THEME: &[u8] =
    include_bytes!("../assets/syntax-highlighting-themes/Monokai Extended.tmTheme");
const MONOKAI_THEME_LIGHT: &[u8] =
    include_bytes!("../assets/syntax-highlighting-themes/Monokai Extended Light.tmTheme");

/// Global theme instance, initialized once at startup.
static THEME: OnceLock<Theme> = OnceLock::new();

/// Initialize the global theme.
///
/// Must be called exactly once, before any call to [`get`].
/// Panics if called more than once.
pub fn init(theme: Theme) {
    THEME
        .set(theme)
        .expect("theme may only be initialized once");
}

/// Return a reference to the global theme.
///
/// Panics if [`init`] has not been called yet.
pub fn get() -> &'static Theme {
    #[cfg(test)]
    {
        THEME.get_or_init(Theme::default)
    }
    #[cfg(not(test))]
    {
        THEME
            .get()
            .expect("theme::init() must be called before getting the theme")
    }
}

/// Load a theme from a JSON file.
///
/// Fields that are absent in the file keep their [`Theme::default`] values.
pub fn load(path: &Path) -> anyhow::Result<Theme> {
    let contents = std::fs::read_to_string(path)?;
    let theme: Theme = serde_json::from_str(&contents)?;
    Ok(theme)
}

/// Extension trait that lets us apply a [`Style`] to "paint" a string with raw ANSI escape codes.
///
/// ```ignore
/// use crate::theme::Paint;
/// let t = crate::theme::get();
/// writeln!(out, "{}", t.local_branch.paint(&name))?;
/// ```
pub trait Paint {
    /// Apply this style to `text`, producing a [`ColoredString`] which can be used by the one-shot
    /// CLI to output ANSI escape code formatted strings.
    ///
    /// Each paint application produces an independently styled [`ColoredString`] that respects its
    /// surrounding during formatting. That is to say, you can nest paint applications and it will
    /// work as expected, with the outer style resuming where the nested style ends.
    ///
    /// Note: The reason we return [`ColoredString`] here instead of directly formatting a
    /// [`String`] is that the former allows for formatting without accounting for the escape codes,
    /// making it much easier to align with padding and the like.
    fn paint<S: AsRef<str>>(&self, text: S) -> ColoredString;
}

impl Paint for Style {
    fn paint<S: AsRef<str>>(&self, text: S) -> ColoredString {
        // This is technically unnecessary as `colored` performs this check internally, it's just
        // here for clarity of intent
        if !colored::control::SHOULD_COLORIZE.should_colorize() {
            return text.as_ref().into();
        }

        let mut styled = text.as_ref().normal();

        if let Some(fg) = self.fg {
            styled = apply_foreground(styled, fg);
        }
        if let Some(bg) = self.bg {
            styled = apply_background(styled, bg);
        }
        styled = apply_modifiers(styled, self.add_modifier);

        styled
    }
}

/// Apply foreground color using `colored`.
fn apply_foreground(styled: ColoredString, color: Color) -> ColoredString {
    match color {
        Color::Black => styled.black(),
        Color::Red => styled.red(),
        Color::Green => styled.green(),
        Color::Yellow => styled.yellow(),
        Color::Blue => styled.blue(),
        Color::Magenta => styled.magenta(),
        Color::Cyan => styled.cyan(),
        Color::Gray => styled.white(),
        Color::White => styled.bright_white(),
        Color::DarkGray => styled.bright_black(),
        Color::LightRed => styled.bright_red(),
        Color::LightGreen => styled.bright_green(),
        Color::LightYellow => styled.bright_yellow(),
        Color::LightBlue => styled.bright_blue(),
        Color::LightMagenta => styled.bright_magenta(),
        Color::LightCyan => styled.bright_cyan(),
        Color::Rgb(r, g, b) => styled.truecolor(r, g, b),
        Color::Indexed(_) | Color::Reset => styled,
    }
}

/// Apply background color using `colored`.
fn apply_background(styled: ColoredString, color: Color) -> ColoredString {
    match color {
        Color::Black => styled.on_black(),
        Color::Red => styled.on_red(),
        Color::Green => styled.on_green(),
        Color::Yellow => styled.on_yellow(),
        Color::Blue => styled.on_blue(),
        Color::Magenta => styled.on_magenta(),
        Color::Cyan => styled.on_cyan(),
        Color::Gray => styled.on_white(),
        Color::White => styled.on_bright_white(),
        Color::DarkGray => styled.on_bright_black(),
        Color::LightRed => styled.on_bright_red(),
        Color::LightGreen => styled.on_bright_green(),
        Color::LightYellow => styled.on_bright_yellow(),
        Color::LightBlue => styled.on_bright_blue(),
        Color::LightMagenta => styled.on_bright_magenta(),
        Color::LightCyan => styled.on_bright_cyan(),
        Color::Rgb(r, g, b) => styled.on_truecolor(r, g, b),
        Color::Indexed(_) | Color::Reset => styled,
    }
}

/// Apply all style modifiers supported by `colored`.
fn apply_modifiers(mut styled: ColoredString, modifiers: Modifier) -> ColoredString {
    if modifiers.contains(Modifier::BOLD) {
        styled = styled.bold();
    }
    if modifiers.contains(Modifier::DIM) {
        styled = styled.dimmed();
    }
    if modifiers.contains(Modifier::ITALIC) {
        styled = styled.italic();
    }
    if modifiers.contains(Modifier::UNDERLINED) {
        styled = styled.underline();
    }
    if modifiers.contains(Modifier::SLOW_BLINK) || modifiers.contains(Modifier::RAPID_BLINK) {
        styled = styled.blink();
    }
    if modifiers.contains(Modifier::REVERSED) {
        styled = styled.reversed();
    }
    if modifiers.contains(Modifier::HIDDEN) {
        styled = styled.hidden();
    }
    if modifiers.contains(Modifier::CROSSED_OUT) {
        styled = styled.strikethrough();
    }
    styled
}

/// Identifiers for the theme presets.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ThemePreset {
    /// The dark preset.
    Dark,
    /// The light preset.
    Light,
}

impl FromStr for ThemePreset {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.to_lowercase();
        match normalized.trim() {
            "dark" => Ok(ThemePreset::Dark),
            "light" => Ok(ThemePreset::Light),
            unknown => Err(anyhow::anyhow!("Unknown theme preset: {unknown}")),
        }
    }
}

/// The CLI color theme.
///
/// Style fields ([`Style`]) control colors and text attributes for semantic
/// elements.  All fields fall back to their defaults when missing from a
/// deserialized file.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    // Concrete "things"
    /// Local branch name
    pub local_branch: Style,
    /// Remote / target branch name
    pub remote_branch: Style,
    /// Commit short hash / object ID wherever it appears outside of CLI IDs.
    pub commit_id: Style,
    /// Short CLI identifiers
    pub cli_id: Style,
    /// PR / review number decorations
    pub pr_number: Style,
    /// Hyperlinks (PR URLs, review links).
    pub link: Style,
    /// Configuration value (user name, email, provider, alias value, etc.).
    pub config_value: Style,
    /// Configuration key / setting name (e.g. git config keys, alias names).
    pub config_key: Style,

    // Modifications
    /// An addition
    pub addition: Style,
    /// A deletion
    pub deletion: Style,
    /// A modification
    pub modification: Style,
    /// A renaming
    pub renaming: Style,
    /// Context surrounding modifications
    pub context: Style,
    /// Rich-text version of [`Self::addition`], meant to preserve foreground text.
    pub addition_rich: Style,
    /// A more intense version of [`Self::addition_rich`], meant to highlight subsections of an
    /// addition.
    pub addition_rich_subsection: Style,
    /// Rich-text version of [`Self::deletion`], meant to preserve foreground text.
    pub deletion_rich: Style,
    /// A more intense version of [`Self::deletion_rich`], meant to highlight subsections of an
    /// deletion.
    pub deletion_rich_subsection: Style,

    // State signals
    /// Something completed successfully or is in a good state
    pub success: Style,
    /// The user should pay attention to this.
    pub attention: Style,
    /// Something went wrong or is in an error state
    pub error: Style,
    /// Highlight something that is purely informational
    pub info: Style,

    // TUI modes
    pub tui_mode_normal: Style,
    pub tui_mode_commit: Style,
    pub tui_mode_rub: Style,
    pub tui_mode_inline_reword: Style,
    pub tui_mode_command: Style,
    pub tui_mode_move: Style,
    pub tui_mode_details: Style,
    pub tui_mark: Style,

    // General purpose
    /// Subdued hint text for supplemental information that should not demand attention
    pub hint: Style,
    /// Something that is important to the user, such as a prompt for input
    pub important: Style,
    /// Suggested command the user can run (e.g. `but config target …`).
    pub command_suggestion: Style,
    /// Default styling
    pub default: Style,
    /// User information, typically author or committer-related
    pub user: Style,
    /// Time-related information
    pub time: Style,
    /// In-progress action labels (e.g. "Pushing...", "Fetching...", "Setting up...").
    pub progress: Style,

    // Layout
    /// Default border style
    pub border: Style,
    /// Active border style
    pub border_active: Style,
    /// Highlight style to denote selection
    pub selection_highlight: Style,
    /// Alternative selection highlight that is meant to be more discrete than the primary selection
    /// highlight. Useful when selecting larger areas and the [`Self::selection_highlight`] is a bit
    /// "too much".
    pub discrete_selection_highlight: Style,
    /// Legend for symbol or keybind explanation
    pub legend: Style,

    #[serde(skip_serializing, skip_deserializing)]
    // This is a bit weird. We need the theme itself to initialize these nested symbols, so we
    // initialize them after all of the colors have been initialized and make the type an Option.
    // But in practice, these symbols _must_ be here after initialization.
    //
    // This weirdness can be fixed by splitting the theme into two substructs: symbols and
    // styles.
    symbols: Option<ThemeSymbols>,

    #[serde(skip_serializing, skip_deserializing)]
    /// Serialized theme to use for syntax highlighting.
    syntax_highlighting_theme_raw: &'static [u8],
}

/// Helper — builds a [`Style`] with the given foreground color.
const fn style_fg(fg: Color) -> Style {
    Style::new().fg(fg)
}

/// Helper — builds a bold + colored [`Style`].
const fn style_fg_bold(fg: Color) -> Style {
    Style::new().fg(fg).add_modifier(Modifier::BOLD)
}

impl Default for Theme {
    /// Produces the canonical color palette.
    fn default() -> Self {
        Self::default_for(ThemePreset::Dark)
    }
}

impl Theme {
    /// Get the symbols for this theme.
    pub fn sym(&self) -> &ThemeSymbols {
        self.symbols
            .as_ref()
            .expect("symbols must always be initialized")
    }

    /// Produces a specific default color palette.
    pub fn default_for(preset: ThemePreset) -> Self {
        let mut t = match preset {
            ThemePreset::Light => Self::default_light(),
            ThemePreset::Dark => Self::default_dark(),
        };
        t.symbols = Some(ThemeSymbols::new(&t));
        t
    }

    /// Load the syntax highlighting theme.
    pub fn load_syntax_highlighting_theme(&self) -> anyhow::Result<highlighting::Theme> {
        Ok(ThemeSet::load_from_reader(&mut std::io::Cursor::new(
            self.syntax_highlighting_theme_raw,
        ))?)
    }

    fn default_dark() -> Self {
        Self {
            // Concrete "things"
            local_branch: style_fg(Color::Green),
            remote_branch: style_fg(Color::Magenta),
            commit_id: style_fg(Color::Cyan),
            cli_id: style_fg_bold(Color::Blue),
            pr_number: style_fg(Color::Blue),
            link: Style::new()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            config_value: style_fg(Color::Cyan),
            config_key: style_fg(Color::Green),

            // Modifications
            addition: style_fg(Color::Green),
            deletion: style_fg(Color::Red),
            modification: style_fg(Color::Yellow),
            renaming: style_fg(Color::Magenta),
            context: style_fg(Color::DarkGray),
            // Colors from Delta for preserving foreground syntax highlighting, with a lightness
            // adjustment for enhanced readability.
            addition_rich: Style::new().bg(Color::from_hsl(Hsl::new(
                120.0,
                1.0,
                0.078 + /*lightness adjustment=*/0.05,
            ))),
            addition_rich_subsection: Style::new().bg(Color::from_hsl(Hsl::new(120.0, 1.0, 0.188))),
            deletion_rich: Style::new().bg(Color::from_hsl(Hsl::new(
                -0.952,
                1.0,
                0.123 + /*lightness adjustment=*/0.05,
            ))),
            deletion_rich_subsection: Style::new()
                .bg(Color::from_hsl(Hsl::new(-0.468, 0.8, 0.313))),

            // State signals
            success: style_fg(Color::Green),
            attention: style_fg(Color::Yellow),
            error: style_fg(Color::Red),
            info: style_fg(Color::Cyan),

            // TUI modes
            tui_mode_normal: Style::new().bg(Color::DarkGray).fg(Color::White),
            tui_mode_commit: Style::new().bg(Color::Green).fg(Color::Black),
            tui_mode_rub: Style::new().bg(Color::Blue).fg(Color::Black),
            tui_mode_inline_reword: Style::new().bg(Color::Magenta).fg(Color::Black),
            tui_mode_command: Style::new().bg(Color::Yellow).fg(Color::Black),
            tui_mode_move: Style::new().bg(Color::Cyan).fg(Color::Black),
            tui_mode_details: Style::new().bg(Color::Rgb(255, 165, 0)).fg(Color::Black),
            tui_mark: Style::new().bg(Color::Blue).fg(Color::Black),

            // General purpose
            hint: Style::new().add_modifier(Modifier::DIM),
            important: Style::new().add_modifier(Modifier::BOLD),
            command_suggestion: Style::new().fg(Color::Blue).add_modifier(Modifier::DIM),
            default: Style::new(),
            user: style_fg(Color::LightYellow),
            time: style_fg(Color::Cyan),
            progress: Style::new().add_modifier(Modifier::DIM),

            // Layout
            border: Style::new().fg(Color::DarkGray),
            border_active: Style::new().fg(Color::Cyan),
            selection_highlight: Style::new()
                .bg(Color::Rgb(69, 71, 90))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            discrete_selection_highlight: Style::new()
                .bg(Color::from_hsl(Hsl::new(236.8, 0.162, 0.229))),
            legend: Style::new().fg(Color::Blue),

            syntax_highlighting_theme_raw: MONOKAI_THEME,

            // Symbols initialized below
            symbols: None,
        }
    }

    fn default_light() -> Self {
        // Note: Many light themes butcher the ANSI white and bright white, we use the lightest
        // possible 8 bit gray for foreground text where we _really_ need contrast against a darker
        // color.
        let white_256 = Color::Indexed(255);
        let dark_t = Self::default_dark();

        Self {
            // Modifications
            // Colors from Delta for preserving foreground syntax highlighting
            addition_rich: Style::new().bg(Color::Rgb(208, 255, 208)),
            addition_rich_subsection: Style::new().bg(Color::Rgb(160, 239, 160)),
            deletion_rich: Style::new().bg(Color::Rgb(255, 224, 224)),
            deletion_rich_subsection: Style::new().bg(Color::Rgb(255, 192, 192)),

            // TUI modes
            tui_mode_normal: dark_t.tui_mode_normal.fg(white_256),
            tui_mode_commit: dark_t.tui_mode_commit.fg(white_256),
            tui_mode_rub: dark_t.tui_mode_rub.fg(white_256),
            tui_mode_inline_reword: dark_t
                .tui_mode_inline_reword
                .bg(Color::Magenta)
                .fg(white_256),
            tui_mode_command: dark_t.tui_mode_command.fg(white_256),
            tui_mode_move: dark_t.tui_mode_move.fg(white_256),
            tui_mode_details: dark_t.tui_mode_details.fg(Color::Black),
            tui_mark: dark_t.tui_mark.fg(white_256),

            // General purpose
            hint: style_fg(Color::DarkGray),
            progress: style_fg(Color::DarkGray),

            // Layout
            selection_highlight: Style::new().fg(Color::Black).bg(Color::Indexed(252)),
            discrete_selection_highlight: Style::new().bg(Color::Indexed(255)),

            syntax_highlighting_theme_raw: MONOKAI_THEME_LIGHT,

            ..dark_t
        }
    }
}

/// Symbols styled with a [`Theme`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ThemeSymbols {
    /// Successful state.
    pub success: StyledSymbol,
    /// Error state.
    pub error: StyledSymbol,
    /// Warning indicator.
    pub warning: StyledSymbol,
    /// Generic dot marker — use `.success()`/`.attention()`/`.error()`/`.info()` for context.
    pub dot: StyledSymbol,
    /// Arrow indicator.
    pub arrow: StyledSymbol,
    /// Force / lightning indicator.
    pub lightning: StyledSymbol,
    /// Line marked in TUI
    pub mark: StyledSymbol,
}

impl ThemeSymbols {
    fn new(t: &Theme) -> Self {
        Self {
            success: StyledSymbol::new("✓", t.success.add_modifier(Modifier::BOLD)),
            error: StyledSymbol::new("✗", t.error.add_modifier(Modifier::BOLD)),
            warning: StyledSymbol::new("⚠", t.attention.add_modifier(Modifier::BOLD)),
            dot: StyledSymbol::new("●", t.default),
            arrow: StyledSymbol::new("→", t.hint),
            lightning: StyledSymbol::new("⚡", t.attention.add_modifier(Modifier::BOLD)),
            mark: StyledSymbol::new("✔︎", t.tui_mark),
        }
    }
}

/// An abstract symbol that can be turned into either a raw ANSI-escaped [`String`] or a styled
/// Ratatui [`Span`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSymbol {
    content: String,
    style: Style,
}

impl StyledSymbol {
    /// Create a new [`StyledSymbol`].
    pub fn new<S: AsRef<str>>(content: S, style: Style) -> Self {
        StyledSymbol {
            content: content.as_ref().to_string(),
            style,
        }
    }

    /// Convert the [`StyledSymbol`] into a styled [`Span`].
    pub fn span(&self) -> Span<'_> {
        Span::styled(&self.content, self.style)
    }

    /// Return a new symbol styled for success.
    pub fn success(&self) -> StyledSymbol {
        let t = get();
        StyledSymbol::new(&self.content, t.success.add_modifier(Modifier::BOLD))
    }

    /// Return a new symbol styled for attention / warning.
    pub fn attention(&self) -> StyledSymbol {
        let t = get();
        StyledSymbol::new(&self.content, t.attention.add_modifier(Modifier::BOLD))
    }

    /// Return a new symbol styled for error.
    pub fn error(&self) -> StyledSymbol {
        let t = get();
        StyledSymbol::new(&self.content, t.error.add_modifier(Modifier::BOLD))
    }

    /// Return a new symbol styled for info.
    pub fn info(&self) -> StyledSymbol {
        let t = get();
        StyledSymbol::new(&self.content, t.info.add_modifier(Modifier::BOLD))
    }
}

impl Display for StyledSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.style.paint(&self.content))
    }
}

/// Combine the styling of an item with another style.
///
/// Doing `widget.style(new_style)` will fully replace `widget`'s styling. This method however will
/// combine the styles using [`Style::patch`].
///
/// This is useful if you have an already styled widget and you want to add for example a
/// background color to indicate that it is selected. Doing that with
/// `widget.style(bg_highlight_style)` would replace any existing style on `widget`.
pub trait PatchStyle {
    type Item;

    fn patch_style(self, new_style: Style) -> Self::Item;
}

impl<T> PatchStyle for T
where
    T: Styled,
{
    type Item = T::Item;

    fn patch_style(self, new_style: Style) -> Self::Item {
        let style = self.style();
        self.set_style(new_style.patch(style))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_default_theme_through_json() {
        let theme = Theme::default();
        let json = serde_json::to_string_pretty(&theme).unwrap();
        let deserialized: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(theme, deserialized);
    }

    #[test]
    fn partial_json_fills_missing_fields_with_defaults() {
        let json = r#"{ "local_branch": { "fg": "Cyan", "add_modifier": "BOLD" } }"#;
        let theme: Theme = serde_json::from_str(json).unwrap();

        assert_eq!(theme.local_branch, style_fg_bold(Color::Cyan));
        assert_eq!(theme.remote_branch, Theme::default().remote_branch);
        assert_eq!(theme.cli_id, Theme::default().cli_id);
        assert_eq!(theme.addition, Theme::default().addition);
    }

    #[test]
    fn empty_json_produces_default_theme() {
        let theme: Theme = serde_json::from_str("{}").unwrap();
        assert_eq!(theme, Theme::default());
    }
}
