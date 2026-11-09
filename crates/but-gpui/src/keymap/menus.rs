//! The menu bar, as `assets/keymaps/menus.json` lays it out: its menus in order, each a list of
//! actions by name with dividers between them. An item shows the keys GPUI finds for its action
//! in the keymap. The command editor's Menu bar tab writes the file, all of it at once, so it's
//! read leniently but written in one shape.

use anyhow::{Context as _, Result};
use gpui::{App, KeyContext, Menu, MenuItem, OsAction, SharedString, SystemMenuType};
use serde::{Deserialize, Serialize};

use crate::commands::title_of;
use crate::ui;

/// Offered only by a build that looks for updates.
const CHECK_FOR_UPDATES: &str = "app::CheckForUpdates";

/// What the file says above its menus, written again each time it's saved.
const HEADER: &str = "\
// The menu bar, menu by menu. macOS names the first after the app. An item is an action, titled
// as the palette titles it, in title case, or {\"action\", \"title\"} to word it otherwise, \"-\" a
// divider, and {\"system\": \"services\"} the menu macOS fills with what other apps offer. The
// Menu bar tab of Edit commands writes this file.
";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BarMenu {
    pub name: String,
    pub items: Vec<BarItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "ItemText", into = "ItemText")]
pub enum BarItem {
    Divider,
    Action {
        action: String,
        /// How it's worded, when not as the palette words it.
        title: Option<String>,
    },
    System(System),
}

impl BarItem {
    /// The action it runs, if it runs one.
    pub fn action(&self) -> Option<&String> {
        match self {
            BarItem::Action { action, .. } => Some(action),
            BarItem::Divider | BarItem::System(_) => None,
        }
    }

    /// What the menu calls it.
    pub fn title(&self) -> String {
        match self {
            BarItem::Divider => String::new(),
            BarItem::Action {
                title: Some(title), ..
            } => title.clone(),
            BarItem::Action {
                action,
                title: None,
            } => title_case(&title_of(action)),
            BarItem::System(System::Services) => "Services".to_owned(),
        }
    }
}

/// A menu macOS fills itself.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum System {
    Services,
}

/// An item as the file writes it.
#[derive(Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum ItemText {
    /// An action's name, or `-` for a divider.
    Name(String),
    Titled {
        action: String,
        title: String,
    },
    System {
        system: System,
    },
}

const DIVIDER: &str = "-";

impl From<ItemText> for BarItem {
    fn from(text: ItemText) -> BarItem {
        match text {
            ItemText::Name(name) if name == DIVIDER => BarItem::Divider,
            ItemText::Name(action) => BarItem::Action {
                action,
                title: None,
            },
            ItemText::Titled { action, title } => BarItem::Action {
                action,
                title: Some(title),
            },
            ItemText::System { system } => BarItem::System(system),
        }
    }
}

impl From<BarItem> for ItemText {
    fn from(item: BarItem) -> ItemText {
        match item {
            BarItem::Divider => ItemText::Name(DIVIDER.to_owned()),
            BarItem::Action {
                action,
                title: None,
            } => ItemText::Name(action),
            BarItem::Action {
                action,
                title: Some(title),
            } => ItemText::Titled { action, title },
            BarItem::System(system) => ItemText::System { system },
        }
    }
}

/// The menus `text` lays out.
pub fn parse(text: &str) -> Result<Vec<BarMenu>> {
    serde_json_lenient::from_str(text).context("not a menu bar")
}

/// The file's text for `menus`.
pub fn written(menus: &[BarMenu]) -> Result<String> {
    let json = serde_json::to_string_pretty(menus)?;
    Ok(format!("{HEADER}{json}\n"))
}

/// The menus for `cx.set_menus`. An item whose action isn't one is left out, as [`unknown`]
/// says, and so is Check for Updates in a build that doesn't look for them.
pub fn built(menus: &[BarMenu], cx: &App) -> Vec<Menu> {
    (menus.iter())
        .map(|menu| {
            let items = (menu.items.iter()).filter_map(|item| match item {
                BarItem::Divider => Some(MenuItem::separator()),
                BarItem::Action { action, .. } if !offered(action, cx) => None,
                BarItem::Action { action, .. } => {
                    let built = cx.build_action(action, None).ok()?;
                    Some(MenuItem::Action {
                        name: item.title().into(),
                        action: built,
                        os_action: os_action(action),
                        checked: false,
                        disabled: false,
                    })
                }
                BarItem::System(System::Services) => {
                    Some(MenuItem::os_submenu(item.title(), SystemMenuType::Services))
                }
            });
            Menu::new(menu.name.clone()).items(items)
        })
        .collect()
}

/// The edit macOS's own panels take from the item for `action`, as the file picker's path field
/// takes ⌘V only from an Edit menu's Paste.
fn os_action(action: &str) -> Option<OsAction> {
    match action {
        "text_input::Cut" => Some(OsAction::Cut),
        "text_input::Copy" => Some(OsAction::Copy),
        "text_input::Paste" => Some(OsAction::Paste),
        "text_input::SelectAll" => Some(OsAction::SelectAll),
        _ => None,
    }
}

/// Whether this build offers `action` in its menus.
pub fn offered(action: &str, cx: &App) -> bool {
    action != CHECK_FOR_UPDATES
        || (cx.try_global::<crate::Embedded>()).is_some_and(|e| e.update_feed.is_some())
}

/// What's wrong with each item of `menus` that names no action.
pub fn unknown(menus: &[BarMenu], cx: &App) -> Vec<String> {
    (menus.iter())
        .flat_map(|menu| {
            (menu.items.iter()).filter_map(move |item| match item {
                BarItem::Action { action, .. } => cx
                    .build_action(action, None)
                    .err()
                    .map(|err| format!("menus.json, {}: {err}", menu.name)),
                BarItem::Divider | BarItem::System(_) => None,
            })
        })
        .collect()
}

/// The keys the menu bar shows beside `action`, as GPUI picks them on macOS: the first binding
/// that works in a window it takes to have `Workspace`, `Pane` and `Editor` focused, else its
/// first. A chord shows nothing.
pub fn shortcut(action: &str, cx: &App) -> Option<SharedString> {
    let action = cx.build_action(action, None).ok()?;
    let mut context = KeyContext::new_with_defaults();
    for name in ["Workspace", "Pane", "Editor"] {
        context.add(name);
    }
    let context = [context];
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let mut bindings = keymap.bindings_for_action(action.as_ref()).peekable();
    let first = bindings.peek().copied();
    let binding = bindings
        .find(|binding| (binding.predicate()).is_none_or(|predicate| predicate.eval(&context)))
        .or(first)?;
    (binding.keystrokes().len() == 1).then(|| ui::keys_of(binding))
}

/// `title` as macOS words menu items, each word capitalised but the short ones that join
/// others, in a hyphenated word too: "Report an issue" is "Report an Issue", and
/// "Side-by-side diff" "Side-by-Side Diff".
pub fn title_case(title: &str) -> String {
    const JOINING: [&str; 13] = [
        "a", "an", "the", "and", "but", "or", "nor", "for", "of", "to", "at", "by", "as",
    ];
    let words: Vec<String> = (title.split(' ').enumerate())
        .map(|(ix, word)| {
            let parts: Vec<String> = (word.split('-').enumerate())
                .map(
                    |(part_ix, part)| match (ix > 0 || part_ix > 0) && JOINING.contains(&part) {
                        true => part.to_owned(),
                        false => capitalised(part),
                    },
                )
                .collect();
            parts.join("-")
        })
        .collect();
    words.join(" ")
}

/// `word` with its first letter in capitals, after any punctuation before it.
fn capitalised(word: &str) -> String {
    let Some((at, letter)) = word.char_indices().find(|(_, c)| c.is_alphabetic()) else {
        return word.to_owned();
    };
    let (before, rest) = word.split_at(at);
    let rest = rest.get(letter.len_utf8()..).unwrap_or_default();
    format!("{before}{}{rest}", letter.to_uppercase())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{BarItem, BarMenu, System, parse, title_case, written};
    use crate::keymap::KeymapFile;

    fn shipped() -> Vec<BarMenu> {
        parse(KeymapFile::Menus.embedded()).expect("the shipped menus parse")
    }

    #[test]
    fn the_shipped_menus_name_actions() {
        let actions: HashSet<&str> = gpui::generate_list_of_all_registered_actions()
            .map(|action| action.name)
            .collect();
        for menu in shipped() {
            for item in menu.items {
                if let BarItem::Action { action, .. } = item {
                    assert!(
                        actions.contains(action.as_str()),
                        "{}: {action:?} isn't an action",
                        menu.name
                    );
                }
            }
        }
    }

    #[test]
    fn the_shipped_menus_are_written_as_the_editor_writes_them() {
        let text = written(&shipped()).expect("menus write");
        assert_eq!(
            text,
            KeymapFile::Menus.embedded(),
            "saving them unchanged changes nothing"
        );
    }

    #[test]
    fn the_shipped_menus_are_worded_as_macos_words_them() {
        let text: Vec<String> = (shipped().iter())
            .map(|menu| {
                let items: Vec<String> = (menu.items.iter())
                    .map(|item| match item {
                        BarItem::Divider => "-".to_owned(),
                        BarItem::Action { .. } | BarItem::System(_) => item.title(),
                    })
                    .collect();
                format!("{}: {}", menu.name, items.join(", "))
            })
            .collect();
        // Each item as the menu bar words it, from the palette's titles or the file's own.
        snapbox::assert_data_eq!(
            text.join("\n"),
            snapbox::str![[r#"
GitButler: About GitButler, Check for Updates, -, Settings…, -, Services, -, Hide GitButler, Hide Others, Show All, -, Quit GitButler
File: Add Project…, Clone Repository…, New Terminal, -, Close Window
Edit: Undo Operation, Redo Operation, -, Cut, Copy, Paste, Select All, -, Copy Selection, Copy More…
Repository: Commit…, Amend…, -, Reword, Discard, Squash…, Move…, -, New Branch, New Branch Above, New Branch Below, New Worktree From Branch, -, Add Empty Commit Above, Add Empty Commit Below, -, Apply Branch, Unapply Stack, -, New Worktree, New Worktree With Name…, Archive Worktree, Unarchive Worktree…
Remote: Push, Fetch, Pull, -, Create Pull Request…, Open Pull Request, Copy Pull Request Link, -, Update Workspace and Worktrees, Update Workspace, Update Worktrees
Go: Command Palette…, -, Go to Anything…, Go to Navigator, Go to Files, Go to Diff, Go to Terminal Overview
View: Actual Size, Zoom In, Zoom Out, -, Toggle Navigator, Toggle Details, Toggle Terminal Overview, -, Move Navigator, -, Toggle Files, Wrap Long Lines, Fold File, Unfold File, Side-by-Side Diff, -, Change Theme…
Window: Minimize, Zoom, -, Split Right, Split Below, -, Zoom Tile, Arrange Tiles, Equalize Tiles, Swap Tiles, Close Tile
Help: GitButler Docs, What's New, Keyboard Shortcuts, -, Report an Issue, Show Settings Folder, Show Logs Folder
"#]]
        );
    }

    #[test]
    fn items_read_in_each_shape() {
        let menus = parse(
            r#"[{ "name": "File", "items": [
                "app::AddProject",
                "-",
                { "action": "app::Quit", "title": "Quit GitButler" },
                { "system": "services" },
            ]}]"#,
        )
        .expect("it parses");
        assert_eq!(
            menus,
            Vec::from([BarMenu {
                name: "File".to_owned(),
                items: Vec::from([
                    BarItem::Action {
                        action: "app::AddProject".to_owned(),
                        title: None,
                    },
                    BarItem::Divider,
                    BarItem::Action {
                        action: "app::Quit".to_owned(),
                        title: Some("Quit GitButler".to_owned()),
                    },
                    BarItem::System(System::Services),
                ]),
            }])
        );
    }

    #[test]
    fn a_menu_with_something_else_in_it_isnt_read() {
        assert!(
            parse(r#"[{ "name": "File", "items": [], "icon": "x" }]"#).is_err(),
            "a typo in a field's name says so rather than dropping it"
        );
    }

    #[test]
    fn an_item_with_something_else_in_it_isnt_read() {
        assert!(
            parse(r#"[{ "name": "File", "items": [{ "action": "app::Quit", "titel": "Q" }] }]"#)
                .is_err(),
            "a typo in an item's field says so rather than dropping it"
        );
    }

    #[test]
    fn items_write_in_each_shape() {
        let menus = Vec::from([BarMenu {
            name: "File".to_owned(),
            items: Vec::from([
                BarItem::Action {
                    action: "app::AddProject".to_owned(),
                    title: None,
                },
                BarItem::Divider,
                BarItem::Action {
                    action: "app::Quit".to_owned(),
                    title: Some("Quit GitButler".to_owned()),
                },
                BarItem::System(System::Services),
            ]),
        }]);
        let text = written(&menus).expect("menus write");
        let json = text.lines().filter(|line| !line.starts_with("//"));
        // One item to a line, as a person would write them.
        snapbox::assert_data_eq!(
            json.collect::<Vec<_>>().join("\n"),
            snapbox::str![[r#"
[
  {
    "name": "File",
    "items": [
      "app::AddProject",
      "-",
      {
        "action": "app::Quit",
        "title": "Quit GitButler"
      },
      {
        "system": "services"
      }
    ]
  }
]
"#]]
        );
    }

    #[test]
    fn a_title_is_in_title_case() {
        assert_eq!(title_case("Add project"), "Add Project");
    }

    #[test]
    fn short_joining_words_stay_small() {
        assert_eq!(title_case("Report an issue"), "Report an Issue");
    }

    #[test]
    fn a_title_starting_with_a_joining_word_capitalises_it() {
        assert_eq!(title_case("a project"), "A Project");
    }

    #[test]
    fn a_word_in_brackets_is_capitalised_inside_them() {
        assert_eq!(
            title_case("What's new (release notes)"),
            "What's New (Release Notes)"
        );
    }

    #[test]
    fn each_part_of_a_hyphenated_word_is_capitalised() {
        assert_eq!(title_case("Side-by-side diff"), "Side-by-Side Diff");
    }

    #[test]
    fn capitals_inside_a_word_stay() {
        assert_eq!(title_case("GitButler docs"), "GitButler Docs");
    }
}
