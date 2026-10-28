//! The app's keys: `assets/keymaps/default.json`, then the user's `keymap.json` in the settings
//! folder over it. Both are written in Zed's keymap format: a list of sections, each binding keys
//! to actions by name in a key context. A section with `os` binds only on the platforms it names.

use std::{collections::HashSet, path::PathBuf, rc::Rc, time::Duration};

use anyhow::{Context as _, Result, bail};
use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    Action, App, BorrowAppContext, Entity, Global, KeyBinding, KeyBindingContextPredicate,
    NoAction, Unbind, actions,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::backend::{self, keymap, settings};
use crate::state::{Notice, Store};

actions!(app, [OpenKeymap, OpenDefaultKeymap]);

const DEFAULT: &str = include_str!("../assets/keymaps/default.json");

/// What a new `keymap.json` starts as, and Reset all leaves.
pub const TEMPLATE: &str = r#"// Your keys, over GitButler's own. Each section binds keys to actions in a context, as Zed's
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

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Section {
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    os: Option<Vec<String>>,
    /// Keys taken away from the action each names, wherever it's bound.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    unbind: IndexMap<String, String>,
    /// Keys to the action each runs, null for none, in the file's order: the later binding
    /// wins, and is the one shown.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    bindings: IndexMap<String, Option<String>>,
}

/// The user's `keymap.json` as last read or written, `None` when there's none, which Settings ›
/// Keyboard shows as changed and changes, and how to have it read and bound again.
pub struct UserKeymap {
    pub json: Option<String>,
    reread: mpsc::UnboundedSender<()>,
}

impl Global for UserKeymap {}

/// One binding, as the file writes it.
struct Entry {
    keys: String,
    bound: Bound,
    context: Option<String>,
}

/// What a binding does to its keys.
#[derive(Debug, PartialEq)]
enum Bound {
    /// Run the action of that name.
    Run(String),
    /// Nothing, so they no longer run what the defaults bound them to: null in the file.
    Off,
    /// No longer run the action of that name, wherever it's bound.
    Unbind(String),
}

impl Entry {
    fn runs(&self, action: &str) -> bool {
        matches!(&self.bound, Bound::Run(name) if name == action)
    }
}

/// Binds the keys, then the user's `keymap.json` over them once it's read, and again each time
/// that file changes. What in it doesn't bind goes on a card. Also handles the commands that open
/// the keymaps.
pub fn watch(store: Entity<Store>, set_menus: impl Fn(&mut App) + 'static, cx: &mut App) {
    let reload = Rc::new({
        let store = store.clone();
        move |user: Result<Option<String>>, changed: bool, cx: &mut App| {
            if let Ok(json) = &user {
                cx.update_global::<UserKeymap, _>(|keymap, _| keymap.json = json.clone());
            }
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
    let (reread, mut rereads) = mpsc::unbounded();
    cx.set_global(UserKeymap {
        json: None,
        reread: reread.clone(),
    });
    // The defaults now, so the window has keys before the file is read, which it is next.
    reload(Ok(None), false, cx);
    reread.unbounded_send(()).ok();
    cx.on_action({
        let store = store.clone();
        move |_: &OpenKeymap, cx| open(store.clone(), || keymap::create(TEMPLATE), cx)
    });
    cx.on_action(move |_: &OpenDefaultKeymap, cx| {
        let file = || keymap::temporary("gitbutler-default-keymap.json", DEFAULT);
        open(store.clone(), file, cx)
    });
    cx.spawn(async move |cx| {
        // Settings › Keyboard asks for a read too, so its changes bind even unwatched.
        let watching = backend::run(move || {
            keymap::watch(move || {
                reread.unbounded_send(()).ok();
            })
        });
        let _watcher = (watching.await)
            .inspect_err(|err| tracing::warn!("couldn't watch keymap.json: {err:#}"))
            .ok();
        let mut first = true;
        while rereads.next().await.is_some() {
            // An editor saves in several writes; read the file once they're done.
            if !first {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }
            while rereads.try_recv().is_ok() {}
            let user = backend::run(keymap::read).await;
            cx.update(|cx| reload(user, !first, cx));
            first = false;
        }
    })
    .detach();
}

/// The actions whose keys the user's keymap `json` changes, with those that had a key it turns off.
pub fn changed_actions(json: &str) -> HashSet<String> {
    let os = std::env::consts::OS;
    let defaults = entries(DEFAULT, os).unwrap_or_default();
    let user = entries(json, os).unwrap_or_default();
    (user.into_iter())
        .flat_map(|entry| match entry.bound {
            Bound::Run(name) | Bound::Unbind(name) => vec![name],
            Bound::Off => (defaults.iter())
                .filter(|default| default.keys == entry.keys)
                .filter_map(|default| match &default.bound {
                    Bound::Run(name) => Some(name.clone()),
                    _ => None,
                })
                .collect(),
        })
        .collect()
}

/// The user's keymap `json`, `None` when there's none yet, with `action` on `keys`: instead of
/// the keys it has, or with `add`, beside them, and taken from the actions `from` that ran them.
/// It goes where `action`'s keys work, as [`default_context`] says, and out of text fields when
/// `keys` would type there, as every such default key is.
pub fn rebind(
    json: Option<&str>,
    action: &str,
    keys: &str,
    add: bool,
    from: &[&str],
) -> Result<String> {
    let mut sections = user_sections(json)?;
    let mut unbinds: Vec<IndexMap<String, String>> = Vec::new();
    if !add {
        forget(&mut sections, action);
        // But the new key, which an unbind in no context would take off it again.
        unbinds.push(
            (default_keys(action)?.into_iter())
                .filter(|default| default != keys)
                .map(|default| (default, action.to_owned()))
                .collect(),
        );
    }
    // A section each, as they unbind the same keys.
    unbinds.extend(
        from.iter()
            .map(|from| IndexMap::from([(keys.to_owned(), (*from).to_owned())])),
    );
    // In no context, as an unbind only applies where its context does, and a key bound in none
    // beats one bound in any: `diff::NextFile` is ↓ in the file list and j in the diff, and ⌘K
    // opens the palette everywhere.
    for unbind in unbinds.into_iter().filter(|unbind| !unbind.is_empty()) {
        sections.push(Section {
            context: None,
            os: None,
            unbind,
            bindings: IndexMap::new(),
        });
    }
    let types = !["secondary-", "ctrl-", "alt-"]
        .iter()
        .any(|modifier| keys.contains(modifier));
    let context = match default_context(action) {
        Some(context) if types && !context.contains("TextInput") => {
            Some(format!("({context}) && !TextInput"))
        }
        None if types => Some("Workspace && !TextInput".to_owned()),
        context => context,
    };
    sections.push(Section {
        context,
        os: None,
        unbind: IndexMap::new(),
        bindings: IndexMap::from([(keys.to_owned(), Some(action.to_owned()))]),
    });
    written(json, sections)
}

/// Where `action`'s keys work: the context of the default key it shows, its last, which a key the
/// user gives it goes in too, so it works where that did; `None` for anywhere.
pub fn default_context(action: &str) -> Option<String> {
    let defaults = entries(DEFAULT, std::env::consts::OS).ok()?;
    (defaults.into_iter().rev())
        .find(|entry| entry.runs(action))
        .and_then(|entry| entry.context)
}

/// The user's keymap `json` saying nothing about `action`, which has its default keys again.
pub fn reset(json: &str, action: &str) -> Result<String> {
    let mut sections = user_sections(Some(json))?;
    let keys = default_keys(action)?;
    for section in &mut sections {
        (section.bindings).retain(|key, bound| bound.is_some() || !keys.contains(key));
    }
    forget(&mut sections, action);
    written(Some(json), sections)
}

/// The keys the default keymap gives `action`.
fn default_keys(action: &str) -> Result<Vec<String>> {
    let defaults = entries(DEFAULT, std::env::consts::OS)?;
    Ok((defaults.into_iter())
        .filter(|entry| entry.runs(action))
        .map(|entry| entry.keys)
        .collect())
}

/// Write `json` as the user's keymap, then read and bind it.
pub fn save(json: String, store: Entity<Store>, cx: &mut App) {
    let reread = cx.update_global::<UserKeymap, _>(|keymap, _| {
        keymap.json = Some(json.clone());
        keymap.reread.clone()
    });
    cx.spawn(
        async move |cx| match backend::run(move || keymap::write(&json)).await {
            Ok(()) => _ = reread.unbounded_send(()),
            Err(err) => store.update(cx, |store, cx| {
                store.failed("Couldn't change the keys", &err, cx)
            }),
        },
    )
    .detach();
}

fn user_sections(json: Option<&str>) -> Result<Vec<Section>> {
    Ok(match json {
        Some(json) => serde_json_lenient::from_str(json).context("keymap.json isn't a keymap")?,
        None => Vec::new(),
    })
}

/// Take what `sections` say about `action` out of them, and the sections left saying nothing.
fn forget(sections: &mut Vec<Section>, action: &str) {
    for section in sections.iter_mut() {
        section.unbind.retain(|_, name| name != action);
        (section.bindings).retain(|_, bound| bound.as_deref() != Some(action));
    }
    sections.retain(|section| !section.unbind.is_empty() || !section.bindings.is_empty());
}

/// `sections` as the user's keymap, under the comments that headed `json`, or the template's.
fn written(json: Option<&str>, sections: Vec<Section>) -> Result<String> {
    let header: String = (json.unwrap_or(TEMPLATE).lines())
        .take_while(|line| line.trim().is_empty() || line.trim_start().starts_with("//"))
        .map(|line| format!("{line}\n"))
        .collect();
    Ok(format!(
        "{header}{}\n",
        serde_json::to_string_pretty(&sections)?
    ))
}

/// Opens the keymap `file` makes in the program the system opens it with.
fn open(
    store: Entity<Store>,
    file: impl FnOnce() -> Result<PathBuf> + Send + 'static,
    cx: &mut App,
) {
    let editor = store.read(cx).prefs.editor.clone();
    // In the editor chosen for files, or with none chosen, the system's app for JSON.
    let open = move || {
        let path = file()?;
        match editor {
            Some(editor) => settings::open_file_in_editor(&editor, &path).map(|()| None),
            None => Ok(Some(path)),
        }
    };
    cx.spawn(async move |cx| match backend::run(open).await {
        Ok(Some(path)) => cx.update(|cx| cx.open_with_system(&path)),
        Ok(None) => {}
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
        let unbinds = (section.unbind.into_iter()).map(|(keys, name)| (keys, Bound::Unbind(name)));
        let bindings = (section.bindings.into_iter())
            .map(|(keys, action)| (keys, action.map_or(Bound::Off, Bound::Run)));
        entries.extend(unbinds.chain(bindings).map(|(keys, bound)| Entry {
            keys,
            bound,
            context: section.context.clone(),
        }));
    }
    Ok(entries)
}

fn binding(entry: &Entry, cx: &App) -> Result<KeyBinding> {
    let action: Box<dyn Action> = match &entry.bound {
        Bound::Run(name) => cx.build_action(name, None)?,
        Bound::Off => Box::new(NoAction),
        Bound::Unbind(name) if cx.all_action_names().contains(&name.as_str()) => {
            Box::new(Unbind(name.clone().into()))
        }
        Bound::Unbind(name) => bail!("there's no action {name:?} to unbind"),
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

    use super::{Bound, DEFAULT, PLATFORMS, TEMPLATE, changed_actions, entries, rebind, reset};

    #[test]
    fn defaults_bind_on_every_platform() {
        let actions: HashSet<&str> = gpui::generate_list_of_all_registered_actions()
            .map(|action| action.name)
            .collect();
        for os in PLATFORMS {
            let mut bound = HashSet::new();
            for entry in entries(DEFAULT, os).expect("the default keymap parses") {
                let at = format!("{os}, {:?} in {:?}", entry.keys, entry.context);
                let name = match &entry.bound {
                    Bound::Run(name) => name.as_str(),
                    bound => panic!("{at}: the defaults only run actions, not {bound:?}"),
                };
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
                    .find(|entry| entry.runs(action))
                    .map(|entry| entry.keys.as_str());
                assert_eq!(last, Some(keys), "{os}: {action} shows its last binding");
            }
        }
    }

    #[test]
    fn go_to_branch_still_names_go_to() {
        let go_to = gpui::generate_list_of_all_registered_actions()
            .find(|action| action.name == "palette::GoTo")
            .expect("go to anything is an action");
        assert!(
            go_to.deprecated_aliases.contains(&"palette::GoToBranch"),
            "a keymap.json written while it went only to branches still binds it"
        );
    }

    #[test]
    fn sections_bind_only_on_their_platforms() {
        let json = r#"[
            { "bindings": { "a": "x::Anywhere" } },
            { "os": ["macos"], "bindings": { "b": "x::Mac" } },
            { "os": ["windows", "linux"], "bindings": { "c": "x::Elsewhere" } }
        ]"#;
        let actions = |os| -> Vec<Bound> {
            let entries = entries(json, os).expect("valid keymap");
            entries.into_iter().map(|entry| entry.bound).collect()
        };
        let run = |name: &str| Bound::Run(name.into());
        assert_eq!(
            actions("macos"),
            [run("x::Anywhere"), run("x::Mac")],
            "macOS skips the others"
        );
        assert_eq!(
            actions("linux"),
            [run("x::Anywhere"), run("x::Elsewhere")],
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
            .map(|entry| (entry.keys.as_str(), &entry.bound))
            .collect();
        assert_eq!(
            read,
            [
                ("j", &Bound::Unbind("diff::NextFile".into())),
                ("n", &Bound::Run("diff::NextHunk".into())),
                ("down", &Bound::Off),
            ],
            "unbinding first lets the section give the freed keys to another action"
        );
    }

    #[test]
    fn rebinding_replaces_the_default_keys_and_keeps_the_comments() {
        let json = "// Mine.\n[]\n";
        let rebound = rebind(Some(json), "app::CommandPalette", "secondary-j", false, &[])
            .expect("an empty keymap rebinds");
        assert!(
            rebound.starts_with("// Mine.\n"),
            "the heading comment stays: {rebound}"
        );
        let read = entries(&rebound, std::env::consts::OS).expect("it writes a keymap");
        let unbound: Vec<(&str, Option<&str>)> = (read.iter())
            .filter(|entry| matches!(entry.bound, Bound::Unbind(_)))
            .map(|entry| (entry.keys.as_str(), entry.context.as_deref()))
            .collect();
        assert_eq!(
            unbound,
            [
                (":", None),
                ("secondary-shift-p", None),
                ("secondary-k", None)
            ],
            "every default key comes off the action, wherever it was bound"
        );
        let bound = read
            .iter()
            .find(|entry| matches!(entry.bound, Bound::Run(_)));
        assert!(
            bound.is_some_and(|entry| entry.keys == "secondary-j"
                && entry.runs("app::CommandPalette")
                && entry.context.is_none()),
            "and the new key goes where ⌘K worked, typing too"
        );
        let again = rebind(
            Some(&rebound),
            "app::CommandPalette",
            "secondary-l",
            false,
            &[],
        )
        .expect("a keymap rebinds");
        let again = entries(&again, std::env::consts::OS).expect("it writes a keymap");
        assert!(
            !again.iter().any(|entry| entry.keys == "secondary-j"),
            "rebinding again replaces the user's own key too"
        );
        let own = rebind(None, "remote::Push", "shift-p", false, &[]).expect("rebinds");
        let own = entries(&own, std::env::consts::OS).expect("it writes a keymap");
        assert!(
            !own.iter()
                .any(|entry| entry.keys == "shift-p" && !entry.runs("remote::Push")),
            "a default key given again isn't unbound as well"
        );
        let commit =
            rebind(None, "app::StartCommit", "secondary-shift-c", false, &[]).expect("rebinds");
        let commit = entries(&commit, std::env::consts::OS).expect("it writes a keymap");
        assert!(
            commit.iter().any(|entry| entry.keys == "secondary-shift-c"
                && entry.context.as_deref() == Some("Workspace && !TextInput")),
            "Commit's key works in the whole window, not only in the file filter it also starts from"
        );
    }

    #[test]
    fn resetting_an_action_forgets_only_it() {
        let json = rebind(None, "app::CommandPalette", "secondary-j", false, &[]).expect("rebinds");
        let json = rebind(
            Some(&json),
            "remote::Push",
            "shift-u",
            true,
            &["navigator::MarkAllRead"],
        )
        .expect("adds");
        assert_eq!(
            changed_actions(&json),
            [
                "app::CommandPalette",
                "remote::Push",
                "navigator::MarkAllRead"
            ]
            .map(String::from)
            .into(),
            "both are changed, and the action the key was taken from"
        );
        let read = entries(&json, std::env::consts::OS).expect("it writes a keymap");
        assert!(
            read.iter().any(|entry| entry.keys == "shift-u"
                && entry.bound == Bound::Unbind("navigator::MarkAllRead".into())
                && entry.context.is_none()),
            "taking a key takes it from the action it ran, in no context so it beats any: {json}"
        );
        let json = reset(&json, "app::CommandPalette").expect("resets");
        assert_eq!(
            changed_actions(&json),
            ["remote::Push", "navigator::MarkAllRead"]
                .map(String::from)
                .into(),
            "the other change stays"
        );
        assert!(
            json.starts_with("// Your keys"),
            "a new keymap starts from the template"
        );
    }

    #[test]
    fn a_key_that_types_stays_out_of_text_fields_and_comes_off_every_action() {
        let taken = ["navigator::SelectNext", "diff::NextFile"];
        let json = rebind(None, "app::CommandPalette", "j", false, &taken).expect("rebinds");
        let read = entries(&json, std::env::consts::OS).expect("it writes a keymap");
        let bound = read.iter().find(|entry| entry.runs("app::CommandPalette"));
        assert_eq!(
            bound.and_then(|entry| entry.context.as_deref()),
            Some("Workspace && !TextInput"),
            "j would type in a text field"
        );
        for from in taken {
            assert!(
                read.iter().any(|entry| entry.keys == "j"
                    && entry.bound == Bound::Unbind(from.into())
                    && entry.context.is_none()),
                "{from} loses j everywhere: {json}"
            );
        }
    }

    #[test]
    fn a_key_turned_off_is_a_change_that_resets() {
        let json = r#"[{ "bindings": { "secondary-k": null } }]"#;
        assert!(
            changed_actions(json).contains("app::CommandPalette"),
            "the action that had the key is changed"
        );
        let json = reset(json, "app::CommandPalette").expect("resets");
        assert!(
            changed_actions(&json).is_empty(),
            "and gets it back: {json}"
        );
    }

    #[test]
    fn a_new_keymap_binds_nothing() {
        let entries = entries(TEMPLATE, "macos").expect("the template is a keymap");
        assert!(entries.is_empty(), "it only shows how a section looks");
    }
}
