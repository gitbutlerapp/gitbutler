//! Settings › Keyboard: every command and the keys that run it. Clicking a command's keys and
//! pressing new ones writes them to the user's `keymap.json`, which the app binds over its own.

use std::collections::{BTreeMap, HashSet};

use gpui::prelude::*;
use gpui::{Action, App, ClickEvent, Context, Keystroke, SharedString, Subscription, Window, div};

use super::{Card, ROW_GROUP, Row, SettingsView};
use crate::keymap::{self, UserKeymap};
use crate::overlays::MenuItem;
use crate::ui::{self, ButtonStyle, Icon, button, dp, h_flex, icon_button};

/// Commands by the area their name starts with, in the order the page lists them.
const GROUPS: [(&str, &str); 9] = [
    ("navigator", "Navigator"),
    ("diff", "Diff and files"),
    ("find", "Find"),
    ("remote", "Remote"),
    ("pr", "Pull requests"),
    ("edits", "Commits and branches"),
    ("conflict", "Conflicts"),
    ("palette", "Go to"),
    ("app", "App"),
];

/// Actions that are only a field's or a list's own, which the page leaves out.
const UNLISTED: [&str; 5] = [
    "text_input::",
    "whats_new::",
    "image_preview::",
    "palette::Palette",
    "zed::",
];

/// The keys the next keystroke gives a command, while the page waits for them.
pub(super) struct Recording {
    action: &'static str,
    /// Beside the keys it has, rather than instead of them.
    add: bool,
    /// Keys pressed that run other actions where these would, and what those are called:
    /// pressing them again takes them from those.
    taken: Option<(String, SharedString)>,
    _keys: Subscription,
}

impl SettingsView {
    pub(super) fn keyboard_cards(&self, cx: &Context<Self>) -> Vec<Card> {
        let theme = cx.global::<crate::theme::Theme>().clone();
        let changed = (user_keymap(cx))
            .map(keymap::changed_actions)
            .unwrap_or_default();
        let file = Row::new(
            "keymap.json",
            "Your keys, over the app's own, in the settings folder. The app reads it again each \
             time it's saved.",
            h_flex()
                .gap(dp(8.))
                .when(!changed.is_empty(), |el| {
                    el.child(
                        button("keys-reset-all", "Reset all")
                            .style(ButtonStyle::Ghost)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                keymap::save(keymap::TEMPLATE.to_owned(), this.store.clone(), cx)
                            })),
                    )
                })
                .child(
                    button("keys-open", "Open").on_click(|_: &ClickEvent, window, cx| {
                        window.dispatch_action(Box::new(keymap::OpenKeymap), cx)
                    }),
                ),
        );
        let mut groups: BTreeMap<usize, Vec<Row>> = BTreeMap::new();
        for (title, action) in commands(cx) {
            let name = action.name();
            let area = name.split("::").next().unwrap_or_default();
            let group = GROUPS
                .iter()
                .position(|(a, _)| *a == area)
                .unwrap_or(GROUPS.len() - 1);
            groups
                .entry(group)
                .or_default()
                .push(self.command_row(title, action, &changed, &theme, cx));
        }
        std::iter::once(Card::new(None, vec![file]))
            .chain((groups.into_iter()).map(|(group, rows)| {
                Card::new(GROUPS.get(group).map(|(_, heading)| *heading), rows)
            }))
            .collect()
    }

    fn command_row(
        &self,
        title: SharedString,
        action: Box<dyn Action>,
        changed: &HashSet<String>,
        theme: &crate::theme::Theme,
        cx: &Context<Self>,
    ) -> Row {
        let name = action.name();
        let bindings = ui::bindings_of(action.as_ref(), cx);
        let mut keys: Vec<SharedString> = Vec::new();
        for (key, _) in &bindings {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        let places = places(bindings.iter().map(|(_, context)| context.as_deref()));
        let changed = changed.contains(name);
        let recording = (self.recording.as_ref()).filter(|recording| recording.action == name);
        let description: SharedString = match recording {
            Some(Recording {
                taken: Some((keys, other)),
                ..
            }) => format!(
                "{} already runs {other} there. Press it again to use it here, or Escape.",
                ui::keys_text(&ui::keys_written(keys, cx).unwrap_or_default())
            )
            .into(),
            Some(_) => "Press the keys for it, or Escape.".into(),
            None => match (bindings.is_empty(), changed) {
                (true, true) => "No keys · Changed".into(),
                (true, false) => "No keys".into(),
                (false, true) => format!("{places} · Changed").into(),
                (false, false) => places.into(),
            },
        };
        // The keys, which a click records new ones for, as a hotkey in Raycast or a key in
        // Xcode. The rest is in the ⋯ menu, as on a row in VS Code's and Zed's keymap editors.
        let field = h_flex()
            .id(SharedString::from(format!("keys-{name}")))
            .h(dp(26.))
            .px(dp(6.))
            .gap(dp(8.))
            .rounded(dp(6.))
            .text_size(dp(12.))
            .cursor_pointer()
            .map(|el| match recording {
                Some(_) => el.border_1().border_color(theme.pop),
                None => el.hover(|style| style.bg(theme.hover())),
            })
            .map(|el| match (recording, keys.is_empty()) {
                (Some(_), _) => el.child("Press keys…"),
                (None, true) => el.child(ui::muted("Add shortcut", theme)),
                (None, false) => el.children(keys.iter().enumerate().map(|(ix, key)| {
                    h_flex()
                        .child(ui::keycap(key, theme))
                        .when(ix + 1 < keys.len(), |el| el.child(ui::muted(",", theme)))
                })),
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.record(name, false, window, cx)
            }));
        let bound = !keys.is_empty();
        let menu = icon_button(SharedString::from(format!("keys-menu-{name}")), Icon::Kebab)
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                this.open_menu(event, move |_, cx| key_menu(name, bound, changed, cx), cx)
            }));
        let control = h_flex()
            .gap(dp(4.))
            .child(field)
            .when(recording.is_none(), |el| {
                el.child(
                    div()
                        .opacity(0.)
                        .group_hover(ROW_GROUP, |s| s.opacity(1.))
                        .child(menu),
                )
            })
            .into_any_element();
        // Found by its keys as written (⌘B), and as typed (cmd b) or bound (secondary-b).
        let words: Vec<String> = (bindings.iter())
            .map(|(key, _)| format!("{key} {}", ui::keys_text(key)))
            .chain(std::iter::once(name.replace("::", " ")))
            .collect();
        Row::new(title, description, control).with_keywords(words.join(" "))
    }

    /// Wait for the keys to give `action`, taking every key pressed until then from the app.
    fn record(&mut self, action: &'static str, add: bool, _: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let keys = cx.intercept_keystrokes(move |event, window, cx| {
            cx.stop_propagation();
            (view.update(cx, |this, cx| this.pressed(&event.keystroke, window, cx))).ok();
        });
        self.recording = Some(Recording {
            action,
            add,
            taken: None,
            _keys: keys,
        });
        cx.notify();
    }

    fn pressed(&mut self, keystroke: &Keystroke, _: &mut Window, cx: &mut Context<Self>) {
        let Some(recording) = self.recording.as_ref() else {
            return;
        };
        if keystroke.key == "escape" && !keystroke.modifiers.modified() {
            self.recording = None;
            return cx.notify();
        }
        let (action, add) = (recording.action, recording.add);
        let keys = written(keystroke);
        let taken = taken_by(action, keystroke, cx);
        let again = (recording.taken.as_ref()).is_some_and(|(taken, _)| *taken == keys);
        match taken.as_slice() {
            [first, rest @ ..] if !again => {
                let others = match rest.len() {
                    0 => title_of(first),
                    more => format!("{} and {more} more", title_of(first)),
                };
                if let Some(recording) = self.recording.as_mut() {
                    recording.taken = Some((keys, others.into()));
                }
                cx.notify()
            }
            _ => {
                self.recording = None;
                match keymap::rebind(user_keymap(cx), action, &keys, add, &taken) {
                    Ok(json) => keymap::save(json, self.store.clone(), cx),
                    Err(err) => self.keys_failed(&err, cx),
                }
            }
        }
    }

    fn reset_keys(&mut self, action: &str, cx: &mut Context<Self>) {
        let Some(json) = user_keymap(cx) else {
            return;
        };
        match keymap::reset(json, action) {
            Ok(json) => keymap::save(json, self.store.clone(), cx),
            Err(err) => self.keys_failed(&err, cx),
        }
    }

    fn keys_failed(&self, err: &anyhow::Error, cx: &mut Context<Self>) {
        (self.store).update(cx, |store, cx| {
            store.failed("Couldn't change the keys", err, cx)
        });
    }
}

/// What a command's ⋯ offers, named as VS Code's keymap editor names them.
fn key_menu(
    name: &'static str,
    bound: bool,
    changed: bool,
    cx: &mut Context<SettingsView>,
) -> Vec<MenuItem> {
    let view = cx.entity().downgrade();
    let record = |label: &str, add: bool| {
        let view = view.clone();
        MenuItem::new(label.to_owned(), move |_, window, cx| {
            (view.update(cx, |this, cx| this.record(name, add, window, cx))).ok();
        })
    };
    let mut items = match bound {
        true => vec![
            record("Change shortcut", false),
            record("Add another shortcut", true),
        ],
        false => vec![record("Add shortcut", false)],
    };
    if changed {
        items.push(
            MenuItem::new("Reset to default", move |_, _, cx| {
                (view.update(cx, |this, cx| this.reset_keys(name, cx))).ok();
            })
            .separated(),
        );
    }
    items
}

fn user_keymap(cx: &App) -> Option<&str> {
    cx.try_global::<UserKeymap>()?.json.as_deref()
}

/// The commands the page lists: the palette's, then every other action a key runs, named after
/// its own name.
fn commands(cx: &App) -> Vec<(SharedString, Box<dyn Action>)> {
    let mut listed: Vec<(SharedString, Box<dyn Action>)> = (crate::palette::COMMANDS.iter())
        .map(|(title, action)| (SharedString::from(*title), action.boxed_clone()))
        .collect();
    let mut names: HashSet<&'static str> = listed.iter().map(|(_, a)| a.name()).collect();
    let keymap = cx.key_bindings();
    for binding in keymap.borrow().bindings() {
        let name = binding.action().name();
        if UNLISTED.iter().any(|prefix| name.starts_with(prefix)) || !names.insert(name) {
            continue;
        }
        listed.push((title_of(name).into(), binding.action().boxed_clone()));
    }
    listed
}

/// What the palette calls the action named `name`, or its name in words: `navigator::SelectNext`
/// is "Select next".
fn title_of(name: &str) -> String {
    if let Some((title, _)) = (crate::palette::COMMANDS.iter()).find(|(_, a)| a.name() == name) {
        return (*title).to_owned();
    }
    let last = name.rsplit("::").next().unwrap_or(name);
    let mut words = String::new();
    for (ix, c) in last.char_indices() {
        if c.is_uppercase() && ix > 0 {
            words.push(' ');
            words.extend(c.to_lowercase());
        } else {
            words.push(c);
        }
    }
    words
}

/// Where keys bound in `contexts` work, in words.
fn places<'a>(contexts: impl Iterator<Item = Option<&'a str>>) -> String {
    let mut said: Vec<&str> = Vec::new();
    for context in contexts {
        let named = match context {
            None => vec!["Anywhere"],
            Some(context) => (identifiers(context).into_iter())
                .filter_map(|id| place(&id))
                .collect(),
        };
        for name in named {
            if !said.contains(&name) {
                said.push(name);
            }
        }
    }
    said.join(", ")
}

fn place(context: &str) -> Option<&'static str> {
    Some(match context {
        "Workspace" => "Main window",
        "Navigator" => "Navigator",
        "Diff" => "Diff",
        "Files" => "File list",
        "List" => "Branches and Operations history",
        "Composer" => "While committing",
        "Conflicts" | "Picked" => "Conflicts",
        "FindBar" => "Find bar",
        "Palette" => "Palette",
        _ => return None,
    })
}

/// The key contexts a binding's `context` names, but the text field and those it's kept out of.
fn identifiers(context: &str) -> Vec<String> {
    (context.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '!'))
        .filter(|id| !id.is_empty() && !id.starts_with('!') && *id != "TextInput")
        .map(str::to_owned)
        .collect()
}

/// The other actions bound to `keystroke` where `action`'s keys would go.
fn taken_by(action: &str, keystroke: &Keystroke, cx: &App) -> Vec<&'static str> {
    // Every other context is inside the window's, so a key bound there meets them all, as one
    // bound in none does.
    let within = |context: Option<String>| {
        (context.map(|context| identifiers(&context))).filter(|ids| *ids != ["Workspace"])
    };
    let ours = within(keymap::default_context(action));
    let mut taken = Vec::new();
    for binding in cx.all_bindings_for_input(std::slice::from_ref(keystroke)) {
        let name = binding.action().name();
        if name == action || name.starts_with("zed::") || taken.contains(&name) {
            continue;
        }
        let theirs = within(binding.predicate().map(|p| p.to_string()));
        let overlap = match (&ours, &theirs) {
            (Some(ours), Some(theirs)) => ours.iter().any(|id| theirs.contains(id)),
            _ => true,
        };
        if overlap {
            taken.push(name);
        }
    }
    taken
}

/// `keystroke` as the keymap writes keys, with `secondary` for the platform's command key so the
/// file works on every platform.
fn written(keystroke: &Keystroke) -> String {
    let m = &keystroke.modifiers;
    let mac = cfg!(target_os = "macos");
    let secondary = if mac { m.platform } else { m.control };
    let parts = [
        (secondary, "secondary"),
        (mac && m.control, "ctrl"),
        (m.alt, "alt"),
        (m.shift, "shift"),
    ];
    let mut keys: Vec<&str> = parts
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| *name)
        .collect();
    keys.push(&keystroke.key);
    keys.join("-")
}

#[cfg(test)]
mod tests {
    use super::{places, title_of};

    #[test]
    fn commands_without_a_palette_title_are_named_from_their_action() {
        assert_eq!(
            title_of("navigator::SelectNext"),
            "Select next",
            "words from the name"
        );
        assert_eq!(
            title_of("remote::Push"),
            "Push branch",
            "the palette's own title first"
        );
    }

    #[test]
    fn places_name_where_keys_work() {
        let contexts = [
            Some("(Navigator || Diff) && !TextInput"),
            None,
            Some("Diff"),
            Some("Files && !Composer > TextInput"),
        ];
        assert_eq!(
            places(contexts.into_iter()),
            "Navigator, Diff, Anywhere, File list",
            "each place once, the text field and places kept out of left out"
        );
    }
}
