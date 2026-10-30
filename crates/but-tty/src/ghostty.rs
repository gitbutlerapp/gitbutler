//! What the embedded terminal takes from the user's Ghostty config: its font size, and the
//! colours of its light and dark themes.

use std::env;
use std::fs;
use std::path::PathBuf;

use libghostty_vt::style::RgbColor;

const CONFIG_FILES: [&str; 2] = ["config", "config.ghostty"];

const SYSTEM_THEMES: &[&str] = if cfg!(target_os = "macos") {
    &["/Applications/Ghostty.app/Contents/Resources/ghostty/themes"]
} else {
    &[
        "/usr/share/ghostty/themes",
        "/usr/local/share/ghostty/themes",
    ]
};

/// A Ghostty colour scheme; each part unset falls back to the terminal's own default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Colors {
    pub background: Option<RgbColor>,
    pub foreground: Option<RgbColor>,
    pub cursor: Option<RgbColor>,
    pub selection_background: Option<RgbColor>,
    pub selection_foreground: Option<RgbColor>,
    pub palette: Vec<(u8, RgbColor)>,
}

/// What the user's Ghostty config sets that the embedded terminal follows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ghostty {
    pub font_size: Option<f32>,
    pub light: Colors,
    pub dark: Colors,
}

/// The theme names `theme` picks for each appearance.
#[derive(Debug, Clone, PartialEq)]
struct Theme {
    light: String,
    dark: String,
}

/// The config's keys as read, before its theme is looked up.
#[derive(Debug, Default)]
struct Settings {
    font_size: Option<f32>,
    theme: Option<Theme>,
    colors: Colors,
}

impl Ghostty {
    /// Reads the config from disk; a missing or unreadable file reads as nothing set.
    pub fn load() -> Self {
        let home = env::var_os("HOME").map(PathBuf::from);
        let xdg = env::var_os("XDG_CONFIG_HOME")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join(".config")))
            .map(|dir| dir.join("ghostty"));
        let app_support = home
            .filter(|_| cfg!(target_os = "macos"))
            .map(|home| home.join("Library/Application Support/com.mitchellh.ghostty"));
        let configs: Vec<String> = (xdg.iter().chain(&app_support))
            .flat_map(|dir| CONFIG_FILES.map(|file| dir.join(file)))
            .filter_map(|path| fs::read_to_string(path).ok())
            .collect();
        let theme_dirs: Vec<PathBuf> = (xdg.iter().map(|dir| dir.join("themes")))
            .chain(SYSTEM_THEMES.iter().map(PathBuf::from))
            .collect();
        Self::parse(configs.iter().map(String::as_str), |name| {
            (theme_dirs.iter()).find_map(|dir| fs::read_to_string(dir.join(name)).ok())
        })
    }

    pub fn colors(&self, dark: bool) -> &Colors {
        if dark { &self.dark } else { &self.light }
    }

    /// `configs` in the order they're read, with `theme` giving a theme file's text by name.
    fn parse<'a>(
        configs: impl IntoIterator<Item = &'a str>,
        theme: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let mut settings = Settings::default();
        for (key, value) in configs.into_iter().flat_map(entries) {
            settings.set(key, value);
        }
        let colors = |name: Option<&str>| {
            let mut colors = (name.and_then(&theme))
                .map(|text| Colors::parse(&text))
                .unwrap_or_default();
            colors.overlay(&settings.colors);
            colors
        };
        Self {
            font_size: settings.font_size,
            light: colors(settings.theme.as_ref().map(|theme| theme.light.as_str())),
            dark: colors(settings.theme.as_ref().map(|theme| theme.dark.as_str())),
        }
    }
}

impl Settings {
    fn set(&mut self, key: &str, value: &str) {
        match key {
            "font-size" => {
                let size = value.parse().ok().filter(|size: &f32| *size > 0.0);
                self.font_size = size.or(self.font_size);
            }
            "theme" => self.theme = Theme::parse(value).or(self.theme.take()),
            _ => self.colors.set(key, value),
        }
    }
}

impl Theme {
    /// A bare name for both appearances, or `light:A,dark:B` in either order.
    fn parse(value: &str) -> Option<Self> {
        let (mut light, mut dark) = (None, None);
        for part in value.split(',') {
            match part
                .split_once(':')
                .map(|(side, name)| (side.trim(), name.trim()))
            {
                Some(("light", name)) => light = Some(name),
                Some(("dark", name)) => dark = Some(name),
                _ if value.is_empty() => return None,
                _ => {
                    return Some(Self {
                        light: value.to_owned(),
                        dark: value.to_owned(),
                    });
                }
            }
        }
        Some(Self {
            light: light?.to_owned(),
            dark: dark?.to_owned(),
        })
    }
}

impl Colors {
    fn parse(text: &str) -> Self {
        let mut colors = Self::default();
        for (key, value) in entries(text) {
            colors.set(key, value);
        }
        colors
    }

    fn set(&mut self, key: &str, value: &str) {
        match key {
            "background" => self.background = rgb(value).or(self.background),
            "foreground" => self.foreground = rgb(value).or(self.foreground),
            "cursor-color" => self.cursor = rgb(value).or(self.cursor),
            "selection-background" => {
                self.selection_background = rgb(value).or(self.selection_background)
            }
            "selection-foreground" => {
                self.selection_foreground = rgb(value).or(self.selection_foreground)
            }
            "palette" => {
                let entry = value.split_once('=').and_then(|(index, color)| {
                    Some((index.trim().parse().ok()?, rgb(color.trim())?))
                });
                if let Some((index, color)) = entry {
                    self.set_palette(index, color);
                }
            }
            _ => {}
        }
    }

    fn set_palette(&mut self, index: u8, color: RgbColor) {
        self.palette.retain(|(i, _)| *i != index);
        self.palette.push((index, color));
    }

    /// Takes every part `over` sets.
    fn overlay(&mut self, over: &Colors) {
        self.background = over.background.or(self.background);
        self.foreground = over.foreground.or(self.foreground);
        self.cursor = over.cursor.or(self.cursor);
        self.selection_background = over.selection_background.or(self.selection_background);
        self.selection_foreground = over.selection_foreground.or(self.selection_foreground);
        for &(index, color) in &over.palette {
            self.set_palette(index, color);
        }
    }
}

/// The `key = value` lines of a config, split on the first `=`, with quotes taken off values.
fn entries(text: &str) -> impl Iterator<Item = (&str, &str)> {
    (text.lines().map(str::trim))
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            let value = value.trim();
            let unquoted = value.strip_prefix('"').and_then(|v| v.strip_suffix('"'));
            (key.trim(), unquoted.unwrap_or(value))
        })
}

/// `#rrggbb` or `rrggbb`.
fn rgb(value: &str) -> Option<RgbColor> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let [_, r, g, b] = u32::from_str_radix(hex, 16).ok()?.to_be_bytes();
    Some(RgbColor { r, g, b })
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn c(r: u8, g: u8, b: u8) -> RgbColor {
        RgbColor { r, g, b }
    }

    #[test]
    fn keys_split_on_the_first_equals_and_palette_accumulates() {
        let ghostty = Ghostty::parse(
            [
                "# comment\n\nfont-size = 13\npalette = 1=#ff0000\npalette = 2 = 00ff00",
                "font-size = \"22\"\npalette=1=#0000ff\nforeground = nonsense",
            ],
            |_| None,
        );
        assert_eq!(
            ghostty.font_size,
            Some(22.0),
            "the later config wins, unquoted"
        );
        assert_eq!(
            ghostty.dark.foreground, None,
            "an unparseable colour is ignored"
        );
        assert_eq!(
            ghostty.dark.palette,
            [(2, c(0, 255, 0)), (1, c(0, 0, 255))],
            "palette entries accumulate and a later index replaces an earlier one"
        );
    }

    #[test]
    fn theme_picks_by_appearance_and_direct_keys_override_it() {
        let ghostty = Ghostty::parse(
            [
                "theme = dark:Night, light: Day\ncursor-color = #010203\nselection-background = #040506",
            ],
            |name| match name {
                "Night" => Some("background = #000000\ncursor-color = #ffffff".into()),
                "Day" => Some("background = #ffffff\nforeground = #000000".into()),
                _ => None,
            },
        );
        assert_eq!(
            ghostty.colors(true),
            &Colors {
                background: Some(c(0, 0, 0)),
                cursor: Some(c(1, 2, 3)),
                selection_background: Some(c(4, 5, 6)),
                ..Colors::default()
            },
            "dark takes its theme, with the config's cursor over the theme's"
        );
        assert_eq!(
            ghostty.colors(false),
            &Colors {
                background: Some(c(255, 255, 255)),
                foreground: Some(c(0, 0, 0)),
                cursor: Some(c(1, 2, 3)),
                selection_background: Some(c(4, 5, 6)),
                ..Colors::default()
            },
            "light takes its own theme, with the same config override"
        );
    }
}
