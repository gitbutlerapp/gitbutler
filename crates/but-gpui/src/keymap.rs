//! The app's keys, read from `assets/keymaps/default.json`, which is written in Zed's keymap
//! format: a list of sections, each binding keys to actions by name in a key context. A section
//! with `os` binds only on the platforms it names.

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
    /// Keys to action names, in the file's order: the later binding wins, and is the one shown.
    bindings: IndexMap<String, String>,
}

/// One binding, as the file writes it.
struct Entry {
    keys: String,
    action: String,
    context: Option<String>,
}

/// Binds the default keys. They ship with the app, so one that doesn't bind is a bug the tests
/// catch; here it's logged and the rest still bind.
pub fn bind_keys(cx: &mut App) {
    let entries = match entries(DEFAULT, std::env::consts::OS) {
        Ok(entries) => entries,
        Err(err) => return tracing::error!("couldn't read the default keymap: {err:#}"),
    };
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
                assert!(
                    actions.contains(entry.action.as_str()),
                    "{at}: {:?} isn't an action",
                    entry.action
                );
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
