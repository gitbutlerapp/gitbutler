//! The app's keys, written in Zed's keymap format: a list of sections, each binding keys to
//! actions by name in a key context. A section with `os` binds only on the platforms it names.
//! `assets/keymaps/base.json` holds the few keys macOS gives every app, as ⌘Q, and a [`Keymap`]
//! has a file of its own for the rest. `assets/keymaps/menus.json` lays out the menu bar, which
//! shows the keys ([`menus`]).
//!
//! The files ship in the binary. A dev build binds them from the checkout it was built from
//! instead ([`source_dir`]), and reads them again as a window comes in front, so the command
//! editor's changes and a hand's take effect without a build.

pub mod edit;
pub mod menus;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use anyhow::{Context as _, Result, bail};
use gpui::{App, Entity, Global, KeyBinding, KeyBindingContextPredicate};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::backend;
use crate::state::Store;

/// The platforms a section can name, as `std::env::consts::OS` names them.
pub const PLATFORMS: [&str; 3] = ["macos", "windows", "linux"];

/// What people call the platform `os`.
pub fn platform_name(os: &str) -> &str {
    match os {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}

/// A file in `assets/keymaps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeymapFile {
    Standard,
    /// The keys every keymap shares.
    Base,
    /// The menu bar.
    Menus,
}

impl KeymapFile {
    pub const ALL: [KeymapFile; 3] = [KeymapFile::Standard, KeymapFile::Base, KeymapFile::Menus];
    const KEYS: [KeymapFile; 2] = [KeymapFile::Standard, KeymapFile::Base];

    fn name(self) -> &'static str {
        match self {
            KeymapFile::Standard => "standard.json",
            KeymapFile::Base => "base.json",
            KeymapFile::Menus => "menus.json",
        }
    }

    /// The text that shipped in the binary.
    fn embedded(self) -> &'static str {
        match self {
            KeymapFile::Standard => include_str!("../assets/keymaps/standard.json"),
            KeymapFile::Base => include_str!("../assets/keymaps/base.json"),
            KeymapFile::Menus => include_str!("../assets/keymaps/menus.json"),
        }
    }

    /// Whether `text` can be this file.
    fn check(self, text: &str) -> Result<()> {
        match self {
            KeymapFile::Standard | KeymapFile::Base => sections(text).map(drop),
            KeymapFile::Menus => menus::parse(text).map(drop),
        }
    }
}

/// A set of keys the app binds, beside the keys every keymap shares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keymap {
    /// Like the `but` TUI's, and the only one. Prefs saved with the keymaps there used to be
    /// read as it, so everyone comes here.
    #[default]
    #[serde(alias = "GitButler", alias = "Tui")]
    Standard,
}

impl Keymap {
    #[cfg(test)]
    const ALL: [Keymap; 1] = [Keymap::Standard];

    /// The files it binds, in their order: its own, then the shared keys, last so it can't take
    /// one of them.
    pub fn files(self) -> [KeymapFile; 2] {
        [self.own_file(), KeymapFile::Base]
    }

    /// The file of the keys only it binds.
    pub fn own_file(self) -> KeymapFile {
        match self {
            Keymap::Standard => KeymapFile::Standard,
        }
    }
}

/// The folder a dev build reads its keymaps from and the command editor writes them to: the
/// checkout's, or `None` when this isn't a dev build or the checkout is gone. A script's run
/// gets a copy in its app data, so what it edits stays out of the checkout.
pub fn source_dir() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(find_source_dir).as_deref()
}

#[expect(
    clippy::disallowed_methods,
    reason = "once, at launch, and only a dev build goes on past the first line"
)]
fn find_source_dir() -> Option<PathBuf> {
    if !backend::settings::dev_build() {
        return None;
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/keymaps");
    if !source.is_dir() {
        return None;
    }
    if !crate::app::scripted() {
        return Some(source);
    }
    let copy = but_path::app_data_dir().ok()?.join("keymaps");
    std::fs::create_dir_all(&copy).ok()?;
    for file in KeymapFile::ALL {
        std::fs::copy(source.join(file.name()), copy.join(file.name())).ok()?;
    }
    Some(copy)
}

/// The text of each file as last read, and what's wrong with the ones that can't be used now.
/// Those keep the text they last had that could.
#[derive(Default)]
pub struct Loaded {
    texts: HashMap<KeymapFile, String>,
    pub errors: Vec<String>,
}

impl Global for Loaded {}

impl Loaded {
    fn text(&self, file: KeymapFile) -> &str {
        (self.texts.get(&file)).map_or(file.embedded(), String::as_str)
    }

    /// What the files say now, on top of what they last said, and the menu bar's items that name
    /// no action.
    fn read(&self, cx: &App) -> Loaded {
        let mut read = Loaded::default();
        for file in KeymapFile::ALL {
            let last = self.text(file);
            let text = match source_dir() {
                Some(dir) => read_file(dir, file),
                None => Ok(last.to_owned()),
            };
            let text = text.unwrap_or_else(|err| {
                read.errors.push(format!("{err:#}"));
                last.to_owned()
            });
            read.texts.insert(file, text);
        }
        if let Ok(menus) = menus::parse(read.text(KeymapFile::Menus)) {
            read.errors.extend(menus::unknown(&menus, cx));
        }
        read
    }

    fn same_as(&self, other: &Loaded) -> bool {
        self.errors == other.errors
            && KeymapFile::ALL
                .into_iter()
                .all(|file| self.text(file) == other.text(file))
    }
}

/// `file`'s text in `dir`, if it can be that file.
#[expect(
    clippy::disallowed_methods,
    reason = "only a dev build gets here, for a small file in its own checkout"
)]
fn read_file(dir: &Path, file: KeymapFile) -> Result<String> {
    let reading = || format!("reading {}", file.name());
    let text = std::fs::read_to_string(dir.join(file.name())).with_context(reading)?;
    file.check(&text).with_context(reading)?;
    Ok(text)
}

/// What binding the keys again takes.
struct Binder {
    store: Entity<Store>,
    set_menus: Rc<dyn Fn(&mut App)>,
}

impl Global for Binder {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Section {
    context: Option<String>,
    os: Option<Vec<String>>,
    /// Keys to the action each runs, in the file's order: the later binding wins, and is the one
    /// shown.
    bindings: IndexMap<String, String>,
}

/// One binding, as the file writes it.
struct Entry {
    keys: String,
    action: String,
    context: Option<String>,
}

/// `file`'s text as last read.
fn text_of(file: KeymapFile, cx: &App) -> String {
    (cx.try_global::<Loaded>())
        .map_or(file.embedded(), |loaded| loaded.text(file))
        .to_owned()
}

/// The menu bar as last read. The shipped file always reads, and one that doesn't is never kept.
pub fn bar_menus(cx: &App) -> Vec<menus::BarMenu> {
    menus::parse(&text_of(KeymapFile::Menus, cx))
        .inspect_err(|err| tracing::error!("couldn't read the menu bar: {err:#}"))
        .unwrap_or_default()
}

/// One binding in a file, whatever the platform.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub file: KeymapFile,
    /// Which of the file's sections it's in.
    pub section: usize,
    pub keys: String,
    pub action: String,
    pub context: Option<String>,
    /// The platforms it binds on, or `None` for all of them.
    pub os: Option<Vec<String>>,
}

impl Binding {
    /// Whether it binds on the platform `os`.
    pub fn binds_on(&self, os: &str) -> bool {
        (self.os.as_ref()).is_none_or(|names| names.iter().any(|name| name == os))
    }
}

/// Every binding of every file as last read, in each file's order.
pub fn bindings(cx: &App) -> Vec<Binding> {
    KeymapFile::KEYS
        .into_iter()
        .flat_map(|file| file_bindings(file, &text_of(file, cx)))
        .collect()
}

fn file_bindings(file: KeymapFile, text: &str) -> Vec<Binding> {
    let sections = sections(text)
        .inspect_err(|err| tracing::error!("couldn't read {}: {err:#}", file.name()))
        .unwrap_or_default();
    (sections.into_iter().enumerate())
        .flat_map(|(section, parsed)| {
            let (context, os) = (parsed.context, parsed.os);
            (parsed.bindings.into_iter()).map(move |(keys, action)| Binding {
                file,
                section,
                keys,
                action,
                context: context.clone(),
                os: os.clone(),
            })
        })
        .collect()
}

/// Change `file` on disk with `change`, from what's there now, and bind what it says. A change
/// that leaves it unreadable isn't written.
#[expect(
    clippy::disallowed_methods,
    reason = "only a dev build gets here, for a small file in its own checkout, and a key is saved \
              once it's been pressed, so nothing's waiting on a frame"
)]
pub fn edit(
    file: KeymapFile,
    change: impl FnOnce(&str) -> Result<String>,
    cx: &mut App,
) -> Result<()> {
    let dir = source_dir().context("this build's keymaps are built in")?;
    let path = dir.join(file.name());
    let text = std::fs::read_to_string(&path)?;
    let changed = change(&text)?;
    file.check(&changed)
        .context("the change would break the file")?;
    std::fs::write(&path, changed)?;
    reload(cx);
    Ok(())
}

/// Read the files again and bind what's changed in them. Only a dev build's can change.
pub fn reload(cx: &mut App) {
    if source_dir().is_none() {
        return;
    }
    let Some(last) = cx.try_global::<Loaded>() else {
        return;
    };
    let read = last.read(cx);
    if read.same_as(last) {
        return;
    }
    cx.set_global(read);
    let Some(binder) = cx.try_global::<Binder>() else {
        return;
    };
    let (store, set_menus) = (binder.store.clone(), binder.set_menus.clone());
    bind_keys(store.read(cx).prefs.keymap, cx);
    set_menus(cx);
    // Views draw the keys they show, and the cached ones draw again only on a refresh.
    cx.refresh_windows();
}

/// Binds the keys of the keymap the store's prefs pick, and again each time they pick another,
/// then sets the menus, as the menu bar takes its keys when it's set.
pub fn bind(store: &Entity<Store>, set_menus: impl Fn(&mut App) + 'static, cx: &mut App) {
    let read = Loaded::default().read(cx);
    for error in &read.errors {
        tracing::error!("couldn't read a keymap: {error}");
    }
    cx.set_global(read);
    let set_menus: Rc<dyn Fn(&mut App)> = Rc::new(set_menus);
    cx.set_global(Binder {
        store: store.clone(),
        set_menus: set_menus.clone(),
    });
    let mut bound = store.read(cx).prefs.keymap;
    bind_keys(bound, cx);
    set_menus(cx);
    cx.observe(store, move |store, cx| {
        let keymap = store.read(cx).prefs.keymap;
        if keymap != bound {
            bound = keymap;
            bind_keys(keymap, cx);
            set_menus(cx);
            // Views draw the keys they show, and the cached ones draw again only on a refresh.
            cx.refresh_windows();
        }
    })
    .detach();
}

/// Binds every key: the text fields' own, then `keymap`'s files.
fn bind_keys(keymap: Keymap, cx: &mut App) {
    cx.clear_key_bindings();
    crate::ui::text_input::bind_keys(cx);
    for file in keymap.files() {
        // They ship with the app, so one that doesn't bind is a bug the tests catch; here it's
        // logged and the rest still bind.
        let json = text_of(file, cx);
        let entries = entries(&json, std::env::consts::OS)
            .inspect_err(|err| tracing::error!("couldn't read the {keymap:?} keymap: {err:#}"))
            .unwrap_or_default();
        let bindings: Vec<KeyBinding> = (entries.iter())
            .filter_map(|entry| {
                binding(entry, cx)
                    .inspect_err(|err| {
                        tracing::error!("{keymap:?} keymap, {:?}: {err:#}", entry.keys)
                    })
                    .ok()
            })
            .collect();
        cx.bind_keys(bindings);
    }
}

pub(crate) fn sections(json: &str) -> Result<Vec<Section>> {
    serde_json_lenient::from_str(json).context("not a keymap")
}

/// The bindings `json` makes on the platform `os`, in its order.
fn entries(json: &str, os: &str) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for section in sections(json)? {
        if let Some(names) = &section.os {
            if let Some(unknown) = names
                .iter()
                .find(|name| !PLATFORMS.contains(&name.as_str()))
            {
                bail!("{unknown:?} isn't one of {PLATFORMS:?}");
            }
            if !names.iter().any(|name| name == os) {
                continue;
            }
        }
        entries.extend(section.bindings.into_iter().map(|(keys, action)| Entry {
            keys,
            action,
            context: section.context.clone(),
        }));
    }
    Ok(entries)
}

fn binding(entry: &Entry, cx: &App) -> Result<KeyBinding> {
    let action = cx.build_action(&entry.action, None)?;
    let context = (entry.context.as_deref())
        .map(KeyBindingContextPredicate::parse)
        .transpose()?;
    Ok(KeyBinding::load(
        &entry.keys,
        action,
        context.map(Rc::new),
        false,
        None,
        cx.keyboard_mapper().as_ref(),
    )?)
}

/// Keys the platform keeps: an app can't have them, or would take them from the system.
fn reserved(os: &str) -> &'static [&'static str] {
    match os {
        "macos" => &[
            "cmd-tab",
            "cmd-`",
            "cmd-space",
            "ctrl-space",
            "ctrl-up",
            "ctrl-down",
            "ctrl-left",
            "ctrl-right",
            "cmd-shift-3",
            "cmd-shift-4",
            "cmd-shift-5",
            "cmd-alt-escape",
            "cmd-ctrl-q",
            "cmd-ctrl-f",
        ],
        // Ctrl+Alt+Backspace kills the X server where that's on; Ctrl+Alt with an arrow
        // switches workspaces on Linux desktops.
        _ => &[
            "ctrl-alt-backspace",
            "ctrl-alt-delete",
            "alt-f4",
            "alt-tab",
            "alt-space",
            "ctrl-escape",
            "ctrl-shift-escape",
            "ctrl-alt-up",
            "ctrl-alt-down",
            "ctrl-alt-left",
            "ctrl-alt-right",
        ],
    }
}

/// A keystroke's modifiers, with `secondary` as the platform writes it, and its key.
fn normal(keystroke: &str, os: &str) -> (std::collections::BTreeSet<String>, String) {
    let (modifiers, key) = match keystroke.strip_suffix("--") {
        Some(modifiers) => (modifiers, "-"),
        None => keystroke.rsplit_once('-').unwrap_or(("", keystroke)),
    };
    let secondary = if os == "macos" { "cmd" } else { "ctrl" };
    let modifiers = (modifiers.split('-').filter(|m| !m.is_empty()))
        .map(|m| if m == "secondary" { secondary } else { m }.to_owned())
        .collect();
    (modifiers, key.to_owned())
}

/// The platform that keeps `keys`, of those a section for `os` binds on, or all of them.
pub fn kept_by_platform(keys: &str, os: Option<&[String]>) -> Option<&'static str> {
    (PLATFORMS.into_iter())
        .filter(|platform| os.is_none_or(|names| names.iter().any(|name| name == platform)))
        .find(|platform| {
            let kept: Vec<_> = (reserved(platform).iter())
                .map(|reserved| normal(reserved, platform))
                .collect();
            (keys.split_whitespace()).any(|stroke| kept.contains(&normal(stroke, platform)))
        })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use gpui::{KeyBindingContextPredicate, Keystroke};

    use super::{Entry, Keymap, PLATFORMS, entries, kept_by_platform, normal, reserved};

    /// What `keymap` binds on `os`, the shared keys too, in the order it binds them.
    fn bound(keymap: Keymap, os: &str) -> Vec<Entry> {
        (keymap.files().into_iter())
            .flat_map(|file| entries(file.embedded(), os).expect("the keymaps parse"))
            .collect()
    }

    /// Whether `entry` works in the context `name`, in the window, however its own context is
    /// written.
    fn works_in(entry: &Entry, name: &str) -> bool {
        let predicate = (entry.context.as_deref())
            .and_then(|context| KeyBindingContextPredicate::parse(context).ok());
        let stack: Vec<gpui::KeyContext> = ["Workspace", name]
            .into_iter()
            .filter_map(|context| gpui::KeyContext::parse(context).ok())
            .collect();
        predicate.is_some_and(|predicate| predicate.depth_of(&stack).is_some())
    }

    /// Whether every key on the drawer's bar does there what it does in the navigator.
    fn drawer_bar_keys_are_the_navigators(keymap: Keymap) -> bool {
        let entries = bound(keymap, "macos");
        let mut bar = (entries.iter())
            .filter(|entry| works_in(entry, "DrawerBar"))
            .peekable();
        bar.peek().is_some()
            && bar.all(|entry| {
                entries.iter().any(|other| {
                    works_in(other, "Navigator")
                        && other.keys == entry.keys
                        && other.action == entry.action
                })
            })
    }

    #[test]
    fn the_drawer_bars_keys_are_the_navigators() {
        assert!(
            drawer_bar_keys_are_the_navigators(Keymap::Standard),
            "a key means the same on the bar as on a row"
        );
    }

    #[test]
    fn a_saved_gitbutler_keymap_comes_to_the_standard_one() {
        assert_eq!(
            serde_json::from_str::<Keymap>("\"GitButler\"").ok(),
            Some(Keymap::Standard)
        );
    }

    #[test]
    fn a_saved_tui_keymap_comes_to_the_standard_one() {
        assert_eq!(
            serde_json::from_str::<Keymap>("\"Tui\"").ok(),
            Some(Keymap::Standard)
        );
    }

    #[test]
    fn the_keymap_saves_as_the_standard_one() {
        assert_eq!(
            serde_json::to_string(&Keymap::Standard).ok().as_deref(),
            Some("\"Standard\"")
        );
    }

    #[test]
    fn prefs_with_a_saved_gitbutler_keymap_keep_the_rest_of_what_they_say() {
        let prefs: Option<crate::state::Prefs> =
            serde_json::from_str(r#"{"keymap":"GitButler","audited_commands":["a::Run"]}"#).ok();
        let prefs = prefs.map(|prefs| (prefs.keymap, prefs.audited_commands.len()));
        assert_eq!(prefs, Some((Keymap::Standard, 1)));
    }

    /// The keys `keymap` binds to Discard on `os`.
    fn discard_keys(keymap: Keymap, os: &str) -> Vec<String> {
        (bound(keymap, os).into_iter())
            .filter(|entry| entry.action == "edits::Discard")
            .map(|entry| entry.keys)
            .collect()
    }

    #[test]
    fn discard_is_x_and_secondary_backspace() {
        for os in PLATFORMS {
            assert_eq!(
                discard_keys(Keymap::Standard, os),
                ["x", "secondary-backspace"],
                "on {os}, x discards as in the TUI, before ⌘⌫ so ⌘⌫ is the key shown"
            );
        }
    }

    #[test]
    fn keymaps_bind_on_every_platform() {
        let actions: HashSet<&str> = gpui::generate_list_of_all_registered_actions()
            .map(|action| action.name)
            .collect();
        for keymap in Keymap::ALL {
            for os in PLATFORMS {
                let mut keys = HashSet::new();
                for entry in bound(keymap, os) {
                    let at = format!(
                        "{keymap:?} on {os}, {:?} in {:?}",
                        entry.keys, entry.context
                    );
                    let name = entry.action.as_str();
                    assert!(actions.contains(name), "{at}: {name:?} isn't an action");
                    if let Some(context) = &entry.context {
                        KeyBindingContextPredicate::parse(context).expect(&at);
                    }
                    for keystroke in entry.keys.split_whitespace() {
                        Keystroke::parse(keystroke).expect(&at);
                    }
                    // A key bound to several actions in one place runs the first of them that's
                    // handled, as r rewords a commit and renames a branch; bound to the same one
                    // twice, the first is never shown.
                    assert!(
                        keys.insert((entry.context, entry.keys, name.to_owned())),
                        "{at}: bound twice to {name:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn keymaps_leave_the_platforms_keys_alone() {
        for keymap in Keymap::ALL {
            for os in PLATFORMS {
                let reserved: Vec<_> = reserved(os).iter().map(|keys| normal(keys, os)).collect();
                for entry in bound(keymap, os) {
                    for keystroke in entry.keys.split_whitespace() {
                        let at = format!("{keymap:?} on {os}: {keystroke}");
                        let (modifiers, key) = normal(keystroke, os);
                        let taken = reserved.contains(&(modifiers.clone(), key));
                        assert!(!taken, "{at} is the platform's");
                        let kept = ["super", "win", "cmd"]
                            .iter()
                            .any(|m| modifiers.contains(*m));
                        // Linux desktops take Super with a letter, but Ctrl+Super chords are
                        // apps' too there, as Ghostty's Ctrl+Super+] goes to the next split.
                        let shared = os == "linux" && modifiers.contains("ctrl");
                        assert!(
                            os == "macos" || shared || !kept,
                            "{at} needs a modifier Windows and Linux keep for the system"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_shown_key_is_the_last_bound() {
        // Right-click menus, tooltips and the status bar show an action's last binding, so a key
        // added after the one meant to show takes its place there.
        for keymap in Keymap::ALL {
            // The letters, which come after the arrows.
            let shown: &[(&str, &str)] = &[
                ("app::CommandPalette", "secondary-k"),
                // ⌘ chords, as a terminal takes the letters.
                ("palette::GoTo", "secondary-p"),
                ("palette::GoToTerminal", "secondary-j"),
                ("terminal::NewTerminal", "secondary-t"),
                ("app::ToggleNavigator", "secondary-b"),
                ("app::ToggleDetail", "secondary-alt-b"),
                ("navigator::CopySelection", "y"),
                // ⌘⌫ over x, as a menu offers discarding.
                ("edits::Discard", "secondary-backspace"),
                ("navigator::SelectNext", "j"),
                ("navigator::SelectPrevious", "k"),
                ("diff::NextHunk", "j"),
                ("diff::PreviousHunk", "k"),
                ("diff::NextFile", "j"),
            ];
            for os in PLATFORMS {
                let entries = bound(keymap, os);
                for (action, keys) in shown {
                    let last = (entries.iter().rev())
                        .find(|entry| entry.action == *action)
                        .map(|entry| entry.keys.as_str());
                    assert_eq!(
                        last,
                        Some(*keys),
                        "{keymap:?} on {os}: {action} shows its last binding"
                    );
                }
            }
        }
    }

    #[test]
    fn a_terminal_keeps_its_letters_but_not_the_keys_between_tiles() {
        for keymap in Keymap::ALL {
            for os in PLATFORMS {
                let entries = bound(keymap, os);
                for letter in ["h", "j", "k", "l"] {
                    assert!(
                        !(entries.iter())
                            .any(|entry| entry.keys == letter
                                && works_in(entry, "Terminal TextInput")),
                        "{keymap:?} on {os}: {letter} is typed into a terminal"
                    );
                }
            }
        }
    }

    /// What `keys` runs on `os` wherever the keys are, a terminal included.
    fn bound_everywhere(os: &str, keys: &str) -> Vec<String> {
        (bound(Keymap::Standard, os).into_iter())
            .filter(|entry| entry.keys == keys && entry.context.is_none())
            .map(|entry| entry.action)
            .collect()
    }

    #[test]
    fn a_terminal_opens_beside_a_tile_from_anywhere_on_macos() {
        assert_eq!(
            bound_everywhere("macos", "cmd-shift-t"),
            ["tiling::NewTerminalRight"],
            "⇧⌘T opens it to the right, even from a terminal"
        );
        assert_eq!(
            bound_everywhere("macos", "cmd-ctrl-t"),
            ["tiling::NewTerminalBelow"],
            "⌃⌘T opens it below, even from a terminal"
        );
    }

    #[test]
    fn a_terminal_with_a_command_opens_beside_a_tile_from_anywhere_on_macos() {
        assert_eq!(
            bound_everywhere("macos", "cmd-shift-i"),
            ["tiling::NewTerminalWithRight"],
            "⇧⌘I picks a command for one to the right, as ⇧⌘T opens a terminal there"
        );
        assert_eq!(
            bound_everywhere("macos", "cmd-ctrl-i"),
            ["tiling::NewTerminalWithBelow"],
            "⌃⌘I picks a command for one below, as ⌃⌘T opens a terminal there"
        );
    }

    #[test]
    fn a_terminal_opens_beside_a_tile_from_anywhere_on_linux() {
        assert_eq!(
            bound_everywhere("linux", "ctrl-shift-t"),
            ["tiling::NewTerminalRight"],
            "Ctrl+Shift+T opens it to the right, even from a terminal"
        );
        assert_eq!(
            bound_everywhere("linux", "ctrl-super-t"),
            ["tiling::NewTerminalBelow"],
            "Ctrl+Super+T opens it below, even from a terminal"
        );
    }

    #[test]
    fn the_keys_between_tiles_work_from_anywhere_in_the_workspace() {
        for keymap in Keymap::ALL {
            for os in PLATFORMS {
                let entries = bound(keymap, os);
                for (letter, action) in [
                    ("h", "app::FocusTileLeft"),
                    ("j", "app::FocusTileDown"),
                    ("k", "app::FocusTileUp"),
                    ("l", "app::FocusTileRight"),
                ] {
                    let keys = format!("secondary-shift-{letter}");
                    for context in [
                        "Navigator",
                        "Files",
                        "Diff",
                        "TextInput",
                        "Terminal TextInput",
                    ] {
                        let found: Vec<&str> = (entries.iter())
                            .filter(|entry| entry.keys == keys && works_in(entry, context))
                            .map(|entry| entry.action.as_str())
                            .collect();
                        assert_eq!(found, [action], "{keymap:?} on {os}: {keys} in {context}");
                    }
                }
            }
        }
    }

    #[test]
    fn find_in_the_navigator_filters_it() {
        for os in PLATFORMS {
            // Of bindings that work as deep in the window, the last one bound wins.
            let last = (bound(Keymap::Standard, os).iter().rev())
                .find(|entry| entry.keys == "secondary-f" && works_in(entry, "Navigator"))
                .map(|entry| entry.action.clone());
            assert_eq!(
                last.as_deref(),
                Some("navigator::FocusFilter"),
                "on {os}: ⌘F in the navigator does what / does, not find in the diff"
            );
        }
    }

    #[test]
    fn a_key_macos_keeps_is_kept_in_a_section_for_every_platform() {
        assert_eq!(kept_by_platform("secondary-tab", None), Some("macos"));
    }

    #[test]
    fn a_key_macos_keeps_is_free_in_a_section_for_windows() {
        let os = ["windows".to_owned()];
        assert_eq!(kept_by_platform("ctrl-up", Some(&os)), None);
    }

    #[test]
    fn a_key_no_platform_keeps_is_free() {
        assert_eq!(kept_by_platform("secondary-k", None), None);
    }

    #[test]
    fn sections_bind_only_on_their_platforms() {
        let json = r#"[
            { "bindings": { "a": "x::Anywhere" } },
            { "os": ["macos"], "bindings": { "b": "x::Mac" } },
            { "os": ["windows", "linux"], "bindings": { "c": "x::Elsewhere" } }
        ]"#;
        let actions = |os| -> Vec<String> {
            let entries = entries(json, os).expect("valid keymap");
            entries.into_iter().map(|entry| entry.action).collect()
        };
        assert_eq!(
            actions("macos"),
            ["x::Anywhere", "x::Mac"],
            "macOS skips the others"
        );
        assert_eq!(
            actions("linux"),
            ["x::Anywhere", "x::Elsewhere"],
            "Linux skips macOS"
        );
        let typo = r#"[{ "os": ["mac"], "bindings": {} }]"#;
        assert!(
            entries(typo, "macos").is_err(),
            "a misspelled platform would never bind"
        );
    }
}
