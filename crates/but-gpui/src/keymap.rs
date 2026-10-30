//! The app's keys, written in Zed's keymap format: a list of sections, each binding keys to
//! actions by name in a key context. A section with `os` binds only on the platforms it names.
//! `assets/keymaps/base.json` holds the few keys every keymap shares, those macOS gives every
//! app, as ⌘Q. Each [`Keymap`] has a file of its own for the rest, which Settings › Keyboard
//! picks.

use std::rc::Rc;

use anyhow::{Context as _, Result, bail};
use gpui::{App, Entity, KeyBinding, KeyBindingContextPredicate};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::state::Store;

const BASE: &str = include_str!("../assets/keymaps/base.json");

/// The platforms a section can name, as `std::env::consts::OS` names them.
const PLATFORMS: [&str; 3] = ["macos", "windows", "linux"];

/// A set of keys the app binds, beside the keys every keymap shares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keymap {
    /// GitButler's own, after macOS's and vim's.
    #[default]
    GitButler,
    /// Like the `but` TUI's. So far its keys for moving around, copying, undoing, renaming,
    /// moving commits and adding an empty commit, Return to fold and unfold, f to show and hide
    /// the files, ⌘B and ⌥⌘B the navigator and the details, / to filter or find, ⌘K for the
    /// command palette, and ⌘R and ⇧⌘R to fetch and pull.
    Tui,
}

impl Keymap {
    /// Every keymap, in the order Settings › Keyboard lists them.
    pub const ALL: [Keymap; 2] = [Keymap::GitButler, Keymap::Tui];

    pub fn name(self) -> &'static str {
        match self {
            Keymap::GitButler => "GitButler",
            Keymap::Tui => "TUI",
        }
    }

    /// The files it binds, in their order: its own, then the shared keys, last so it can't take
    /// one of them.
    fn files(self) -> [&'static str; 2] {
        let own = match self {
            Keymap::GitButler => include_str!("../assets/keymaps/gitbutler.json"),
            Keymap::Tui => include_str!("../assets/keymaps/tui.json"),
        };
        [own, BASE]
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Section {
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

/// Binds the keys of the keymap the store's prefs pick, and again each time they pick another,
/// then sets the menus, as the menu bar takes its keys when it's set.
pub fn bind(store: &Entity<Store>, set_menus: impl Fn(&mut App) + 'static, cx: &mut App) {
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
    for json in keymap.files() {
        // They ship with the app, so one that doesn't bind is a bug the tests catch; here it's
        // logged and the rest still bind.
        let entries = entries(json, std::env::consts::OS)
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

/// The bindings `json` makes on the platform `os`, in its order.
fn entries(json: &str, os: &str) -> Result<Vec<Entry>> {
    let sections: Vec<Section> = serde_json_lenient::from_str(json).context("not a keymap")?;
    let mut entries = Vec::new();
    for section in sections {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use gpui::{KeyBindingContextPredicate, Keystroke};

    use super::{Entry, Keymap, PLATFORMS, entries};

    /// What `keymap` binds on `os`, the shared keys too, in the order it binds them.
    fn bound(keymap: Keymap, os: &str) -> Vec<Entry> {
        (keymap.files().into_iter())
            .flat_map(|json| entries(json, os).expect("the keymaps parse"))
            .collect()
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
                        assert!(
                            os == "macos" || !kept,
                            "{at} needs a modifier Windows and Linux keep for the system"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_shown_key_is_the_last_bound() {
        // Menus, tooltips and the status bar show an action's last binding, so a key added
        // after the one meant to show takes its place everywhere.
        for keymap in Keymap::ALL {
            let shown: &[(&str, &str)] = match keymap {
                Keymap::GitButler => &[
                    ("app::CommandPalette", "secondary-k"),
                    ("app::ToggleNavigator", "secondary-b"),
                    ("palette::GoTo", "secondary-p"),
                    ("edits::Reword", "r"),
                    ("edits::RenameBranch", "r"),
                    ("navigator::CopySelection", "secondary-c"),
                    ("palette::CopyMore", "secondary-shift-c"),
                    ("diff::NextHunk", "down"),
                    ("find::FindNext", "secondary-g"),
                ],
                // The TUI's letters, which come after the arrows.
                Keymap::Tui => &[
                    ("app::CommandPalette", ":"),
                    ("palette::GoTo", "t"),
                    // ⌘ chords, as a terminal takes the letters.
                    ("app::ToggleNavigator", "secondary-b"),
                    ("app::ToggleDetail", "secondary-alt-b"),
                    ("navigator::SelectNext", "j"),
                    ("navigator::SelectPrevious", "k"),
                    ("diff::NextHunk", "j"),
                    ("diff::PreviousHunk", "k"),
                    ("diff::NextFile", "j"),
                ],
            };
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
