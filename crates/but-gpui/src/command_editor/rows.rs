//! The command editor's table: every command by where it's bound, with its keys in each keymap.

use std::collections::{BTreeSet, HashMap, HashSet};

use gpui::SharedString;

use crate::keymap::{self, Binding, Keymap, KeymapFile};
use crate::ui;
use crate::ui::text_input::BoundKey;

/// What the table's group of text fields' keys is called.
pub const TEXT_FIELDS: &str = "Text fields, as bound in code on this platform";

/// The modifiers, as keys are written.
const MODIFIERS: [char; 4] = ['⌃', '⌥', '⇧', '⌘'];

/// An action, by what the table calls it.
pub struct Command {
    pub name: String,
    pub title: String,
    pub aliases: Vec<String>,
}

/// Keys in a cell.
#[derive(Clone)]
pub struct Key {
    /// What the keys are in a keymap's file. `None` for a text field's, which are in code.
    pub binding: Option<Binding>,
    pub label: SharedString,
    /// The action's last binding on this platform, which menus and tooltips show.
    pub shown: bool,
    /// The platforms it binds on, in words, unless that's all of them.
    pub platforms: Option<String>,
    /// What else the keys do in the same place.
    pub conflicts: Vec<String>,
}

impl Key {
    /// In `base.json`, so in both keymaps.
    pub fn shared(&self) -> bool {
        (self.binding.as_ref()).is_some_and(|binding| binding.file == KeymapFile::Base)
    }
}

/// Where a row's keys work.
#[derive(Clone, PartialEq)]
pub enum Place {
    /// Nowhere: the action has no keys.
    Unbound,
    /// In this key context, or anywhere for `None`.
    In(Option<String>),
}

pub struct Row {
    /// The heading it goes under.
    pub group: String,
    pub action: String,
    pub title: String,
    pub place: Place,
    pub keys: Vec<Key>,
    /// Its command has no keys, here or in any other place.
    pub unbound: bool,
    /// All typing finds it by, in lowercase.
    pub words: String,
    /// What says its command has been audited: every row of a command has the same.
    pub audit_key: String,
    pub audited: bool,
}

impl Row {
    /// Whether it's a text field's, which the editor can't change.
    pub fn locked(&self) -> bool {
        self.group == TEXT_FIELDS
    }

    fn conflicted(&self) -> bool {
        self.keys.iter().any(|key| !key.conflicts.is_empty())
    }

    /// Whether every word of `query`, in lowercase, finds it.
    pub fn found_by(&self, query: &str) -> bool {
        (query.split_whitespace()).all(|word| self.words.contains(word))
    }

    /// Whether one of its keys has all of `pressed`, whatever else it has: P is in ⌘P and ⌥⌘P,
    /// and ⌘P in ⌥⌘P but not ⌃P.
    pub fn bound_to(&self, pressed: &str) -> bool {
        let pressed = key_parts(pressed);
        (self.keys.iter()).any(|key| pressed.is_subset(&key_parts(&key.label)))
    }
}

/// The modifiers and keys of every stroke of `label`.
fn key_parts(label: &str) -> HashSet<String> {
    let mut parts = HashSet::new();
    for stroke in label.split_whitespace() {
        parts.extend((stroke.chars().filter(|c| MODIFIERS.contains(c))).map(String::from));
        parts.insert(stroke.trim_start_matches(MODIFIERS).to_owned());
    }
    parts
}

/// Which rows the table shows.
#[derive(Clone, Copy, PartialEq)]
pub enum Filter {
    All,
    NoKeys,
    Conflicts,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::All, Filter::NoKeys, Filter::Conflicts];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::NoKeys => "No keys",
            Filter::Conflicts => "Conflicts",
        }
    }

    pub fn keeps(self, row: &Row) -> bool {
        match self {
            Filter::All => true,
            Filter::NoKeys => row.unbound,
            Filter::Conflicts => row.conflicted(),
        }
    }

    /// How many commands it shows, or rows for conflicts, which are in one place, not a whole
    /// command.
    pub fn count(self, rows: &[Row]) -> usize {
        match self {
            Filter::Conflicts => rows.iter().filter(|row| self.keeps(row)).count(),
            Filter::All | Filter::NoKeys => commands_in(rows.iter().filter(|row| self.keeps(row))),
        }
    }
}

/// Which commands the table shows, by whether they've been audited. It goes with a [`Filter`].
#[derive(Clone, Copy, PartialEq)]
pub enum AuditFilter {
    Any,
    Audited,
    NotAudited,
}

impl AuditFilter {
    pub const ALL: [AuditFilter; 3] = [
        AuditFilter::Any,
        AuditFilter::Audited,
        AuditFilter::NotAudited,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AuditFilter::Any => "Any",
            AuditFilter::Audited => "Audited",
            AuditFilter::NotAudited => "Not audited",
        }
    }

    pub fn keeps(self, row: &Row) -> bool {
        match self {
            AuditFilter::Any => true,
            AuditFilter::Audited => row.audited,
            AuditFilter::NotAudited => !row.audited,
        }
    }

    /// How many commands it shows.
    pub fn count(self, rows: &[Row]) -> usize {
        commands_in(rows.iter().filter(|row| self.keeps(row)))
    }
}

/// How many commands `rows` are of, as a command can have many.
fn commands_in<'a>(rows: impl Iterator<Item = &'a Row>) -> usize {
    let commands: BTreeSet<&str> = rows.map(|row| row.audit_key.as_str()).collect();
    commands.len()
}

/// Everything the table is made from. `places` are where commands have been given a place to
/// bind keys in that no key has yet, and `audited` the keys of those checked as audited.
pub struct Sources<'a> {
    pub commands: &'a [Command],
    pub bindings: &'a [Binding],
    pub text_keys: &'a [BoundKey],
    pub places: &'a [(String, Option<String>)],
    pub audited: &'a HashSet<String>,
    pub os: &'a str,
}

/// The table's rows: by the group the action's name starts with, then by title, a row for each
/// place a command is bound in, in the order the files first bind it there, or one row when it's
/// bound nowhere. The text fields' keys come last.
pub fn rows(sources: &Sources<'_>) -> Vec<Row> {
    let mut commands: Vec<&Command> = sources.commands.iter().collect();
    commands.sort_by_key(|command| (group_of(&command.name), command.title.to_lowercase()));
    let keymap = Keymap::default();
    let shown = shown_keys(sources, keymap);
    let mut rows = Vec::new();
    for command in commands {
        let bound: Vec<&Binding> = (sources.bindings.iter())
            .filter(|binding| binding.action == command.name)
            .collect();
        let mut places: Vec<Option<String>> = Vec::new();
        let given = (sources.places.iter())
            .filter(|(action, _)| *action == command.name)
            .map(|(_, context)| context);
        for context in bound.iter().map(|binding| &binding.context).chain(given) {
            if !places.contains(context) {
                places.push(context.clone());
            }
        }
        let unbound = bound.is_empty();
        let places = match places.is_empty() {
            true => Vec::from([Place::Unbound]),
            false => places.into_iter().map(Place::In).collect(),
        };
        for place in places {
            let keys = keys_in(&bound, &place, keymap, &shown, sources);
            let words = words(command, &place, keys.iter());
            rows.push(Row {
                group: group_of(&command.name).to_owned(),
                action: command.name.clone(),
                title: command.title.clone(),
                place,
                keys,
                unbound,
                words,
                audit_key: command.name.clone(),
                audited: sources.audited.contains(&command.name),
            });
        }
    }
    rows.extend(text_rows(sources.text_keys, sources.audited));
    rows
}

/// The start of an action's name.
fn group_of(name: &str) -> &str {
    name.split("::").next().unwrap_or(name)
}

/// Each action's last binding in `keymap` on this platform: the one it shows.
fn shown_keys<'a>(sources: &Sources<'a>, keymap: Keymap) -> HashMap<&'a str, &'a Binding> {
    (sources.bindings.iter())
        .filter(|binding| keymap.files().contains(&binding.file) && binding.binds_on(sources.os))
        .map(|binding| (binding.action.as_str(), binding))
        .collect()
}

fn keys_in(
    bound: &[&Binding],
    place: &Place,
    keymap: Keymap,
    shown: &HashMap<&str, &Binding>,
    sources: &Sources<'_>,
) -> Vec<Key> {
    let Place::In(context) = place else {
        return Vec::new();
    };
    (bound.iter())
        .filter(|binding| keymap.files().contains(&binding.file) && binding.context == *context)
        .map(|binding| Key {
            binding: Some((*binding).clone()),
            label: ui::keys_written(&binding.keys),
            shown: shown.get(binding.action.as_str()) == Some(binding),
            platforms: binding.os.as_ref().map(|os| platforms(os)),
            conflicts: conflicts(binding, keymap, sources),
        })
        .collect()
}

fn platforms(os: &[String]) -> String {
    let names: Vec<&str> = (os.iter())
        .map(|name| keymap::platform_name(name))
        .collect();
    names.join(", ")
}

/// What else `binding`'s keys do in `keymap`, in the same key context on a platform they share.
/// A key bound to several actions in one place runs the first one handled, as r rewords a commit
/// and renames a branch, so it can be meant.
fn conflicts(binding: &Binding, keymap: Keymap, sources: &Sources<'_>) -> Vec<String> {
    let label = ui::keys_written(&binding.keys);
    let mut titles: Vec<String> = (sources.bindings.iter())
        .filter(|other| {
            keymap.files().contains(&other.file)
                && other.action != binding.action
                && other.context == binding.context
                && ui::keys_written(&other.keys) == label
                && shares_a_platform(binding, other)
        })
        .map(|other| title_of(&other.action, sources.commands))
        .collect();
    // Text fields' keys are this platform's.
    if reaches_text_fields(binding.context.as_deref()) && binding.binds_on(sources.os) {
        titles.extend(
            (sources.text_keys.iter())
                .filter(|key| key.label == label)
                .map(|key| format!("{} in a text field", key.title)),
        );
    }
    titles
}

/// Whether keys bound in `context` work all over the window, in text fields too, where a field's
/// own keys come first. Other contexts name a part of it that keeps to itself or leaves fields
/// out, as `!TextInput` does, or that means to take a field's key, as a menu's Return does.
fn reaches_text_fields(context: Option<&str>) -> bool {
    matches!(context, None | Some("Workspace"))
}

fn shares_a_platform(a: &Binding, b: &Binding) -> bool {
    match (&a.os, &b.os) {
        (Some(a), Some(b)) => a.iter().any(|name| b.contains(name)),
        (None, _) | (_, None) => true,
    }
}

fn title_of(action: &str, commands: &[Command]) -> String {
    (commands.iter())
        .find(|command| command.name == action)
        .map_or_else(|| action.to_owned(), |command| command.title.clone())
}

fn words<'a>(command: &Command, place: &Place, keys: impl Iterator<Item = &'a Key>) -> String {
    let mut words = Vec::from([command.title.clone(), command.name.clone()]);
    words.extend(command.aliases.iter().cloned());
    if let Place::In(context) = place {
        words.push(context.clone().unwrap_or_else(|| "Anywhere".to_owned()));
    }
    for key in keys {
        words.push(key.label.to_string());
        words.push(ui::keys_text(&key.label));
        words.extend(key.binding.iter().map(|binding| binding.keys.clone()));
    }
    words.join(" ").to_lowercase()
}

/// A row for each thing a text field's keys do.
fn text_rows(keys: &[BoundKey], audited: &HashSet<String>) -> Vec<Row> {
    let mut titles: Vec<&str> = Vec::new();
    for key in keys {
        if !titles.contains(&key.title.as_str()) {
            titles.push(&key.title);
        }
    }
    (titles.into_iter())
        .map(|title| {
            let bound: Vec<&BoundKey> = keys.iter().filter(|key| key.title == title).collect();
            let cell: Vec<Key> = (bound.iter())
                .map(|key| Key {
                    binding: None,
                    label: key.label.clone(),
                    shown: false,
                    platforms: None,
                    conflicts: Vec::new(),
                })
                .collect();
            let action = bound.first().map_or("", |key| key.action);
            let labels: Vec<&str> = bound.iter().map(|key| key.label.as_ref()).collect();
            let words = format!("{title} {action} TextInput {}", labels.join(" "));
            // `Move` and `Delete` do many things, each its own row.
            let audit_key = format!("{action} {title}");
            Row {
                group: TEXT_FIELDS.to_owned(),
                action: action.to_owned(),
                title: title.to_owned(),
                place: Place::In(Some("TextInput".to_owned())),
                keys: cell,
                unbound: false,
                words: words.to_lowercase(),
                audited: audited.contains(&audit_key),
                audit_key,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use gpui::SharedString;

    use super::{AuditFilter, BoundKey, Command, Filter, Place, Row, Sources, TEXT_FIELDS, rows};
    use crate::keymap::{Binding, KeymapFile};

    fn binding(file: KeymapFile, keys: &str, action: &str, context: Option<&str>) -> Binding {
        Binding {
            file,
            section: 0,
            keys: keys.to_owned(),
            action: action.to_owned(),
            context: context.map(str::to_owned),
            os: None,
        }
    }

    fn command(name: &str, title: &str) -> Command {
        Command {
            name: name.to_owned(),
            title: title.to_owned(),
            aliases: Vec::new(),
        }
    }

    fn table(commands: &[Command], bindings: &[Binding]) -> Vec<Row> {
        rows(&Sources {
            commands,
            bindings,
            text_keys: &[],
            places: &[],
            audited: &HashSet::new(),
            os: "macos",
        })
    }

    fn labels(keys: &[super::Key]) -> Vec<&str> {
        keys.iter().map(|key| key.label.as_ref()).collect()
    }

    #[test]
    fn a_command_has_a_row_for_each_place_its_bound() {
        let commands = [command("a::Run", "Run")];
        let bindings = [
            binding(KeymapFile::Standard, "r", "a::Run", Some("Diff")),
            binding(KeymapFile::Standard, "x", "a::Run", Some("Files")),
        ];
        let rows = table(&commands, &bindings);
        let places: Vec<&Place> = rows.iter().map(|row| &row.place).collect();
        assert_eq!(rows.len(), 2);
        assert!(places[0] == &Place::In(Some("Diff".to_owned())));
        assert_eq!(labels(&rows[1].keys), ["X"]);
    }

    #[test]
    fn a_command_with_no_keys_has_one_unbound_row() {
        let rows = table(&[command("a::Run", "Run")], &[]);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].place == Place::Unbound);
        assert!(rows[0].unbound);
    }

    #[test]
    fn a_given_place_makes_a_row_with_no_keys() {
        let commands = [command("a::Run", "Run")];
        let places = [("a::Run".to_owned(), Some("Diff".to_owned()))];
        let rows = rows(&Sources {
            commands: &commands,
            bindings: &[],
            text_keys: &[],
            places: &places,
            audited: &HashSet::new(),
            os: "macos",
        });
        assert_eq!(rows.len(), 1);
        assert!(rows[0].place == Place::In(Some("Diff".to_owned())));
    }

    #[test]
    fn the_shared_keys_show() {
        let commands = [command("a::Run", "Run")];
        let bindings = [binding(KeymapFile::Base, "r", "a::Run", None)];
        let rows = table(&commands, &bindings);
        assert_eq!(labels(&rows[0].keys), ["R"]);
        assert!(rows[0].keys[0].shared());
    }

    #[test]
    fn the_last_binding_on_this_platform_is_the_one_shown() {
        let commands = [command("a::Run", "Run")];
        let mut elsewhere = binding(KeymapFile::Standard, "w", "a::Run", None);
        elsewhere.os = Some(Vec::from(["windows".to_owned()]));
        let bindings = [
            binding(KeymapFile::Standard, "a", "a::Run", None),
            binding(KeymapFile::Standard, "b", "a::Run", None),
            elsewhere,
        ];
        let rows = table(&commands, &bindings);
        let shown: Vec<bool> = rows[0].keys.iter().map(|key| key.shown).collect();
        assert_eq!(shown, [false, true, false]);
        assert_eq!(rows[0].keys[2].platforms.as_deref(), Some("Windows"));
    }

    #[test]
    fn the_same_keys_in_the_same_place_conflict() {
        let commands = [command("a::Run", "Run"), command("a::Walk", "Walk")];
        let bindings = [
            binding(KeymapFile::Standard, "r", "a::Run", Some("Diff")),
            binding(KeymapFile::Standard, "r", "a::Walk", Some("Diff")),
        ];
        let rows = table(&commands, &bindings);
        assert_eq!(rows[0].keys[0].conflicts, ["Walk"]);
        assert_eq!(Filter::Conflicts.count(&rows), 2);
    }

    #[test]
    fn the_same_keys_in_another_place_dont_conflict() {
        let commands = [command("a::Run", "Run"), command("a::Walk", "Walk")];
        let bindings = [
            binding(KeymapFile::Standard, "r", "a::Run", Some("Diff")),
            binding(KeymapFile::Standard, "r", "a::Walk", Some("Files")),
        ];
        assert!(table(&commands, &bindings)[0].keys[0].conflicts.is_empty());
    }

    #[test]
    fn keys_on_different_platforms_dont_conflict() {
        let commands = [command("a::Run", "Run"), command("a::Walk", "Walk")];
        let mut mac = binding(KeymapFile::Standard, "r", "a::Run", None);
        mac.os = Some(Vec::from(["macos".to_owned()]));
        let mut windows = binding(KeymapFile::Standard, "r", "a::Walk", None);
        windows.os = Some(Vec::from(["windows".to_owned()]));
        assert!(
            table(&commands, &[mac, windows])[0].keys[0]
                .conflicts
                .is_empty()
        );
    }

    #[test]
    fn a_key_for_the_whole_window_conflicts_with_a_text_fields_own() {
        let commands = [command("a::Run", "Run")];
        let bindings = [binding(KeymapFile::Standard, "secondary-a", "a::Run", None)];
        let text_keys = [BoundKey {
            title: "Select all".to_owned(),
            action: "text_input::SelectAll",
            // ⌘A or ⌃A, as this platform writes the keys.
            label: crate::ui::keys_written("secondary-a"),
        }];
        let rows = rows(&Sources {
            commands: &commands,
            bindings: &bindings,
            text_keys: &text_keys,
            places: &[],
            audited: &HashSet::new(),
            os: "macos",
        });
        assert_eq!(rows[0].keys[0].conflicts, ["Select all in a text field"]);
    }

    #[test]
    fn a_key_for_another_platform_doesnt_conflict_with_this_ones_text_fields() {
        let commands = [command("a::Run", "Run")];
        let mut linux = binding(KeymapFile::Standard, "ctrl-shift-e", "a::Run", None);
        linux.os = Some(Vec::from(["linux".to_owned()]));
        let text_keys = [BoundKey {
            title: "Select to line end".to_owned(),
            action: "text_input::Move",
            label: SharedString::from("⌃⇧E"),
        }];
        let rows = rows(&Sources {
            commands: &commands,
            bindings: &[linux],
            text_keys: &text_keys,
            places: &[],
            audited: &HashSet::new(),
            os: "macos",
        });
        assert!(rows[0].keys[0].conflicts.is_empty());
    }

    #[test]
    fn a_key_kept_out_of_text_fields_doesnt_conflict_with_theirs() {
        let commands = [command("a::Run", "Run")];
        let bindings = [binding(
            KeymapFile::Standard,
            "secondary-a",
            "a::Run",
            Some("Diff && !TextInput"),
        )];
        let text_keys = [BoundKey {
            title: "Select all".to_owned(),
            action: "text_input::SelectAll",
            label: SharedString::from("⌘A"),
        }];
        let rows = rows(&Sources {
            commands: &commands,
            bindings: &bindings,
            text_keys: &text_keys,
            places: &[],
            audited: &HashSet::new(),
            os: "macos",
        });
        assert!(rows[0].keys[0].conflicts.is_empty());
    }

    #[test]
    fn a_text_fields_keys_are_one_locked_row_for_each_thing_they_do() {
        let text_keys = [
            BoundKey {
                title: "Move to line end".to_owned(),
                action: "text_input::Move",
                label: SharedString::from("⌘→"),
            },
            BoundKey {
                title: "Move to line end".to_owned(),
                action: "text_input::Move",
                label: SharedString::from("⌃E"),
            },
        ];
        let rows = rows(&Sources {
            commands: &[],
            bindings: &[],
            text_keys: &text_keys,
            places: &[],
            audited: &HashSet::new(),
            os: "macos",
        });
        assert_eq!(rows.len(), 1);
        assert!(rows[0].locked());
        assert_eq!(rows[0].group, TEXT_FIELDS);
        assert_eq!(labels(&rows[0].keys), ["⌘→", "⌃E"]);
    }

    #[test]
    fn commands_are_grouped_then_sorted_by_title() {
        let commands = [
            command("b::Zebra", "Zebra"),
            command("a::Walk", "Walk"),
            command("b::Apple", "apple"),
        ];
        let rows = table(&commands, &[]);
        let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(titles, ["Walk", "apple", "Zebra"]);
    }

    #[test]
    fn a_row_is_found_by_its_title_action_place_and_keys() {
        let commands = [Command {
            name: "a::Run".to_owned(),
            title: "Run fast".to_owned(),
            aliases: Vec::from(["Sprint".to_owned()]),
        }];
        let bindings = [binding(
            KeymapFile::Standard,
            "secondary-r",
            "a::Run",
            Some("Diff"),
        )];
        let rows = table(&commands, &bindings);
        assert!(rows[0].found_by("run fast"));
        assert!(rows[0].found_by("sprint"));
        assert!(rows[0].found_by("a::run diff"));
        // ⌘R or ⌃R, as this platform writes the keys.
        assert!(rows[0].found_by(&crate::ui::keys_written("secondary-r").to_lowercase()));
        assert!(rows[0].found_by("secondary-r"));
        assert!(!rows[0].found_by("walk"));
    }

    /// A row bound to keys written as they are, as a text field's are.
    fn row_bound_to(labels: &[&str]) -> Row {
        let text_keys: Vec<BoundKey> = (labels.iter())
            .map(|label| BoundKey {
                title: "Run".to_owned(),
                action: "text_input::Run",
                label: SharedString::from((*label).to_owned()),
            })
            .collect();
        let mut rows = rows(&Sources {
            commands: &[],
            bindings: &[],
            text_keys: &text_keys,
            places: &[],
            audited: &HashSet::new(),
            os: "macos",
        });
        rows.remove(0)
    }

    #[test]
    fn a_key_pressed_finds_it_alone() {
        assert!(row_bound_to(&["P"]).bound_to("P"));
    }

    #[test]
    fn a_key_pressed_finds_it_under_any_modifiers() {
        assert!(row_bound_to(&["⌃P"]).bound_to("P"));
        assert!(row_bound_to(&["⌥⌘P"]).bound_to("P"));
    }

    #[test]
    fn another_key_doesnt_find_it() {
        assert!(!row_bound_to(&["⌘K"]).bound_to("P"));
    }

    #[test]
    fn a_key_is_a_whole_key_not_part_of_a_name() {
        assert!(!row_bound_to(&["pageup"]).bound_to("p"));
    }

    #[test]
    fn modifiers_pressed_have_to_be_in_its_keys() {
        let row = row_bound_to(&["⌥⌘P"]);
        assert!(row.bound_to("⌘P"));
        assert!(row.bound_to("⌥⌘P"));
        assert!(!row.bound_to("⌃P"));
        assert!(!row.bound_to("⇧⌘P"));
    }

    #[test]
    fn any_of_a_rows_keys_can_be_the_one_found() {
        assert!(row_bound_to(&["⌘K", "⌃P"]).bound_to("⌃P"));
    }

    #[test]
    fn what_was_pressed_has_to_be_in_one_key() {
        assert!(!row_bound_to(&["⌘K", "⌃P"]).bound_to("⌘P"));
    }

    #[test]
    fn a_chord_is_found_by_any_of_its_strokes() {
        assert!(row_bound_to(&["⌘K ⌘S"]).bound_to("⌘S"));
    }

    #[test]
    fn keys_bound_in_a_keymap_are_found_as_they_show() {
        let commands = [command("a::Run", "Run")];
        let bindings = [binding(KeymapFile::Standard, "alt-p", "a::Run", None)];
        let rows = table(&commands, &bindings);
        assert!(rows[0].bound_to("P"));
        assert!(rows[0].bound_to("⌥P"));
        assert!(!rows[0].bound_to("⇧P"));
    }

    #[test]
    fn a_row_with_no_keys_isnt_found() {
        let rows = table(&[command("a::Prune", "Prune")], &[]);
        assert!(!rows[0].bound_to("P"));
    }

    fn audited_table(commands: &[Command], bindings: &[Binding], audited: &[&str]) -> Vec<Row> {
        let audited: HashSet<String> = audited.iter().map(|key| (*key).to_owned()).collect();
        rows(&Sources {
            commands,
            bindings,
            text_keys: &[],
            places: &[],
            audited: &audited,
            os: "macos",
        })
    }

    #[test]
    fn every_row_of_an_audited_command_is_audited() {
        let commands = [command("a::Run", "Run")];
        let bindings = [
            binding(KeymapFile::Standard, "r", "a::Run", Some("Diff")),
            binding(KeymapFile::Standard, "r", "a::Run", Some("Files")),
        ];
        let rows = audited_table(&commands, &bindings, &["a::Run"]);
        assert!(rows.iter().all(|row| row.audited));
    }

    #[test]
    fn the_audit_filters_split_the_commands() {
        let commands = [command("a::Run", "Run"), command("a::Walk", "Walk")];
        let rows = audited_table(&commands, &[], &["a::Run"]);
        assert!(AuditFilter::Audited.keeps(&rows[0]));
        assert!(!AuditFilter::Audited.keeps(&rows[1]));
        assert!(!AuditFilter::NotAudited.keeps(&rows[0]));
        assert!(AuditFilter::NotAudited.keeps(&rows[1]));
    }

    #[test]
    fn the_audit_counts_are_of_commands() {
        let commands = [command("a::Run", "Run"), command("a::Walk", "Walk")];
        let bindings = [
            binding(KeymapFile::Standard, "r", "a::Run", Some("Diff")),
            binding(KeymapFile::Standard, "r", "a::Run", Some("Files")),
        ];
        let rows = audited_table(&commands, &bindings, &["a::Run"]);
        assert_eq!(AuditFilter::Audited.count(&rows), 1);
        assert_eq!(AuditFilter::NotAudited.count(&rows), 1);
    }

    #[test]
    fn each_thing_a_text_fields_action_does_is_audited_on_its_own() {
        let text_keys = [
            BoundKey {
                title: "Move to left".to_owned(),
                action: "text_input::Move",
                label: SharedString::from("←"),
            },
            BoundKey {
                title: "Select to left".to_owned(),
                action: "text_input::Move",
                label: SharedString::from("⇧←"),
            },
        ];
        let audited = HashSet::from(["text_input::Move Move to left".to_owned()]);
        let rows = rows(&Sources {
            commands: &[],
            bindings: &[],
            text_keys: &text_keys,
            places: &[],
            audited: &audited,
            os: "macos",
        });
        let audited: Vec<bool> = rows.iter().map(|row| row.audited).collect();
        assert_eq!(audited, [true, false]);
    }

    #[test]
    fn counts_are_of_commands_not_places() {
        let commands = [command("a::Run", "Run")];
        let bindings = [
            binding(KeymapFile::Standard, "r", "a::Run", Some("Diff")),
            binding(KeymapFile::Standard, "r", "a::Run", Some("Files")),
        ];
        let rows = table(&commands, &bindings);
        assert_eq!(rows.len(), 2);
        assert_eq!(Filter::All.count(&rows), 1);
    }
}
