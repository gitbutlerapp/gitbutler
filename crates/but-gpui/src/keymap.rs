//! The app's keys, in `assets/keymaps/default.json`, written in Zed's keymap format: a list of
//! sections, each binding keys to actions by name in a key context. A section with `os` binds
//! only on the platforms it names.

use std::rc::Rc;

use anyhow::{Context as _, Result, bail};
use gpui::{App, KeyBinding, KeyBindingContextPredicate};
use indexmap::IndexMap;
use serde::Deserialize;

const DEFAULT: &str = include_str!("../assets/keymaps/default.json");

/// The platforms a section can name, as `std::env::consts::OS` names them.
const PLATFORMS: [&str; 3] = ["macos", "windows", "linux"];

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

/// Binds every key: the text fields' own, then the app's.
pub fn bind_keys(cx: &mut App) {
    crate::ui::text_input::bind_keys(cx);
    // They ship with the app, so one that doesn't bind is a bug the tests catch; here it's
    // logged and the rest still bind.
    let entries = entries(DEFAULT, std::env::consts::OS)
        .inspect_err(|err| tracing::error!("couldn't read the default keymap: {err:#}"))
        .unwrap_or_default();
    let bindings: Vec<KeyBinding> = (entries.iter())
        .filter_map(|entry| {
            binding(entry, cx)
                .inspect_err(|err| tracing::error!("default keymap, {:?}: {err:#}", entry.keys))
                .ok()
        })
        .collect();
    cx.bind_keys(bindings);
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

    use super::{DEFAULT, PLATFORMS, entries};

    #[test]
    fn defaults_bind_on_every_platform() {
        let actions: HashSet<&str> = gpui::generate_list_of_all_registered_actions()
            .map(|action| action.name)
            .collect();
        for os in PLATFORMS {
            let mut bound = HashSet::new();
            for entry in entries(DEFAULT, os).expect("the default keymap parses") {
                let at = format!("{os}, {:?} in {:?}", entry.keys, entry.context);
                let name = entry.action.as_str();
                assert!(actions.contains(name), "{at}: {name:?} isn't an action");
                if let Some(context) = &entry.context {
                    KeyBindingContextPredicate::parse(context).expect(&at);
                }
                for keystroke in entry.keys.split_whitespace() {
                    Keystroke::parse(keystroke).expect(&at);
                }
                assert!(
                    bound.insert((entry.context, entry.keys)),
                    "{at}: bound twice, so the first does nothing"
                );
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
    fn defaults_leave_the_platforms_keys_alone() {
        for os in PLATFORMS {
            let reserved: Vec<_> = reserved(os).iter().map(|keys| normal(keys, os)).collect();
            for entry in entries(DEFAULT, os).expect("the default keymap parses") {
                for keystroke in entry.keys.split_whitespace() {
                    let (modifiers, key) = normal(keystroke, os);
                    let taken = reserved.contains(&(modifiers.clone(), key));
                    assert!(!taken, "{os}: {keystroke} is the platform's");
                    let kept = ["super", "win", "cmd"]
                        .iter()
                        .any(|m| modifiers.contains(*m));
                    assert!(
                        os == "macos" || !kept,
                        "{os}: {keystroke} needs a modifier Windows and Linux keep for the system"
                    );
                }
            }
        }
    }

    #[test]
    fn the_shown_key_is_the_last_bound() {
        // Menus, tooltips and the status bar show an action's last binding, so a key added
        // after the one meant to show takes its place everywhere.
        let shown = [
            ("app::CommandPalette", "secondary-k"),
            ("app::ToggleNavigator", "secondary-b"),
            ("palette::GoTo", "secondary-p"),
            ("navigator::RenameSelection", "r"),
            ("navigator::CopySelection", "secondary-c"),
            ("palette::CopyMore", "secondary-shift-c"),
            ("diff::NextHunk", "down"),
            ("find::FindNext", "secondary-g"),
        ];
        for os in PLATFORMS {
            let entries = entries(DEFAULT, os).expect("the default keymap parses");
            for (action, keys) in shown {
                let last = (entries.iter().rev())
                    .find(|entry| entry.action == action)
                    .map(|entry| entry.keys.as_str());
                assert_eq!(last, Some(keys), "{os}: {action} shows its last binding");
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
