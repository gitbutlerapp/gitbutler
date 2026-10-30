//! What the embedded terminal takes from the user's Ghostty config: its font, and the colours
//! of its light and dark themes. Ghostty's theme files can also be listed and read by
//! name, to colour the app as a whole.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
    /// The families `font-family` lists, the first wanted most.
    pub font_families: Vec<String>,
    /// The OpenType features `font-feature` sets, by tag.
    pub font_features: Vec<(String, u32)>,
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
    font_families: Vec<String>,
    font_features: Vec<(String, u32)>,
    font_size: Option<f32>,
    theme: Option<Theme>,
    colors: Colors,
}

impl Ghostty {
    /// Reads the config from disk; a missing or unreadable file reads as nothing set.
    pub fn load() -> Self {
        let home = env::var_os("HOME").map(PathBuf::from);
        let app_support = home
            .filter(|_| cfg!(target_os = "macos"))
            .map(|home| home.join("Library/Application Support/com.mitchellh.ghostty"));
        let xdg = config_dir();
        let configs: Vec<String> = (xdg.iter().chain(&app_support))
            .flat_map(|dir| CONFIG_FILES.map(|file| dir.join(file)))
            .filter_map(|path| fs::read_to_string(path).ok())
            .collect();
        let theme_dirs = theme_dirs();
        Self::parse(configs.iter().map(String::as_str), |name| {
            (theme_dirs.iter()).find_map(|dir| fs::read_to_string(dir.join(name)).ok())
        })
    }

    /// The name of every theme Ghostty can load, from the user's folder and Ghostty's own,
    /// sorted and each once. Empty when Ghostty isn't installed.
    pub fn themes() -> Vec<String> {
        theme_names(&theme_dirs())
    }

    /// The colours of the theme called `name`, the user's own before Ghostty's of that name,
    /// or `None` when there's no such theme.
    pub fn theme(name: &str) -> Option<Colors> {
        read_theme(&theme_dirs(), name).map(|text| Colors::parse(&text))
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
            light: colors(settings.theme.as_ref().map(|theme| theme.light.as_str())),
            dark: colors(settings.theme.as_ref().map(|theme| theme.dark.as_str())),
            font_size: settings.font_size,
            font_families: settings.font_families,
            font_features: settings.font_features,
        }
    }
}

impl Settings {
    fn set(&mut self, key: &str, value: &str) {
        match key {
            "font-family" if value.is_empty() => self.font_families.clear(),
            "font-family" => self.font_families.push(value.to_owned()),
            "font-feature" if value.is_empty() => self.font_features.clear(),
            "font-feature" => {
                (self.font_features).extend(value.split(',').filter_map(font_feature))
            }
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

/// Ghostty's folder in the user's config.
fn config_dir() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|dir| dir.join("ghostty"))
}

/// The folders themes are looked up in, the user's first.
fn theme_dirs() -> Vec<PathBuf> {
    (config_dir().map(|dir| dir.join("themes")).into_iter())
        .chain(SYSTEM_THEMES.iter().map(PathBuf::from))
        .collect()
}

fn theme_names(dirs: &[PathBuf]) -> Vec<String> {
    let names: BTreeSet<String> = dirs
        .iter()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.into_iter().collect()
}

/// The text of the theme file called `name` in the first of `dirs` that has one. A name that
/// isn't a plain file name finds nothing.
fn read_theme(dirs: &[PathBuf], name: &str) -> Option<String> {
    let plain = Path::new(name).file_name().is_some_and(|file| file == name);
    if !plain || name.starts_with('.') {
        return None;
    }
    dirs.iter()
        .find_map(|dir| fs::read_to_string(dir.join(name)).ok())
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

/// A feature as Ghostty writes it, `-calt`, `+calt`, `calt=2` or `calt off`, its name quoted or
/// not. Anything else is ignored, as Ghostty does.
fn font_feature(setting: &str) -> Option<(String, u32)> {
    let setting = setting.trim();
    let (tag, value) = match setting.strip_prefix('-') {
        Some(tag) => (tag, "off"),
        None => {
            let setting = setting.strip_prefix('+').unwrap_or(setting);
            (setting.split_once(['=', ' '])).unwrap_or((setting, "on"))
        }
    };
    let value = match value.trim_matches(['=', ' ']) {
        "on" => 1,
        "off" => 0,
        value => value.parse().ok()?,
    };
    let tag = tag.trim_matches(['"', '\'']);
    (tag.len() == 4 && tag.is_ascii()).then(|| (tag.to_owned(), value))
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
    fn font_families_add_up_until_a_blank_one_resets_them() {
        let ghostty = Ghostty::parse(
            [
                "font-family = Old\nfont-family = \"\"\nfont-family = \"Commit Mono\"",
                "font-family = Menlo",
            ],
            |_| None,
        );
        assert_eq!(
            ghostty.font_families,
            ["Commit Mono", "Menlo"],
            "a blank clears what came before, and later configs add fallbacks"
        );
    }

    #[test]
    fn font_features_read_every_way_ghostty_writes_them() {
        let ghostty = Ghostty::parse(
            [
                "font-feature = -calt\nfont-feature = liga, +dlig\nfont-feature = ss01 off\n\
                 font-feature = ss02=0\nfont-feature = cv01 = 3\nfont-feature = \"zero\" on\n\
                 font-feature = toolong\nfont-feature = ss03=nope",
            ],
            |_| None,
        );
        let features: Vec<_> = (ghostty.font_features.iter())
            .map(|(tag, value)| (tag.as_str(), *value))
            .collect();
        assert_eq!(
            features,
            [
                ("calt", 0),
                ("liga", 1),
                ("dlig", 1),
                ("ss01", 0),
                ("ss02", 0),
                ("cv01", 3),
                ("zero", 1),
            ],
            "tags that aren't four letters and values that aren't numbers are ignored"
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

    #[test]
    fn themes_are_found_by_name_with_the_users_first() -> std::io::Result<()> {
        let user = tempfile::tempdir()?;
        let system = tempfile::tempdir()?;
        fs::write(user.path().join("Night"), "background = #010101")?;
        fs::write(system.path().join("Night"), "background = #020202")?;
        fs::write(system.path().join("Day"), "background = #ffffff")?;
        fs::write(system.path().join(".hidden"), "background = #030303")?;
        fs::create_dir(system.path().join("folder"))?;
        let dirs = [user.path().to_owned(), system.path().to_owned()];

        assert_eq!(
            theme_names(&dirs),
            ["Day", "Night"],
            "each name once, sorted, without hidden files or folders"
        );
        let night = read_theme(&dirs, "Night").map(|text| Colors::parse(&text));
        assert_eq!(
            night.and_then(|colors| colors.background),
            Some(c(1, 1, 1)),
            "the user's theme shadows Ghostty's of the same name"
        );
        assert_eq!(
            read_theme(&dirs, "../Night"),
            None,
            "a name can't reach outside the theme folders"
        );
        Ok(())
    }
}
