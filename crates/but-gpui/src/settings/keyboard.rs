//! Settings › Keyboard: every command and the keys that run it.

use std::collections::{BTreeMap, HashSet};

use gpui::prelude::*;
use gpui::{
    Action, AnyElement, App, Context, KeymapVersion, List, ListAlignment, ListState, SharedString,
    div, list,
};

use super::{Card, Row, SettingsView};
use crate::ui::{self, dp, fixed, h_flex};

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

/// The page's state: a list, so only the rows in view are laid out, as a row for each of a
/// couple of hundred commands takes longer than a frame.
pub(super) struct Keyboard {
    lines: Vec<Line>,
    /// The keymap `lines` were read from, as the command editor changes it.
    keymap: Option<KeymapVersion>,
    /// The interface scale the list's rows were measured at.
    scale: f32,
    pub(super) list: ListState,
}

impl Default for Keyboard {
    fn default() -> Self {
        Keyboard {
            lines: Vec::new(),
            keymap: None,
            scale: 0.,
            list: ListState::new(0, ListAlignment::Top, fixed(400.)),
        }
    }
}

/// A command and the keys that run it.
#[derive(Clone, PartialEq)]
struct Shortcut {
    title: SharedString,
    /// Where its keys work.
    description: SharedString,
    keys: Vec<SharedString>,
    /// What a search finds it by.
    keywords: SharedString,
}

/// A line of the list: a group's heading, or a command in the group's card, its first or last
/// row rounding the card's corners.
#[derive(PartialEq)]
enum Line {
    Heading(&'static str),
    Command {
        shortcut: Shortcut,
        first: bool,
        last: bool,
    },
}

impl SettingsView {
    pub(super) fn keyboard_cards(&self, cx: &Context<Self>) -> Vec<Card> {
        let theme = cx.global::<crate::theme::Theme>().clone();
        (shortcut_groups(cx).into_iter())
            .map(|(heading, shortcuts)| {
                let rows = (shortcuts.into_iter())
                    .map(|shortcut| shortcut_row(shortcut, &theme))
                    .collect();
                Card::new(Some(heading), rows)
            })
            .collect()
    }

    /// The page as a list, its lines read again once the keymap changes.
    pub(super) fn keyboard_list(&mut self, cx: &mut Context<Self>) -> List {
        let theme = crate::theme::Theme::get(cx);
        let keymap = cx.key_bindings().borrow().version();
        if self.keyboard.keymap != Some(keymap) {
            self.keyboard.keymap = Some(keymap);
            let lines = keyboard_lines(cx);
            if lines != self.keyboard.lines {
                // A row's height, with its line of where its keys work, until it's laid out,
                // for the scrollbar.
                (self.keyboard.list).reset_with_uniform_height(lines.len(), theme.scaled(75.));
                self.keyboard.lines = lines;
            }
        }
        // It keeps its rows' heights until its width changes, and the page may have been away
        // while the scale changed.
        if self.keyboard.scale != theme.scale {
            self.keyboard.scale = theme.scale;
            self.keyboard.list.remeasure();
        }
        list(
            self.keyboard.list.clone(),
            cx.processor(|this, ix, _, cx| this.render_keyboard_line(ix, cx)),
        )
    }

    /// A line as it would be in the page's cards, with their spacing around it.
    fn render_keyboard_line(&self, ix: usize, cx: &App) -> AnyElement {
        let theme = crate::theme::Theme::get(cx);
        let Some(line) = self.keyboard.lines.get(ix) else {
            return div().into_any_element();
        };
        let item = div()
            .px(dp(20.))
            .when(ix + 1 == self.keyboard.lines.len(), |el| el.pb(dp(20.)));
        match line {
            Line::Heading(heading) => item
                .pt(dp(if ix == 0 { 20. } else { 12. }))
                .pb(dp(8.))
                .child(
                    div()
                        .pl(dp(16.))
                        .text_size(dp(12.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text_3)
                        .child(*heading),
                ),
            Line::Command {
                shortcut,
                first,
                last,
            } => item.child(
                div()
                    .border_x_1()
                    .border_color(theme.border_3)
                    .bg(theme.bg_1)
                    .overflow_hidden()
                    .when(*first, |el| {
                        el.border_t_1().rounded_t(dp(theme.shape.popup))
                    })
                    .when(*last, |el| el.border_b_1().rounded_b(dp(theme.shape.popup)))
                    .child(Self::render_row(
                        shortcut_row(shortcut.clone(), theme),
                        !first,
                        theme,
                    )),
            ),
        }
        .into_any_element()
    }
}

/// Each group's heading, then its commands.
fn keyboard_lines(cx: &App) -> Vec<Line> {
    let mut lines = Vec::new();
    for (heading, shortcuts) in shortcut_groups(cx) {
        lines.push(Line::Heading(heading));
        let count = shortcuts.len();
        lines.extend(
            shortcuts
                .into_iter()
                .enumerate()
                .map(|(ix, shortcut)| Line::Command {
                    shortcut,
                    first: ix == 0,
                    last: ix + 1 == count,
                }),
        );
    }
    lines
}

/// Every command the page lists, under the heading of its group, in the page's order.
fn shortcut_groups(cx: &App) -> Vec<(&'static str, Vec<Shortcut>)> {
    let mut groups: BTreeMap<usize, Vec<Shortcut>> = BTreeMap::new();
    for (title, action) in commands(cx) {
        let area = action.name().split("::").next().unwrap_or_default();
        let group = GROUPS
            .iter()
            .position(|(a, _)| *a == area)
            .unwrap_or(GROUPS.len() - 1);
        groups
            .entry(group)
            .or_default()
            .push(shortcut(title, action, cx));
    }
    (groups.into_iter())
        .filter_map(|(group, shortcuts)| Some((GROUPS.get(group)?.1, shortcuts)))
        .collect()
}

fn shortcut_row(shortcut: Shortcut, theme: &crate::theme::Theme) -> Row {
    let keys = &shortcut.keys;
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
    Row::new(shortcut.title, shortcut.description, control).with_keywords(shortcut.keywords)
}

fn shortcut(title: SharedString, action: Box<dyn Action>, cx: &App) -> Shortcut {
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
    // Found by its keys as written (⌘B), and as typed (cmd b) or bound (secondary-b), and by
    // what else ⌘K finds it by.
    let aliases = (crate::commands::COMMANDS.iter())
        .find(|command| command.name() == name)
        .map_or(&[][..], |command| command.aliases());
    let words: Vec<String> = (bindings.iter())
        .map(|(key, _)| format!("{key} {}", ui::keys_text(key)))
        .chain(std::iter::once(name.replace("::", " ")))
        .chain(aliases.iter().map(|alias| alias.to_string()))
        .collect();
    Shortcut {
        title,
        description: description.into(),
        keys,
        keywords: words.join(" ").into(),
    }
}

/// The commands the page lists: the palette's, then every other action a key runs, named after
/// its own name.
fn commands(cx: &App) -> Vec<(SharedString, Box<dyn Action>)> {
    let mut listed: Vec<(SharedString, Box<dyn Action>)> = (crate::commands::COMMANDS.iter())
        .map(|command| (SharedString::from(command.title()), command.boxed_clone()))
        .collect();
    let mut names: HashSet<&'static str> = listed.iter().map(|(_, a)| a.name()).collect();
    let keymap = cx.key_bindings();
    for binding in keymap.borrow().bindings() {
        let name = binding.action().name();
        if UNLISTED.iter().any(|prefix| name.starts_with(prefix)) || !names.insert(name) {
            continue;
        }
        listed.push((
            crate::commands::title_of(name).into(),
            binding.action().boxed_clone(),
        ));
    }
    listed
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
        "Navigator" | "DrawerBar" => "Navigator",
        "Moving" => "While moving",
        "Squashing" => "While squashing",
        "Diff" => "Diff",
        "Graph" => "Graph",
        "Files" => "File list",
        "List" => "Branches and Operations history",
        "Composer" | "Committing" | "Amending" => "While committing",
        "Conflicts" | "Picked" => "Conflicts",
        "FindBar" => "Find bar",
        "Palette" => "Palette",
        "Settings" => "Settings",
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
    use super::places;

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
