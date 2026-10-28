//! Settings › Keyboard: every command and the keys that run it.

use std::collections::{BTreeMap, HashSet};

use gpui::prelude::*;
use gpui::{Action, App, Context, SharedString};

use super::{Card, Row, SettingsView};
use crate::ui::{self, dp, h_flex};

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

impl SettingsView {
    pub(super) fn keyboard_cards(&self, cx: &Context<Self>) -> Vec<Card> {
        let theme = cx.global::<crate::theme::Theme>().clone();
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
                .push(command_row(title, action, &theme, cx));
        }
        (groups.into_iter())
            .map(|(group, rows)| Card::new(GROUPS.get(group).map(|(_, heading)| *heading), rows))
            .collect()
    }
}

fn command_row(
    title: SharedString,
    action: Box<dyn Action>,
    theme: &crate::theme::Theme,
    cx: &App,
) -> Row {
    let name = action.name();
    let bindings = ui::bindings_of(action.as_ref(), cx);
    let mut keys: Vec<SharedString> = Vec::new();
    for (key, _) in &bindings {
        if !keys.contains(key) {
            keys.push(key.clone());
        }
    }
    let description = match bindings.is_empty() {
        true => "No keys".to_owned(),
        false => places(bindings.iter().map(|(_, context)| context.as_deref())),
    };
    let control = h_flex()
        .h(dp(26.))
        .px(dp(6.))
        .gap(dp(8.))
        .text_size(dp(12.))
        .children(keys.iter().enumerate().map(|(ix, key)| {
            h_flex()
                .child(ui::keycap(key, theme))
                .when(ix + 1 < keys.len(), |el| el.child(ui::muted(",", theme)))
        }));
    // Found by its keys as written (⌘B), and as typed (cmd b) or bound (secondary-b).
    let words: Vec<String> = (bindings.iter())
        .map(|(key, _)| format!("{key} {}", ui::keys_text(key)))
        .chain(std::iter::once(name.replace("::", " ")))
        .collect();
    Row::new(title, description, control).with_keywords(words.join(" "))
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
