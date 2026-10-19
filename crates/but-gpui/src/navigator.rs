//! The left column: every project as a card, each listing its uncommitted changes,
//! stacks, branches and commits as one tree of rows.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, DragMoveEvent, ElementId, Entity, FocusHandle,
    Focusable, Hsla, KeyBinding, ListAlignment, ListOffset, ListState, Modifiers, MouseButton,
    MouseDownEvent, Pixels, ScrollWheelEvent, SharedString, Stateful, Subscription, WeakEntity,
    Window, actions, deferred, div, list, prelude::*, px,
};

use crate::app::{AddProject, AppView, Collapse, Confirm, Expand, SelectNext, SelectPrevious};
use crate::backend::{
    Branch, CommitId, CommitState, FileChange, PushState, Review, ReviewState, Workspace, Worktree,
    short_ref,
};
use crate::detail::{ChecksChanged, Detail};
use crate::overlays::MenuItem;
use crate::state::{
    Cut, NewAt, Pane, Prefs, ProjectEntry, ProjectId, Selection, Store, StoreEvent,
};
use crate::theme::Theme;
use crate::transfer::{Place, Transfer, drag_preview};
use crate::ui::illustration::Illustration;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, button, commit_title, dot, empty_state,
    faint, h_flex, icon, icon_button, just_now, list_row, plural, relative_time, resize_handle,
    review, section_label, shortcut, small_icon, v_flex,
};

mod account;
mod menu;

/// How far a worktree's rows sit in from its heading.
const NEST: f32 = 14.;

/// How far a stack's branches sit in from its caption: one of ui-react's SidebarRow steps
/// and the gap after it, so their chevrons line up under the caption's glyph.
const STACK_STEP: f32 = 22.;

/// Commits shown per branch before folding the rest into "N more".
const FOLD_AFTER: usize = 8;

/// The eye of a project the app watches, or struck through for one it doesn't.
fn watch_glyph(watched: bool) -> gpui::Svg {
    let eye = match watched {
        true => Icon::Eye,
        false => Icon::EyeOff,
    };
    gpui::svg().path(eye.path()).size(px(14.)).flex_none()
}

/// How long a worktree with nothing uncommitted stays listed after it last moved.
const INACTIVE_AFTER: Duration = Duration::from_secs(12 * 60 * 60);

/// How recently a project was last active for "Active in the last 7 days" to keep it.
const ACTIVE_WITHIN: Duration = Duration::from_secs(7 * 24 * 60 * 60);

actions!(
    navigator,
    [
        SelectFirst,
        SelectLast,
        PreviousSection,
        NextSection,
        PickPrevious,
        PickNext
    ]
);

/// Jumps through the tree: to either end, and between branches and projects. ⇧ with an arrow
/// picks commits, as ⇧-click does.
pub fn bind_keys(cx: &mut App) {
    let nav = Some("Navigator && !TextInput");
    cx.bind_keys([
        KeyBinding::new("g g", SelectFirst, nav),
        KeyBinding::new("home", SelectFirst, nav),
        KeyBinding::new("shift-g", SelectLast, nav),
        KeyBinding::new("end", SelectLast, nav),
        KeyBinding::new("alt-up", PreviousSection, nav),
        KeyBinding::new("alt-down", NextSection, nav),
        KeyBinding::new("shift-up", PickPrevious, nav),
        KeyBinding::new("shift-down", PickNext, nav),
    ]);
}

/// Rows in display order. Only [`Row::Item`] can be selected.
#[derive(PartialEq)]
enum Row {
    /// The space between two cards.
    Gap,
    /// A project, its uncommitted changes, a branch or a commit.
    Item(Selection),
    Note(SharedString, bool),
    /// Over the projects not expanded: what it calls them, how many it lists, and how many
    /// there are, filters aside.
    ProjectsHeading(&'static str, usize, usize),
    /// Projects past the fold, and how many.
    MoreProjects(usize),
    /// The end of the projects unfolded, which folds them again.
    FewerProjects,
    /// A branch's commits folded away, and how many.
    More(ProjectId, String, usize),
    /// The end of a branch's commits unfolded, which folds them again.
    Fewer(ProjectId, String),
    /// A commit an operation is still making, by its title.
    /// A commit a mutation is making, by its title, and whether it's been long enough to
    /// show it's still on its way.
    Pending(SharedString, bool),
    /// Where a commit of the project being dragged lands in a new branch of its own.
    NewBranch(ProjectId),
    /// After a stack, while one of the project's branches is dragged: where it lands below
    /// the stack's last branch, by its ref.
    LaneEnd(ProjectId, String),
    /// After a project's worktrees: how many are inactive, which it shows or hides.
    Inactive(ProjectId, usize),
    /// The line after a worktree's section.
    WorktreeDivider,
}

/// The rows as they're built, each with how far in it sits: a worktree's section and a
/// stack's branches step in from what holds them. Each also has the guides it draws, as
/// but.dev's sidebar does: a line down from under the chevron of each row it's under.
#[derive(Default)]
struct Rows {
    rows: Vec<Row>,
    insets: Vec<f32>,
    guides: Vec<Vec<f32>>,
    /// What the rows pushed next sit in by.
    inset: f32,
    /// Where the guides of the rows pushed next run.
    open: Vec<f32>,
}

/// Where a chevron's middle sits in a row set in by nothing: the row's margin and padding,
/// the indent of a row with a chevron, and half the chevron's step.
const GUIDE_AT: f32 = 3. + 8. + 6. + 7.;

impl Rows {
    fn push(&mut self, row: Row) {
        self.rows.push(row);
        self.insets.push(self.inset);
        self.guides.push(self.open.clone());
    }

    /// Set the rows pushed until [`Self::close`] under the row just pushed, a guide running
    /// down beside them from under its chevron.
    fn open(&mut self) {
        let under = self.insets.last().copied().unwrap_or_default();
        self.open.push(GUIDE_AT + under);
    }

    fn close(&mut self) {
        self.open.pop();
    }
}

impl Extend<Row> for Rows {
    fn extend<I: IntoIterator<Item = Row>>(&mut self, rows: I) {
        rows.into_iter().for_each(|row| self.push(row));
    }
}

/// How an item row draws: `solid` is the selection while the navigator has focus, `cut` is
/// the row waiting for ⌘V, and `inset` is how far in its section or stack sets it.
#[derive(Clone, Copy)]
struct RowStyle<'a> {
    theme: &'a Theme,
    selected: bool,
    solid: bool,
    cut: bool,
    inset: f32,
    /// The rows above and below are picked with it: see [`crate::ui::joined`].
    joined: (bool, bool),
}

impl RowStyle<'_> {
    fn row(&self, id: String, indent: f32) -> Stateful<Div> {
        let theme = self.theme;
        list_row(ElementId::Name(id.into()), self.selected, theme)
            .mx(px(3.))
            .pl(px(8. + indent + self.inset))
            .when(self.solid, |el| {
                el.bg(theme.fill_gray).text_color(theme.text_inverted)
            })
            .when(self.cut, |el| {
                el.opacity(0.5)
                    .border_1()
                    .border_dashed()
                    .border_color(theme.pop)
            })
            .map(|el| crate::ui::joined(el, self.joined, self.solid, theme))
    }

    fn muted(&self) -> Hsla {
        if self.solid {
            self.theme.text_inverted.opacity(0.7)
        } else {
            self.theme.text_2
        }
    }

    /// Meaningful colours wash out on the dark selection ground; keep them legible.
    fn meta(&self, color: Hsla) -> Hsla {
        if self.solid {
            self.theme.text_inverted
        } else {
            color
        }
    }
}

pub struct Navigator {
    app: WeakEntity<AppView>,
    store: Entity<Store>,
    detail: Entity<Detail>,
    focus: FocusHandle,
    /// The rows as last drawn, which the list draws as they scroll into view.
    rows: Vec<Row>,
    /// How far in each row sits.
    insets: Vec<f32>,
    /// Where each row's guides run.
    guides: Vec<Vec<f32>>,
    list: ListState,
    /// The selection changed: scroll to it once the rows are up to date.
    reveal_selection: bool,
    /// Branches showing all their commits instead of the first few.
    unfolded_branches: HashSet<(ProjectId, String)>,
    /// Branches showing none of their commits, unless one is selected.
    closed_branches: HashSet<(ProjectId, String)>,
    /// Projects showing their inactive worktrees too.
    shown_inactive: HashSet<ProjectId>,
    /// Worktrees showing only their heading, the main one by `None`.
    folded_worktrees: HashSet<(ProjectId, Option<Arc<str>>)>,
    /// Stacks showing only their caption, by their bottom branch.
    folded_stacks: HashSet<(ProjectId, String)>,
    /// The filter over the projects not expanded, while it's open.
    projects_filter: Option<(Entity<TextInput>, Subscription)>,
    /// The projects not expanded show all of them rather than the first few.
    projects_unfolded: bool,
    /// Whether the pointer is over the navigator.
    pointer_over: bool,
    /// The project whose watch button the pointer is on, which shows what a click makes it.
    watch_hovered: Option<ProjectId>,
    /// The row under the pointer holds still through the next change to the rows: one a
    /// workspace's load made, as an agent's commit, or "Show fewer" folding what's above it,
    /// which would leave the list scrolled past the fold. What else the user folds or does
    /// moves the rows as usual.
    hold_still: bool,
    _subscriptions: Vec<Subscription>,
}

impl Navigator {
    pub fn new(
        app: WeakEntity<AppView>,
        store: Entity<Store>,
        detail: Entity<Detail>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.subscribe(&store, |this, _, event, _| match event {
                StoreEvent::SelectionChanged => this.reveal_selection = true,
                StoreEvent::Loaded(_) => this.hold_still = true,
                _ => {}
            }),
            // Dragging the uncommitted changes carries the checked ones.
            cx.subscribe(&detail, |_, _, _: &ChecksChanged, cx| cx.notify()),
        ];
        Self {
            app,
            store,
            detail,
            focus: cx.focus_handle(),
            rows: Vec::new(),
            insets: Vec::new(),
            guides: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(56.)),
            reveal_selection: false,
            unfolded_branches: HashSet::new(),
            closed_branches: HashSet::new(),
            shown_inactive: HashSet::new(),
            folded_worktrees: HashSet::new(),
            folded_stacks: HashSet::new(),
            projects_filter: None,
            projects_unfolded: false,
            pointer_over: false,
            watch_hovered: None,
            hold_still: false,
            _subscriptions: subscriptions,
        }
    }

    /// Rows of every card, with a gap between two cards. Expanded projects get a card
    /// each; the collapsed ones share one compact card at the end.
    fn build_rows(&self, cx: &App) -> Rows {
        let store = self.store.read(cx);
        let mut rows = Rows::default();
        for entry in store.projects.iter().filter(|entry| entry.expanded) {
            rows.inset = 0.;
            if !rows.rows.is_empty() {
                rows.push(Row::Gap);
            }
            let id = &entry.id;
            rows.push(Row::Item(Selection::Project(id.clone())));
            let Some(workspace) = &entry.workspace else {
                rows.push(match &entry.error {
                    Some(err) => Row::Note(err.clone(), true),
                    None => Row::Note("Loading…".into(), false),
                });
                continue;
            };
            if let Some(err) = &entry.error {
                rows.push(Row::Note(err.clone(), true));
            }
            rows.push(Row::Item(Selection::Uncommitted(id.clone())));
            // Nothing but the edit goes on while a commit is checked out, its files listed
            // where the uncommitted ones are.
            if workspace.editing.is_some() {
                continue;
            }
            // The main worktree's heading folds its branches away; with linked worktrees each is
            // a section too, and the branches step in under their headings. Alone, they don't:
            // the heading lines up with them, and they with the project.
            let headed = !workspace.worktrees.is_empty();
            let selection = store.selection.as_ref().filter(|s| s.project() == id);
            if headed {
                rows.open();
                rows.inset = NEST;
            }
            let main_selected = selection.is_some_and(|s| within(s, workspace.branches()));
            if self.worktree_open(id, None, main_selected) {
                // A stack being unapplied leaves at once.
                let stacks = (workspace.stacks.iter()).filter(|stack| {
                    !(stack.branches.iter()).any(|b| store.hides_branch(id, &b.ref_name))
                });
                for stack in stacks {
                    self.lane_rows(store, id, &stack.branches, &mut rows);
                    if let Some(dragged @ Cut::Branch(..)) = &store.dragging
                        && let Some(last) = stack.branches.last()
                    {
                        let below = Selection::Branch(id.clone(), last.ref_name.clone());
                        if Transfer::resolve(dragged, &below, Place::Below, store).is_some() {
                            rows.push(Row::LaneEnd(id.clone(), last.ref_name.clone()));
                        }
                    }
                }
                if workspace.stacks.is_empty() {
                    // Off the workspace, as after a plain `git checkout`, the branches are
                    // still there, just not applied.
                    let note = match (workspace.managed, &workspace.checked_out) {
                        (true, _) => "No branches yet".into(),
                        (false, Some(branch)) => format!("Working on {branch} directly").into(),
                        (false, None) => "Working outside the workspace".into(),
                    };
                    rows.push(Row::Note(note, false));
                }
            }
            if headed {
                rows.close();
                rows.inset = 0.;
                rows.push(Row::WorktreeDivider);
                let on_branches =
                    |wt: &Worktree| selection.is_some_and(|s| within(s, wt.branches.iter()));
                let heads = |wt: &Worktree| matches!(selection, Some(Selection::Worktree(_, name)) if *name == wt.name);
                // The inactive ones wait behind a row of their own, unless the selection's in one.
                let (inactive, active): (Vec<_>, Vec<_>) = (workspace.worktrees.iter())
                    .filter(|wt| !store.hides_worktree(id, &wt.name))
                    .partition(|wt| is_inactive(wt) && !heads(wt) && !on_branches(wt));
                for wt in active {
                    self.worktree_rows(store, id, wt, on_branches(wt), &mut rows);
                }
                if !inactive.is_empty() {
                    rows.push(Row::Inactive(id.clone(), inactive.len()));
                    if self.shown_inactive.contains(id) {
                        for wt in inactive {
                            self.worktree_rows(store, id, wt, false, &mut rows);
                        }
                    }
                }
            }
            // After the worktrees, so a drag starting in one doesn't push its rows down. Outside
            // the workspace there's nowhere to put a branch of its own.
            if workspace.managed
                && matches!(&store.dragging, Some(Cut::Commit(project, _)) if project == id)
            {
                rows.push(Row::NewBranch(id.clone()));
            }
            // The pages are another kind of row than the worktrees', so a line rules them off,
            // as the last worktree's does when there are linked ones.
            if !matches!(rows.rows.last(), Some(Row::WorktreeDivider)) {
                rows.push(Row::WorktreeDivider);
            }
            rows.push(Row::Item(Selection::Branches(id.clone())));
            rows.push(Row::Item(Selection::History(id.clone())));
        }
        // The rest share one card under a heading that also adds projects, so it's there
        // when every project is expanded too.
        rows.inset = 0.;
        let title = match rows.rows.is_empty() {
            true => "All projects",
            false => "More projects",
        };
        if !rows.rows.is_empty() {
            rows.push(Row::Gap);
        }
        let others: Vec<&ProjectEntry> = (store.projects.iter())
            .filter(|entry| !entry.expanded)
            .collect();
        let prefs = &store.prefs;
        let terms = self.filter_terms(cx);
        let selected = |entry: &ProjectEntry| {
            store.selection.as_ref() == Some(&Selection::Project(entry.id.clone()))
        };
        let mut listed: Vec<(&ProjectEntry, Option<i64>)> = (others.iter())
            .map(|entry| (*entry, last_active(entry)))
            .filter(|(entry, at)| {
                !prefs.active_projects_only || at.is_some_and(active_lately) || selected(entry)
            })
            .filter(|(entry, _)| {
                let name = entry.project.name.to_lowercase();
                terms.iter().all(|term| name.contains(term.as_str()))
            })
            .collect();
        // Stable, so projects active at the same moment stay by name.
        if prefs.projects_by_activity {
            listed.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
        }
        rows.push(Row::ProjectsHeading(title, listed.len(), others.len()));
        // A filter finds a project even in a folded list.
        if prefs.projects_folded && terms.is_empty() {
            return rows;
        }
        // Past the fold, but never past the selected project.
        let fold_at = FOLD_AFTER
            .max((listed.iter().position(|(entry, _)| selected(entry))).map_or(0, |at| at + 1));
        let unfolded = self.projects_unfolded || !terms.is_empty();
        let shown = match unfolded {
            true => listed.len(),
            false => fold_at,
        };
        rows.extend(
            (listed.iter().take(shown))
                .map(|(entry, _)| Row::Item(Selection::Project(entry.id.clone()))),
        );
        if listed.len() > shown {
            rows.push(Row::MoreProjects(listed.len() - shown));
        } else if self.projects_unfolded && terms.is_empty() && listed.len() > fold_at {
            rows.push(Row::FewerProjects);
        }
        // A filter that matches nothing says what it looked for.
        if listed.is_empty() && !others.is_empty() {
            let miss = match self.projects_filter.as_ref() {
                Some((input, _)) if !terms.is_empty() => {
                    format!("No projects match \"{}\"", input.read(cx).text().trim())
                }
                _ => "None active in the last 7 days".to_owned(),
            };
            rows.push(Row::Note(miss.into(), false));
        }
        rows
    }

    /// A stack's or a linked worktree's branches: over two or more a caption, which folds
    /// them away unless the selection's in them, and they step in under it; then each branch
    /// with its commits, up to the fold, unless it's closed.
    fn lane_rows(&self, store: &Store, id: &ProjectId, branches: &[Branch], rows: &mut Rows) {
        let base = rows.inset;
        let stacked = branches.len() > 1;
        if let [_, .., bottom] = branches {
            let key = (id.clone(), bottom.ref_name.clone());
            let selected = (store.selection.as_ref())
                .is_some_and(|s| s.project() == id && branches.iter().any(|b| s.on(b)));
            rows.push(Row::Item(Selection::Stack(key.0.clone(), key.1.clone())));
            if self.folded_stacks.contains(&key) && !selected {
                return;
            }
            rows.open();
            rows.inset = base + STACK_STEP;
        }
        for branch in branches {
            let key = (id.clone(), branch.ref_name.clone());
            // Past the fold, but never past the selected commit, as after moving it down a step.
            let selected_at = match &store.selection {
                Some(Selection::Commit(p, c)) if p == id => {
                    branch.commits.iter().position(|commit| commit.id == *c)
                }
                _ => None,
            };
            let unfolded = self.unfolded_branches.contains(&key);
            let fold_at = FOLD_AFTER.max(selected_at.map_or(0, |at| at + 1));
            let shown = match unfolded {
                true => branch.commits.len(),
                false => fold_at,
            };
            let closed = self.closed_branches.contains(&key) && selected_at.is_none();
            rows.push(Row::Item(Selection::Branch(key.0, key.1)));
            if closed {
                continue;
            }
            rows.open();
            let pending = |at: &dyn Fn(&NewAt) -> bool| {
                (store.new_commits(id, at).into_iter())
                    .map(|(title, slow)| Row::Pending(title, slow))
            };
            rows.extend(pending(
                &|at| matches!(at, NewAt::Top(ref_name) if *ref_name == branch.ref_name),
            ));
            // Commits a pending operation takes away go, unless selected and going nowhere
            // else, as a moved one shows twice otherwise; ones it makes show where they go.
            let commits: Vec<_> = (branch.commits.iter().enumerate())
                .filter(|&(at, commit)| {
                    !store.hides_commit(id, &commit.key)
                        || (selected_at == Some(at) && !store.moves_commit(id, &commit.key))
                })
                .collect();
            let count = commits.len();
            rows.extend((commits.into_iter()).take(shown).flat_map(|(_, commit)| {
                let item = Row::Item(Selection::Commit(id.clone(), commit.id));
                (pending(&|at| matches!(at, NewAt::Above(key) if *key == commit.key)))
                    .chain([item])
                    .chain(pending(
                        &|at| matches!(at, NewAt::Below(key) if *key == commit.key),
                    ))
            }));
            if count > shown {
                rows.push(Row::More(
                    id.clone(),
                    branch.ref_name.clone(),
                    count - shown,
                ));
            } else if unfolded && count > fold_at {
                rows.push(Row::Fewer(id.clone(), branch.ref_name.clone()));
            }
            rows.close();
        }
        if stacked {
            rows.close();
        }
        rows.inset = base;
    }

    /// A linked worktree's heading, its uncommitted changes, and, unless it's folded without
    /// the selection in them, its branches; then a line ruling it off from what follows.
    fn worktree_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        worktree: &Worktree,
        selected: bool,
        rows: &mut Rows,
    ) {
        let name = worktree.name.clone();
        rows.push(Row::Item(Selection::Worktree(id.clone(), name.clone())));
        if self.worktree_open(id, Some(&name), selected) {
            rows.open();
            rows.inset = NEST;
            self.lane_rows(store, id, &worktree.branches, rows);
            rows.inset = 0.;
            rows.close();
        }
        rows.push(Row::WorktreeDivider);
    }

    /// Whether worktree `name` of `project`, the main one for `None`, shows what's in it: it
    /// isn't folded, or `selected` is in it.
    fn worktree_open(&self, project: &ProjectId, name: Option<&Arc<str>>, selected: bool) -> bool {
        selected
            || !self
                .folded_worktrees
                .contains(&(project.clone(), name.cloned()))
    }

    /// The words the projects filter looks for, each in a project's name.
    fn filter_terms(&self, cx: &App) -> Vec<String> {
        (self.projects_filter.iter())
            .flat_map(|(input, _)| {
                input
                    .read(cx)
                    .text()
                    .split_whitespace()
                    .map(str::to_lowercase)
            })
            .collect()
    }

    /// Open the filter over the projects not expanded, or go back to it.
    fn open_projects_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = match &self.projects_filter {
            Some((input, _)) => input.clone(),
            None => {
                let input = cx.new(|cx| TextInput::new("Filter projects", false, cx));
                let listen = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
                    match event {
                        InputEvent::Changed => cx.notify(),
                        InputEvent::Cancel => this.close_projects_filter(window, cx),
                        // Enter goes to the first project it found.
                        InputEvent::Submit => {
                            let first = (this.rows.iter())
                                .skip_while(|row| !matches!(row, Row::ProjectsHeading(..)))
                                .find_map(|row| match row {
                                    Row::Item(item @ Selection::Project(_)) => Some(item.clone()),
                                    _ => None,
                                });
                            this.close_projects_filter(window, cx);
                            if let Some(first) = first {
                                this.select(first, window, cx);
                            }
                        }
                    }
                });
                self.projects_filter = Some((input.clone(), listen));
                input
            }
        };
        window.focus(&input.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_projects_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.projects_filter = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.select(Some(selection), cx));
        window.focus(&self.focus, cx);
    }

    /// The rows that can be selected, in order.
    fn items(&self) -> Vec<&Selection> {
        (self.rows.iter())
            .filter_map(|row| match row {
                Row::Item(item) => Some(item),
                _ => None,
            })
            .collect()
    }

    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.items();
        if items.is_empty() {
            return;
        }
        let current = self.store.read(cx).selection.as_ref();
        let next = match current.and_then(|s| items.iter().position(|item| *item == s)) {
            Some(ix) => (ix as isize + delta).clamp(0, items.len() as isize - 1) as usize,
            None => 0,
        };
        let next = items[next].clone();
        self.select(next, window, cx);
    }

    /// Step `delta` commits from the selected one, picking the run as ⇧-click does. Off a
    /// commit, it's a plain step.
    fn pick_step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let (project, id) = match &store.selection {
            Some(Selection::Commit(project, id)) => (project.clone(), *id),
            _ => return self.step(delta, window, cx),
        };
        let order = self.commit_order(&project);
        let next = (order.iter().position(|commit| *commit == id))
            .and_then(|at| order.get(at.checked_add_signed(delta)?).copied());
        if let Some(next) = next {
            (self.store).update(cx, |store, cx| store.pick(&project, next, Some(order), cx));
        }
    }

    /// The commits of `project` that can be picked together, as shown: not those folded
    /// away.
    fn commit_order(&self, project: &ProjectId) -> Vec<CommitId> {
        (self.rows.iter())
            .filter_map(|row| match row {
                Row::Item(Selection::Commit(p, commit)) if p == project => Some(*commit),
                _ => None,
            })
            .collect()
    }

    /// Select the first item before (`forward` false) or after the selection that starts a
    /// section: a project or a branch.
    fn jump_section(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.items();
        let current = self.store.read(cx).selection.as_ref();
        let at = current.and_then(|s| items.iter().position(|item| *item == s));
        let heads =
            |item: &&&Selection| matches!(item, Selection::Project(_) | Selection::Branch(..));
        let next = match (at, forward) {
            (Some(at), true) => items[at + 1..].iter().find(heads),
            (Some(at), false) => items[..at].iter().rev().find(heads),
            (None, _) => items.first(),
        };
        if let Some(next) = next.map(|item| (*item).clone()) {
            self.select(next, window, cx);
        }
    }

    fn expand(&mut self, _: &Expand, _: &mut Window, cx: &mut Context<Self>) {
        match self.store.read(cx).selection.clone() {
            Some(Selection::Project(id)) => {
                self.store
                    .update(cx, |store, cx| store.set_expanded(&id, true, cx));
            }
            Some(Selection::Branch(project, ref_name)) => {
                self.closed_branches.remove(&(project, ref_name));
                cx.notify();
            }
            Some(Selection::Uncommitted(project)) => {
                self.folded_worktrees.remove(&(project, None));
                cx.notify();
            }
            Some(Selection::Worktree(project, name)) => {
                self.folded_worktrees.remove(&(project, Some(name)));
                cx.notify();
            }
            Some(Selection::Stack(project, bottom)) => {
                self.folded_stacks.remove(&(project, bottom));
                cx.notify();
            }
            _ => {}
        }
    }

    /// Step out the way a tree does: a commit to its branch, an open branch, stack or worktree
    /// closed, a branch up to its stack or worktree, and anything else up to its project.
    fn collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let selection = store.selection.clone();
        let closes = matches!(&selection, Some(item @ Selection::Branch(project, ref_name))
            if store.branch_for(item).is_some_and(|branch| !branch.commits.is_empty())
                && !self.closed_branches.contains(&(project.clone(), ref_name.clone())));
        let folds = (self.rows.iter())
            .position(|row| matches!(row, Row::Item(item) if Some(item) == selection.as_ref()))
            .is_some_and(|ix| self.section_open(ix));
        match selection {
            Some(Selection::Project(id)) => {
                self.store
                    .update(cx, |store, cx| store.set_expanded(&id, false, cx));
            }
            Some(Selection::Branch(project, ref_name)) if closes => {
                self.closed_branches.insert((project, ref_name));
                cx.notify();
            }
            Some(Selection::Commit(project, id)) => {
                let branch = store
                    .find_commit(&project, id)
                    .map(|(b, _)| b.ref_name.clone());
                if let Some(ref_name) = branch {
                    self.select(Selection::Branch(project, ref_name), window, cx);
                }
            }
            Some(Selection::Uncommitted(project)) if folds => {
                self.toggle_section(&project, None, true, window, cx)
            }
            Some(Selection::Worktree(project, name)) if folds => {
                self.toggle_section(&project, Some(name), true, window, cx)
            }
            Some(Selection::Stack(project, bottom)) if folds => {
                self.toggle_stack(&project, &bottom, true, window, cx)
            }
            // Jump up, like closing a folder in a tree.
            Some(selection) => {
                let up = parent(store, &selection)
                    .unwrap_or_else(|| Selection::Project(selection.project().clone()));
                self.select(up, window, cx)
            }
            None => {}
        }
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        match self.store.read(cx).selection.clone() {
            Some(Selection::Project(id)) => {
                let expanded = self.store.read(cx).project(&id).is_some_and(|p| p.expanded);
                self.store
                    .update(cx, |store, cx| store.set_expanded(&id, !expanded, cx));
            }
            Some(Selection::Uncommitted(_) | Selection::Worktree(..)) => {
                self.detail
                    .update(cx, |detail, cx| detail.focus_message(window, cx));
            }
            _ => {}
        }
    }

    /// Bring the list up to date with `rows`, and show the selection if it just changed.
    /// Rows that didn't change keep their place on screen. When a load changed them while
    /// the pointer is over the list, so does the row under it: rows added above it, like an
    /// agent's new commit, push the rows above it up rather than it down.
    fn sync_list(&mut self, rows: Vec<Row>, window: &Window, cx: &App) {
        // At the very top the list stays there, showing what's added, as the diff does;
        // scrolled, the row under the pointer stays where it is.
        let top = self.list.logical_scroll_top();
        let at_top = top.item_ix == 0 && top.offset_in_item == px(0.);
        let hovered = (std::mem::take(&mut self.hold_still) && !at_top)
            .then(|| self.hovered_row(window))
            .flatten();
        // The hovered row's index before and after, and how far below the top it is.
        let anchor = hovered.and_then(|(old_ix, offset)| {
            let new_ix = (0..rows.len())
                .filter(|&ix| same_place(&self.rows[old_ix], &rows[ix]))
                .min_by_key(|ix| ix.abs_diff(old_ix))?;
            Some((old_ix, new_ix, offset))
        });
        let changed = match anchor {
            // Below the hovered row first, so the indexes above it still hold.
            Some((old_ix, new_ix, _)) => {
                let below = splice(&self.list, &self.rows[old_ix..], &rows[new_ix..], old_ix);
                splice(&self.list, &self.rows[..old_ix], &rows[..new_ix], 0) || below
            }
            None => splice(&self.list, &self.rows, &rows, 0),
        };
        // Rows added above the first one count as added above the top, which would leave
        // the list scrolled to their end.
        if at_top && changed {
            self.list.scroll_to(ListOffset::default());
        }
        self.rows = rows;
        if changed {
            // Rows not drawn yet count as one row tall. The clone shares the list's state.
            self.list.clone().with_uniform_item_height(px(ROW_HEIGHT));
            if let Some((_, item_ix, offset)) = anchor {
                self.list.scroll_to(ListOffset {
                    item_ix,
                    offset_in_item: px(0.),
                });
                self.list.scroll_by(-offset);
            }
        }
        if std::mem::take(&mut self.reveal_selection)
            && let Some(selection) = &self.store.read(cx).selection
            && let Some(ix) = self
                .rows
                .iter()
                .position(|row| matches!(row, Row::Item(item) if item == selection))
        {
            self.list.scroll_to_reveal_item(ix);
        }
    }

    /// The row under the pointer, if it's over the list, and how far below the list's top.
    fn hovered_row(&self, window: &Window) -> Option<(usize, Pixels)> {
        if !self.pointer_over {
            return None;
        }
        let pointer = window.mouse_position();
        let top = self.list.viewport_bounds().top();
        (self.list.logical_scroll_top().item_ix..self.rows.len())
            .map_while(|ix| Some((ix, self.list.bounds_for_item(ix)?)))
            .find(|(_, bounds)| bounds.contains(&pointer))
            .map(|(ix, bounds)| (ix, bounds.top() - top))
    }

    /// The row at `ix` of the list, drawn inside its card.
    fn render_row(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let inset = self.insets.get(ix).copied().unwrap_or_default();
        let el = match self.rows.get(ix) {
            None | Some(Row::Gap) => return div().h(px(10.)).into_any_element(),
            Some(Row::Item(item)) => self.render_item(ix, item, inset, window, theme, cx),
            // A dashed line, as ui-react's WorktreeDivider.
            Some(Row::WorktreeDivider) => h_flex()
                .h(px(12.))
                .px(px(12.))
                .child(
                    div()
                        .flex_1()
                        .border_t_1()
                        .border_dashed()
                        .border_color(theme.border_3),
                )
                .into_any_element(),
            Some(Row::Note(text, is_error)) => h_flex()
                .min_h(px(ROW_HEIGHT))
                .px(px(12.))
                .py(px(4.))
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(if *is_error {
                    theme.danger
                } else {
                    theme.text_2
                })
                .when(*is_error, |el| {
                    el.child(small_icon(Icon::Warning).text_color(theme.danger))
                })
                .child(div().flex_1().min_w(px(0.)).child(text.clone()))
                .into_any_element(),
            // As made at once; dimmed, with an ellipsis, only once it's taking a while.
            Some(Row::Pending(title, slow)) => h_flex()
                .min_h(px(ROW_HEIGHT))
                .mx(px(3.))
                .pl(px(38.))
                .pr(px(12.))
                .gap(px(10.))
                .when(*slow, |el| el.opacity(0.5))
                .child(small_icon(Icon::Commit).text_color(theme.blue))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .when(title.is_empty(), |el| el.italic())
                        .child(commit_title(title)),
                )
                .when(*slow, |el| {
                    el.child(div().text_size(px(11.)).text_color(theme.text_2).child("…"))
                })
                .into_any_element(),
            Some(Row::ProjectsHeading(title, listed, total)) => {
                self.projects_heading(title, *listed, *total, theme, cx)
            }
            // The glyph under the projects' chevrons, the words under their names.
            Some(Row::MoreProjects(hidden)) => {
                fold_row(("more-projects", ix), Icon::Kebab, 11., 35., theme)
                    .child(format!("{hidden} more"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.projects_unfolded = true;
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Some(Row::FewerProjects) => {
                fold_row(("fewer-projects", ix), Icon::ChevronUp, 11., 35., theme)
                    .child("Show fewer")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.projects_unfolded = false;
                        this.hold_still = true;
                        cx.notify();
                    }))
                    .into_any_element()
            }
            // Both drop zones are padded, not margined: a margin collapses through the card's
            // ground.
            Some(Row::NewBranch(project)) => {
                let project = project.clone();
                let zone = drop_zone(("new-branch-drop", ix), "New branch", theme)
                    .h(px(30.))
                    .on_drag_move(cx.listener(Self::over_zone))
                    .on_drop(cx.listener(move |this, payload: &Cut, _, cx| {
                        // A row still lit says where the pill told the drop goes, as when the
                        // zone slid under a pointer that didn't move.
                        if let Cut::Commit(from, ids) = payload
                            && *from == project
                            && this.store.read(cx).drop_at.is_none()
                        {
                            let ids = ids.clone();
                            this.store
                                .update(cx, |store, cx| store.move_to_new_branch(from, ids, cx));
                        }
                    }));
                div().py(px(2.)).child(zone).into_any_element()
            }
            Some(Row::LaneEnd(project, ref_name)) => {
                let store = self.store.read(cx);
                let named = (store
                    .workspace(project)
                    .and_then(|ws| ws.find_branch(ref_name)))
                .map_or(short_ref(ref_name), |branch| {
                    store.label_of(project, branch)
                });
                let label = format!("Move below {named}");
                let (project, ref_name) = (project.clone(), ref_name.clone());
                let zone = drop_zone(("lane-end-drop", ix), label, theme)
                    .h(px(24.))
                    .on_drag_move(cx.listener(Self::over_zone))
                    .on_drop(cx.listener(move |this, payload: &Cut, window, cx| {
                        if this.store.read(cx).drop_at.is_some() {
                            return;
                        }
                        let below = Selection::Branch(project.clone(), ref_name.clone());
                        this.app
                            .update(cx, |app, cx| {
                                app.transfer(payload, &below, Place::Below, window, cx)
                            })
                            .ok();
                    }));
                div().py(px(2.)).child(zone).into_any_element()
            }
            Some(Row::More(project, ref_name, hidden)) => {
                let key = (project.clone(), ref_name.clone());
                // The glyph under the commits' glyphs, the words under their titles.
                fold_row(("more", ix), Icon::Kebab, 41., 63., theme)
                    .child(format!("{hidden} more"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.unfolded_branches.insert(key.clone());
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Some(Row::Fewer(project, ref_name)) => {
                let key = (project.clone(), ref_name.clone());
                fold_row(("fewer", ix), Icon::ChevronUp, 41., 63., theme)
                    .child("Show fewer")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.unfolded_branches.remove(&key);
                        this.hold_still = true;
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Some(Row::Inactive(project, n)) => {
                let (project, shown) = (project.clone(), self.shown_inactive.contains(project));
                let chevron = match shown {
                    true => Icon::ChevronDown,
                    false => Icon::ChevronRight,
                };
                // The chevron under the worktrees' chevrons, the words under their names.
                fold_row(("inactive", ix), chevron, 11., 55., theme)
                    .child(plural(*n, "inactive worktree"))
                    .tooltip(Tooltip::text(
                        "Nothing uncommitted, and no commits or checkouts in 12 hours",
                    ))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        toggle(&mut this.shown_inactive, project.clone());
                        cx.notify();
                    }))
                    .into_any_element()
            }
        };
        // Items step in by their style, keeping the highlight full width; the rest as a whole.
        let el = match inset > 0. && !matches!(self.rows[ix], Row::Item(_)) {
            true => div().pl(px(inset)).child(el).into_any_element(),
            false => el,
        };
        let first = ix == 0 || matches!(self.rows[ix - 1], Row::Gap);
        let last = self
            .rows
            .get(ix + 1)
            .is_none_or(|next| matches!(next, Row::Gap));
        // A guide for each row it's under, joining those of the rows above and below into
        // one line, as but.dev's sidebar draws them.
        let guides = (self.guides.get(ix).into_iter().flatten()).map(|at| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(*at))
                .w(px(1.))
                .bg(theme.text_2.opacity(0.2))
        });
        // Padded, not margined: a list lays its rows out from their left edge.
        div()
            .px(px(10.))
            .child(
                div()
                    .relative()
                    .bg(theme.bg_1)
                    .border_color(theme.border_3)
                    .border_l_1()
                    .border_r_1()
                    .when(first, |el| el.border_t_1().rounded_t(px(6.)).pt(px(3.)))
                    .when(last, |el| el.border_b_1().rounded_b(px(6.)).pb(px(3.)))
                    .child(el)
                    .children(guides),
            )
            .into_any_element()
    }

    fn render_item(
        &self,
        ix: usize,
        item: &Selection,
        inset: f32,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let lit = |item: &Selection| {
            store.selection.as_ref() == Some(item)
                || matches!(item, Selection::Commit(project, id) if store.is_picked(project, *id))
        };
        let selected = lit(item);
        // Picked commits next to each other join up.
        let lit_at = |ix: usize| matches!(self.rows.get(ix), Some(Row::Item(other)) if lit(other));
        let joined = match selected && !store.picked.is_empty() {
            true => (ix > 0 && lit_at(ix - 1), lit_at(ix + 1)),
            false => (false, false),
        };
        let cut = match (&store.cut, item) {
            (Some(Cut::Branch(p, r)), Selection::Branch(project, ref_name)) => {
                p == project && r == ref_name
            }
            (Some(Cut::Commit(p, ids)), Selection::Commit(project, commit)) => {
                p == project && ids.contains(commit)
            }
            (Some(Cut::CherryPick(p, c)), Selection::Commit(project, commit)) => {
                p == project && c == commit
            }
            _ => false,
        };
        let style = RowStyle {
            theme,
            selected,
            solid: selected && self.focus.contains_focused(window, cx),
            cut,
            inset,
            joined,
        };
        let el = match item {
            Selection::Project(id) => self.project_row(item, id, style, cx),
            Selection::Uncommitted(id)
                if store.workspace(id).is_some_and(|ws| ws.editing.is_some()) =>
            {
                Some(self.editing_row(item, id, style, cx))
            }
            Selection::Uncommitted(_) | Selection::Worktree(..) => {
                self.worktree_heading(ix, item, style, cx)
            }
            Selection::Branch(id, ref_name) => self.branch_row(item, id, ref_name, style, cx),
            Selection::Commit(id, commit_id) => self.commit_row(item, id, commit_id, style, cx),
            Selection::Branches(_) => {
                Some(self.page_row(item, Icon::Branch, "Branches", style, cx))
            }
            Selection::History(_) => Some(self.page_row(item, Icon::Clock, "History", style, cx)),
            Selection::Stack(id, bottom) => self.stack_row(ix, item, id, bottom, style, cx),
        };
        match el {
            Some(el) => self.drop_target(el, item.clone(), window, theme, cx),
            None => div().into_any_element(),
        }
    }

    /// Select `item` on press, as native lists do: a click waits for the release. ⌘ (ctrl off
    /// macOS) or ⇧ picks commits along with the selected one. A press on one of several
    /// picked commits leaves them picked, so dragging it takes them all; letting go without
    /// dragging selects just it (see [`Self::commit_row`]).
    fn select_on_press(
        &self,
        item: &Selection,
        cx: &Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        let item = item.clone();
        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
            let (toggle, shift) = (event.modifiers.secondary(), event.modifiers.shift);
            match &item {
                Selection::Commit(project, id) if toggle || shift => {
                    let range = shift.then(|| this.commit_order(project));
                    this.store
                        .update(cx, |store, cx| store.pick(project, *id, range, cx));
                    window.focus(&this.focus, cx);
                }
                Selection::Commit(project, id) if this.store.read(cx).among_picks(project, *id) => {
                    window.focus(&this.focus, cx)
                }
                _ => this.select(item.clone(), window, cx),
            }
        })
    }

    /// The heading over the projects not expanded: a fold, how many it lists of how many
    /// there are, and ways to find, order and add projects. While the filter's open, the
    /// filter in its place.
    fn projects_heading(
        &self,
        title: &'static str,
        listed: usize,
        total: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        // The chevron over the projects' chevrons.
        let row = h_flex()
            .h(px(ROW_HEIGHT))
            .pl(px(12.))
            .pr(px(4.))
            .gap(px(2.));
        let count = match listed == total {
            true => total.to_string(),
            false => format!("{listed} of {total}"),
        };
        if let Some((input, _)) = &self.projects_filter {
            return row
                .gap(px(6.))
                .child(small_icon(Icon::Search).text_color(theme.text_3))
                .child(div().flex_1().min_w(px(0.)).child(input.clone()))
                .child(faint(count, theme).text_size(px(12.)).flex_none())
                .child(
                    icon_button("close-projects-filter", Icon::Cross)
                        .small()
                        .tooltip("Close the filter")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_projects_filter(window, cx)
                        })),
                )
                .into_any_element();
        }
        let folded = self.store.read(cx).prefs.projects_folded;
        let fold = h_flex()
            .id("projects-fold")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .gap(px(6.))
            .when(total > 0, |el| {
                el.cursor_pointer()
                    .child(
                        small_icon(match folded {
                            true => Icon::ChevronRight,
                            false => Icon::ChevronDown,
                        })
                        .text_color(theme.text_3),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        (this.store).update(cx, |store, cx| {
                            store.set_prefs(
                                |prefs| prefs.projects_folded = !prefs.projects_folded,
                                cx,
                            )
                        })
                    }))
            })
            .child(section_label(title, theme).truncate())
            .when(total > 0, |el| {
                el.child(faint(count, theme).text_size(px(12.)))
            });
        row.child(fold)
            .when(total > 0, |el| {
                el.child(
                    icon_button("filter-projects", Icon::Search)
                        .small()
                        .tooltip("Filter projects")
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.open_projects_filter(window, cx)
                            }),
                        ),
                )
            })
            .child(
                icon_button("add-project", Icon::Plus)
                    .small()
                    .tooltip_with_key("Add project", shortcut(&AddProject, cx))
                    .dispatch(&AddProject),
            )
            .when(total > 0, |el| {
                el.child(
                    icon_button("project-view-options", Icon::Kebab)
                        .small()
                        .tooltip("Sort and show")
                        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                            let position = event.position();
                            let items = this.view_options(cx);
                            (this
                                .app
                                .update(cx, |app, cx| app.open_menu(position, items, cx)))
                            .ok();
                        })),
                )
            })
            .into_any_element()
    }

    /// How the projects not expanded are listed, with headed choices as ui-react's view
    /// options menu has them: their order, and which to show.
    fn view_options(&self, cx: &App) -> Vec<MenuItem> {
        fn set(
            update: fn(&mut Prefs),
        ) -> impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) {
            move |app, _, cx| {
                app.store
                    .update(cx, |store, cx| store.set_prefs(update, cx))
            }
        }
        let prefs = &self.store.read(cx).prefs;
        let (by_activity, active_only) = (prefs.projects_by_activity, prefs.active_projects_only);
        vec![
            MenuItem::new("Last activity", set(|p| p.projects_by_activity = true))
                .under("Sort by")
                .ticked(by_activity),
            MenuItem::new("Name", set(|p| p.projects_by_activity = false)).ticked(!by_activity),
            MenuItem::new("All", set(|p| p.active_projects_only = false))
                .under("Show")
                .ticked(!active_only),
            MenuItem::new(
                "Active in the last 7 days",
                set(|p| p.active_projects_only = true),
            )
            .ticked(active_only),
        ]
    }

    fn project_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let entry = store.project(id)?;
        let workspace = entry.workspace.as_ref();
        let (toggle_id, chevron_id) = (id.clone(), id.clone());
        let expanded = entry.expanded;
        let hovered = self.watch_hovered.as_ref() == Some(id);
        // Folded, the row sums up the whole card; open, it leaves out what the rows under it
        // say: their uncommitted files, unpushed commits and worktrees.
        let summary = workspace.filter(|ws| ws.editing.is_none()).map(|ws| {
            let folded = |n: usize| if expanded { 0 } else { n };
            let in_worktrees = ws.worktrees.iter().map(|w| w.uncommitted.len());
            (
                folded(ws.uncommitted.len() + in_worktrees.sum::<usize>()),
                folded(ws.unpushed_commits()),
                ws.target.as_ref(),
                folded(ws.worktrees.len()),
            )
        });
        let unread =
            workspace.is_some_and(|ws| ws.commits().any(|commit| store.is_unread(id, commit)));
        // An open project's from what it loaded; a closed one's from what Git last recorded,
        // as nothing's loaded or watched for it.
        let activity = match expanded {
            true => workspace.and_then(|ws| ws.last_activity()),
            false => last_active(entry),
        };
        let needs_update =
            (workspace.and_then(|ws| ws.target.as_ref())).is_some_and(|t| t.needs_update);
        Some(
            style
                .row(format!("project-{id}"), 0.)
                .aria_label(entry.project.name.clone())
                .aria_expanded(expanded)
                .h(px(if expanded { 32. } else { ROW_HEIGHT }))
                .pl(px(11.))
                // The eye over the chevrons of the rows below and the emoji, 5 points on,
                // over their icons, so the name sits over theirs.
                .child(
                    h_flex()
                        .flex_none()
                        .gap(px(5.))
                        .child(
                            // Whether the app watches the project, as an open one is loaded and
                            // kept current, and under the pointer what a click makes it.
                            div()
                                .id(ElementId::Name(format!("watch-{id}").into()))
                                .flex_none()
                                .size(px(20.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .hover(|el| el.bg(theme.hover()))
                                .child(watch_glyph(expanded != hovered).text_color(
                                    match (hovered, style.solid) {
                                        (false, _) => style.muted(),
                                        (true, true) => theme.text_inverted,
                                        (true, false) => theme.text_1,
                                    },
                                ))
                                .on_hover(cx.listener({
                                    let id = id.clone();
                                    move |this, on: &bool, _, cx| {
                                        this.watch_hovered = on.then(|| id.clone());
                                        cx.notify()
                                    }
                                }))
                                .tooltip(Tooltip::text(match expanded {
                                    true => "Stop watching",
                                    false => "Watch this project",
                                }))
                                // Watching it or not doesn't select it.
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    // The row moves to the other card, from under the pointer.
                                    this.watch_hovered = None;
                                    this.store.update(cx, |store, cx| {
                                        store.set_expanded(&chevron_id, !expanded, cx)
                                    });
                                })),
                        )
                        .children(store.project_icon(id).map(|emoji| {
                            // In an icon's 16 points, a little smaller than the text.
                            div()
                                .flex_none()
                                .w(px(16.))
                                .flex()
                                .justify_center()
                                .text_size(px(13.))
                                .child(emoji.to_owned())
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .when(expanded, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                        .child(entry.project.name.clone()),
                )
                .when(unread, |el| el.child(dot(style.meta(theme.pop))))
                .when_some(summary, |el, (files, unpushed, target, worktrees)| {
                    let (warn, blue) = (style.meta(theme.warn), style.meta(theme.blue));
                    el.children(meta_count(
                        Icon::File,
                        files,
                        warn,
                        plural(files, "uncommitted file"),
                    ))
                    .children(meta_count(
                        Icon::ArrowUp,
                        unpushed,
                        blue,
                        plural(unpushed, "unpushed commit"),
                    ))
                    .children(target.and_then(|t| {
                        let tip = format!("{} behind {}", plural(t.behind, "commit"), t.name);
                        meta_count(Icon::ArrowDown, t.behind, warn, tip)
                    }))
                    .children(meta_count(
                        Icon::Folder,
                        worktrees,
                        style.muted(),
                        plural(worktrees, "worktree"),
                    ))
                })
                .when(entry.is_loading(), |el| {
                    el.child(small_icon(Icon::Clock).text_color(theme.text_3))
                })
                // A project something wrote to in the last two minutes is likely an
                // agent at work; otherwise say how long it's been quiet.
                .when_some(activity, |el, at| {
                    if just_now(at) {
                        el.child(
                            h_flex()
                                .id(ElementId::Name(format!("active-{id}").into()))
                                .gap(px(4.))
                                .text_size(px(11.))
                                .text_color(style.meta(theme.safe))
                                .child(dot(style.meta(theme.safe)))
                                .child("active")
                                .tooltip(Tooltip::text("Changed in the last 2 minutes")),
                        )
                    } else {
                        let age = relative_time(at);
                        el.child(
                            div()
                                .id(ElementId::Name(format!("quiet-{id}").into()))
                                .text_size(px(11.))
                                .text_color(style.muted())
                                .tooltip(Tooltip::text(format!("Last active {age} ago")))
                                .child(age),
                        )
                    }
                })
                .when(expanded && needs_update, |el| {
                    let project = id.clone();
                    let updating = store.updating(id);
                    // Pressing it leaves the selection where it is, as pressing the row wouldn't.
                    el.child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                button(
                                    ElementId::Name(format!("update-{id}").into()),
                                    if updating { "Updating…" } else { "Update" },
                                )
                                .small()
                                .style(ButtonStyle::Gray)
                                .inverted(style.solid)
                                .disabled_because(updating.then_some("Updating already"))
                                .tooltip("Rebase your branches onto the target's new commits")
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        let project = project.clone();
                                        this.app
                                            .update(cx, |app, cx| {
                                                app.update_from_target(project, cx)
                                            })
                                            .ok();
                                    },
                                )),
                            ),
                    )
                })
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() == 2 {
                        this.store.update(cx, |store, cx| {
                            store.set_expanded(&toggle_id, !expanded, cx)
                        });
                    }
                }))
                .into_any_element(),
        )
    }

    /// A quiet row at the end of a project that opens one of its pages.
    fn page_row(
        &self,
        item: &Selection,
        glyph: Icon,
        label: &'static str,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        style
            .row(format!("{label}-{}", item.project()), 6.)
            .text_color(style.muted())
            .child(icon(glyph).text_color(style.muted()))
            .child(div().flex_1().child(label))
            .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
            .into_any_element()
    }

    /// The commit being edited, whose files are listed where the uncommitted ones are, and
    /// go nowhere else until it's finished.
    fn editing_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let files = store.workspace(id).map_or(&[][..], |ws| &ws.uncommitted);
        style
            .row(format!("uncommitted-{id}"), 6.)
            .aria_label("Editing a commit")
            .child(icon(Icon::Diff).text_color(style.meta(style.theme.warn)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child("Editing a commit"),
            )
            .children(change_counts(files, style))
            .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
            .into_any_element()
    }

    /// A worktree's heading, which selects its uncommitted changes: the main one's for
    /// [`Selection::Uncommitted`], a linked one's for [`Selection::Worktree`]. With linked
    /// worktrees, a chevron or a double-click folds its branches; folded, it also counts their
    /// unpushed commits. Without, the main one's branches sit beside it, and still fold.
    fn worktree_heading(
        &self,
        ix: usize,
        item: &Selection,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let id = item.project();
        let (entry, workspace) = (store.project(id)?, store.workspace(id)?);
        let name = match item {
            Selection::Worktree(_, name) => Some(name.clone()),
            _ => None,
        };
        let worktree = (name.as_ref()).and_then(|name| workspace.find_worktree(name));
        let (label, path, uncommitted) = match worktree {
            Some(wt) => (wt.name.to_string(), &wt.path, &wt.uncommitted),
            None => (
                "Main worktree".to_owned(),
                &entry.project.path,
                &workspace.uncommitted,
            ),
        };
        let branches = section_branches(workspace, name.as_ref());
        let unpushed_n = unpushed(branches.iter().copied());
        let unread = (branches.iter().flat_map(|branch| &branch.commits))
            .any(|commit| store.is_unread(id, commit));
        // Open even when folded while the selection's in it.
        let open = self.section_open(ix);
        // Written to in the last two minutes, likely by the agent working in it.
        let active = worktree.is_some_and(|wt| wt.last_activity().is_some_and(just_now));
        let step_color = match style.solid {
            true => style.muted(),
            false => theme.text_3,
        };
        // As a branch's: a chevron, or a dot with no branches to fold.
        // As wide as a branch's, so the two line up.
        let step = div()
            .id(("worktree-fold", ix))
            .flex_none()
            .w(px(14.))
            .flex()
            .justify_center();
        let step = match branches.is_empty() {
            true => step.child(small_icon(Icon::BulletDisc).text_color(step_color)),
            false => {
                let (project, name) = (id.clone(), name.clone());
                step.child(
                    small_icon(match open {
                        true => Icon::ChevronDown,
                        false => Icon::ChevronRight,
                    })
                    .text_color(step_color),
                )
                // Folding the worktree doesn't select it.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    this.toggle_section(&project, name.clone(), open, window, cx);
                }))
            }
        };
        // In as far as a branch is, so its chevron and icon line up with theirs.
        let row = style
            .row(
                format!("worktree-{id}-{}", name.as_deref().unwrap_or("")),
                6.,
            )
            .aria_label(label.clone())
            .aria_expanded(open)
            .child(step)
            .child(
                icon(match name {
                    Some(_) => Icon::FolderCopy,
                    None => Icon::Folder,
                })
                .text_color(style.muted()),
            )
            .child(
                div()
                    .id(("worktree-name", ix))
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .tooltip(Tooltip::text(path.display().to_string()))
                    .child(label),
            )
            .children(change_counts(uncommitted, style))
            .when(!open, |el| {
                el.children(meta_count(
                    Icon::ArrowUp,
                    unpushed_n,
                    style.meta(theme.blue),
                    plural(unpushed_n, "unpushed commit"),
                ))
                .when(unread, |el| el.child(dot(style.meta(theme.pop))))
            })
            .when(active, |el| {
                el.child(
                    div()
                        .id(("worktree-active", ix))
                        .child(dot(style.meta(theme.safe)))
                        .tooltip(Tooltip::text("Changed in the last 2 minutes")),
                )
            })
            .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
            .when(!branches.is_empty(), |el| {
                let (project, name) = (id.clone(), name.clone());
                el.on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        this.toggle_section(&project, name.clone(), open, window, cx);
                    }
                }))
            });
        // The worktree's checked files while it's shown, else all of them.
        let drags = !uncommitted.is_empty();
        Some(
            row.when(drags, |el| {
                let files = self.detail.read(cx).files_to_move(id, name.clone(), cx);
                let label: SharedString = match files.len() {
                    1 => files[0].file.path.clone().into(),
                    n => format!("{n} files").into(),
                };
                let payload = Cut::Files {
                    project: id.clone(),
                    from: None,
                    worktree: name.clone(),
                    files: files.into(),
                };
                el.on_drag(payload, drag_preview(self.store.clone(), label))
            })
            .into_any_element(),
        )
    }

    /// Fold worktree `name`'s branches, the main one's for `None`, or unfold them. Folding
    /// them over the selection takes it up to the worktree's heading, as a tree does.
    fn toggle_section(
        &mut self,
        project: &ProjectId,
        name: Option<Arc<str>>,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = (project.clone(), name);
        if !open {
            self.folded_worktrees.remove(&key);
            return cx.notify();
        }
        let store = self.store.read(cx);
        let inside = (store.selection.as_ref())
            .zip(store.workspace(project))
            .is_some_and(|(s, ws)| {
                s.project() == project
                    && within(s, section_branches(ws, key.1.as_ref()).into_iter())
            });
        let heading = match &key.1 {
            Some(name) => Selection::Worktree(project.clone(), name.clone()),
            None => Selection::Uncommitted(project.clone()),
        };
        self.folded_worktrees.insert(key);
        match inside {
            true => self.select(heading, window, cx),
            false => cx.notify(),
        }
    }

    /// Whether row `ix` heads rows stepped in under it that show: an open worktree or stack.
    /// The main worktree's branches, alone, aren't stepped in: its heading is open while
    /// they, or the note that there are none, follow it.
    fn section_open(&self, ix: usize) -> bool {
        let next = self.insets.get(ix + 1);
        match self.rows.get(ix) {
            Some(Row::Item(Selection::Uncommitted(_))) if next == self.insets.get(ix) => matches!(
                self.rows.get(ix + 1),
                Some(Row::Item(Selection::Stack(..) | Selection::Branch(..)) | Row::Note(..))
            ),
            _ => next.is_some_and(|next| *next > self.insets[ix]),
        }
    }

    /// Over a stack of two or more branches, as ui-react's StackCaption: a chevron, its
    /// glyph and how many. Selecting it shows what its branches change together; its chevron,
    /// or a double-click, folds them away, and folded, it says how many commits they have to
    /// push and when they last changed.
    fn stack_row(
        &self,
        ix: usize,
        item: &Selection,
        id: &ProjectId,
        bottom: &str,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let branches = store.workspace(id)?.lane(bottom)?;
        // Open even when folded while the selection's in it.
        let open = self.section_open(ix);
        let names: Vec<&str> = (branches.iter())
            .map(|branch| store.label_of(id, branch))
            .collect();
        let unpushed_n = unpushed(branches.iter());
        let latest = (branches.iter().flat_map(|branch| &branch.commits))
            .map(|commit| commit.time)
            .max();
        let label = format!("Stack of {}", branches.len());
        let step_color = match style.solid {
            true => style.muted(),
            false => theme.text_3,
        };
        let fold = (id.clone(), bottom.to_owned());
        let step = div()
            .id(("stack-fold", ix))
            .flex_none()
            .w(px(14.))
            .h(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.))
            .hover(|el| el.bg(theme.hover()))
            .child(
                small_icon(match open {
                    true => Icon::ChevronDown,
                    false => Icon::ChevronRight,
                })
                .text_color(step_color),
            )
            // Folding the stack doesn't select it.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                cx.stop_propagation();
                this.toggle_stack(&fold.0, &fold.1, open, window, cx);
            }));
        let fold = (id.clone(), bottom.to_owned());
        Some(
            style
                .row(format!("stack-{id}-{bottom}"), 6.)
                .aria_label(label.clone())
                .aria_expanded(open)
                .child(step)
                .child(icon(Icon::Stack).text_color(style.muted()))
                .child(
                    h_flex()
                        .id(("stack-name", ix))
                        .flex_1()
                        .min_w(px(0.))
                        .gap(px(6.))
                        .tooltip(Tooltip::text(format!("Stack: {}", names.join(" ← "))))
                        .child(div().flex_none().text_color(style.muted()).child(label))
                        // Folded, its top branch tells it from another stack as tall, named
                        // as the branch rows name it; open, that row is right under it.
                        .children(branches.first().filter(|_| !open).map(|top| {
                            branch_name(store.name_of(id, top), top.review.as_ref(), &style)
                        })),
                )
                .when(!open, |el| {
                    el.children(meta_count(
                        Icon::ArrowUp,
                        unpushed_n,
                        style.meta(theme.blue),
                        plural(unpushed_n, "unpushed commit"),
                    ))
                    .children(latest.map(|at| {
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(style.muted())
                            .child(relative_time(at))
                    }))
                })
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        this.toggle_stack(&fold.0, &fold.1, open, window, cx);
                    }
                }))
                .into_any_element(),
        )
    }

    /// Fold the stack over `bottom` away, or unfold it. Folding it over the selection takes
    /// the selection up to the stack.
    fn toggle_stack(
        &mut self,
        project: &ProjectId,
        bottom: &str,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = (project.clone(), bottom.to_owned());
        if !open {
            self.folded_stacks.remove(&key);
            return cx.notify();
        }
        let store = self.store.read(cx);
        let inside = (store.selection.as_ref())
            .zip(store.workspace(project).and_then(|ws| ws.lane(bottom)))
            .is_some_and(|(s, lane)| s.project() == project && lane.iter().any(|b| s.on(b)));
        self.folded_stacks.insert(key);
        match inside {
            true => self.select(
                Selection::Stack(project.clone(), bottom.to_owned()),
                window,
                cx,
            ),
            false => cx.notify(),
        }
    }

    fn branch_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        ref_name: &str,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let branch = store.branch_for(item)?;
        let name = store.name_of(id, branch);
        let unpushed = branch
            .commits
            .iter()
            .filter(|c| c.state == CommitState::Local)
            .count();
        let conflicted = branch.commits.iter().filter(|c| c.conflicted).count();
        let (glyph, glyph_color) = match (&branch.review, branch.push) {
            (Some(review), _) => review::state_glyph(review.state, theme),
            (None, PushState::Integrated) => (Icon::Tick, theme.safe),
            (None, _) => (Icon::Branch, style.muted()),
        };
        // A pull request's title names the row (see `branch_name`); its branch's name, number
        // and how CI stands are in the tooltip, and on the branch's page.
        let tip = (branch.review.as_ref())
            .filter(|review| !review.title.is_empty())
            .map(|review| {
                let checks = (review.checks)
                    .map(|checks| format!(" · {}", review::checks_summary(checks, theme).2))
                    .unwrap_or_default();
                format!("{name} · #{}{checks}", review.number)
            });
        let key = (id.clone(), ref_name.to_owned());
        let closed = self.closed_branches.contains(&key) && !branch.commits.is_empty();
        // A selected commit keeps its branch open.
        let holds_selected = matches!(&store.selection, Some(Selection::Commit(p, c))
            if p == id && branch.commits.iter().any(|commit| commit.id == *c));
        let open = !closed || holds_selected;
        // As ui-react's SidebarRow: a chevron in a step of its own folds the commits away; a
        // branch without any has a dot there, keeping it in line with those that fold.
        let step_color = match style.solid {
            true => style.muted(),
            false => theme.text_3,
        };
        let step = div()
            .id(ElementId::Name(format!("fold-{id}-{ref_name}").into()))
            .flex_none()
            .w(px(14.))
            .h(px(16.))
            .flex()
            .items_center()
            .justify_center();
        let step = match branch.commits.is_empty() {
            true => step.child(small_icon(Icon::BulletDisc).text_color(step_color)),
            false => {
                let fold_key = key.clone();
                step.rounded(px(3.))
                    .hover(|el| el.bg(theme.hover()))
                    .child(
                        small_icon(match open {
                            true => Icon::ChevronDown,
                            false => Icon::ChevronRight,
                        })
                        .text_color(step_color),
                    )
                    // Folding the branch doesn't select it.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        if !open {
                            this.closed_branches.remove(&fold_key);
                            return cx.notify();
                        }
                        this.closed_branches.insert(fold_key.clone());
                        // Folding it over its selected commit takes the selection up to it,
                        // as ← does.
                        match holds_selected {
                            true => {
                                let (project, ref_name) = fold_key.clone();
                                this.select(Selection::Branch(project, ref_name), window, cx)
                            }
                            false => cx.notify(),
                        }
                    }))
            }
        };
        Some(
            style
                .row(format!("branch-{id}-{ref_name}"), 6.)
                .aria_label(format!("Branch {name}"))
                .child(step)
                .child(icon(glyph).text_color(style.meta(glyph_color)))
                .child(
                    branch_name(name, branch.review.as_ref(), &style)
                        .id(ElementId::Name(format!("name-{id}-{ref_name}").into()))
                        .flex_1()
                        .when_some(tip, |el, tip| el.tooltip(Tooltip::text(tip))),
                )
                // As ui-react's empty MetaCount, the quietest meta, ahead of any counts.
                .when(branch.commits.is_empty(), |el| {
                    let quiet = match style.solid {
                        true => style.muted(),
                        false => theme.text_3,
                    };
                    el.child(
                        h_flex()
                            .id(ElementId::Name(format!("empty-{id}-{ref_name}").into()))
                            .flex_none()
                            .gap(px(3.))
                            .text_size(px(11.))
                            .text_color(quiet)
                            .child(small_icon(Icon::Commit).text_color(quiet).opacity(0.7))
                            .child("empty")
                            .tooltip(Tooltip::text("No commits yet")),
                    )
                })
                // CI only while it runs or after it failed: passing is the usual state.
                .when_some(branch.review.as_ref(), |el, review| {
                    let unsettled = review.checks.filter(|c| c.running > 0 || c.failed > 0);
                    el.when_some(unsettled, |el, checks| {
                        let (glyph, color, text) = review::checks_summary(checks, theme);
                        let tip = format!("{text}, click to check again");
                        let (store, project) = (self.store.clone(), id.clone());
                        el.child(
                            div()
                                .id(ElementId::Name(format!("checks-{ref_name}").into()))
                                .child(small_icon(glyph).text_color(style.meta(color)))
                                .tooltip(Tooltip::text(tip))
                                .on_click(move |_, _, cx| {
                                    store.update(cx, |store, cx| {
                                        store.refresh_forge(&project);
                                        store.snack("Checking CI again", cx)
                                    })
                                }),
                        )
                    })
                })
                // So conflicts show while the commits are hidden too.
                .children(meta_count(
                    Icon::Warning,
                    conflicted,
                    style.meta(theme.danger),
                    plural(conflicted, "conflicted commit"),
                ))
                .children(meta_count(
                    Icon::ArrowDown,
                    branch.incoming.len(),
                    style.meta(theme.warn),
                    format!(
                        "{} only on the remote",
                        plural(branch.incoming.len(), "commit")
                    ),
                ))
                .children(meta_count(
                    Icon::ArrowUp,
                    unpushed,
                    style.meta(theme.blue),
                    plural(unpushed, "unpushed commit"),
                ))
                .when(store.pushing(id, ref_name), |el| {
                    el.child(faint("pushing", theme))
                })
                // Folded, how many commits it holds; open, they're listed under it.
                .when(closed, |el| {
                    let n = branch.commits.len();
                    el.children(meta_count(
                        Icon::Commit,
                        n,
                        style.muted(),
                        plural(n, "commit"),
                    ))
                })
                .when(
                    branch
                        .commits
                        .iter()
                        .any(|commit| store.is_unread(id, commit)),
                    |el| el.child(dot(style.meta(theme.pop))),
                )
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                // Double-clicking closes the branch to its row or opens it again, as with a
                // folder.
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() == 2 {
                        toggle(&mut this.closed_branches, key.clone());
                        cx.notify();
                    }
                }))
                .when(!ref_name.is_empty(), |el| {
                    let payload = Cut::Branch(id.clone(), ref_name.to_owned());
                    let label = store.label_of(id, branch).to_owned();
                    el.on_drag(payload, drag_preview(self.store.clone(), label.into()))
                })
                .into_any_element(),
        )
    }

    fn commit_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        commit_id: &CommitId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let (_, commit) = store.find_commit(id, *commit_id)?;
        let (glyph, color) = if commit.conflicted {
            (Icon::Warning, theme.danger)
        } else {
            match commit.state {
                CommitState::Local => (Icon::Commit, theme.blue),
                CommitState::Pushed => (Icon::CommitFill, style.muted()),
                CommitState::Integrated => (Icon::Tick, theme.safe),
            }
        };
        // A picked commit takes the other picks along.
        let moved = store.with_picks(id, *commit_id);
        let (title, _) = store.message_of(id, commit);
        let label: SharedString = title.to_owned().into();
        let several = moved.len() > 1;
        let dragged: SharedString = match several {
            false => label.clone(),
            true => format!("{} commits", moved.len()).into(),
        };
        let payload = Cut::Commit(id.clone(), moved);
        let unread = store.is_unread(id, commit);
        Some(
            style
                // Under its branch's glyph, its title under the branch's name.
                .row(format!("commit-{id}-{}", commit_id.0), 30.)
                .gap(px(10.))
                .aria_label(label.clone())
                .child(small_icon(glyph).text_color(style.meta(color)))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .when(title.is_empty(), |el| el.italic().text_color(style.muted()))
                        .when(unread, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                        .child(commit_title(title)),
                )
                .when(unread, |el| el.child(dot(style.meta(theme.pop))))
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(style.muted())
                        .child(relative_time(commit.time)),
                )
                // Selected but on its way out.
                .when(store.hides_commit(id, &commit.key), |el| el.opacity(0.5))
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .when(several, |el| {
                    let item = item.clone();
                    el.on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        let held = event.modifiers();
                        if !held.secondary() && !held.shift {
                            this.select(item.clone(), window, cx)
                        }
                    }))
                })
                .on_drag(payload, drag_preview(self.store.clone(), dragged))
                .into_any_element(),
        )
    }

    /// A pointer moving over a drop zone drops there, so a row still lit lets go, even one a
    /// reload took away, which no row would clear.
    fn over_zone(&mut self, event: &DragMoveEvent<Cut>, _: &mut Window, cx: &mut Context<Self>) {
        if event.bounds.contains(&event.event.position) && self.store.read(cx).drop_at.is_some() {
            self.store.update(cx, |store, _| store.drop_at = None);
            cx.notify();
        }
    }

    /// Make a row take what's dragged or cut where it has a meaning for it, and offer its
    /// right-click menu. While dragging, the row under the pointer shows where letting go
    /// lands: a line above or below a commit, or an outline around what it goes into. While
    /// something is cut, the selected row shows where ⌘V puts it. Rows it can't go on step back.
    fn drop_target(
        &self,
        el: AnyElement,
        target: Selection,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let dragging = cx.has_active_drag();
        // What a drop or a paste would carry: the drag while there is one, else the cut.
        let pending = match dragging {
            true => store.dragging.as_ref(),
            false => store.cut.as_ref(),
        };
        let lands = |place: Place| {
            pending
                .is_some_and(|payload| Transfer::resolve(payload, &target, place, store).is_some())
        };
        let landing = match dragging {
            true => (store.drop_at.as_ref())
                .filter(|(at, _)| *at == target)
                .map(|(_, place)| *place),
            false => (pending.filter(|_| store.selection.as_ref() == Some(&target)))
                .map(|cut| Place::from_keys(window.modifiers(), cut, &target)),
        }
        .filter(|place| lands(*place))
        // A branch goes below another under its last commit, rows further down, so a line
        // under its heading would point at the wrong place: the heading is outlined, as the
        // pill says where.
        .map(|place| match target {
            Selection::Branch(..) => Place::Into,
            _ => place,
        });
        let steps_back = pending.is_some()
            && ![Place::Above, Place::Into, Place::Below]
                .into_iter()
                .any(lands);
        let highlight = theme.pop;
        let (store, move_target, can_drop_target, menu_target) = (
            self.store.clone(),
            target.clone(),
            target.clone(),
            target.clone(),
        );
        div()
            .relative()
            .rounded(px(4.))
            .child(el)
            .when(steps_back, |el| el.opacity(0.35))
            .map(|el| match landing {
                Some(Place::Into) => el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(highlight)
                        .bg(highlight.opacity(0.14)),
                ),
                // Painted last, as the next row's slice of the card would cover what of it
                // crosses into that row.
                Some(place) => el.child(deferred(insertion_line(place == Place::Above, highlight))),
                None => el,
            })
            // Every row hears every move of a drag: the one under the pointer says where on
            // it a drop lands, and the one it leaves lets go of it.
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<Cut>, window, cx| {
                    let (at, bounds) = (event.event.position, event.bounds);
                    let store = this.store.read(cx);
                    let next = if bounds.contains(&at) {
                        let payload = event.drag(cx);
                        let y = (at.y - bounds.top()) / bounds.size.height;
                        let place = place_at(payload, &move_target, y, window.modifiers());
                        (Transfer::resolve(payload, &move_target, place, store))
                            .map(|_| (move_target.clone(), place))
                    } else if store
                        .drop_at
                        .as_ref()
                        .is_some_and(|(t, _)| *t == move_target)
                    {
                        None
                    } else {
                        return;
                    };
                    if store.drop_at != next {
                        this.store.update(cx, |store, _| store.drop_at = next);
                    }
                }),
            )
            .can_drop(move |payload, _, cx| {
                let store = store.read(cx);
                (payload.downcast_ref::<Cut>()).is_some_and(|payload| {
                    (store.drop_at.as_ref()).is_some_and(|(at, place)| {
                        *at == can_drop_target
                            && Transfer::resolve(payload, at, *place, store).is_some()
                    })
                })
            })
            .on_drop(cx.listener(move |this, payload: &Cut, window, cx| {
                let Some((_, place)) = this.store.read(cx).drop_at.clone() else {
                    return;
                };
                (this.app)
                    .update(cx, |app, cx| {
                        app.transfer(payload, &target, place, window, cx)
                    })
                    .ok();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let items = this.menu_items(&menu_target, cx);
                    let picked = matches!(&menu_target,
                        Selection::Commit(project, id) if this.store.read(cx).is_picked(project, *id));
                    if !items.is_empty() {
                        if picked {
                            // The picks' actions are the navigator's, dispatched from focus.
                            window.focus(&this.focus, cx);
                        } else {
                            this.select(menu_target.clone(), window, cx);
                        }
                        this.app
                            .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                            .ok();
                    }
                }),
            )
            .into_any_element()
    }
}

/// Tell `list` its rows from `at` changed from `old` to `new`: the ones between what both
/// start and end with, which it measures again. Whether any did.
fn splice(list: &ListState, old: &[Row], new: &[Row], at: usize) -> bool {
    let head = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let (old, new) = (&old[head..], &new[head..]);
    let tail = (old.iter().rev().zip(new.iter().rev()))
        .take_while(|(a, b)| a == b)
        .count();
    let changed = old.len() > tail || new.len() > tail;
    if changed {
        list.splice(at + head..at + head + old.len() - tail, new.len() - tail);
    }
    changed
}

/// A row that unfolds a list, "N more", or folds it again, "Show fewer", as ui-react's
/// MoreItem: `glyph` at `icon_at` under the list's glyphs, the words at `text_at` under its
/// titles.
fn fold_row(
    id: impl Into<ElementId>,
    glyph: Icon,
    icon_at: f32,
    text_at: f32,
    theme: &Theme,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .h(px(24.))
        .pl(px(icon_at))
        .gap(px(text_at - icon_at - 12.))
        .text_size(px(12.))
        .text_color(theme.text_2)
        .hover(|el| el.text_color(theme.text_1))
        .child(small_icon(glyph).text_color(theme.text_3))
}

/// Whether `new` stands where `old` did, to keep it under the pointer: the same row, or the
/// other side of a fold, as "Show fewer" becomes "N more".
fn same_place(old: &Row, new: &Row) -> bool {
    match (old, new) {
        (Row::Fewer(project, ref_name), Row::More(p, r, _)) => project == p && ref_name == r,
        (Row::FewerProjects, Row::MoreProjects(_)) => true,
        _ => old == new,
    }
}

/// When `entry` was last active: what Git last recorded in it, or what its loaded workspace
/// shows since, as files an agent is writing.
fn last_active(entry: &ProjectEntry) -> Option<i64> {
    let loaded = entry.workspace.as_ref().and_then(|ws| ws.last_activity());
    entry.project.last_active.max(loaded)
}

fn active_lately(at: i64) -> bool {
    (UNIX_EPOCH + Duration::from_secs(at.max(0) as u64))
        .elapsed()
        .is_ok_and(|age| age <= ACTIVE_WITHIN)
}

/// What ← goes up to from `selection`, a branch or a stack: a branch in a stack to the stack,
/// and a stack, or a branch alone, to its worktree's heading, when its project has linked
/// worktrees.
fn parent(store: &Store, selection: &Selection) -> Option<Selection> {
    let project = selection.project().clone();
    let workspace = store.workspace(&project)?;
    let bottom = match selection {
        Selection::Branch(_, ref_name) => {
            let lane = (workspace.lanes())
                .find(|lane| lane.iter().any(|branch| branch.ref_name == *ref_name))?;
            match lane {
                [_, .., bottom] => {
                    return Some(Selection::Stack(project, bottom.ref_name.clone()));
                }
                _ => ref_name.clone(),
            }
        }
        Selection::Stack(_, bottom) => bottom.clone(),
        _ => return None,
    };
    if workspace.worktrees.is_empty() {
        return None;
    }
    Some(
        match store.worktree_of(&Selection::Branch(project.clone(), bottom)) {
            Some(worktree) => Selection::Worktree(project, worktree.name.clone()),
            None => Selection::Uncommitted(project),
        },
    )
}

/// The branches in worktree `name`'s section, the main one's for `None`.
fn section_branches<'a>(workspace: &'a Workspace, name: Option<&Arc<str>>) -> Vec<&'a Branch> {
    match name {
        None => workspace.branches().collect(),
        Some(name) => {
            (workspace.find_worktree(name)).map_or_else(Vec::new, |wt| wt.branches.iter().collect())
        }
    }
}

/// What a worktree's uncommitted `files` count: what a merge left conflicted, as an update
/// can, first, then the lines they add and remove, or with none counted, how many there are.
fn change_counts(files: &[FileChange], style: RowStyle) -> impl Iterator<Item = Stateful<Div>> {
    let (theme, n) = (style.theme, files.len());
    let conflicted = files.iter().filter(|f| f.conflicted).count();
    let conflicts = meta_count(
        Icon::Warning,
        conflicted,
        style.meta(theme.danger),
        plural(conflicted, "conflicted file"),
    );
    // A conflicted file's lines are its markers, which the file list doesn't count either.
    let (added, removed) = (files.iter().filter(|f| !f.conflicted))
        .filter_map(|f| f.lines)
        .fold((0, 0), |(a, r), (fa, fr)| (a + fa, r + fr));
    let all = match added + removed {
        0 => meta_count(
            Icon::File,
            n,
            style.meta(theme.warn),
            plural(n, "uncommitted file"),
        ),
        _ => {
            let tip = format!(
                "{}: {} added, {removed} removed",
                plural(n, "uncommitted file"),
                plural(added as usize, "line"),
            );
            Some(
                div()
                    .id(ElementId::Name(tip.clone().into()))
                    .child(crate::ui::diff::stats(added, removed, theme))
                    .tooltip(Tooltip::text(tip)),
            )
        }
    };
    conflicts.into_iter().chain(all)
}

/// Whether `selection` is one of `branches`, a commit on one, or a stack of them.
fn within<'a>(selection: &Selection, mut branches: impl Iterator<Item = &'a Branch>) -> bool {
    branches.any(|branch| {
        selection.on(branch)
            || matches!(selection, Selection::Stack(_, bottom) if *bottom == branch.ref_name)
    })
}

/// How many of `branches`' commits are only local.
fn unpushed<'a>(branches: impl Iterator<Item = &'a Branch>) -> usize {
    (branches.flat_map(|branch| &branch.commits))
        .filter(|commit| commit.state == CommitState::Local)
        .count()
}

/// A worktree with nothing uncommitted, so not active either, that hasn't moved in
/// [`INACTIVE_AFTER`]. One with no reflog to date it stays listed.
fn is_inactive(worktree: &Worktree) -> bool {
    worktree.uncommitted.is_empty()
        && worktree.updated_at.is_some_and(|at| {
            (UNIX_EPOCH + Duration::from_secs(at.max(0) as u64))
                .elapsed()
                .is_ok_and(|age| age > INACTIVE_AFTER)
        })
}

/// A branch as the navigator names it: its pull request's title once the forge cache has one,
/// toned by the request's state, or else `name` in blue mono. Drawn from what the last load
/// read, so no row waits on the network, and offline the cached title stays.
fn branch_name(name: &str, review: Option<&Review>, style: &RowStyle) -> Div {
    let theme = style.theme;
    let el = div().min_w(px(0.)).truncate();
    match review.filter(|review| !review.title.is_empty()) {
        Some(review) => el
            .text_color(match review.state {
                ReviewState::Open => style.meta(theme.text_1),
                ReviewState::Draft => style.muted(),
                ReviewState::Merged | ReviewState::Closed => match style.solid {
                    true => style.muted(),
                    false => theme.text_3,
                },
            })
            .child(review.title.clone()),
        None => el
            .font_family(theme.mono_font.clone())
            .text_size(px(12.))
            .text_color(style.meta(theme.blue))
            .child(name.to_owned()),
    }
}

/// A small count in a row's meta, coloured by meaning (amber waits on you, blue is
/// unpushed), saying on hover what it counts; nothing at zero.
fn meta_count(glyph: Icon, n: usize, color: Hsla, tip: String) -> Option<Stateful<Div>> {
    let tip = SharedString::from(tip);
    (n > 0).then(|| {
        h_flex()
            .id(ElementId::Name(tip.clone()))
            .flex_none()
            .gap(px(3.))
            .text_size(px(11.))
            .text_color(color)
            .child(small_icon(glyph).text_color(color))
            .child(n.to_string())
            .tooltip(Tooltip::text(tip))
    })
}

/// Where on `target` a drop of `payload` at `y`, from 0 at its top to 1 at its bottom,
/// lands. A commit dragged onto another goes above it in the top quarter, below it in the
/// bottom one, and into it between; a copy of one, which can't go into another, splits it in
/// halves. Anything else goes into its target, or with ⇧ a branch below another.
fn place_at(payload: &Cut, target: &Selection, y: f32, held: Modifiers) -> Place {
    match (payload, target) {
        (Cut::Commit(..), Selection::Commit(..)) if y < 0.25 => Place::Above,
        (Cut::Commit(..), Selection::Commit(..)) if y > 0.75 => Place::Below,
        (Cut::CherryPick(..), Selection::Commit(..)) if y < 0.5 => Place::Above,
        (Cut::CherryPick(..), Selection::Commit(..)) => Place::Below,
        _ if held.shift && !matches!(target, Selection::Commit(..)) => Place::Below,
        _ => Place::Into,
    }
}

/// The line a drop lands on, along the row's top edge or its bottom one, so the rows either
/// side of a gap draw it in the same place.
fn insertion_line(above: bool, color: Hsla) -> Div {
    let line = div()
        .absolute()
        .left(px(10.))
        .right(px(6.))
        .h(px(2.))
        .rounded_full()
        .bg(color)
        .child(
            div()
                .absolute()
                .left(px(-4.))
                .top(px(-2.))
                .size(px(6.))
                .rounded_full()
                .bg(color),
        );
    match above {
        true => line.top(px(-1.)),
        false => line.bottom(px(-1.)),
    }
}

/// A dashed box to drop a dragged row on, lit while one is over it.
fn drop_zone(id: impl Into<ElementId>, label: impl IntoElement, theme: &Theme) -> Stateful<Div> {
    let highlight = theme.pop;
    h_flex()
        .id(id)
        .mx(px(6.))
        .justify_center()
        .gap(px(6.))
        .rounded(px(4.))
        .border_1()
        .border_dashed()
        .border_color(theme.border_2)
        .text_size(px(12.))
        .text_color(theme.text_2)
        .child(small_icon(Icon::Branch).text_color(theme.text_2))
        // A pull request's title can be long.
        .child(div().min_w(px(0.)).truncate().child(label))
        .drag_over::<Cut>(move |style, _, _, _| {
            style
                .bg(highlight.opacity(0.14))
                .border_color(highlight)
                .text_color(highlight)
        })
}

/// Take `key` out of `set`, or put it in when it isn't there.
fn toggle<T: Eq + Hash>(set: &mut HashSet<T>, key: T) {
    if !set.remove(&key) {
        set.insert(key);
    }
}

impl Focusable for Navigator {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Navigator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Rows {
            rows,
            insets,
            guides,
            ..
        } = self.build_rows(cx);
        self.insets = insets;
        self.guides = guides;
        self.sync_list(rows, window, cx);
        let theme = Theme::get(cx).clone();
        let store = self.store.read(cx);
        let project_count = store.projects.len();
        let empty = store.projects_loaded && project_count == 0;

        // Once the cards scroll under it, a rule along its bottom, as but.dev draws one, and
        // level with the location bar's: the same height and border, from the window's top.
        let top = self.list.logical_scroll_top();
        let under = top.item_ix > 0 || top.offset_in_item > px(0.);
        let header = h_flex()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .border_b_1()
            .border_color(match under {
                true => theme.border_3,
                false => gpui::transparent_black(),
            })
            .pl(px(84.))
            .pr(px(8.))
            .gap(px(6.))
            .child(
                div()
                    .min_w(px(0.))
                    .truncate()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_size(px(14.))
                    .child("Projects"),
            )
            .when(project_count > 0, |el| {
                el.child(
                    div()
                        .px(px(6.))
                        .rounded_full()
                        .bg(theme.fill_gray.opacity(0.1))
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(project_count.to_string()),
                )
            })
            .child(div().flex_1())
            .child(self.account(&theme, cx));

        v_flex()
            .id("navigator")
            .key_context("Navigator")
            .track_focus(&self.focus)
            .on_hover(cx.listener(|this, hovered: &bool, _, _| this.pointer_over = *hovered))
            .on_action(
                cx.listener(|this, _: &SelectPrevious, window, cx| this.step(-1, window, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectNext, window, cx| this.step(1, window, cx)))
            .on_action(cx.listener(|this, _: &SelectFirst, window, cx| {
                this.step(isize::MIN / 2, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &SelectLast, window, cx| {
                    this.step(isize::MAX / 2, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &PickPrevious, window, cx| this.pick_step(-1, window, cx)),
            )
            .on_action(cx.listener(|this, _: &PickNext, window, cx| this.pick_step(1, window, cx)))
            .on_action(cx.listener(|this, _: &PreviousSection, window, cx| {
                this.jump_section(false, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &NextSection, window, cx| {
                    this.jump_section(true, window, cx)
                }),
            )
            .on_action(cx.listener(Self::expand))
            .on_action(cx.listener(Self::collapse))
            .on_action(cx.listener(Self::confirm))
            .size_full()
            .relative()
            .bg(theme.bg_2)
            .border_r_1()
            .border_color(theme.border_3)
            .child(header)
            .map(|el| {
                if empty {
                    el.child(div().flex_1().min_h(px(0.)).child(empty_state(
                        Some(Illustration::Cactus),
                        "No projects yet",
                        "Add a Git repository with ⌘O",
                        &theme,
                        cx,
                    )))
                } else {
                    let rows = list(
                        self.list.clone(),
                        cx.processor(|this, ix: usize, window, cx| this.render_row(ix, window, cx)),
                    )
                    .size_full()
                    .pt(px(2.))
                    .pb(px(12.));
                    let scroll = Scroll::List(self.list.clone());
                    el.child(scrolled(rows, "navigator-scrollbar", scroll, &theme))
                }
            })
            // Scrolling moves the rows from under a pointer that stays put, so where a drag
            // would land goes until the pointer moves again, rather than stay on a row that
            // isn't under it.
            .on_scroll_wheel(cx.listener(|this, _: &ScrollWheelEvent, _, cx| {
                if cx.has_active_drag() && this.store.read(cx).drop_at.is_some() {
                    this.store.update(cx, |store, _| store.drop_at = None);
                    cx.notify();
                }
            }))
            .child(resize_handle(Pane::Navigator, self.store.clone(), &theme))
    }
}
