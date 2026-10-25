//! The app's keys: `assets/keymaps/default.json`, then the user's `keymap.json` in the settings
//! folder over it. Both are written in Zed's keymap format: a list of sections, each binding keys
//! to actions by name in a key context. A section with `os` binds only on the platforms it names.

use std::{path::PathBuf, rc::Rc, time::Duration};

use anyhow::{Context as _, Result, bail};
use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    Action, App, Entity, KeyBinding, KeyBindingContextPredicate, NoAction, Unbind, actions,
};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use crate::backend::{self, keymap};
use crate::state::{Notice, Store};

actions!(app, [OpenKeymap, OpenDefaultKeymap]);

const DEFAULT: &str = include_str!("../assets/keymaps/default.json");

/// What a new `keymap.json` starts as.
const TEMPLATE: &str = r#"// Your keys, over GitButler's own. Each section binds keys to actions in a context, as Zed's
// keymap.json does. "secondary" is cmd on macOS and ctrl elsewhere, null switches a key off,
// "unbind" takes a default key away from one action, and "os" keeps a section to "macos",
// "windows" or "linux". The command palette's "Open default keymap" lists every action.
// GitButler reads this file again each time it's saved.
[
  // {
  //   "context": "Diff",
  //   "bindings": { "n": "diff::NextHunk" }
  // }
]
"#;

/// The cards about the user's keymap, which its next read takes back.
const PROBLEMS: &str = "Some keys in keymap.json didn't load";
const UNREADABLE: &str = "keymap.json didn't load, so the keys stay as they were";

/// The platforms a section can name, as `std::env::consts::OS` names them.
const PLATFORMS: [&str; 3] = ["macos", "windows", "linux"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Section {
    context: Option<String>,
    os: Option<Vec<String>>,
    /// Keys taken away from the action each names, wherever it's bound.
    #[serde(default)]
    unbind: IndexMap<String, String>,
    /// Keys to actions, in the file's order: the later binding wins, and is the one shown.
    #[serde(default)]
    bindings: IndexMap<String, Value>,
}

/// One binding, as the file writes it: to an action's name, `[name, arguments]` or null, which
/// switches the keys off. With `unbind`, the keys are taken away from the action `action` names.
struct Entry {
    keys: String,
    action: Value,
    unbind: bool,
    context: Option<String>,
}

/// Binds the keys, then the user's `keymap.json` over them once it's read, and again each time
/// that file changes. What in it doesn't bind goes on a card. Also handles the commands that open
/// the keymaps.
pub fn watch(store: Entity<Store>, set_menus: impl Fn(&mut App) + 'static, cx: &mut App) {
    let reload = Rc::new({
        let store = store.clone();
        move |user: Result<Option<String>>, changed: bool, cx: &mut App| {
            let bound = user.and_then(|user| bind_keys(user.as_deref(), cx));
            if bound.is_ok() {
                // The menu bar takes its keys when it's set.
                set_menus(cx);
            }
            store.update(cx, |store, cx| {
                // What was said about the file before no longer holds.
                store.toasts.retain(|toast| {
                    !matches!(&toast.notice, Notice::Error { title, .. } if [PROBLEMS, UNREADABLE].contains(&title.as_ref()))
                });
                cx.notify();
                match bound {
                    Ok(problems) if problems.is_empty() => {
                        if changed {
                            store.snack("Keys updated from keymap.json", cx)
                        }
                    }
                    Ok(problems) => store.error(PROBLEMS, problems.join("\n"), cx),
                    Err(err) => store.failed(UNREADABLE, &err, cx),
                }
            });
        }
    });
    // The defaults now, so the window has keys before the file is read.
    reload(Ok(None), false, cx);
    cx.on_action({
        let store = store.clone();
        move |_: &OpenKeymap, cx| open(store.clone(), || keymap::create(TEMPLATE), cx)
    });
    cx.on_action(move |_: &OpenDefaultKeymap, cx| {
        let file = || keymap::temporary("gitbutler-default-keymap.json", DEFAULT);
        open(store.clone(), file, cx)
    });
    let (changed, mut changes) = mpsc::unbounded();
    cx.spawn(async move |cx| {
        let user = backend::run(keymap::read).await;
        cx.update(|cx| reload(user, false, cx));
        let watching = backend::run(move || {
            keymap::watch(move || {
                changed.unbounded_send(()).ok();
            })
        });
        let _watcher = match watching.await {
            Ok(watcher) => watcher,
            Err(err) => return tracing::warn!("couldn't watch keymap.json: {err:#}"),
        };
        while changes.next().await.is_some() {
            // An editor saves in several writes; read the file once they're done.
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            while changes.try_recv().is_ok() {}
            let user = backend::run(keymap::read).await;
            cx.update(|cx| reload(user, true, cx));
        }
    })
    .detach();
}

/// Opens the keymap `file` makes in the program the system opens it with.
fn open(
    store: Entity<Store>,
    file: impl FnOnce() -> Result<PathBuf> + Send + 'static,
    cx: &mut App,
) {
    cx.spawn(async move |cx| match backend::run(file).await {
        Ok(path) => cx.update(|cx| cx.open_with_system(&path)),
        Err(err) => store.update(cx, |store, cx| {
            store.failed("Couldn't open the keymap", &err, cx)
        }),
    })
    .detach();
}

/// Binds every key: the text fields' own, the defaults, then the `user`'s over them. Returns what
/// in the user's keymap didn't bind. When it isn't a keymap at all, the keys stay as they were.
fn bind_keys(user: Option<&str>, cx: &mut App) -> Result<Vec<String>> {
    let os = std::env::consts::OS;
    let user = user
        .map(|json| entries(json, os))
        .transpose()?
        .unwrap_or_default();
    // They ship with the app, so one that doesn't bind is a bug the tests catch; here it's
    // logged and the rest still bind.
    let defaults = entries(DEFAULT, os)
        .inspect_err(|err| tracing::error!("couldn't read the default keymap: {err:#}"))
        .unwrap_or_default();
    cx.clear_key_bindings();
    crate::ui::text_input::bind_keys(cx);
    let bindings: Vec<KeyBinding> = (defaults.iter())
        .filter_map(|entry| {
            binding(entry, cx)
                .inspect_err(|err| tracing::error!("default keymap, {:?}: {err:#}", entry.keys))
                .ok()
        })
        .collect();
    cx.bind_keys(bindings);
    let mut problems = Vec::new();
    let bindings: Vec<KeyBinding> = (user.iter())
        .filter_map(|entry| {
            binding(entry, cx)
                .inspect_err(|err| problems.push(format!("{:?}: {err:#}", entry.keys)))
                .ok()
        })
        .collect();
    cx.bind_keys(bindings);
    Ok(problems)
}

/// The bindings `json` makes on the platform `os`, in its order, a section's `unbind` first.
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
        let unbinds =
            (section.unbind.into_iter()).map(|(keys, name)| (keys, Value::String(name), true));
        let bindings = (section.bindings.into_iter()).map(|(keys, action)| (keys, action, false));
        entries.extend(unbinds.chain(bindings).map(|(keys, action, unbind)| Entry {
            keys,
            action,
            unbind,
            context: section.context.clone(),
        }));
    }
    Ok(entries)
}

fn binding(entry: &Entry, cx: &App) -> Result<KeyBinding> {
    let action: Box<dyn Action> = match (&entry.action, entry.unbind) {
        (Value::String(name), true) => match cx.all_action_names().contains(&name.as_str()) {
            true => Box::new(Unbind(name.clone().into())),
            false => bail!("there's no action {name:?} to unbind"),
        },
        (Value::Null, false) => Box::new(NoAction),
        (Value::String(name), false) => cx.build_action(name, None)?,
        (Value::Array(pair), false) => match pair.as_slice() {
            [Value::String(name), arguments] => cx.build_action(name, Some(arguments.clone()))?,
            _ => bail!("an action with arguments is written [name, arguments]"),
        },
        _ => bail!("an action is its name, [name, arguments], or null"),
    };
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
    use serde_json::Value;

    use super::{DEFAULT, PLATFORMS, TEMPLATE, entries};

    #[test]
    fn defaults_bind_on_every_platform() {
        let actions: HashSet<&str> = gpui::generate_list_of_all_registered_actions()
            .map(|action| action.name)
            .collect();
        for os in PLATFORMS {
            let mut bound = HashSet::new();
            for entry in entries(DEFAULT, os).expect("the default keymap parses") {
                let at = format!("{os}, {:?} in {:?}", entry.keys, entry.context);
                let name = entry.action.as_str().unwrap_or_default();
                assert!(
                    actions.contains(name),
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
        let actions = |os| -> Vec<Value> {
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

    #[test]
    fn a_section_unbinds_before_it_binds() {
        let json = r#"[{
            "context": "Diff",
            "bindings": { "n": "diff::NextHunk", "down": null },
            "unbind": { "j": "diff::NextFile" }
        }]"#;
        let entries = entries(json, "macos").expect("valid keymap");
        let read: Vec<_> = (entries.iter())
            .map(|entry| (entry.keys.as_str(), entry.unbind, &entry.action))
            .collect();
        assert_eq!(
            read,
            [
                ("j", true, &Value::from("diff::NextFile")),
                ("n", false, &Value::from("diff::NextHunk")),
                ("down", false, &Value::Null),
            ],
            "unbinding first lets the section give the freed keys to another action"
        );
    }

    #[test]
    fn a_new_keymap_binds_nothing() {
        let entries = entries(TEMPLATE, "macos").expect("the template is a keymap");
        assert!(entries.is_empty(), "it only shows how a section looks");
    }
}
