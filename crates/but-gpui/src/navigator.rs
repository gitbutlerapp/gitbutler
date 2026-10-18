//! The left column: every project as a card, each listing its uncommitted changes,
//! stacks, branches and commits as one tree of rows.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, Entity, FocusHandle, Focusable, Hsla,
    KeyBinding, ListAlignment, ListOffset, ListState, Modifiers, MouseButton, MouseDownEvent,
    Pixels, SharedString, Stateful, Subscription, WeakEntity, Window, actions, div, list,
    prelude::*, px,
};

use crate::app::{AddProject, AppView, Collapse, Confirm, Expand, SelectNext, SelectPrevious};
use crate::backend::{Branch, CommitId, CommitState, PushState, Workspace, Worktree, short_ref};
use crate::detail::{ChecksChanged, Detail};
use crate::overlays::MenuItem;
use crate::state::{Cut, Pane, Prefs, ProjectEntry, ProjectId, Selection, Store, StoreEvent};
use crate::theme::Theme;
use crate::transfer::{Transfer, drag_preview};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, button, commit_title, dot, faint,
    h_flex, icon, icon_button, just_now, list_row, plural, relative_time, resize_handle, review,
    section_label, shortcut, small_icon, v_flex,
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

/// How long a worktree with nothing uncommitted stays listed after it last moved.
const INACTIVE_AFTER: Duration = Duration::from_secs(12 * 60 * 60);

/// How recently a project was last active for "Active in the last 7 days" to keep it.
const ACTIVE_WITHIN: Duration = Duration::from_secs(7 * 24 * 60 * 60);

actions!(
    navigator,
    [SelectFirst, SelectLast, PreviousSection, NextSection]
);

/// Jumps through the tree, as in Lite: to either end, and between branches and projects.
pub fn bind_keys(cx: &mut App) {
    let nav = Some("Navigator && !TextInput");
    cx.bind_keys([
        KeyBinding::new("g g", SelectFirst, nav),
        KeyBinding::new("home", SelectFirst, nav),
        KeyBinding::new("shift-g", SelectLast, nav),
        KeyBinding::new("end", SelectLast, nav),
        KeyBinding::new("shift-up", PreviousSection, nav),
        KeyBinding::new("shift-down", NextSection, nav),
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
    /// Above a stack of more than one branch, which folds it: the project, the stack's bottom
    /// branch, which names it, and how many branches it has.
    StackCaption(ProjectId, String, usize),
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
    Pending(SharedString),
    /// Where a commit of the project being dragged lands in a new branch of its own.
    NewBranch(ProjectId),
    /// After a stack, while one of the project's branches is dragged: where it lands below
    /// the stack's last branch, by its ref.
    LaneEnd(ProjectId, String),
    /// After a project's worktrees: how many are inactive, which it shows or hides.
    Inactive(ProjectId, usize),
    /// Over a worktree's uncommitted changes and branches, the main one for `None`, which
    /// folds them.
    WorktreeHeading(ProjectId, Option<Arc<str>>),
    /// The line after a worktree's section.
    WorktreeDivider,
}

/// The rows as they're built, each with how far in it sits: a worktree's section and a
/// stack's branches step in from what holds them.
#[derive(Default)]
struct Rows {
    rows: Vec<Row>,
    insets: Vec<f32>,
    /// What the rows pushed next sit in by.
    inset: f32,
}

impl Rows {
    fn push(&mut self, row: Row) {
        self.rows.push(row);
        self.insets.push(self.inset);
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
            // With linked worktrees, the main one and each linked one are a section, under a
            // heading that folds it; without, the project is its one worktree.
            let headed = !workspace.worktrees.is_empty() && workspace.editing.is_none();
            let selection = store.selection.as_ref().filter(|s| s.project() == id);
            if headed {
                rows.push(Row::WorktreeHeading(id.clone(), None));
                rows.inset = NEST;
            }
            let main_selected = selection.is_some_and(|s| section_holds(workspace, None, s));
            if !headed || self.worktree_open(id, None, main_selected) {
                rows.push(Row::Item(Selection::Uncommitted(id.clone())));
                // Nothing but the edit goes on while a commit is checked out, its files listed
                // where the uncommitted ones are.
                if workspace.editing.is_some() {
                    continue;
                }
                for stack in &workspace.stacks {
                    self.lane_rows(store, id, &stack.branches, &mut rows);
                    if let Some(dragged @ Cut::Branch(..)) = &store.dragging
                        && let Some(last) = stack.branches.last()
                    {
                        let below = Selection::Branch(id.clone(), last.ref_name.clone());
                        if Transfer::resolve(dragged, &below, Modifiers::shift(), store).is_some() {
                            rows.push(Row::LaneEnd(id.clone(), last.ref_name.clone()));
                        }
                    }
                }
                if workspace.stacks.is_empty() {
                    rows.push(Row::Note("No branches yet".into(), false));
                }
                // Outside the workspace there's nowhere to put a branch of its own.
                if workspace.managed
                    && matches!(&store.dragging, Some(Cut::Commit(project, _)) if project == id)
                {
                    rows.push(Row::NewBranch(id.clone()));
                }
            }
            if headed {
                rows.inset = 0.;
                rows.push(Row::WorktreeDivider);
                let holds_selected =
                    |wt: &Worktree| selection.is_some_and(|s| worktree_holds(s, wt));
                // The inactive ones wait behind a row of their own, unless the selection's in one.
                let (inactive, active): (Vec<_>, Vec<_>) = (workspace.worktrees.iter())
                    .partition(|wt| is_inactive(wt) && !holds_selected(wt));
                for wt in active {
                    self.worktree_rows(store, id, wt, holds_selected(wt), &mut rows);
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
        if let [_, .., bottom] = branches {
            let key = (id.clone(), bottom.ref_name.clone());
            let selected = (store.selection.as_ref())
                .is_some_and(|s| s.project() == id && branches.iter().any(|b| s.on(b)));
            rows.push(Row::StackCaption(
                key.0.clone(),
                key.1.clone(),
                branches.len(),
            ));
            if self.folded_stacks.contains(&key) && !selected {
                return;
            }
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
            rows.extend(
                store
                    .new_commits(id, &branch.ref_name)
                    .into_iter()
                    .map(Row::Pending),
            );
            // Commits a pending operation takes away go, unless selected.
            rows.extend(
                (branch.commits.iter().enumerate())
                    .filter(|&(at, commit)| {
                        !store.hides_commit(id, &commit.key) || selected_at == Some(at)
                    })
                    .take(shown)
                    .map(|(_, commit)| Row::Item(Selection::Commit(id.clone(), commit.id))),
            );
            if branch.commits.len() > shown {
                let hidden = branch.commits.len() - shown;
                rows.push(Row::More(id.clone(), branch.ref_name.clone(), hidden));
            } else if unfolded && branch.commits.len() > fold_at {
                rows.push(Row::Fewer(id.clone(), branch.ref_name.clone()));
            }
        }
        rows.inset = base;
    }

    /// A linked worktree's heading and, unless it's folded without the selection in it, its
    /// uncommitted changes and branches; then a line ruling it off from what follows.
    fn worktree_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        worktree: &Worktree,
        selected: bool,
        rows: &mut Rows,
    ) {
        rows.push(Row::WorktreeHeading(
            id.clone(),
            Some(worktree.name.clone()),
        ));
        if self.worktree_open(id, Some(&worktree.name), selected) {
            rows.inset = NEST;
            rows.push(Row::Item(Selection::Worktree(
                id.clone(),
                worktree.name.clone(),
            )));
            self.lane_rows(store, id, &worktree.branches, rows);
            rows.inset = 0.;
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
            _ => {}
        }
    }

    /// Step out the way a tree does: a commit to its branch, an open branch closed, and
    /// anything else up to its project.
    fn collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let selection = store.selection.clone();
        let closes = matches!(&selection, Some(item @ Selection::Branch(project, ref_name))
            if store.branch_for(item).is_some_and(|branch| !branch.commits.is_empty())
                && !self.closed_branches.contains(&(project.clone(), ref_name.clone())));
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
            // Jump up to the project, like closing a folder in a tree.
            Some(selection) => {
                self.select(Selection::Project(selection.project().clone()), window, cx)
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
            Some(Selection::Uncommitted(_)) => {
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
            Some(Row::Item(item)) => self.render_item(item, inset, window, theme, cx),
            Some(Row::WorktreeHeading(project, name)) => {
                self.worktree_heading(ix, project, name.as_ref(), theme, cx)
            }
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
            Some(Row::Pending(title)) => h_flex()
                .min_h(px(ROW_HEIGHT))
                .mx(px(3.))
                .pl(px(38.))
                .pr(px(12.))
                .gap(px(10.))
                .opacity(0.5)
                .child(small_icon(Icon::Commit).text_color(theme.blue))
                .child(div().flex_1().min_w(px(0.)).truncate().child(title.clone()))
                .child(div().text_size(px(11.)).text_color(theme.text_2).child("…"))
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
            Some(Row::StackCaption(project, bottom, n)) => {
                self.stack_caption(ix, project, bottom, *n, theme, cx)
            }
            // Both drop zones are padded, not margined: a margin collapses through the card's
            // ground.
            Some(Row::NewBranch(project)) => {
                let project = project.clone();
                let zone = drop_zone(("new-branch-drop", ix), "New branch", theme)
                    .h(px(30.))
                    .on_drop(cx.listener(move |this, payload: &Cut, _, cx| {
                        if let Cut::Commit(from, id) = payload
                            && *from == project
                        {
                            this.store
                                .update(cx, |store, cx| store.move_to_new_branch(from, *id, cx));
                        }
                    }));
                div().py(px(2.)).child(zone).into_any_element()
            }
            Some(Row::LaneEnd(project, ref_name)) => {
                let label = format!("Move below {}", short_ref(ref_name));
                let (project, ref_name) = (project.clone(), ref_name.clone());
                let zone = drop_zone(("lane-end-drop", ix), label, theme)
                    .h(px(24.))
                    .on_drop(cx.listener(move |this, payload: &Cut, window, cx| {
                        let below = Selection::Branch(project.clone(), ref_name.clone());
                        this.app
                            .update(cx, |app, cx| {
                                app.transfer(payload, &below, Modifiers::shift(), window, cx)
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
        // Padded, not margined: a list lays its rows out from their left edge.
        div()
            .px(px(10.))
            .child(
                div()
                    .bg(theme.bg_1)
                    .border_color(theme.border_3)
                    .border_l_1()
                    .border_r_1()
                    .when(first, |el| el.border_t_1().rounded_t(px(6.)).pt(px(3.)))
                    .when(last, |el| el.border_b_1().rounded_b(px(6.)).pb(px(3.)))
                    .child(el),
            )
            .into_any_element()
    }

    fn render_item(
        &self,
        item: &Selection,
        inset: f32,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let selected = store.selection.as_ref() == Some(item)
            || matches!(item, Selection::Commit(_, id) if store.picked.contains(id));
        let cut = match (&store.cut, item) {
            (Some(Cut::Branch(p, r)), Selection::Branch(project, ref_name)) => {
                p == project && r == ref_name
            }
            (
                Some(Cut::Commit(p, c) | Cut::CherryPick(p, c)),
                Selection::Commit(project, commit),
            ) => p == project && c == commit,
            _ => false,
        };
        let style = RowStyle {
            theme,
            selected,
            solid: selected && self.focus.contains_focused(window, cx),
            cut,
            inset,
        };
        let el = match item {
            Selection::Project(id) => self.project_row(item, id, style, cx),
            Selection::Uncommitted(_) | Selection::Worktree(..) => {
                self.uncommitted_row(item, style, cx)
            }
            Selection::Branch(id, ref_name) => self.branch_row(item, id, ref_name, style, cx),
            Selection::Commit(id, commit_id) => self.commit_row(item, id, commit_id, style, cx),
            Selection::Branches(_) => {
                Some(self.page_row(item, Icon::Branch, "Branches", style, cx))
            }
            Selection::History(_) => Some(self.page_row(item, Icon::Clock, "History", style, cx)),
        };
        match el {
            Some(el) => self.drop_target(el, item.clone(), window, theme, cx),
            None => div().into_any_element(),
        }
    }

    /// Select `item` on press, as native lists do: a click waits for the release. ⌘ or ⇧
    /// picks commits along with the selected one.
    fn select_on_press(
        &self,
        item: &Selection,
        cx: &Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        let item = item.clone();
        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
            let (platform, shift) = (event.modifiers.platform, event.modifiers.shift);
            match &item {
                // A linked worktree's commits change there, so they aren't picked to change.
                Selection::Commit(project, id)
                    if (platform || shift) && this.store.read(cx).worktree_of(&item).is_none() =>
                {
                    // A run of commits takes the ones shown, not those folded away.
                    let range = shift.then(|| {
                        (this.rows.iter())
                            .filter_map(|row| match row {
                                Row::Item(Selection::Commit(p, commit)) if p == project => {
                                    Some(*commit)
                                }
                                _ => None,
                            })
                            .collect()
                    });
                    this.store
                        .update(cx, |store, cx| store.pick(project, *id, range, cx));
                    window.focus(&this.focus, cx);
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
                .pl(px(4.))
                .child(
                    div()
                        .id(ElementId::Name(format!("chevron-{id}").into()))
                        .flex_none()
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .hover(|el| el.bg(theme.hover()))
                        .child(
                            small_icon(if expanded {
                                Icon::ChevronDown
                            } else {
                                Icon::ChevronRight
                            })
                            .text_color(style.muted()),
                        )
                        // Folding the project doesn't select it.
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.store.update(cx, |store, cx| {
                                store.set_expanded(&chevron_id, !expanded, cx)
                            });
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
                    // Pressing it leaves the selection where it is, as pressing the row wouldn't.
                    el.child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                button(ElementId::Name(format!("update-{id}").into()), "Update")
                                    .small()
                                    .style(ButtonStyle::Gray)
                                    .inverted(style.solid)
                                    .tooltip("Rebase your branches onto the target's new commits")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let project = project.clone();
                                        this.app
                                            .update(cx, |app, cx| {
                                                app.update_from_target(project, cx)
                                            })
                                            .ok();
                                    })),
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

    /// A worktree's uncommitted changes: the main one's for [`Selection::Uncommitted`], a
    /// linked one's for [`Selection::Worktree`].
    fn uncommitted_row(
        &self,
        item: &Selection,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let id = item.project();
        let workspace = store.workspace(id)?;
        let (row_id, uncommitted, scope, editing) = match item {
            Selection::Worktree(_, name) => {
                let worktree = (workspace.worktrees.iter()).find(|w| w.name == *name)?;
                let scope = crate::state::worktree_review_scope(id, name);
                (
                    format!("worktree-{id}-{name}"),
                    &worktree.uncommitted,
                    scope,
                    false,
                )
            }
            _ => {
                let editing = workspace.editing.is_some();
                (
                    format!("uncommitted-{id}"),
                    &workspace.uncommitted,
                    id.clone(),
                    editing,
                )
            }
        };
        let n = uncommitted.len();
        let conflicted = uncommitted.iter().filter(|f| f.conflicted).count();
        let label = match (editing, n) {
            (true, _) => "Editing a commit",
            (false, 0) => "No uncommitted changes",
            (false, _) => "Uncommitted changes",
        };
        // Only the main worktree's files go to the workspace's branches, and the commit being
        // edited's go nowhere else until it's finished.
        let drags = matches!(item, Selection::Uncommitted(_)) && n > 0 && !editing;
        Some(
            style
                .row(row_id, 6.)
                .aria_label(label)
                .child(icon(Icon::Diff).text_color(if n > 0 {
                    style.meta(theme.warn)
                } else {
                    style.muted()
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .when(n == 0 && !editing, |el| el.text_color(style.muted()))
                        .child(label),
                )
                // What a merge left conflicted, as an update can, before anything else.
                .children(meta_count(
                    Icon::Warning,
                    conflicted,
                    style.meta(theme.danger),
                    plural(conflicted, "conflicted file"),
                ))
                .children(review_count(
                    store.unreviewed(&scope, uncommitted),
                    n,
                    style,
                ))
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .when(drags, |el| {
                    let files = self.detail.read(cx).files_to_move(id, cx);
                    let label: SharedString = match files.len() {
                        1 => files[0].file.path.clone().into(),
                        n => format!("{n} files").into(),
                    };
                    let payload = Cut::Files {
                        project: id.clone(),
                        from: None,
                        files: files.into(),
                    };
                    el.on_drag(payload, drag_preview(self.store.clone(), label))
                })
                .into_any_element(),
        )
    }

    /// Over a worktree's section: a chevron that folds it, its folder and name, and, folded,
    /// the counts the rows under it would show. A linked one's right-click menu is its own.
    fn worktree_heading(
        &self,
        ix: usize,
        project: &ProjectId,
        name: Option<&Arc<str>>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let (Some(entry), Some(workspace)) = (store.project(project), store.workspace(project))
        else {
            return div().into_any_element();
        };
        let worktree = name.and_then(|name| workspace.worktrees.iter().find(|w| w.name == *name));
        let (label, path, files, branches): (_, _, _, Vec<&Branch>) = match worktree {
            Some(wt) => (
                wt.name.to_string(),
                &wt.path,
                wt.uncommitted.len(),
                wt.branches.iter().collect(),
            ),
            None => (
                "Main worktree".to_owned(),
                &entry.project.path,
                workspace.uncommitted.len(),
                workspace.branches().collect(),
            ),
        };
        let unpushed_n = unpushed(branches.iter().copied());
        let commits = || branches.iter().flat_map(|branch| &branch.commits);
        let unread = commits().any(|commit| store.is_unread(project, commit));
        // Open even when folded while the selection's in it.
        let open = (self.rows.get(ix + 1)).is_some_and(|row| *row != Row::WorktreeDivider);
        // Written to in the last two minutes, likely by the agent working in it.
        let active = worktree.is_some_and(|wt| wt.last_activity().is_some_and(just_now));
        let key = (project.clone(), name.cloned());
        let menu_target = name.map(|name| Selection::Worktree(project.clone(), name.clone()));
        h_flex()
            .id(("worktree-heading", ix))
            .h(px(ROW_HEIGHT))
            .mx(px(3.))
            .px(px(8.))
            .gap(px(8.))
            .rounded(px(4.))
            .hover(|el| el.bg(theme.hover()))
            .child(
                small_icon(match open {
                    true => Icon::ChevronDown,
                    false => Icon::ChevronRight,
                })
                .text_color(theme.text_3),
            )
            .child(
                icon(if name.is_some() {
                    Icon::FolderCopy
                } else {
                    Icon::Folder
                })
                .text_color(theme.text_2),
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
            .when(!open, |el| {
                el.children(meta_count(
                    Icon::File,
                    files,
                    theme.warn,
                    plural(files, "uncommitted file"),
                ))
                .children(meta_count(
                    Icon::ArrowUp,
                    unpushed_n,
                    theme.blue,
                    plural(unpushed_n, "unpushed commit"),
                ))
                .when(unread, |el| el.child(dot(theme.pop)))
            })
            .when(active, |el| {
                el.child(
                    div()
                        .id(("worktree-active", ix))
                        .child(dot(theme.safe))
                        .tooltip(Tooltip::text("Changed in the last 2 minutes")),
                )
            })
            // Folding what holds the selection takes it up to the project, as closing a folder
            // in a tree does.
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if !open {
                    this.folded_worktrees.remove(&key);
                    return cx.notify();
                }
                this.folded_worktrees.insert(key.clone());
                let store = this.store.read(cx);
                let inside = (store.selection.as_ref())
                    .zip(store.workspace(&key.0))
                    .is_some_and(|(s, ws)| {
                        s.project() == &key.0 && section_holds(ws, key.1.as_ref(), s)
                    });
                match inside {
                    true => this.select(Selection::Project(key.0.clone()), window, cx),
                    false => cx.notify(),
                }
            }))
            .when_some(menu_target, |el, target| {
                el.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        let items = this.menu_items(&target, cx);
                        (this.app)
                            .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                            .ok();
                    }),
                )
            })
            .into_any_element()
    }

    /// Over a stack of two or more branches, as ui-react's StackCaption: a chevron, its
    /// glyph and how many. A click on it folds the branches away, and folded, it says how many
    /// commits they have to push and when they last changed.
    fn stack_caption(
        &self,
        ix: usize,
        project: &ProjectId,
        bottom: &str,
        n: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let branches = (store.workspace(project))
            .and_then(|ws| lane_of(ws, bottom))
            .unwrap_or_default();
        // Open even when folded while the selection's in it.
        let open = (self.insets.get(ix + 1)).is_some_and(|next| *next > self.insets[ix]);
        let names: Vec<&str> = branches.iter().map(|branch| branch.name.as_str()).collect();
        let unpushed_n = unpushed(branches.iter());
        let latest = (branches.iter().flat_map(|branch| &branch.commits))
            .map(|commit| commit.time)
            .max();
        let key = (project.clone(), bottom.to_owned());
        h_flex()
            .id(("stack", ix))
            .h(px(ROW_HEIGHT))
            .mx(px(3.))
            .pl(px(14.))
            .pr(px(8.))
            .gap(px(8.))
            .rounded(px(4.))
            .cursor_pointer()
            .hover(|el| el.bg(theme.hover()))
            .child(
                div().w(px(14.)).flex().justify_center().child(
                    small_icon(match open {
                        true => Icon::ChevronDown,
                        false => Icon::ChevronRight,
                    })
                    .text_color(theme.text_3),
                ),
            )
            .child(icon(Icon::Stack).text_color(theme.text_3))
            .child(
                div()
                    .id(("stack-name", ix))
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .tooltip(Tooltip::text(format!("Stack: {}", names.join(" ← "))))
                    .child(format!("Stack of {n}")),
            )
            .when(!open, |el| {
                el.children(meta_count(
                    Icon::ArrowUp,
                    unpushed_n,
                    theme.blue,
                    plural(unpushed_n, "unpushed commit"),
                ))
                .children(latest.map(|at| faint(relative_time(at), theme).text_size(px(11.))))
            })
            // Folding what holds the selection takes it up to the project, as a worktree's
            // heading does.
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if !open {
                    this.folded_stacks.remove(&key);
                    return cx.notify();
                }
                this.folded_stacks.insert(key.clone());
                let store = this.store.read(cx);
                let inside = (store.selection.as_ref())
                    .zip(store.workspace(&key.0).and_then(|ws| lane_of(ws, &key.1)))
                    .is_some_and(|(s, lane)| {
                        s.project() == &key.0 && lane.iter().any(|branch| s.on(branch))
                    });
                match inside {
                    true => this.select(Selection::Project(key.0.clone()), window, cx),
                    false => cx.notify(),
                }
            }))
            .into_any_element()
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
                .aria_label(format!("Branch {}", branch.name))
                .child(step)
                .child(icon(glyph).text_color(style.meta(glyph_color)))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .font_family(theme.mono_font.clone())
                        .text_size(px(12.))
                        .text_color(style.meta(theme.blue))
                        .child(branch.name.clone()),
                )
                // As in t3code: the number in the state's colour, then one glyph for CI.
                .when_some(branch.review.as_ref(), |el, review| {
                    el.child(
                        div()
                            .text_size(px(11.))
                            .text_color(style.meta(glyph_color))
                            .child(format!("#{}", review.number)),
                    )
                    .when_some(review.checks, |el, checks| {
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
                .when(branch.commits.is_empty(), |el| {
                    el.child(faint("no commits yet", theme))
                })
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
                // A linked worktree's branches stay where its agent works on them.
                .when(
                    !ref_name.is_empty() && store.worktree_of(item).is_none(),
                    |el| {
                        let payload = Cut::Branch(id.clone(), ref_name.to_owned());
                        el.on_drag(
                            payload,
                            drag_preview(self.store.clone(), branch.name.clone().into()),
                        )
                    },
                )
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
        let payload = Cut::Commit(id.clone(), *commit_id);
        let label: SharedString = commit.title.clone().into();
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
                        .when(commit.title.is_empty(), |el| {
                            el.italic().text_color(style.muted())
                        })
                        .when(unread, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                        .child(commit_title(&commit.title)),
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
                .when(store.worktree_of(item).is_none(), |el| {
                    el.on_drag(payload, drag_preview(self.store.clone(), label))
                })
                .into_any_element(),
        )
    }

    /// Make a row accept drags it has a meaning for, and offer its right-click menu.
    /// While something is cut or dragged, the row says what landing there would do, or
    /// steps back when it would do nothing.
    fn drop_target(
        &self,
        el: AnyElement,
        target: Selection,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        // What a drop or a paste would carry: the drag while there is one, else the cut.
        let pending = if cx.has_active_drag() {
            store.dragging.as_ref()
        } else {
            store.cut.as_ref()
        };
        let held = window.modifiers();
        let verdict = pending
            .map(|payload| Transfer::resolve(payload, &target, held, store).map(|t| t.verb()));
        let store = self.store.clone();
        let can_drop_target = target.clone();
        let menu_target = target.clone();
        let paste_target = target.clone();
        let highlight = theme.pop;
        div()
            .relative()
            .rounded(px(4.))
            .child(el)
            .when(matches!(verdict, Some(None)), |el| el.opacity(0.35))
            .when_some(verdict.flatten(), |el, verb| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(11.))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .id(ElementId::Name(format!("paste-{paste_target:?}").into()))
                                .cursor_pointer()
                                // Opaque, as the row's own text sits under it.
                                .hover(|el| el.bg(theme.bg_1.blend(highlight.opacity(0.14))))
                                // It reads as a button, so it pastes, as ⌘V on the row does,
                                // without pressing the row under it, which would select or
                                // pick it, or start dragging it.
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    let Some(cut) = this.store.read(cx).cut.clone() else {
                                        return;
                                    };
                                    let (target, held) = (paste_target.clone(), window.modifiers());
                                    (this.app.update(cx, |app, cx| {
                                        app.transfer(&cut, &target, held, window, cx)
                                    }))
                                    .ok();
                                }))
                                .px(px(6.))
                                .py(px(1.))
                                .rounded(px(4.))
                                .bg(theme.bg_1)
                                .border_1()
                                .border_color(highlight.opacity(0.6))
                                .text_size(px(11.))
                                .text_color(highlight)
                                .child(verb),
                        ),
                )
            })
            .can_drop(move |payload, window, cx| {
                payload.downcast_ref::<Cut>().is_some_and(|payload| {
                    let held = window.modifiers();
                    Transfer::resolve(payload, &can_drop_target, held, store.read(cx)).is_some()
                })
            })
            .drag_over::<Cut>(move |style, _, _, _| {
                style
                    .bg(highlight.opacity(0.14))
                    .border_1()
                    .border_color(highlight)
            })
            .on_drop(cx.listener(move |this, payload: &Cut, window, cx| {
                let held = window.modifiers();
                this.app
                    .update(cx, |app, cx| {
                        app.transfer(payload, &target, held, window, cx)
                    })
                    .ok();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let items = this.menu_items(&menu_target, cx);
                    let picked = matches!(&menu_target,
                        Selection::Commit(_, id) if this.store.read(cx).picked.contains(id));
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

/// Whether `selection` is in `worktree`: its uncommitted changes, a branch or a commit.
fn worktree_holds(selection: &Selection, worktree: &Worktree) -> bool {
    matches!(selection, Selection::Worktree(_, name) if *name == worktree.name)
        || worktree.branches.iter().any(|branch| selection.on(branch))
}

/// Whether `selection` is in the section of worktree `name`, the main one's for `None`.
fn section_holds(workspace: &Workspace, name: Option<&Arc<str>>, selection: &Selection) -> bool {
    match name {
        None => {
            matches!(selection, Selection::Uncommitted(_))
                || workspace.branches().any(|branch| selection.on(branch))
        }
        Some(name) => (workspace.worktrees.iter())
            .find(|wt| wt.name == *name)
            .is_some_and(|wt| worktree_holds(selection, wt)),
    }
}

/// The branches of the stack, or linked worktree's lane, whose bottom branch is `bottom`.
fn lane_of<'a>(workspace: &'a Workspace, bottom: &str) -> Option<&'a [Branch]> {
    (workspace.stacks.iter().map(|stack| &stack.branches[..]))
        .chain(workspace.worktrees.iter().map(|wt| &wt.branches[..]))
        .find(|branches| {
            branches
                .last()
                .is_some_and(|branch| branch.ref_name == bottom)
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

/// Of `n` uncommitted files, how many are left to review, or that none are.
fn review_count(to_review: usize, n: usize, style: RowStyle) -> Option<Stateful<Div>> {
    if to_review > 0 {
        let tip = format!("{} to review", plural(to_review, "file"));
        meta_count(Icon::Eye, to_review, style.meta(style.theme.warn), tip)
    } else {
        let tip = format!("{} reviewed", plural(n, "file"));
        meta_count(Icon::Tick, n, style.muted(), tip)
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
        .child(label)
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
        let Rows { rows, insets, .. } = self.build_rows(cx);
        self.insets = insets;
        self.sync_list(rows, window, cx);
        let theme = Theme::get(cx).clone();
        let store = self.store.read(cx);
        let project_count = store.projects.len();
        let empty = store.projects_loaded && project_count == 0;

        let header = h_flex()
            .h(px(HEADER_HEIGHT))
            .flex_none()
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
                    el.child(
                        v_flex()
                            .p(px(20.))
                            .gap(px(6.))
                            .text_color(theme.text_2)
                            .child("No projects yet")
                            .child("Add a Git repository with ⌘O"),
                    )
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
            .child(resize_handle(Pane::Navigator, self.store.clone(), &theme))
    }
}
