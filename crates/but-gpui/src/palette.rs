//! The command palette (⌘K): every action that applies where focus was, with its keys,
//! filtered as you type. It doubles as the list of shortcuts, as in Lite and Zed. With nothing
//! typed, the command run last comes first while it still applies, then what right-clicking
//! the selection offers, worded as there.

use std::ops::Range;

use gpui::{
    Action, AnyElement, ClickEvent, Context, Entity, FocusHandle, Focusable as _, ScrollHandle,
    SharedString, Subscription, Window, actions, div, prelude::*, relative,
};

use crate::app::AppView;
use crate::backend::{Branch, Checkout, Commit, Workspace, Worktree, short_ref};
use crate::commands::{COMMANDS, Command};
use crate::overlays::backdrop;
use crate::state::{Notice, ProjectEntry, Section, Selection, Store, snack};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    Icon, POPOVER_RADIUS, commit_title, dp, h_flex, keycap, list_row, matched, muted, popover,
    shortcut_here, small_icon, v_flex,
};

actions!(palette, [PaletteUp, PaletteDown, GoTo, CopyMore]);

/// What picking a choice does, given what's typed.
type Run = Box<dyn FnOnce(&mut AppView, &str, &mut Window, &mut Context<AppView>)>;

/// A row of the palette: what it's called, what it says at its end (keys, or a detail like
/// the project a branch is in), and what picking it does.
pub struct Choice {
    /// What kind of thing it is, before its label, where one list holds several kinds.
    icon: Option<Icon>,
    /// What typing finds it by, and what it shows once something is typed.
    label: SharedString,
    /// What it shows instead while nothing is typed, as the selection's menu words it.
    phrase: Option<SharedString>,
    key: Option<SharedString>,
    detail: Option<SharedString>,
    /// It throws work away, so letters scattered through its label don't find it.
    risky: bool,
    /// It makes something named as typed: any text but another choice's label finds it,
    /// shown after its label.
    names: bool,
    /// A commit's full id, which finds it typed from its start, as `git log` and Copy commit ID
    /// give it, though its detail shows less. Never by letters scattered through it, which
    /// would find every commit.
    id: Option<SharedString>,
    /// Where picking it goes, which the navigator outlines while it's selected.
    goes_to: Option<Selection>,
    run: Run,
}

impl Choice {
    pub fn new(
        label: impl Into<SharedString>,
        run: impl FnOnce(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
    ) -> Self {
        Self {
            icon: None,
            label: label.into(),
            phrase: None,
            key: None,
            detail: None,
            risky: false,
            names: false,
            id: None,
            goes_to: None,
            run: Box::new(move |this, _, window, cx| run(this, window, cx)),
        }
    }

    /// A choice that makes something named as typed, or with nothing typed, as it likes.
    pub fn naming(
        label: impl Into<SharedString>,
        run: impl FnOnce(&mut AppView, &str, &mut Window, &mut Context<AppView>) + 'static,
    ) -> Self {
        Self {
            names: true,
            run: Box::new(run),
            ..Self::new(label, |_, _, _| {})
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Its detail as `parts` joined with " · ", or none when there are none.
    fn details<'a>(self, parts: impl IntoIterator<Item = &'a str>) -> Choice {
        let parts: Vec<&str> = parts.into_iter().collect();
        match parts.is_empty() {
            true => self,
            false => self.detail(parts.join(" · ")),
        }
    }

    fn icon(mut self, icon: Icon) -> Choice {
        self.icon = Some(icon);
        self
    }

    fn goes_to(mut self, selection: Selection) -> Choice {
        self.goes_to = Some(selection);
        self
    }
}

/// The command palette, or another list to pick from the same way.
pub struct Palette {
    input: Entity<TextInput>,
    /// Where its choices act, as [`Store::place`] says, beside the input.
    place: Option<SharedString>,
    choices: Vec<Choice>,
    /// What Enter does, for the hint at the bottom.
    verb: &'static str,
    /// What the query finds, best first, worked out as it changes: nothing else changes it.
    found: Vec<Found>,
    selected: usize,
    /// The rows' scroll, which follows the selection.
    scroll: ScrollHandle,
    pub(crate) restore_focus: Option<FocusHandle>,
    /// What it opened on: see [`Store::acting_on`].
    pub acting_on: (Option<Selection>, Vec<crate::backend::CommitId>),
    /// Its choices hold what they act on, as a menu's do, rather than read it when run.
    pub holds_ids: bool,
    /// Go to anything's: wider, for commit and pull request titles, with each detail cut short
    /// before its label is.
    wide: bool,
    _subscription: Subscription,
}

impl Palette {
    /// Select the row at `row`, scrolling it into view.
    fn select(&mut self, row: usize) {
        self.selected = row;
        self.scroll.scroll_to_item(row);
    }

    /// Select the first row, or while nothing's typed the first that doesn't throw work away,
    /// so Return alone never runs one.
    fn select_first(&mut self, cx: &gpui::App) {
        let typed = !self.input.read(cx).text().trim().is_empty();
        let safe = (self.found.iter())
            .position(|found| typed || self.choices.get(found.ix).is_some_and(|c| !c.risky));
        self.select(safe.unwrap_or(0));
    }

    /// The choices the query finds, best first.
    fn find(&self, cx: &gpui::App) -> Vec<Found> {
        let query = self.input.read(cx).text().trim().to_owned();
        let taken = (self.choices.iter()).any(|c| !c.names && c.label.eq_ignore_ascii_case(&query));
        let mut scored: Vec<(i32, Found)> = self
            .choices
            .iter()
            .enumerate()
            .filter_map(|(ix, choice)| {
                // Last, under what the text finds.
                if choice.names && !query.is_empty() {
                    let found = Found {
                        ix,
                        label: Vec::new(),
                        detail: Vec::new(),
                    };
                    return (!taken).then_some((-1, found));
                }
                let label = score(&choice.label, &query);
                // The label first on a tie, as a branch named as another's detail says.
                let detail = (choice.detail.as_ref())
                    .and_then(|detail| detail_score(detail, choice.id.as_deref(), &query))
                    .map(|(score, found)| (score - 1, found))
                    .filter(|(score, _)| label.as_ref().is_none_or(|(label, _)| score > label));
                let (score, found) = match (label, detail) {
                    (_, Some((score, detail))) => (
                        score,
                        Found {
                            ix,
                            label: Vec::new(),
                            detail,
                        },
                    ),
                    (Some((score, label)), None) => (
                        score,
                        Found {
                            ix,
                            label,
                            detail: Vec::new(),
                        },
                    ),
                    (None, None) => return None,
                };
                // One that throws work away shows for its words, not for letters scattered
                // through it, so typing a command missing here doesn't land on it.
                let scattered = score == 0 && !query.is_empty();
                (!scattered || !choice.risky).then_some((score, found))
            })
            .collect();
        // Stable, so equal scores keep the recent-first order.
        scored.sort_by_key(|(score, _)| -score);
        scored.into_iter().map(|(_, found)| found).collect()
    }
}

/// A choice the query finds, with the letters it found to underline: in its label, or in its
/// detail when that matched better.
struct Found {
    ix: usize,
    label: Vec<Range<usize>>,
    detail: Vec<Range<usize>>,
}

/// How well `query` matches `label`, and the byte ranges of `label` it matched: all of it
/// beats the start of a word, which beats the middle of one, which beats a scattered match,
/// and earlier beats later. `None` when the letters don't appear in order.
pub(crate) fn score(label: &str, query: &str) -> Option<(i32, Vec<Range<usize>>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    // Compared a character at a time, so what's found keeps its place in `label`.
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let letters: Vec<(Range<usize>, char)> = (label.char_indices())
        .map(|(at, c)| (at..at + c.len_utf8(), lower(c)))
        .collect();
    let query: Vec<char> = query.chars().map(lower).collect();
    // A branch named in full, as `min-size`, before the ones it starts, as `min-size-docs`.
    if letters.iter().map(|(_, c)| c).eq(&query) {
        let all = 0..label.len();
        return Some((3000, vec![all]));
    }
    // Where all of it appears: the letter it starts at, and the bytes it covers.
    let whole: Vec<(usize, Range<usize>)> = (letters.windows(query.len()).enumerate())
        .filter(|(_, run)| run.iter().map(|(_, c)| c).eq(&query))
        .filter_map(|(at, run)| Some((at, run.first()?.0.start..run.last()?.0.end)))
        .collect();
    let starts_word = |at: usize| at == 0 || letters.get(at - 1).is_some_and(|(_, c)| *c == ' ');
    if let Some((at, range)) = whole.iter().find(|(at, _)| starts_word(*at)) {
        return Some((2000 - *at as i32, vec![range.clone()]));
    }
    if let Some((at, range)) = whole.first() {
        return Some((1000 - *at as i32, vec![range.clone()]));
    }
    // Each letter at the first place it appears after the one before, neighbours in one range.
    let mut rest = letters.iter();
    let mut found: Vec<Range<usize>> = Vec::new();
    for wanted in &query {
        let (range, _) = rest.find(|(_, c)| c == wanted)?;
        match found.last_mut() {
            Some(last) if last.end == range.start => last.end = range.end,
            _ => found.push(range.clone()),
        }
    }
    Some((0, found))
}

/// How well `query` matches a row's `detail`, as [`score`] says, but a commit's `id` only typed
/// from its start, the short one the detail starts with too: letters scattered through it would
/// find nearly every commit. The rest of the detail, its branch and project, matches as ever.
fn detail_score(detail: &str, id: Option<&str>, query: &str) -> Option<(i32, Vec<Range<usize>>)> {
    let Some(id) = id else {
        return score(detail, query);
    };
    if !query.is_empty() && id_starts(id, query) {
        return Some((2000, shared_start(detail, query).into_iter().collect()));
    }
    let shown = shared_start(detail, id).map_or(0, |shown| shown.end);
    let (score, found) = score(detail.get(shown..)?, query)?;
    let found = (found.into_iter())
        .map(|range| range.start + shown..range.end + shown)
        .collect();
    Some((score, found))
}

/// Whether `query` is `id` typed from its start, in either case.
fn id_starts(id: &str, query: &str) -> bool {
    (id.get(..query.len())).is_some_and(|start| start.eq_ignore_ascii_case(query))
}

/// The bytes at the start of `text` that `query` starts with too, in either case: a commit's
/// short id at the start of its detail when its full id was typed.
fn shared_start(text: &str, query: &str) -> Option<Range<usize>> {
    (text.char_indices().zip(query.chars()))
        .take_while(|((_, shown), typed)| shown.eq_ignore_ascii_case(typed))
        .last()
        .map(|((at, shown), _)| 0..at + shown.len_utf8())
}

/// What ⇧Y offers to copy from `selection`, labelled, the first what y copies.
fn copies(store: &Store, selection: &Selection) -> Vec<(&'static str, String)> {
    let mut copies = Vec::new();
    match selection {
        Selection::Commit(project, id) => {
            let Some((_, commit)) = store.find_commit(project, *id) else {
                return copies;
            };
            if commit.has_change_id() {
                copies.push(("Change ID", commit.key.clone()));
            }
            copies.extend([
                ("Commit ID", id.0.to_string()),
                ("Short commit ID", id.0.to_hex_with_len(7).to_string()),
            ]);
            if !commit.message.trim().is_empty() {
                copies.extend([
                    ("Message title", commit.title.clone()),
                    ("Whole message", commit.message.clone()),
                ]);
            }
            // Git's author, which the byline swaps for the committer when an agent wrote it.
            let (name, email) = match &commit.agent {
                Some(agent) => (&agent.name, &agent.email),
                None => (&commit.author, &commit.email),
            };
            copies.push(("Author", format!("{name} <{email}>")));
        }
        Selection::Branch(project, ref_name) => {
            let Some(branch) = store.branch_for(selection) else {
                return copies;
            };
            if let Some(ref_name) = ref_name {
                copies.push(("Branch name", short_ref(ref_name).into_owned()));
            }
            if let Some(url) = branch.review.as_ref().and_then(|r| r.url.clone()) {
                copies.push(("Pull request link", url));
            }
            // A branch in a linked worktree's lane, the one it has checked out or one below.
            let checkout = store.checkout_of(selection);
            let worktree = (checkout.as_ref().and_then(Checkout::linked))
                .and_then(|name| store.workspace(project)?.find_worktree(name));
            copies.extend(worktree.map(worktree_copies).into_iter().flatten());
        }
        Selection::Worktree(project, name) => {
            let worktree =
                (store.workspace(project)).and_then(|ws| ws.find_worktree(name.as_ref()));
            copies.extend(worktree.map(worktree_copies).into_iter().flatten());
        }
        // The main worktree's row, which has no name of its own.
        Selection::Uncommitted(project) => {
            if let Some(path) = store.path(project) {
                copies.push(("Worktree path", path.display().to_string()));
            }
        }
        Selection::Project(_)
        | Selection::Upstream(..)
        | Selection::Mesh(..)
        | Selection::Branches(_)
        | Selection::History(_)
        | Selection::Graph(_)
        | Selection::Stack(..)
        | Selection::Section(..)
        | Selection::Terminal(..) => {}
    }
    copies
}

fn worktree_copies(worktree: &Worktree) -> [(&'static str, String); 2] {
    [
        ("Worktree path", worktree.path.display().to_string()),
        ("Worktree name", worktree.name.to_string()),
    ]
}

/// Go to anything's row for a project, watched or not.
fn project_choice(entry: &ProjectEntry) -> Choice {
    let id = entry.id.clone();
    let gone = format!("{} was removed meanwhile", entry.project.name);
    let choice = Choice::new(entry.project.name.clone(), move |this, window, cx| {
        if this.store.read(cx).project(&id).is_none() {
            return snack(&this.store, gone, cx);
        }
        this.select(Selection::Project(id), window, cx)
    });
    choice
        .icon(Icon::Repo)
        .goes_to(Selection::Project(entry.id.clone()))
}

/// Go to anything's rows for a loaded project's pages, as the navigator names them.
fn page_choices(entry: &ProjectEntry, several: bool) -> [Choice; 3] {
    let id = &entry.id;
    let pages = [
        (Selection::Branches(id.clone()), "Branches", Icon::Branch),
        (
            Selection::History(id.clone()),
            "Operations history",
            Icon::Clock,
        ),
        (Selection::Graph(id.clone()), "Graph", Icon::Fork),
    ];
    let project = several.then_some(entry.project.name.as_str());
    pages.map(|(page, label, icon)| {
        let gone = format!("{} was removed meanwhile", entry.project.name);
        let goes_to = page.clone();
        let choice = Choice::new(label, move |this, window, cx| {
            if this.store.read(cx).project(page.project()).is_none() {
                return snack(&this.store, gone, cx);
            }
            this.select(page, window, cx)
        });
        choice.icon(icon).details(project).goes_to(goes_to)
    })
}

/// Go to anything's row for the heading over what's new on a loaded project's target, as the
/// navigator names it, while there's anything.
fn upstream_choice(entry: &ProjectEntry, workspace: &Workspace, several: bool) -> Option<Choice> {
    if workspace.upstream(None).is_empty() {
        return None;
    }
    let target = &workspace.target.as_ref()?.name;
    let id = entry.id.clone();
    let heading = Selection::Section(id.clone(), Section::Upstream(None));
    let goes_to = heading.clone();
    let gone = format!("Nothing new on {target} anymore");
    let choice = Choice::new(format!("New on {target}"), move |this, window, cx| {
        // Taken in since the picker opened, as by an update.
        if (this.store.read(cx).workspace(&id)).is_none_or(|ws| ws.upstream(None).is_empty()) {
            return snack(&this.store, gone, cx);
        }
        this.select(heading, window, cx)
    });
    let project = several.then_some(entry.project.name.as_str());
    let choice = choice.icon(Icon::ArrowLineDown).details(project);
    Some(choice.goes_to(goes_to))
}

/// Go to anything's row for a linked worktree, the branch it has checked out beside it.
fn worktree_choice(entry: &ProjectEntry, worktree: &Worktree, several: bool) -> Choice {
    let (id, name) = (entry.id.clone(), worktree.name.clone());
    let goes_to = Selection::Worktree(id.clone(), name.clone());
    let gone = format!("{name} was archived or deleted meanwhile");
    let branch = worktree.branch.as_ref().map(short_ref);
    let project = several.then_some(entry.project.name.as_str());
    let choice = Choice::new(name.to_string(), move |this, window, cx| {
        if (this.store.read(cx).workspace(&id))
            .is_none_or(|ws| ws.find_worktree(name.as_ref()).is_none())
        {
            return snack(&this.store, gone, cx);
        }
        this.select(Selection::Worktree(id, name), window, cx)
    });
    let details = branch.as_deref().into_iter().chain(project);
    choice
        .icon(Icon::FolderCopy)
        .details(details)
        .goes_to(goes_to)
}

/// Go to anything's rows for the terminals open on a loaded project's worktrees, the main
/// one's first, as the navigator lists them under each.
fn terminal_choices(
    store: &Store,
    entry: &ProjectEntry,
    workspace: &Workspace,
    several: bool,
) -> Vec<Choice> {
    let linked = (workspace.worktrees.iter())
        .filter(|wt| !store.hides_worktree(&entry.id, wt.name.as_ref()))
        .map(|wt| Checkout::Linked(wt.name.clone()));
    let mut choices = Vec::new();
    for checkout in std::iter::once(Checkout::Main).chain(linked) {
        for (id, _) in store.terminals.on(&entry.id, &checkout) {
            if let Some(title) = store.terminals.title(id) {
                choices.push(terminal_choice(entry, &checkout, id, title, several));
            }
        }
    }
    choices
}

/// Go to anything's row for a terminal, by its title, the worktree it's open on beside it.
fn terminal_choice(
    entry: &ProjectEntry,
    checkout: &Checkout,
    id: TerminalId,
    title: &SharedString,
    several: bool,
) -> Choice {
    let goes_to = Selection::Terminal(entry.id.clone(), id);
    let selection = goes_to.clone();
    let gone = format!("{title} was closed meanwhile");
    let worktree = (checkout.linked()).map_or("Main worktree".to_owned(), |name| name.to_string());
    let project = several.then_some(entry.project.name.as_str());
    let choice = Choice::new(title.clone(), move |this, window, cx| {
        // Closed since the picker opened, as when its shell exited.
        if this.store.read(cx).terminals.get(id).is_none() {
            return snack(&this.store, gone, cx);
        }
        this.select(selection, window, cx);
        this.enter_terminal(window, cx);
    });
    let details = std::iter::once(worktree.as_str()).chain(project);
    choice
        .icon(Icon::TextCode)
        .details(details)
        .goes_to(goes_to)
}

/// Go to anything's row for a branch, as the navigator names it: a pull request by its title,
/// its branch's name beside it, where typing either finds it.
fn branch_choice(store: &Store, entry: &ProjectEntry, branch: &Branch, several: bool) -> Choice {
    let (id, ref_name) = (entry.id.clone(), branch.ref_name.clone());
    let goes_to = Selection::Branch(id.clone(), ref_name.clone());
    let label = store.label_of(&entry.id, branch).to_owned();
    let name = (label != branch.name).then_some(branch.name.as_str());
    let project = several.then_some(entry.project.name.as_str());
    let gone = format!(
        "{} was renamed or left the workspace meanwhile",
        branch.name
    );
    let choice = Choice::new(label, move |this, window, cx| {
        // Gone since the picker opened, as an agent renamed it.
        if (this.store.read(cx).workspace(&id))
            .is_none_or(|ws| ws.find_branch(ref_name.as_ref()).is_none())
        {
            return snack(&this.store, gone, cx);
        }
        this.select(Selection::Branch(id, ref_name), window, cx)
    });
    choice
        .icon(Icon::Branch)
        .details(name.into_iter().chain(project))
        .goes_to(goes_to)
}

/// Go to anything's row for a commit, by its title, its short id and branch beside it, where
/// typing either finds it.
fn commit_choice(
    store: &Store,
    entry: &ProjectEntry,
    branch: &Branch,
    commit: &Commit,
    several: bool,
) -> Choice {
    let (id, key) = (entry.id.clone(), commit.key.clone());
    let goes_to = Selection::Commit(id.clone(), commit.id);
    let short = commit.id.0.to_hex_with_len(7).to_string();
    let gone = format!("Commit {short} left the workspace meanwhile");
    let project = several.then_some(entry.project.name.as_str());
    let details = [short.as_str(), store.label_of(&entry.id, branch)];
    let choice = Choice::new(commit_title(&commit.title), move |this, window, cx| {
        // By its change ID, so one an agent reworded or rebased since the picker opened opens.
        let now = (this.store.read(cx).workspace(&id))
            .and_then(|ws| ws.commits().find(|commit| commit.key == key))
            .map(|commit| commit.id);
        match now {
            Some(commit) => this.select(Selection::Commit(id, commit), window, cx),
            None => snack(&this.store, gone, cx),
        }
    });
    let choice = (choice.icon(Icon::Commit))
        .details(details.into_iter().chain(project))
        .goes_to(goes_to);
    Choice {
        id: Some(commit.id.0.to_string().into()),
        ..choice
    }
}

impl AppView {
    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // What's offered is where the keys were before a menu took them.
        self.close_menu(window, cx);
        let (items, _) = self.selection_menu(window, cx);
        let items = self.offered(items, window, cx);
        let type_of = |command: &dyn Command| {
            let action: &dyn Action = command;
            action.as_any().type_id()
        };
        let in_menu: Vec<std::any::TypeId> = (items.iter())
            .filter_map(|item| item.command.map(type_of))
            .collect();
        // Commands the app handles everywhere, as the Help menu's, count too.
        // `is_action_available` only looks at the focused element's.
        let available: Vec<std::any::TypeId> = (window.available_actions(cx).iter())
            .map(|action| action.as_any().type_id())
            .collect();
        let mut commands: Vec<&'static dyn Command> = (COMMANDS.iter().copied())
            .filter(|&command| {
                let ty = type_of(command);
                available.contains(&ty) && !in_menu.contains(&ty) && command.listed(self, cx)
            })
            .collect();
        // Recently run commands first, as in Zed.
        commands.sort_by_key(|command| {
            self.recent_commands
                .iter()
                .position(|recent| *recent == command.title())
                .unwrap_or(usize::MAX)
        });
        let command_choice = |command: &'static dyn Command, key: Option<SharedString>| {
            let (label, risky) = (command.title(), command.risky());
            let action = command.boxed_clone();
            Choice {
                key,
                risky,
                ..Choice::new(label, move |this, window, cx| {
                    // The last one run comes first, so ⌘K then Return runs it again; never one
                    // that throws work away.
                    if !risky {
                        this.recent_commands.retain(|recent| *recent != label);
                        this.recent_commands.insert(0, label);
                    }
                    window.dispatch_action(action, cx);
                })
            }
        };
        // An item that isn't a command holds what it acts on, as the menu's do.
        let holds_ids = items.iter().any(|item| item.command.is_none());
        let place = (!items.is_empty())
            .then(|| self.store.read(cx).selection.clone())
            .flatten()
            .map(|selection| self.store.read(cx).place(&selection));
        // What right-clicking the selection offers, worded as there until something's typed,
        // which finds a command by its title.
        let selected = items.into_iter().map(|item| match item.command {
            Some(command) => {
                let phrase = (item.label != command.title()).then_some(item.label);
                let risky = item.danger || command.risky();
                let choice = command_choice(command, item.key);
                (
                    Some(command.title()),
                    Choice {
                        phrase,
                        risky,
                        ..choice
                    },
                )
            }
            None => {
                let handler = item.handler;
                let choice = Choice {
                    key: item.key,
                    detail: item.detail,
                    risky: item.danger || item.risky,
                    ..Choice::new(item.label, move |this, window, cx| {
                        handler(this, window, cx)
                    })
                };
                (None, choice)
            }
        });
        let rest = commands.into_iter().map(|command| {
            // What pressing it would run from where the keys were, not another place's.
            let key = shortcut_here(command, window, cx);
            (Some(command.title()), command_choice(command, key))
        });
        let mut choices: Vec<(Option<&str>, Choice)> = selected.chain(rest).collect();
        // The command run last before them, while it's offered.
        if let Some(&last) = self.recent_commands.first()
            && let Some(at) = choices.iter().position(|(title, _)| *title == Some(last))
        {
            let choice = choices.remove(at);
            choices.insert(0, choice);
        }
        let mut choices: Vec<Choice> = choices.into_iter().map(|(_, choice)| choice).collect();
        // What an error card offers to try instead, as Update anyway, found by typing: last,
        // so ⌘K then Enter never runs one.
        choices.extend((self.store.read(cx).toasts.iter()).filter_map(
            |toast| match &toast.notice {
                Notice::Error {
                    title,
                    retry: Some((label, run)),
                    ..
                } => {
                    let (id, run) = (toast.id, run.clone());
                    let choice = Choice::new(label.clone(), move |this, _, cx| {
                        this.store.update(cx, |store, cx| {
                            if store.take_toast(id, cx) {
                                run(store, cx)
                            }
                        })
                    });
                    Some(choice.detail(title.clone()))
                }
                _ => None,
            },
        ));
        self.open_picker(place, "Search commands", "Run", choices, window, cx);
        if let Some(palette) = &mut self.palette {
            palette.holds_ids = holds_ids;
        }
    }

    /// Pick a project, or a page, what's new on the target, linked worktree, terminal, branch or
    /// commit of a loaded one, by name, pull request title or commit id, and select it.
    pub fn go_to(&mut self, _: &GoTo, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        // Only expanded projects' insides: a collapsed one's rows aren't drawn, or kept current.
        let open: Vec<(&ProjectEntry, &Workspace)> = (store.projects.iter())
            .filter(|entry| entry.expanded)
            .filter_map(|entry| Some((entry, entry.workspace.as_ref()?)))
            .collect();
        let several = open.len() > 1;
        // Broadest first, which an equal match keeps: a project before a branch of its name.
        let projects = store.projects.iter().map(project_choice);
        // Not while a commit is checked out, which the navigator hides them for.
        let pages = (open.iter())
            .filter(|(_, workspace)| workspace.editing.is_none())
            .flat_map(|(entry, _)| page_choices(entry, several));
        let upstream = (open.iter())
            .filter_map(|(entry, workspace)| upstream_choice(entry, workspace, several));
        let worktrees = open.iter().flat_map(|(entry, workspace)| {
            (workspace.worktrees.iter())
                .filter(move |wt| !store.hides_worktree(&entry.id, wt.name.as_ref()))
                .map(move |wt| worktree_choice(entry, wt, several))
        });
        // Not while a commit is checked out either, which the navigator hides them for too.
        let terminals = (open.iter())
            .filter(|(_, workspace)| workspace.editing.is_none())
            .flat_map(|(entry, workspace)| terminal_choices(store, entry, workspace, several));
        let branches = open.iter().flat_map(|(entry, workspace)| {
            (workspace.all_branches())
                .filter(|branch| branch.ref_name.is_some())
                .map(move |branch| branch_choice(store, entry, branch, several))
        });
        let commits = open.iter().flat_map(|(entry, workspace)| {
            workspace.all_branches().flat_map(move |branch| {
                (branch.commits.iter())
                    .filter(move |commit| !store.hides_commit(&entry.id, &commit.key))
                    .map(move |commit| commit_choice(store, entry, branch, commit, several))
            })
        });
        let choices = (projects.chain(pages).chain(upstream).chain(worktrees))
            .chain(terminals.chain(branches).chain(commits))
            .collect();
        let placeholder = "Go to a project, worktree, terminal, branch or commit";
        self.open_picker(None, placeholder, "Go", choices, window, cx);
        if let Some(palette) = &mut self.palette {
            palette.wide = true;
        }
    }

    /// What can be copied from the selection, to pick from with the keys, as the TUI's ⇧Y
    /// offers: the file under the cursor while the file list or the diff has the keys, as
    /// right-clicking takes it. The first is what y copies, unless text is selected in the diff.
    pub fn copy_more(&mut self, _: &CopyMore, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(selection) = store.selection.clone() else {
            return;
        };
        let file = self.detail.read(cx).focused_file_paths(window, cx);
        let copies = match file {
            Some((path, full)) => vec![("Relative path", path), ("Absolute path", full)],
            None => copies(store, &selection),
        };
        if copies.is_empty() {
            return;
        }
        let choices = (copies.into_iter())
            .map(|(label, text)| Choice::new(label, move |this, _, cx| this.copy_text(text, cx)))
            .collect();
        let place = store.place(&selection);
        self.open_picker(Some(place), "Copy", "Copy", choices, window, cx);
        if let Some(palette) = &mut self.palette {
            palette.holds_ids = true;
        }
    }

    /// Show `choices` to filter by typing and pick one with Enter or a click.
    pub fn open_picker(
        &mut self,
        place: Option<SharedString>,
        placeholder: &'static str,
        verb: &'static str,
        choices: Vec<Choice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(window, cx);
        // Opened from another picker, go back to where that one came from.
        let restore_focus = match &self.palette {
            Some(open) => open.restore_focus.clone(),
            None => window.focused(cx),
        };
        let input = cx.new(|cx| TextInput::new(placeholder, false, cx));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Changed => {
                    if let Some(palette) = &mut this.palette {
                        palette.found = palette.find(cx);
                        palette.select_first(cx);
                    }
                    this.outline_choice(cx);
                    cx.notify();
                }
                InputEvent::Submit => this.run_selected(window, cx),
                InputEvent::Cancel => this.close_palette(window, cx),
            });
        window.focus(&input.focus_handle(cx), cx);
        let mut palette = Palette {
            input,
            place,
            choices,
            verb,
            found: Vec::new(),
            selected: 0,
            scroll: ScrollHandle::new(),
            restore_focus,
            acting_on: self.store.read(cx).acting_on(),
            holds_ids: false,
            wide: false,
            _subscription: subscription,
        };
        palette.found = palette.find(cx);
        palette.select_first(cx);
        self.palette = Some(palette);
        self.outline_choice(cx);
        cx.notify();
    }

    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = self.palette.take()
            && let Some(focus) = palette.restore_focus
        {
            window.focus(&focus, cx);
        }
        (self.navigator).update(cx, |navigator, cx| navigator.stop_going(true, cx));
        cx.notify();
    }

    /// Have the navigator outline where the selected choice goes, if it goes anywhere.
    fn outline_choice(&mut self, cx: &mut Context<Self>) {
        let goes_to = self.palette.as_ref().and_then(|palette| {
            let found = palette.found.get(palette.selected)?;
            palette.choices.get(found.ix)?.goes_to.clone()
        });
        (self.navigator).update(cx, |navigator, cx| navigator.go_toward(goes_to, cx));
    }

    fn run_command(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(palette) = self.palette.take() else {
            return;
        };
        // Where it goes shows as the selection, from where the navigator scrolled to.
        (self.navigator).update(cx, |navigator, cx| navigator.stop_going(false, cx));
        let typed = palette.input.read(cx).text().trim().to_owned();
        let Some(choice) = palette.choices.into_iter().nth(ix) else {
            return;
        };
        // Run it where it was offered.
        if let Some(focus) = palette.restore_focus {
            window.focus(&focus, cx);
        }
        Self::once_drawn(window, cx, move |this, window, cx| {
            (choice.run)(this, &typed, window, cx)
        });
        cx.notify();
    }

    fn run_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let found = (self.palette.as_ref()).and_then(|p| p.found.get(p.selected));
        if let Some(ix) = found.map(|found| found.ix) {
            self.run_command(ix, window, cx);
        }
    }

    fn palette_up(&mut self, _: &PaletteUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.palette {
            palette.select(palette.selected.saturating_sub(1));
            self.outline_choice(cx);
            cx.notify();
        }
    }

    fn palette_down(&mut self, _: &PaletteDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.palette {
            let last = palette.found.len().saturating_sub(1);
            palette.select((palette.selected + 1).min(last));
            self.outline_choice(cx);
            cx.notify();
        }
    }

    /// Lite's picker dialog: near the top, a search row, 28px rows, and a footer of hints.
    pub fn render_palette(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.palette.as_ref()?;
        let theme = Theme::get(cx).clone();
        let typed = palette.input.read(cx).text().trim().to_owned();
        let rows: Vec<AnyElement> = (palette.found.iter())
            .enumerate()
            .filter_map(|(row, found)| {
                let ix = found.ix;
                let choice = palette.choices.get(ix)?;
                let selected = row == palette.selected;
                let label: SharedString = match (&choice.phrase, typed.is_empty()) {
                    (Some(phrase), true) => phrase.clone(),
                    _ if choice.names && !typed.is_empty() => {
                        format!("{} \"{typed}\"", choice.label).into()
                    }
                    _ => choice.label.clone(),
                };
                let el = list_row(("command", ix), selected, &theme)
                    .gap(dp(12.))
                    .when_some(choice.icon, |el, icon| {
                        el.child(small_icon(icon).text_color(theme.text_2))
                    })
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .child(matched(label, found.label.iter().cloned())),
                    )
                    .when_some(choice.detail.clone(), |el, detail| {
                        let detail = muted(matched(detail, found.detail.iter().cloned()), &theme);
                        el.child(detail.when(palette.wide, |detail| {
                            detail.min_w_0().max_w(relative(0.4)).truncate()
                        }))
                    })
                    .when_some(choice.key.clone(), |el, key| el.child(keycap(&key, &theme)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.run_command(ix, window, cx)
                    }));
                Some(el.into_any_element())
            })
            .collect();
        let empty = rows.is_empty();
        let card = popover(&theme)
            .id("palette")
            .key_context("Palette")
            .on_action(cx.listener(Self::palette_up))
            .on_action(cx.listener(Self::palette_down))
            .w(dp(match palette.wide {
                true => 720.,
                false => 520.,
            }))
            .max_w(relative(0.92))
            .max_h(dp(440.))
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .h(dp(36.))
                    .px(dp(12.))
                    .flex()
                    .items_center()
                    .gap(dp(12.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .child(div().flex_1().min_w_0().child(palette.input.clone()))
                    .children((palette.place.clone()).map(|place| muted(place, &theme))),
            )
            .child(
                v_flex()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id("palette-rows")
                            .flex_1()
                            .min_h_0()
                            .py(dp(4.))
                            .overflow_y_scroll()
                            .track_scroll(&palette.scroll)
                            .children(rows)
                            .when(empty, |el| {
                                el.child(
                                    div()
                                        .px(dp(12.))
                                        .py(dp(6.))
                                        .text_color(theme.text_2)
                                        .child("Nothing matches"),
                                )
                            }),
                    )
                    .children(scrollbar(
                        "palette-scrollbar",
                        Scroll::Div(palette.scroll.clone()),
                        &theme,
                    )),
            )
            .child(
                h_flex()
                    .flex_none()
                    .h(dp(36.))
                    .px(dp(12.))
                    .gap(dp(12.))
                    .border_t_1()
                    .border_color(theme.border_3)
                    .bg(theme.bg_2)
                    // Its ground fills the card's bottom corners, which it rounds as the card
                    // does, inside its border.
                    .rounded_b(dp(POPOVER_RADIUS - 1.))
                    .text_size(dp(12.))
                    .text_color(theme.text_2)
                    .children(
                        [("↑ ↓", "Move"), ("↩", palette.verb), ("esc", "Close")].map(
                            |(key, hint)| {
                                h_flex().gap(dp(4.)).child(keycap(key, &theme)).child(hint)
                            },
                        ),
                    ),
            );
        // Over a modal, a click outside closes only the palette. Down a column, it shrinks to
        // fit the window, zoomed in or small, its rows scrolling.
        let backdrop = backdrop("palette-backdrop", card, cx, Self::close_palette);
        let backdrop = backdrop.flex_col().justify_start().items_center();
        Some(backdrop.pt(dp(72.)).pb(dp(16.)).into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::{detail_score, id_starts, score, shared_start};

    const ID: &str = "3a79334c0b5e8f1d2a6b7c9e0f1a2b3c4d5e6f70";

    #[test]
    fn a_short_id_is_not_found_by_letters_scattered_through_it() {
        assert_eq!(
            detail_score("3a79334 · cache", Some(ID), "373"),
            None,
            "3, 7 and 3 appear in order in 3a79334, but that isn't an id typed"
        );
    }

    #[test]
    fn a_short_id_is_found_typed_from_its_start() {
        let detail = "3a79334 · cache";
        let found = detail_score(detail, Some(ID), "3a79").map(|(_, found)| found);
        assert_eq!(
            found.map(|found| found
                .into_iter()
                .map(|range| &detail[range])
                .collect::<Vec<_>>()),
            Some(Vec::from(["3a79"])),
            "the start of the id shown is underlined"
        );
    }

    #[test]
    fn a_commit_is_still_found_by_its_branch() {
        let detail = "3a79334 · cart-checkout";
        let found = detail_score(detail, Some(ID), "checkout").map(|(_, found)| found);
        assert_eq!(
            found.map(|found| found
                .into_iter()
                .map(|range| &detail[range])
                .collect::<Vec<_>>()),
            Some(Vec::from(["checkout"])),
            "past the id, the detail matches as any other, its underline where it was found"
        );
    }

    #[test]
    fn a_full_id_is_found_typed_from_its_start_in_either_case() {
        assert!(
            id_starts(ID, "3A79334C0B"),
            "an id as `git log` abbreviates it, or pasted upper case, finds its commit"
        );
    }

    #[test]
    fn a_full_id_is_found_whole() {
        assert!(
            id_starts(ID, ID),
            "Copy commit ID's whole id finds its commit"
        );
    }

    #[test]
    fn a_full_id_is_not_found_by_letters_inside_it() {
        assert!(
            !id_starts(ID, "c0b5e8"),
            "only from its start, or most queries would find some commit"
        );
    }

    #[test]
    fn a_full_id_is_not_found_by_more_than_it_has() {
        assert!(
            !id_starts(ID, &format!("{ID}0")),
            "a longer id is another commit's"
        );
    }

    #[test]
    fn typing_a_full_id_underlines_the_short_one_shown() {
        let detail = "3a79334 · cart-checkout";
        assert_eq!(
            shared_start(detail, "3a79334c0b5e").map(|range| &detail[range]),
            Some("3a79334"),
            "the short id the detail starts with is why the row is there"
        );
    }

    #[test]
    fn a_name_typed_in_full_comes_before_the_longer_ones_it_starts() {
        let rank = |label| score(label, "min-size").map(|(score, _)| score);
        assert!(
            rank("min-size") > rank("min-size-docs"),
            "Enter takes the first, so it must be the one named"
        );
    }

    #[test]
    fn the_letters_found_are_the_ones_underlined() {
        let underlined = |label: &'static str, query| -> Option<Vec<&str>> {
            let (_, found) = score(label, query)?;
            found.into_iter().map(|range| label.get(range)).collect()
        };
        assert_eq!(
            underlined("Open pull request", "PULL"),
            Some(vec!["pull"]),
            "a word typed in any case underlines that word"
        );
        assert_eq!(
            underlined("Next hunk", "nhu"),
            Some(vec!["N", "hu"]),
            "scattered letters underline each, neighbours as one"
        );
        assert_eq!(
            underlined("Über-Änderung", "üänd"),
            Some(vec!["Ü", "Änd"]),
            "ranges are bytes of the label as shown, so a letter two bytes long stays whole"
        );
    }
}
