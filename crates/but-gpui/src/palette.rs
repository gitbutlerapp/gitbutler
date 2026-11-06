//! The command palette (⌘K): every action that applies where focus was, with its keys,
//! filtered as you type. It doubles as the list of shortcuts, as in Lite and Zed. With nothing
//! typed, the command run last comes first while it still applies, then what right-clicking
//! the selection offers, worded as there.

use std::ops::Range;

use but_tty::Status;
use strum::IntoEnumIterator as _;

use gpui::{
    Action, AnyElement, ClickEvent, Context, Entity, FocusHandle, Focusable as _, Role,
    ScrollHandle, SharedString, Subscription, Window, actions, div, prelude::*, relative,
};

use crate::app::AppView;
use crate::backend::{
    Branch, Checkout, Commit, CommitId, FullName, RemoteCommit, Workspace, Worktree, short_ref,
};
use crate::commands::{COMMANDS, Command};
use crate::edits::Discard;
use crate::navigator::{
    RowStyle, lists_changes, mesh_choices, mesh_heading_choice, status_mark, worktrees_listed,
};
use crate::overlays::backdrop;
use crate::state::{
    Axis, Notice, ProjectEntry, ProjectId, ProjectTab, RowKey, Section, SectionKind, Selection,
    SelectionKind, Store, Tile, snack,
};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    Icon, POPOVER_RADIUS, commit_title, dp, h_flex, keycap, list_row, matched, muted, popover,
    shortcut_here, small_icon, tab_after, tab_stop, v_flex,
};

actions!(
    palette,
    [
        PaletteUp,
        PaletteDown,
        NextGroup,
        PreviousGroup,
        GoTo,
        GoToTerminal,
        CopyMore
    ]
);

/// What picking a choice does, given what's typed.
type Run = Box<dyn FnOnce(&mut AppView, &str, &mut Window, &mut Context<AppView>)>;

/// A row of the palette: what it's called, what it says at its end (keys, or a detail like
/// the project a branch is in), and what picking it does.
pub struct Choice {
    /// What kind of thing it is, before its label, where one list holds several kinds.
    icon: Option<Icon>,
    /// What typing finds it by, and what it shows once something is typed.
    label: SharedString,
    /// Other names typing finds it by, as a command's: see [`Command::aliases`].
    aliases: &'static [&'static str],
    /// What it shows instead while nothing is typed, as the selection's menu words it.
    phrase: Option<SharedString>,
    key: Option<SharedString>,
    detail: Option<SharedString>,
    /// It throws work away, so letters scattered through its label don't find it.
    risky: bool,
    /// It makes something named as typed: any text but another choice's label finds it,
    /// shown after its label.
    names: bool,
    /// A commit's full ids, its sha and change ID, either of which finds it typed from its start,
    /// as `git log` and the copy commands give them, though its detail shows less. Never by
    /// letters scattered through them, which would find every commit.
    ids: Vec<SharedString>,
    /// Where picking it goes, which the navigator outlines while it's selected. Picked, it runs
    /// at once rather than once the keys are drawn back where they were, so its `run` must
    /// dispatch no action, which would miss the handlers registered only there.
    goes_to: Option<Selection>,
    pins: Option<RowKey>,
    /// The terminal's row whose status it shows, kept when another picker takes it from Go to
    /// anything to do something else with it.
    status_of: Option<Selection>,
    /// It's in the selected project, so it comes before what's found as well elsewhere.
    in_selected_project: bool,
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
            pins: None,
            status_of: None,
            in_selected_project: false,
            aliases: &[],
            phrase: None,
            key: None,
            detail: None,
            risky: false,
            names: false,
            ids: Vec::new(),
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
    pub(crate) fn details<'a>(self, parts: impl IntoIterator<Item = &'a str>) -> Choice {
        let parts: Vec<&str> = parts.into_iter().collect();
        match parts.is_empty() {
            true => self,
            false => self.detail(parts.join(" · ")),
        }
    }

    pub(crate) fn icon(mut self, icon: Icon) -> Choice {
        self.icon = Some(icon);
        self
    }

    /// Found by commit `id` typed from its start: see [`Choice::ids`].
    pub(crate) fn commit_id(mut self, id: CommitId) -> Choice {
        self.ids.push(id.0.to_string().into());
        self
    }

    /// Found by `commit`'s sha and any change ID typed from their start: see [`Choice::ids`].
    fn commit_ids(mut self, commit: &Commit) -> Choice {
        if commit.has_change_id() {
            self.ids.push(commit.key.clone().into());
        }
        self.commit_id(commit.id)
    }

    /// What it pins beside the selected row, rather than the row it goes to.
    pub(crate) fn pins(mut self, row: RowKey) -> Choice {
        self.pins = Some(row);
        self
    }

    pub(crate) fn goes_to(mut self, selection: Selection) -> Choice {
        self.goes_to = Some(selection);
        self
    }

    /// Shows the status of terminal row `selection`, as the navigator does.
    fn status_of(mut self, selection: Selection) -> Choice {
        self.status_of = Some(selection);
        self
    }

    /// Ranks it with what's in the selected project: see [`Choice::in_selected_project`].
    fn in_selected_project(mut self) -> Choice {
        self.in_selected_project = true;
        self
    }
}

/// What a terminal's program is at, as the terminal picker groups its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusGroup {
    Blocked,
    Failed,
    Done,
    Working,
    Idle,
}

impl StatusGroup {
    /// As Tab goes through them: what wants the user first.
    const ALL: [StatusGroup; 5] = [
        StatusGroup::Blocked,
        StatusGroup::Failed,
        StatusGroup::Done,
        StatusGroup::Working,
        StatusGroup::Idle,
    ];

    fn of(status: &Status) -> StatusGroup {
        match status {
            Status::Blocked(_) => StatusGroup::Blocked,
            Status::Failed(_) => StatusGroup::Failed,
            Status::Done => StatusGroup::Done,
            Status::Working => StatusGroup::Working,
            Status::Idle => StatusGroup::Idle,
        }
    }

    fn label(self) -> &'static str {
        match self {
            StatusGroup::Blocked => "Blocked",
            StatusGroup::Failed => "Failed",
            StatusGroup::Done => "Done",
            StatusGroup::Working => "Working",
            StatusGroup::Idle => "Idle",
        }
    }
}

/// The terminal picker's groups, which Tab goes through. They're sorted out as the query or the
/// group changes, not as a program's status does, so what an agent does moves no row from under
/// the keys. The row's own mark still follows it.
#[derive(Default)]
struct Groups {
    /// The group listed alone, or with none, every row.
    shown: Option<StatusGroup>,
    /// How many rows the query finds in each group it finds any in, and in the one shown.
    counts: Vec<(StatusGroup, usize)>,
    /// How many rows the query finds.
    total: usize,
}

impl Groups {
    /// Count `found`, each with its group, and keep the rows of the group shown.
    fn sort_out(&mut self, found: Vec<(Option<StatusGroup>, Found)>) -> Vec<Found> {
        self.total = found.len();
        self.counts = (StatusGroup::ALL.into_iter())
            .filter_map(|group| {
                let count = found.iter().filter(|(of, _)| *of == Some(group)).count();
                (count > 0 || self.shown == Some(group)).then_some((group, count))
            })
            .collect();
        (found.into_iter())
            .filter(|(of, _)| self.shown.is_none() || *of == self.shown)
            .map(|(_, found)| found)
            .collect()
    }

    /// The group after the one shown, or before it, going round through every row.
    fn step(&self, forward: bool) -> Option<StatusGroup> {
        let groups = self.counts.iter().map(|(group, _)| Some(*group));
        let stops: Vec<Option<StatusGroup>> = std::iter::once(None).chain(groups).collect();
        let at = stops.iter().position(|stop| *stop == self.shown);
        let to = tab_after(stops.len(), at.unwrap_or(0), forward)?;
        stops.get(to).copied().flatten()
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
    /// What the query finds in the group shown, best first, worked out as either changes:
    /// nothing else changes it.
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
    /// The terminal picker's: what was selected and whether the details showed when it
    /// opened. The row it selects shows behind it until Esc puts these back.
    previewed_from: Option<(Option<Selection>, bool)>,
    /// The terminal picker's, which lists its rows by status.
    groups: Option<Groups>,
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

    /// List the choices the query finds, best first, of the group shown if there is one.
    fn find(&mut self, store: &Store, cx: &gpui::App) {
        let query = self.input.read(cx).text().trim().to_owned();
        let found = find_in(&self.choices, &query);
        self.found = match &mut self.groups {
            Some(groups) => {
                let grouped = found.into_iter().map(|found| {
                    let terminal = (self.choices.get(found.ix)).and_then(|c| c.status_of.as_ref());
                    let status = terminal.and_then(|terminal| store.row_status(terminal));
                    (status.map(StatusGroup::of), found)
                });
                groups.sort_out(grouped.collect())
            }
            None => found,
        };
    }
}

/// What being in the selected project adds to a label's match: more than where in the label it
/// falls, so it comes before matches of its kind elsewhere, and less than the step between kinds
/// of match, so what's named in full in another project still comes first.
const SELECTED_PROJECT_BONUS: i32 = 500;

/// The `choices` that `query` finds, best first.
fn find_in(choices: &[Choice], query: &str) -> Vec<Found> {
    let taken = (choices.iter()).any(|c| !c.names && c.label.eq_ignore_ascii_case(query));
    let mut scored: Vec<(i32, Found)> = choices
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
            let label = score(&choice.label, query);
            // Under the label on a tie, and with nothing underlined: what matched isn't shown.
            let alias = alias_score(choice.aliases, query)
                .map(|score| (score - 1, Vec::new()))
                .filter(|(score, _)| label.as_ref().is_none_or(|(label, _)| score > label));
            let label = alias.or(label);
            // The label first on a tie, as a branch named as another's detail says.
            let detail = (choice.detail.as_ref())
                .and_then(|detail| detail_score(detail, &choice.ids, query))
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
            // Not by its detail, as every row of a project has its name there. The order of the
            // choices settles a tie, and all of it with nothing typed: see `rank_selected_project`.
            let by_label = found.detail.is_empty() && !query.is_empty();
            let bonus = if choice.in_selected_project && by_label {
                SELECTED_PROJECT_BONUS
            } else {
                0
            };
            (!scattered || !choice.risky).then_some((score + bonus, found))
        })
        .collect();
    // Stable, so equal scores keep the recent-first order.
    scored.sort_by_key(|(score, _)| -score);
    scored.into_iter().map(|(_, found)| found).collect()
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

/// How well `query` matches the best of `aliases`, as [`score`] says, but only by their words, as
/// "remove" in Remove branch: letters scattered through them would find what the label doesn't.
fn alias_score(aliases: &[&str], query: &str) -> Option<i32> {
    (aliases.iter())
        .filter_map(|alias| score(alias, query))
        .map(|(score, _)| score)
        .filter(|score| *score > 0)
        .max()
}

/// How much of a commit's id it takes to find it, as Git's shortest abbreviation does. Ids are
/// mostly pasted, and a letter or two is more likely the start of a word: a change ID's letters
/// are the ones people type, so `s` alone would find one commit in sixteen.
const ID_QUERY_MIN: usize = 4;

/// How well `query` matches a row's `detail`, as [`score`] says, but a commit's `ids` only typed
/// from their start, at least [`ID_QUERY_MIN`] of them, the short one the detail starts with too:
/// letters scattered through it would find nearly every commit. The rest of the detail, its branch
/// and project, matches as ever.
fn detail_score(
    detail: &str,
    ids: &[SharedString],
    query: &str,
) -> Option<(i32, Vec<Range<usize>>)> {
    if query.len() >= ID_QUERY_MIN && ids.iter().any(|id| id_starts(id, query)) {
        return Some((2000, shared_start(detail, query).into_iter().collect()));
    }
    let shown = (ids.iter())
        .filter_map(|id| shared_start(detail, id))
        .map(|shown| shown.end)
        .max()
        .unwrap_or(0);
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
        Selection::Checkout(project, Checkout::Linked(name)) => {
            let worktree =
                (store.workspace(project)).and_then(|ws| ws.find_worktree(name.as_ref()));
            copies.extend(worktree.map(worktree_copies).into_iter().flatten());
        }
        // The main worktree's row, which has no name of its own.
        Selection::Checkout(project, Checkout::Main) => {
            if let Some(path) = store.path(project) {
                copies.push(("Worktree path", path.display().to_string()));
            }
        }
        Selection::Project(_)
        | Selection::Uncommitted(..)
        | Selection::Upstream(..)
        | Selection::Mesh(..)
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
        .pins(RowKey::Tab(entry.id.clone(), ProjectTab::Overview))
}

/// Go to a tab of a project's page, which beside the project's own row pins on that tab.
fn tab_choice(
    entry: &ProjectEntry,
    tab: ProjectTab,
    label: &'static str,
    icon: Icon,
    several: bool,
) -> Choice {
    let gone = format!("{} was removed meanwhile", entry.project.name);
    let id = entry.id.clone();
    let choice = Choice::new(label, move |this, window, cx| {
        if this.store.read(cx).project(&id).is_none() {
            return snack(&this.store, gone, cx);
        }
        this.open_project_tab(id, tab, window, cx)
    });
    let project = several.then_some(entry.project.name.as_str());
    choice
        .icon(icon)
        .details(project)
        .goes_to(Selection::Project(entry.id.clone()))
        .pins(RowKey::Tab(entry.id.clone(), tab))
}

/// Go to anything's rows of `kind`, as the navigator lists them in the projects `open`. There's
/// no wildcard: a new kind of row doesn't compile until it's decided here.
fn kind_choices(
    kind: SelectionKind,
    store: &Store,
    open: &[(&ProjectEntry, &Workspace)],
) -> Vec<Choice> {
    let several = open.len() > 1;
    // Not while a commit is checked out, which the navigator hides most rows for.
    let unedited = || (open.iter()).filter(|(_, workspace)| workspace.editing.is_none());
    match kind {
        SelectionKind::Project => {
            let tabs = [
                (ProjectTab::Branches, "Branches", Icon::Branch),
                (ProjectTab::History, "Operations history", Icon::Clock),
                (ProjectTab::Graph, "Graph", Icon::Fork),
            ];
            let tabs = tabs.into_iter().flat_map(|(tab, label, icon)| {
                unedited().map(move |(entry, _)| tab_choice(entry, tab, label, icon, several))
            });
            (store.projects.iter().map(project_choice))
                .chain(tabs)
                .collect()
        }
        SelectionKind::Section => (SectionKind::iter())
            .flat_map(|section| {
                (open.iter()).flat_map(move |(entry, workspace)| {
                    section_choices(section, store, entry, workspace, several)
                })
            })
            .collect(),
        SelectionKind::Checkout => (open.iter())
            .flat_map(|(entry, workspace)| {
                let main = (workspace.editing.is_none()).then(|| {
                    let main = Selection::Checkout(entry.id.clone(), Checkout::Main);
                    page_choice(entry, main, "Main worktree", Icon::Folder, several)
                });
                let linked = (worktrees_listed(store, &entry.id, workspace).into_iter())
                    .map(move |wt| worktree_choice(entry, wt, several));
                main.into_iter().chain(linked)
            })
            .collect(),
        SelectionKind::Uncommitted => (open.iter())
            .flat_map(|(entry, workspace)| uncommitted_choices(store, entry, workspace, several))
            .collect(),
        SelectionKind::Terminal => unedited()
            .flat_map(|(entry, workspace)| terminal_choices(store, entry, workspace, several))
            .collect(),
        SelectionKind::Branch => (open.iter())
            .flat_map(|(entry, workspace)| {
                (workspace.all_branches())
                    .filter(|branch| branch.ref_name.is_some())
                    .map(move |branch| branch_choice(store, entry, branch, several))
            })
            .collect(),
        // Not a row: the navigator lists a stack's branches, not the stack.
        SelectionKind::Stack => Vec::new(),
        SelectionKind::Commit => (open.iter())
            .flat_map(|(entry, workspace)| {
                workspace.all_branches().flat_map(move |branch| {
                    (branch.commits.iter())
                        .filter(move |commit| !store.hides_commit(&entry.id, &commit.key))
                        .map(move |commit| commit_choice(store, entry, branch, commit, several))
                })
            })
            .collect(),
        SelectionKind::Upstream => (open.iter())
            .flat_map(|(entry, workspace)| {
                upstream_commit_choices(store, entry, workspace, several)
            })
            .collect(),
        SelectionKind::Mesh => (open.iter())
            .flat_map(|(entry, workspace)| mesh_choices(store, entry, workspace, several))
            .collect(),
    }
}

/// Go to anything's rows for the headings over a loaded project's sections of `kind`, as the
/// navigator names them, while they hold anything.
fn section_choices(
    kind: SectionKind,
    store: &Store,
    entry: &ProjectEntry,
    workspace: &Workspace,
    several: bool,
) -> Vec<Choice> {
    match kind {
        SectionKind::Upstream => (workspace.all_branches())
            .filter_map(|branch| remote_choice(store, entry, branch, several))
            .collect(),
        SectionKind::Inactive => inactive_choice(store, entry, several).into_iter().collect(),
        SectionKind::Mesh => mesh_heading_choice(store, entry, workspace, several)
            .into_iter()
            .collect(),
    }
}

/// Go to anything's row for one of a loaded project's pages or its main worktree, as the
/// navigator names it.
fn page_choice(
    entry: &ProjectEntry,
    page: Selection,
    label: &'static str,
    icon: Icon,
    several: bool,
) -> Choice {
    let gone = format!("{} was removed meanwhile", entry.project.name);
    let goes_to = page.clone();
    let choice = Choice::new(label, move |this, window, cx| {
        if this.store.read(cx).project(page.project()).is_none() {
            return snack(&this.store, gone, cx);
        }
        this.select(page, window, cx)
    });
    let project = several.then_some(entry.project.name.as_str());
    choice.icon(icon).details(project).goes_to(goes_to)
}

/// Go to anything's row for the heading over `section` of `project`'s card, as the navigator
/// names it; `gone` once `heads` says it heads nothing, as after a pull or an update.
pub(crate) fn heading_choice(
    project: &ProjectId,
    section: Section,
    label: impl Into<SharedString>,
    icon: Icon,
    gone: impl Into<SharedString>,
    heads: impl Fn(&Store) -> bool + 'static,
) -> Choice {
    let heading = Selection::Section(project.clone(), section);
    let goes_to = heading.clone();
    let gone = gone.into();
    let choice = Choice::new(label, move |this, window, cx| {
        if !heads(this.store.read(cx)) {
            return snack(&this.store, gone, cx);
        }
        this.select(heading, window, cx)
    });
    choice.icon(icon).goes_to(goes_to)
}

/// Go to anything's row for the heading over the commits only `branch`'s remote has, as the
/// navigator names it, the branch beside it, while there are any.
fn remote_choice(
    store: &Store,
    entry: &ProjectEntry,
    branch: &Branch,
    several: bool,
) -> Option<Choice> {
    let ref_name = branch.ref_name.clone()?;
    if branch.incoming.is_empty() || store.hides_branch(&entry.id, &ref_name) {
        return None;
    }
    let label = match &branch.remote {
        Some(remote) => format!("On {remote}"),
        None => "On the remote".to_owned(),
    };
    let gone = format!("Nothing new on {}'s remote anymore", branch.name);
    let id = entry.id.clone();
    let section = Section::Upstream(ref_name.clone());
    let heads = move |store: &Store| {
        (store.workspace(&id)).is_some_and(|ws| !ws.upstream(&ref_name).is_empty())
    };
    let choice = heading_choice(&entry.id, section, label, Icon::ArrowLineDown, gone, heads);
    let project = several.then_some(entry.project.name.as_str());
    Some(choice.details(std::iter::once(store.label_of(&entry.id, branch)).chain(project)))
}

/// Go to anything's rows for the commits only a branch's remote has, as the navigator lists
/// them under their headings. Not those a merge brought in, which load only as it opens.
fn upstream_commit_choices(
    store: &Store,
    entry: &ProjectEntry,
    workspace: &Workspace,
    several: bool,
) -> Vec<Choice> {
    (workspace.all_branches())
        .filter_map(|branch| {
            let ref_name = branch.ref_name.as_ref()?;
            (!store.hides_branch(&entry.id, ref_name)).then_some((branch, ref_name))
        })
        .flat_map(|(branch, ref_name)| {
            let place = store.label_of(&entry.id, branch);
            (branch.incoming.iter()).map(move |commit| {
                upstream_commit_choice(entry, ref_name.clone(), place, commit, several)
            })
        })
        .collect()
}

/// Go to anything's row for a commit only branch `owner`'s remote has, by its title, its short
/// id and where it's new beside it, where typing either finds it.
fn upstream_commit_choice(
    entry: &ProjectEntry,
    owner: FullName,
    place: &str,
    commit: &RemoteCommit,
    several: bool,
) -> Choice {
    let (id, commit_id) = (entry.id.clone(), commit.id);
    let goes_to = Selection::Upstream(id.clone(), owner.clone(), commit.id);
    let selection = goes_to.clone();
    let short = commit.id.short();
    let gone = format!("Commit {short} was taken in meanwhile");
    let choice = Choice::new(commit_title(&commit.title), move |this, window, cx| {
        // Pulled, or the workspace updated, since the picker opened.
        if (this.store.read(cx).workspace(&id))
            .is_none_or(|ws| !ws.upstream(&owner).iter().any(|c| c.id == commit_id))
        {
            return snack(&this.store, gone, cx);
        }
        this.select(selection, window, cx)
    });
    let project = several.then_some(entry.project.name.as_str());
    let details = [short.as_str(), place];
    (choice.icon(Icon::ArrowDown))
        .details(details.into_iter().chain(project))
        .goes_to(goes_to)
        .commit_id(commit.id)
}

/// Go to anything's row for the heading over a loaded project's inactive worktrees, as the
/// navigator names it, while there are any.
fn inactive_choice(store: &Store, entry: &ProjectEntry, several: bool) -> Option<Choice> {
    if store.inactive_worktrees(&entry.id).is_empty() {
        return None;
    }
    let id = entry.id.clone();
    // Worked in since the picker opened.
    let heads = move |store: &Store| !store.inactive_worktrees(&id).is_empty();
    let (label, gone) = ("Inactive worktrees", "No inactive worktrees anymore");
    let choice = heading_choice(
        &entry.id,
        Section::Inactive,
        label,
        Icon::Clock,
        gone,
        heads,
    );
    Some(choice.details(several.then_some(entry.project.name.as_str())))
}

/// Go to anything's rows for a loaded project's uncommitted changes, the main worktree's first,
/// as the navigator lists them under each worktree while there are some; while a commit is
/// checked out, the edit's, which takes their place.
fn uncommitted_choices(
    store: &Store,
    entry: &ProjectEntry,
    workspace: &Workspace,
    several: bool,
) -> Vec<Choice> {
    if workspace.editing.is_some() {
        return vec![uncommitted_choice(entry, Checkout::Main, true, several)];
    }
    let linked = (worktrees_listed(store, &entry.id, workspace).into_iter())
        .map(|wt| Checkout::Linked(wt.name.clone()));
    std::iter::once(Checkout::Main)
        .chain(linked)
        .filter(|checkout| lists_changes(store, workspace, &entry.id, checkout))
        .map(|checkout| uncommitted_choice(entry, checkout, false, several))
        .collect()
}

/// Go to anything's row for a worktree's uncommitted changes, or while `editing`, the commit
/// checked out's, the worktree they're in beside it.
fn uncommitted_choice(
    entry: &ProjectEntry,
    checkout: Checkout,
    editing: bool,
    several: bool,
) -> Choice {
    let id = entry.id.clone();
    let goes_to = Selection::Uncommitted(id.clone(), checkout.clone());
    let selection = goes_to.clone();
    let worktree = (checkout.linked()).map_or("Main worktree".to_owned(), |name| name.to_string());
    let (label, icon, gone) = match editing {
        true => (
            "Editing a commit",
            Icon::Diff,
            "The edit ended meanwhile".to_owned(),
        ),
        false => (
            "Uncommitted changes",
            Icon::FileDiff,
            format!("No uncommitted changes in {worktree} anymore"),
        ),
    };
    let choice = Choice::new(label, move |this, window, cx| {
        // Committed or discarded since the picker opened, or the edit finished.
        let listed = {
            let store = this.store.read(cx);
            (store.workspace(&id)).is_some_and(|ws| match editing {
                true => ws.editing.is_some(),
                false => ws.editing.is_none() && lists_changes(store, ws, &id, &checkout),
            })
        };
        if !listed {
            return snack(&this.store, gone, cx);
        }
        this.select(selection, window, cx)
    });
    let project = several.then_some(entry.project.name.as_str());
    let details = (!editing)
        .then_some(worktree.as_str())
        .into_iter()
        .chain(project);
    choice.icon(icon).details(details).goes_to(goes_to)
}

/// Go to anything's row for a linked worktree, the branch it has checked out beside it.
fn worktree_choice(entry: &ProjectEntry, worktree: &Worktree, several: bool) -> Choice {
    let (id, name) = (entry.id.clone(), worktree.name.clone());
    let goes_to = Selection::Checkout(id.clone(), Checkout::Linked(name.clone()));
    let gone = format!("{name} was archived or deleted meanwhile");
    let branch = worktree.branch.as_ref().map(short_ref);
    let project = several.then_some(entry.project.name.as_str());
    let choice = Choice::new(name.to_string(), move |this, window, cx| {
        if (this.store.read(cx).workspace(&id))
            .is_none_or(|ws| ws.find_worktree(name.as_ref()).is_none())
        {
            return snack(&this.store, gone, cx);
        }
        this.select(Selection::Checkout(id, Checkout::Linked(name)), window, cx)
    });
    let details = branch.as_deref().into_iter().chain(project);
    choice
        .icon(Icon::FolderCopy)
        .details(details)
        .goes_to(goes_to)
}

/// The projects go to anything looks inside: only expanded ones, as a collapsed one's rows
/// aren't drawn, or kept current.
fn open_workspaces(store: &Store) -> Vec<(&ProjectEntry, &Workspace)> {
    (store.projects.iter())
        .filter(|entry| entry.expanded)
        .filter_map(|entry| Some((entry, entry.workspace.as_ref()?)))
        .collect()
}

/// What Go to anything lists: every row of the navigator's it can go to, broadest first, as
/// `Selection` lists its kinds, which an equal match in one project keeps: a project before a
/// branch of its name. The selected project's rows lead: see [`rank_selected_project`].
fn go_to_choices(store: &Store) -> Vec<Choice> {
    let open = open_workspaces(store);
    let mut choices: Vec<Choice> = (SelectionKind::iter())
        .flat_map(|kind| kind_choices(kind, store, &open))
        .collect();
    rank_selected_project(&mut choices, store.selection.as_ref());
    choices
}

/// Note which of `choices` go somewhere in the project of `selection`, and list them as they show
/// with nothing typed: that project's own row, the other projects' so switching stays a key or
/// two away, the rest of that project's rows, then everything else.
fn rank_selected_project(choices: &mut [Choice], selection: Option<&Selection>) {
    let Some(selected) = selection.map(Selection::project) else {
        return;
    };
    for choice in choices.iter_mut() {
        choice.in_selected_project =
            (choice.goes_to.as_ref()).is_some_and(|goes_to| goes_to.project() == selected);
    }
    // Stable, so rows that tie keep their order.
    choices.sort_by_key(|choice| {
        let project_row = matches!(choice.pins, Some(RowKey::Tab(_, ProjectTab::Overview)));
        (!project_row, !choice.in_selected_project)
    });
}

/// The row Go to anything goes to typed `query` in full: the one it names whole, ignoring case,
/// else the first whose label has it, in the selected project before the others.
#[cfg(feature = "dev-script")]
pub(crate) fn go_to_row(store: &Store, query: &str) -> Option<Selection> {
    let query = query.to_lowercase();
    let mut rows: Vec<(bool, bool, Selection)> = (go_to_choices(store).into_iter())
        .filter_map(|choice| {
            let label = choice.label.to_lowercase();
            let goes_to = choice.goes_to?;
            let elsewhere = !choice.in_selected_project;
            (label.contains(&query)).then(|| (label != query, elsewhere, goes_to))
        })
        .collect();
    // Stable, so rows that tie keep the navigator's order.
    rows.sort_by_key(|(partly, elsewhere, _)| (*partly, *elsewhere));
    rows.into_iter().next().map(|(_, _, selection)| selection)
}

/// Go to anything's rows for the terminals open on a loaded project's worktrees, the main
/// one's first, as the navigator lists them under each.
fn terminal_choices(
    store: &Store,
    entry: &ProjectEntry,
    workspace: &Workspace,
    several: bool,
) -> Vec<Choice> {
    let linked = (worktrees_listed(store, &entry.id, workspace).into_iter())
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
        .status_of(goes_to.clone())
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
    let short = commit.short_reference();
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
    (choice.icon(Icon::Commit))
        .details(details.into_iter().chain(project))
        .goes_to(goes_to)
        .commit_ids(commit)
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
                // Only as the selection's menu words it, for what it takes: alone, typing what
                // another row's Discard is called would land on this one's.
                ty != type_of(&Discard)
                    && available.contains(&ty)
                    && !in_menu.contains(&ty)
                    && command.listed(self, cx)
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
                aliases: command.aliases(),
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
        // Of the page with the keys, a pinned one's rather than the selected row's.
        let place = (self.detail_with_keys(window, cx))
            .and_then(|detail| detail.read(cx).selection())
            .or_else(|| self.store.read(cx).selection.clone())
            .filter(|_| !items.is_empty())
            .map(|row| self.store.read(cx).place(&row));
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
                    aliases: item.aliases,
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
        // For the metrics: what a choice here starts was started from ⌘K, which other pickers'
        // choices weren't.
        let choices = (choices.into_iter())
            .map(|mut choice| {
                let run = choice.run;
                choice.run = Box::new(move |this, typed, window, cx| {
                    crate::backend::metrics::input(crate::backend::metrics::Via::Palette);
                    run(this, typed, window, cx)
                });
                choice
            })
            .collect();
        self.open_picker(place, "Search commands", "Run", choices, window, cx);
        if let Some(palette) = &mut self.palette {
            palette.holds_ids = holds_ids;
        }
    }

    /// Pick any row of the navigator's, a loaded project's by name, pull request title or
    /// commit id, and select it.
    pub fn go_to(&mut self, _: &GoTo, window: &mut Window, cx: &mut Context<Self>) {
        let choices = go_to_choices(self.store.read(cx));
        let placeholder = "Go to anything in the navigator";
        self.open_picker(None, placeholder, "Go", choices, window, cx);
        if let Some(palette) = &mut self.palette {
            palette.wide = true;
        }
    }

    /// Divide the tile with the keys along `axis`, giving the new tile a new terminal or what
    /// Go to anything lists that can be pinned and the tiling doesn't show yet, picked here.
    pub fn split(&mut self, axis: Axis, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.focused_tile(window, cx);
        self.split_tile(at, axis, window, cx);
    }

    /// Pick what shows beside `at` of the selected row's tiling, along `axis`.
    pub(crate) fn split_tile(
        &mut self,
        at: Tile,
        axis: Axis,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let Some(selection) = store.selection.clone() else {
            return;
        };
        let (row, shown) = (store.row_key(&selection), store.pinned());
        let beside = at.clone();
        let new_terminal = Choice::new("New terminal", move |this, _, cx| {
            (this.store).update(cx, |store, cx| store.pin_terminal(beside, axis, cx))
        })
        .icon(Icon::TextCode)
        .in_selected_project();
        let rest = go_to_choices(store).into_iter().filter_map(|choice| {
            let pinned =
                (choice.pins.clone()).or_else(|| Some(store.row_key(choice.goes_to.as_ref()?)))?;
            if pinned == row || shown.contains(&pinned) {
                return None;
            }
            // Not where it goes, which would select it rather than pin it, and so run once the
            // keys are back in the tile it divides, which the new one takes from it.
            let at = at.clone();
            Some(Choice {
                goes_to: None,
                run: Box::new(move |this, _, _, cx| {
                    (this.store).update(cx, |store, cx| store.pin(at, axis, pinned, cx))
                }),
                ..choice
            })
        });
        let choices = std::iter::once(new_terminal).chain(rest).collect();
        let placeholder = match axis {
            Axis::Across => "Show to the right",
            Axis::Down => "Show below",
        };
        self.open_picker(None, placeholder, "Show", choices, window, cx);
        if let Some(palette) = &mut self.palette {
            palette.wide = true;
        }
    }

    /// Go to anything's terminals alone.
    pub fn go_to_terminal(
        &mut self,
        _: &GoToTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let open = open_workspaces(store);
        let several = open.len() > 1;
        let mut choices: Vec<Choice> = (open.iter())
            .filter(|(_, workspace)| workspace.editing.is_none())
            .flat_map(|(entry, workspace)| terminal_choices(store, entry, workspace, several))
            .collect();
        if choices.is_empty() {
            return snack(&self.store, "No terminals open", cx);
        }
        let selection = store.selection.clone();
        rank_selected_project(&mut choices, selection.as_ref());
        self.open_picker(None, "Go to a terminal", "Go", choices, window, cx);
        if let Some(palette) = &mut self.palette {
            palette.wide = true;
            palette.groups = Some(Groups::default());
            palette.find(self.store.read(cx), cx);
            // On the terminal shown, so opening it shows no other.
            let shown = (palette.found.iter()).position(|found| {
                let goes_to = palette
                    .choices
                    .get(found.ix)
                    .and_then(|c| c.goes_to.as_ref());
                goes_to.is_some() && goes_to == selection.as_ref()
            });
            if let Some(row) = shown {
                palette.select(row);
            }
            palette.previewed_from = Some((selection, self.detail_open));
        }
        self.outline_choice(window, cx);
    }

    /// What can be copied from the selection, to pick from with the keys, as the TUI's ⇧Y
    /// offers: the file under the cursor while the file list or the diff has the keys, as
    /// right-clicking takes it, its hunk and diff and then its paths. The first is what y
    /// copies, unless text is selected in the diff.
    pub fn copy_more(&mut self, _: &CopyMore, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(selection) = store.selection.clone() else {
            return;
        };
        let detail = self.detail.read(cx);
        let copies = match detail.focused_file_paths(window, cx) {
            Some((path, full)) => {
                let mut copies = detail.patch_copies(window, cx);
                copies.extend([("Relative path", path), ("Absolute path", full)]);
                copies
            }
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
                        palette.find(this.store.read(cx), cx);
                        palette.select_first(cx);
                    }
                    this.outline_choice(window, cx);
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
            previewed_from: None,
            groups: None,
            _subscription: subscription,
        };
        palette.find(self.store.read(cx), cx);
        palette.select_first(cx);
        self.palette = Some(palette);
        self.outline_choice(window, cx);
        cx.notify();
    }

    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = self.palette.take() {
            // Back to what the terminal picker opened over.
            if let Some((selection, detail_open)) = palette.previewed_from {
                self.select_behind(selection, cx);
                self.set_detail(detail_open, window, cx);
            }
            if let Some(focus) = palette.restore_focus {
                window.focus(&focus, cx);
            }
        }
        (self.navigator).update(cx, |navigator, cx| navigator.stop_going(true, cx));
        cx.notify();
    }

    /// Have the navigator outline where the selected choice goes, if it goes anywhere, and the
    /// terminal picker select it behind, the details showing, while the keys stay in it.
    fn outline_choice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(palette) = &self.palette else {
            return (self.navigator).update(cx, |navigator, cx| navigator.go_toward(None, cx));
        };
        let goes_to = (palette.found.get(palette.selected))
            .and_then(|found| palette.choices.get(found.ix)?.goes_to.clone());
        // Not while something is being moved, which the selection stays on.
        if palette.previewed_from.is_some()
            && self.store.read(cx).moving.is_none()
            && let Some(selection) = goes_to.clone()
        {
            self.select_behind(Some(selection), cx);
            self.set_detail(true, window, cx);
            // It moved the selection itself, so it isn't what closes the palette as an agent's
            // move would.
            let acting_on = self.store.read(cx).acting_on();
            if let Some(palette) = &mut self.palette {
                palette.acting_on = acting_on;
            }
        }
        (self.navigator).update(cx, |navigator, cx| navigator.go_toward(goes_to, cx));
    }

    /// Select `selection` behind the palette, the keys staying in it. Not again once it's
    /// selected, which would drop the commits picked with it.
    fn select_behind(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        (self.store).update(cx, |store, cx| {
            if store.selection != selection {
                store.select(selection, cx);
            }
        });
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
        // While something is being moved, a choice that goes somewhere moves it there instead,
        // as a click on its row does, and does nothing else: the selection stays on what moves.
        if let Some(goes_to) =
            (choice.goes_to.clone()).filter(|_| self.store.read(cx).moving.is_some())
        {
            (self.store).update(cx, |store, cx| store.select_by_hand(goes_to, cx));
            return cx.notify();
        }
        // Going somewhere selects it and gives the keys away itself, so at once: drawn with the
        // keys back first, the navigator would flash its selection solid before a terminal
        // entered took them.
        if choice.goes_to.is_some() {
            (choice.run)(self, &typed, window, cx);
        } else {
            Self::once_drawn(window, cx, move |this, window, cx| {
                (choice.run)(this, &typed, window, cx)
            });
        }
        cx.notify();
    }

    fn run_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let found = (self.palette.as_ref()).and_then(|p| p.found.get(p.selected));
        if let Some(ix) = found.map(|found| found.ix) {
            self.run_command(ix, window, cx);
        }
    }

    fn palette_up(&mut self, _: &PaletteUp, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.palette {
            palette.select(palette.selected.saturating_sub(1));
            self.outline_choice(window, cx);
            cx.notify();
        }
    }

    fn palette_down(&mut self, _: &PaletteDown, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.palette {
            let last = palette.found.len().saturating_sub(1);
            palette.select((palette.selected + 1).min(last));
            self.outline_choice(window, cx);
            cx.notify();
        }
    }

    fn next_group(&mut self, _: &NextGroup, window: &mut Window, cx: &mut Context<Self>) {
        self.show_group(|groups| groups.step(true), window, cx);
    }

    fn previous_group(&mut self, _: &PreviousGroup, window: &mut Window, cx: &mut Context<Self>) {
        self.show_group(|groups| groups.step(false), window, cx);
    }

    /// List the group `pick` says alone, from what the programs are at now, on its first row.
    fn show_group(
        &mut self,
        pick: impl FnOnce(&Groups) -> Option<StatusGroup>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        // Other pickers have no groups, and Tab reaches them too.
        let Some(palette) = (self.palette.as_mut()).filter(|p| p.groups.is_some()) else {
            return;
        };
        // Sorted out again first, so Tab skips a group that has emptied since.
        palette.find(store, cx);
        if let Some(groups) = &mut palette.groups {
            groups.shown = pick(groups);
        }
        palette.find(store, cx);
        palette.select_first(cx);
        self.outline_choice(window, cx);
        cx.notify();
    }

    /// Lite's picker dialog: near the top, a search row, 28px rows, and a footer of hints.
    pub fn render_palette(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.palette.as_ref()?;
        let theme = Theme::get(cx).clone();
        let typed = palette.input.read(cx).text().trim().to_owned();
        let store = self.store.read(cx);
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
                // Read here, not kept in the choice, so it changes while the palette is open.
                let status = (choice.status_of.as_ref())
                    .and_then(|terminal| store.row_status(terminal))
                    .and_then(|status| {
                        status_mark(status, !store.hidden(), RowStyle::plain(&theme, 0.))
                    });
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
                    .children(status)
                    .when_some(choice.key.clone(), |el, key| el.child(keycap(&key, &theme)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.run_command(ix, window, cx)
                    }));
                Some(el.into_any_element())
            })
            .collect();
        let empty = rows.is_empty();
        let group_tabs = palette.groups.as_ref().map(|groups| {
            let every = (None, "All", groups.total);
            let counted =
                (groups.counts.iter()).map(|(group, count)| (Some(*group), group.label(), *count));
            let text = theme.text_1;
            let tabs = std::iter::once(every)
                .chain(counted)
                .map(|(group, name, count)| {
                    let on = groups.shown == group;
                    let tab = h_flex()
                        .id(name)
                        .h_full()
                        .gap(dp(4.))
                        // Always there, so picking another moves nothing.
                        .border_b_2()
                        .border_color(if on { text } else { gpui::transparent_black() })
                        .text_color(if on { text } else { theme.text_2 })
                        .hover(move |el| el.text_color(text))
                        .aria_label(format!("{name} {count}"))
                        .aria_selected(on)
                        .child(name)
                        .child(div().text_color(theme.text_3).child(count.to_string()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.show_group(|_| group, window, cx)
                        }));
                    tab_stop(tab, &theme).role(Role::Tab)
                });
            h_flex()
                .id("status-tabs")
                .role(Role::TabList)
                .aria_label("Status")
                .flex_none()
                .h(dp(28.))
                .px(dp(12.))
                .gap(dp(14.))
                .border_b_1()
                .border_color(theme.border_3)
                .text_size(dp(12.))
                .children(tabs)
        });
        let tab_hint = palette.groups.as_ref().map(|_| ("⇥", "Filter"));
        let card = popover(&theme)
            .id("palette")
            .key_context("Palette")
            .on_action(cx.listener(Self::palette_up))
            .on_action(cx.listener(Self::palette_down))
            .on_action(cx.listener(Self::next_group))
            .on_action(cx.listener(Self::previous_group))
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
            .children(group_tabs)
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
                        [
                            Some(("↑ ↓", "Move")),
                            tab_hint,
                            Some(("↩", palette.verb)),
                            Some(("esc", "Close")),
                        ]
                        .into_iter()
                        .flatten()
                        .map(|(key, hint)| {
                            h_flex().gap(dp(4.)).child(keycap(key, &theme)).child(hint)
                        }),
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
    use super::{
        Choice, Found, Groups, ProjectTab, RowKey, Selection, StatusGroup, alias_score,
        detail_score, find_in, id_starts, rank_selected_project, score, shared_start,
    };

    const ID: &str = "3a79334c0b5e8f1d2a6b7c9e0f1a2b3c4d5e6f70";

    #[test]
    fn a_short_id_is_not_found_by_letters_scattered_through_it() {
        assert_eq!(
            detail_score("3a79334 · cache", &[ID.into()], "373"),
            None,
            "3, 7 and 3 appear in order in 3a79334, but that isn't an id typed"
        );
    }

    #[test]
    fn a_short_id_is_found_typed_from_its_start() {
        let detail = "3a79334 · cache";
        let found = detail_score(detail, &[ID.into()], "3a79").map(|(_, found)| found);
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
        let found = detail_score(detail, &[ID.into()], "checkout").map(|(_, found)| found);
        assert_eq!(
            found.map(|found| found
                .into_iter()
                .map(|range| &detail[range])
                .collect::<Vec<_>>()),
            Some(Vec::from(["checkout"])),
            "past the id, the detail matches as any other, its underline where it was found"
        );
    }

    const CHANGE_ID: &str = "vtlrvnlmsxrmrsnrkvqlwrzqokpsoxzk";

    #[test]
    fn a_shown_change_id_is_not_found_by_letters_scattered_through_it() {
        assert_eq!(
            detail_score("vtlrvnl · cache", &[CHANGE_ID.into(), ID.into()], "vrl"),
            None,
            "v, r and l appear in order in vtlrvnl, but that isn't an id typed"
        );
    }

    #[test]
    fn a_shown_change_id_is_found_typed_from_its_start() {
        let detail = "vtlrvnl · cache";
        let ids = [CHANGE_ID.into(), ID.into()];
        let found = detail_score(detail, &ids, "vtlr").map(|(_, found)| found);
        assert_eq!(
            found.map(|found| found
                .into_iter()
                .map(|range| &detail[range])
                .collect::<Vec<_>>()),
            Some(Vec::from(["vtlr"])),
            "the start of the id shown is underlined"
        );
    }

    #[test]
    fn a_commit_showing_its_change_id_is_still_found_by_its_sha() {
        let found = detail_score("vtlrvnl · cache", &[CHANGE_ID.into(), ID.into()], "3a79");
        assert_eq!(
            found,
            Some((2000, Vec::new())),
            "as `git log` gives it, with nothing to underline as the sha isn't shown"
        );
    }

    #[test]
    fn a_commit_showing_its_change_id_is_still_found_by_its_branch() {
        let detail = "vtlrvnl · cart-checkout";
        let ids = [CHANGE_ID.into(), ID.into()];
        let found = detail_score(detail, &ids, "checkout").map(|(_, found)| found);
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
    fn a_change_id_is_not_found_by_its_first_few_letters() {
        assert_eq!(
            detail_score("vtlrvnl · cache", &[CHANGE_ID.into(), ID.into()], "vtl"),
            None,
            "three letters are more likely the start of a word than of an id"
        );
    }

    #[test]
    fn a_sha_is_not_found_by_its_first_few_letters() {
        assert_eq!(
            detail_score("3a79334 · cache", &[ID.into()], "3a7"),
            None,
            "three letters are more likely the start of a word than of an id"
        );
    }

    #[test]
    fn a_few_letters_starting_an_id_still_find_the_branch() {
        let id = "ca79334c0b5e8f1d2a6b7c9e0f1a2b3c4d5e6f70";
        let found = detail_score("ca79334 · cache", &[id.into()], "ca")
            .and_then(|(_, found)| found.first().cloned());
        assert_eq!(
            found,
            Some(11..13),
            "the underline is on the branch, past the id they also start"
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
    fn a_command_is_found_by_a_word_of_its_alias() {
        assert!(
            alias_score(&["Remove commit", "Delete commit"], "delete").is_some(),
            "Discard commit is there for whoever calls it deleting"
        );
    }

    #[test]
    fn an_alias_is_not_found_by_letters_scattered_through_it() {
        assert_eq!(
            alias_score(&["Remove commit"], "rmc"),
            None,
            "r, m and c appear in order in Remove commit, but that isn't a word of it typed"
        );
    }

    #[test]
    fn nothing_typed_finds_no_alias() {
        assert_eq!(
            alias_score(&["Remove commit"], ""),
            None,
            "with nothing typed the label alone places it"
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

    fn labels_found(choices: &[Choice], query: &str) -> Vec<String> {
        (find_in(choices, query).into_iter())
            .filter_map(|found| Some(choices.get(found.ix)?.label.to_string()))
            .collect()
    }

    fn row(label: &'static str) -> Choice {
        Choice::new(label, |_, _, _| {})
    }

    fn project_row(label: &'static str) -> Choice {
        row(label)
            .goes_to(Selection::Project(label.into()))
            .pins(RowKey::Tab(label.into(), ProjectTab::Overview))
    }

    #[test]
    fn nothing_typed_lists_the_projects_then_the_selected_projects_rows_then_the_rest() {
        let (here, elsewhere) = (Selection::Project("here".into()), "elsewhere".into());
        let mut choices = [
            project_row("elsewhere"),
            project_row("here"),
            row("Graph elsewhere").goes_to(Selection::Project(elsewhere)),
            row("Graph here").goes_to(here.clone()),
        ];
        rank_selected_project(&mut choices, Some(&here));
        assert_eq!(
            labels_found(&choices, ""),
            ["here", "elsewhere", "Graph here", "Graph elsewhere"],
            "switching project stays a key away, and nothing is left out"
        );
    }

    #[test]
    fn nothing_typed_keeps_the_order_the_choices_came_in() {
        let choices = [
            row("New terminal"),
            row("here").in_selected_project(),
            row("elsewhere"),
        ];
        assert_eq!(
            labels_found(&choices, ""),
            ["New terminal", "here", "elsewhere"],
            "a picker's own first row stays first"
        );
    }

    #[test]
    fn an_equal_match_in_the_selected_project_comes_first() {
        let choices = [row("fix a"), row("fix b").in_selected_project()];
        assert_eq!(
            labels_found(&choices, "fix"),
            ["fix b", "fix a"],
            "both start with what's typed"
        );
    }

    #[test]
    fn a_later_match_in_the_selected_project_beats_an_earlier_one_of_its_kind() {
        let choices = [
            row("fix elsewhere"),
            row("quick fix here").in_selected_project(),
        ];
        assert_eq!(
            labels_found(&choices, "fix"),
            ["quick fix here", "fix elsewhere"],
            "both have it starting a word, so where in the label matters less than the project"
        );
    }

    #[test]
    fn a_name_typed_in_full_elsewhere_beats_a_longer_one_in_the_selected_project() {
        let choices = [row("min-size-docs").in_selected_project(), row("min-size")];
        assert_eq!(
            labels_found(&choices, "min-size"),
            ["min-size", "min-size-docs"],
            "Enter takes the first, so it must be the one named"
        );
    }

    #[test]
    fn a_word_elsewhere_beats_scattered_letters_in_the_selected_project() {
        let choices = [
            row("find it here").in_selected_project(),
            row("fit elsewhere"),
        ];
        assert_eq!(
            labels_found(&choices, "fit"),
            ["fit elsewhere", "find it here"],
            "a better kind of match stays better wherever it is"
        );
    }

    #[test]
    fn scattered_letters_in_the_selected_project_stay_under_a_match_inside_a_word() {
        let choices = [
            row("find it here").in_selected_project(),
            row("outfit elsewhere"),
        ];
        assert_eq!(
            labels_found(&choices, "fit"),
            ["outfit elsewhere", "find it here"],
            "the bonus stops short of the step between kinds of match"
        );
    }

    #[test]
    fn a_label_elsewhere_beats_a_detail_naming_the_selected_project() {
        let choices = [
            row("git-fixes").detail("other"),
            row("Refactor store")
                .detail("my-branch · gitbutler")
                .in_selected_project(),
        ];
        assert_eq!(
            labels_found(&choices, "git"),
            ["git-fixes", "Refactor store"],
            "every row there has the project's name in its detail"
        );
    }

    #[test]
    fn a_project_elsewhere_beats_rows_naming_the_selected_project_it_shares_a_prefix_with() {
        let choices = [
            row("Graph").detail("but-app").in_selected_project(),
            row("but-web"),
        ];
        assert_eq!(
            labels_found(&choices, "but"),
            ["but-web", "Graph"],
            "typing the start of a sibling's name finds the sibling"
        );
    }

    #[test]
    fn an_equal_detail_match_in_the_selected_project_comes_first() {
        let here = Selection::Project("here".into());
        let mut choices = [
            row("Docs pass")
                .detail("dp-branch-3 · other")
                .goes_to(Selection::Project("elsewhere".into())),
            row("Speed up picker")
                .detail("dp-branch-21 · gitbutler")
                .goes_to(here.clone()),
        ];
        rank_selected_project(&mut choices, Some(&here));
        assert_eq!(
            labels_found(&choices, "dp-branch"),
            ["Speed up picker", "Docs pass"],
            "a branch with a pull request is found by its name in its detail"
        );
    }

    #[test]
    fn a_risky_choice_in_the_selected_project_is_still_not_found_by_scattered_letters() {
        let mut risky = row("delete it").in_selected_project();
        risky.risky = true;
        assert!(
            labels_found(&[risky], "dlt").is_empty(),
            "the bonus must not read as a match of its words"
        );
    }

    #[test]
    fn only_rows_going_to_the_selected_project_are_marked() {
        let (here, elsewhere) = ("here".into(), "elsewhere".into());
        let selection = Selection::Project(here);
        let mut choices = [
            row("in it").goes_to(selection.clone()),
            row("in another").goes_to(Selection::Project(elsewhere)),
            row("going nowhere"),
        ];
        rank_selected_project(&mut choices, Some(&selection));
        let marked = choices.iter().filter(|choice| choice.in_selected_project);
        assert_eq!(
            marked.map(|c| c.label.as_ref()).collect::<Vec<_>>(),
            ["in it"],
            "its project decides, and a row going nowhere has none"
        );
    }

    fn found(ix: usize) -> Found {
        Found {
            ix,
            label: Vec::new(),
            detail: Vec::new(),
        }
    }

    fn groups_of(rows: &[Option<StatusGroup>], shown: Option<StatusGroup>) -> (Groups, Vec<usize>) {
        let mut groups = Groups {
            shown,
            ..Groups::default()
        };
        let rows = rows.iter().enumerate().map(|(ix, of)| (*of, found(ix)));
        let kept = groups.sort_out(rows.collect());
        (groups, kept.into_iter().map(|found| found.ix).collect())
    }

    #[test]
    fn no_group_shown_keeps_every_row_in_its_order() {
        let rows = [Some(StatusGroup::Idle), None, Some(StatusGroup::Blocked)];
        let (groups, kept) = groups_of(&rows, None);
        assert_eq!(kept, [0, 1, 2], "All lists what the query finds");
        assert_eq!(groups.total, 3, "a row with no status is still a row");
    }

    #[test]
    fn a_group_shown_keeps_only_its_rows() {
        let rows = [
            Some(StatusGroup::Working),
            Some(StatusGroup::Blocked),
            Some(StatusGroup::Working),
        ];
        let (groups, kept) = groups_of(&rows, Some(StatusGroup::Working));
        assert_eq!(kept, [0, 2], "the others wait for their own stop");
        assert_eq!(groups.total, 3, "All still counts every row");
    }

    #[test]
    fn groups_are_counted_most_wanted_first_leaving_out_the_empty() {
        let rows = [
            Some(StatusGroup::Idle),
            Some(StatusGroup::Blocked),
            Some(StatusGroup::Idle),
        ];
        let (groups, _) = groups_of(&rows, None);
        assert_eq!(
            groups.counts,
            [(StatusGroup::Blocked, 1), (StatusGroup::Idle, 2)],
            "there's nothing to go to in the rest"
        );
    }

    #[test]
    fn the_group_shown_stays_listed_when_the_query_finds_none_of_it() {
        let (groups, kept) = groups_of(&[Some(StatusGroup::Idle)], Some(StatusGroup::Done));
        assert!(kept.is_empty(), "nothing matches here");
        assert_eq!(
            groups.counts,
            [(StatusGroup::Done, 0), (StatusGroup::Idle, 1)],
            "it says why the list is empty, and Tab goes on from it"
        );
    }

    #[test]
    fn tab_stays_on_every_row_with_no_group_to_go_to() {
        let (groups, _) = groups_of(&[None], None);
        assert_eq!(groups.step(true), None, "All is the only stop");
    }

    #[test]
    fn tab_goes_from_every_row_to_the_first_group() {
        let rows = [Some(StatusGroup::Done), Some(StatusGroup::Idle)];
        let (groups, _) = groups_of(&rows, None);
        assert_eq!(
            groups.step(true),
            Some(StatusGroup::Done),
            "All comes first"
        );
    }

    #[test]
    fn tab_passes_over_groups_with_nothing_in_them() {
        let rows = [Some(StatusGroup::Blocked), Some(StatusGroup::Idle)];
        let (groups, _) = groups_of(&rows, Some(StatusGroup::Blocked));
        assert_eq!(
            groups.step(true),
            Some(StatusGroup::Idle),
            "nothing failed, is done or is working"
        );
    }

    #[test]
    fn tab_from_the_last_group_goes_back_to_every_row() {
        let rows = [Some(StatusGroup::Blocked), Some(StatusGroup::Idle)];
        let (groups, _) = groups_of(&rows, Some(StatusGroup::Idle));
        assert_eq!(groups.step(true), None, "it goes round");
    }

    #[test]
    fn shift_tab_from_every_row_goes_to_the_last_group() {
        let rows = [Some(StatusGroup::Blocked), Some(StatusGroup::Idle)];
        let (groups, _) = groups_of(&rows, None);
        assert_eq!(groups.step(false), Some(StatusGroup::Idle), "it goes round");
    }

    #[test]
    fn nothing_is_marked_with_nothing_selected() {
        let selection = Selection::Project("here".into());
        let mut choices = [row("in it").goes_to(selection)];
        rank_selected_project(&mut choices, None);
        assert!(
            !choices.iter().any(|choice| choice.in_selected_project),
            "there's no project to prefer"
        );
    }
}
