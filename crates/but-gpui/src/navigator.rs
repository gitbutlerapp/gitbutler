//! The left column: every project as a card, each listing its uncommitted changes,
//! stacks, branches and commits as one tree of rows.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, DragMoveEvent, ElementId, Entity,
    FocusHandle, Focusable, Hsla, KeyBinding, ListAlignment, ListOffset, ListState, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, PlatformInput, Point, ScrollWheelEvent,
    SharedString, Stateful, Subscription, Task, WeakEntity, Window, actions, deferred, div, list,
    prelude::*, px,
};

use crate::app::{AddProject, AppView, Collapse, Confirm, Expand, SelectNext, SelectPrevious};
use crate::backend::{
    Branch, CommitId, CommitState, FileChange, PushState, Review, ReviewState, Workspace, Worktree,
};
use crate::detail::{ChecksChanged, Detail};
use crate::overlays::MenuItem;
use crate::state::{
    Cut, NewAt, Pane, Prefs, ProjectEntry, ProjectId, Selection, Store, StoreEvent, WatchedOrder,
};
use crate::theme::Theme;
use crate::transfer::{Place, Transfer, drag_pill, drag_preview};
use crate::ui::emoji::{Colour, ProjectIcon};
use crate::ui::illustration::Illustration;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, button, commit_title, count_badge, dot,
    empty_state, faint, field, guide, h_flex, icon, icon_button, just_now, list_row, meta_count,
    plural, relative_time, resize_handle, review, section_label, shortcut, small_icon, v_flex,
};

mod account;
mod menu;
mod mesh;
mod order;

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
        PickNext,
        FocusFilter
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
        KeyBinding::new("/", FocusFilter, nav),
    ]);
}

/// A branch whose commits a click on its row opened, here or on another machine, or a
/// remote's merge whose commits it opened.
#[derive(Clone, PartialEq)]
enum ClickOpened {
    Here((ProjectId, String)),
    Elsewhere(crate::state::MeshBranch),
    Merge(ProjectId, CommitId),
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
    /// A commit a mutation is making, by its title, and whether it's been long enough to
    /// show it's still on its way.
    Pending(SharedString, bool),
    /// Where a commit of the project being dragged lands in a new branch of its own.
    NewBranch(ProjectId),
    /// After a project's active worktrees, the heading over its inactive ones: how many, and
    /// whether the selection's in one, which keeps them listed.
    Inactive(ProjectId, usize, bool),
    /// The line after a worktree's section.
    WorktreeDivider,
    /// Over a project's checkouts on other machines, which folds them all away.
    MeshHeading(ProjectId),
    /// Over the commits only a remote has, branch `String`'s or with `None` the target's,
    /// which folds them away.
    UpstreamHeading(ProjectId, Option<String>),
    /// After the target's new commits listed, how many more its page lists.
    UpstreamMore(ProjectId, usize),
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
    /// The first item the filter found, which Enter in it selects.
    first_hit: Option<Selection>,
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
        self.open_at(GUIDE_AT);
    }

    /// As [`Self::open`], for a row whose chevron's middle is `at` instead.
    fn open_at(&mut self, at: f32) {
        let under = self.insets.last().copied().unwrap_or_default();
        self.open.push(at + under);
    }

    fn close(&mut self) {
        self.open.pop();
    }

    /// Push `item`, the filter's first hit if it's `hit` and none came before it.
    fn push_found(&mut self, item: Selection, hit: bool) {
        if hit && self.first_hit.is_none() {
            self.first_hit = Some(item.clone());
        }
        self.push(Row::Item(item));
    }

    /// Take back the rows pushed since there were `len`.
    fn truncate(&mut self, len: usize) {
        self.rows.truncate(len);
        self.insets.truncate(len);
        self.guides.truncate(len);
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

/// How far a remote's commits sit in: the target's as a branch's commits do, a branch's a
/// step in from their heading, which is under the branch's commit glyphs.
fn upstream_indent(ref_name: Option<&str>) -> f32 {
    30. + if ref_name.is_some() { STACK_STEP } else { 0. }
}

/// Where the middles of a remote's heading's chevron and of its merges' chevrons sit: the
/// target's under the worktree chevrons, a branch's under its commit glyphs.
fn upstream_chevrons(ref_name: Option<&str>) -> (f32, f32) {
    let heading = if ref_name.is_some() { 47. } else { 23. };
    (heading, 3. + 8. + upstream_indent(ref_name) + 6.)
}

/// A fold's glyph in a step as wide as a branch's: a chevron, down while `open`, or with
/// nothing to fold (`None`) a dot at half strength, as ui-react's.
fn step(open: Option<bool>, color: gpui::Hsla) -> Div {
    let glyph = match open {
        None => Icon::BulletDisc,
        Some(true) => Icon::ChevronDown,
        Some(false) => Icon::ChevronRight,
    };
    div()
        .flex_none()
        .w(px(14.))
        .h(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .child(small_icon(glyph).text_color(match open {
            None => color.opacity(0.5),
            Some(_) => color,
        }))
}

/// A branch row's click handler: a plain click opens its commits, as
/// [`Navigator::open_on_click`] does; one with ⌘ or ⇧, picking commits, doesn't.
fn open_on_click(
    branch: ClickOpened,
    cx: &Context<Navigator>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |this, event: &ClickEvent, _, cx| {
        let held = event.modifiers();
        if !held.secondary() && !held.shift {
            this.open_on_click(branch.clone(), cx)
        }
    })
}

/// A row's click handler that runs `toggle` on a double-click, as a folder opens.
fn double_click(
    toggle: impl Fn(&mut Navigator, &mut Window, &mut Context<Navigator>) + 'static,
    cx: &Context<Navigator>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |this, event: &ClickEvent, window, cx| {
        if event.click_count() == 2 {
            toggle(this, window, cx)
        }
    })
}

impl Navigator {
    /// A row's fold, as a branch's: a chevron that folds it without selecting it, down while
    /// `open`, or a dot when there's nothing to fold (`None`).
    fn fold_step(
        &self,
        id: impl Into<ElementId>,
        open: Option<bool>,
        color: gpui::Hsla,
        cx: &Context<Self>,
        toggle: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let theme = Theme::get(cx);
        let el = div().id(id).child(step(open, color));
        match open {
            None => el,
            Some(_) => el
                .rounded(px(3.))
                .hover(|el| el.bg(theme.hover()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    toggle(this, window, cx)
                })),
        }
    }
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

    /// The faintest text, as a fold's glyph, or on a solid selection its muted ink.
    fn quiet(&self) -> Hsla {
        match self.solid {
            true => self.muted(),
            false => self.theme.text_3,
        }
    }

    /// How many of a row's commits aren't pushed, when any aren't.
    fn unpushed(&self, n: usize) -> Option<Stateful<Div>> {
        let color = self.meta(self.theme.blue);
        meta_count(Icon::ArrowUp, n, color, plural(n, "unpushed commit"))
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
    /// Branches folded or opened against how Settings say branches start: folded, unless
    /// they say open. A branch with the selected commit shows its commits either way.
    flipped_branches: HashSet<(ProjectId, String)>,
    /// Whether branches start open, as the flips were made against. When Settings change
    /// it, the flips go and every branch starts over.
    branches_start_open: bool,
    /// Worktrees showing only their heading, the main one by `None`.
    folded_worktrees: HashSet<(ProjectId, Option<Arc<str>>)>,
    /// Stacks showing only their caption, by their bottom branch.
    folded_stacks: HashSet<(ProjectId, String)>,
    /// Checkouts on other machines showing only their heading, by the mesh's id.
    folded_checkouts: HashSet<(ProjectId, Arc<str>)>,
    /// The commits only a remote has listed under their heading, a branch's (by its full
    /// ref) or with `None` the target's. They start folded to the heading.
    open_upstream: HashSet<(ProjectId, Option<String>)>,
    /// Branches on other machines folded or opened against how Settings say branches start,
    /// as [`Self::flipped_branches`].
    flipped_mesh_branches: HashSet<crate::state::MeshBranch>,
    /// The branch a click on its row opened, which a click on another's folds again; one
    /// opened or folded by hand since stays as it's left.
    click_opened: Option<ClickOpened>,
    /// Merges only a remote has whose commits show under them.
    open_merges: HashSet<CommitId>,
    /// The filter in the header, over every project's rows.
    filter: Entity<TextInput>,
    /// The first item the filter found, which Enter in it selects.
    first_hit: Option<Selection>,
    /// The projects not expanded show all of them rather than the first few.
    projects_unfolded: bool,
    /// Whether the pointer is over the navigator.
    pointer_over: bool,
    /// The project whose watch button the pointer is on, which shows what a click makes it.
    watch_hovered: Option<ProjectId>,
    /// Where a dragged project card would go, while one is over a card's heading.
    project_drop: Option<CardSlot>,
    /// The row under the pointer holds still through the next change to the rows: one a
    /// workspace's load made, as an agent's commit, or "Show fewer" folding what's above it,
    /// which would leave the list scrolled past the fold. What else the user folds or does
    /// moves the rows as usual.
    hold_still: bool,
    /// How far a frame the list scrolls while something dragged is near its top or bottom
    /// edge, up when negative, and the task that scrolls it.
    edge_scroll: f32,
    edge_scrolling: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Navigator {
    pub fn new(
        app: WeakEntity<AppView>,
        store: Entity<Store>,
        detail: Entity<Detail>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| TextInput::new("Filter", false, cx));
        let subscriptions = vec![
            cx.subscribe_in(&filter, window, |this, input, event, window, cx| {
                match event {
                    InputEvent::Changed => this.filter_changed(cx),
                    // Esc empties it, then leaves it.
                    InputEvent::Cancel if input.read(cx).text().is_empty() => {
                        this.focus_list(window, cx)
                    }
                    InputEvent::Cancel => {
                        input.update(cx, |input, cx| input.set_text("", cx));
                        this.filter_changed(cx);
                    }
                    InputEvent::Submit => match this.first_hit.clone() {
                        Some(hit) => this.select(hit, window, cx),
                        None => this.focus_list(window, cx),
                    },
                }
            }),
            cx.observe(&store, |this, store, cx| {
                let start_open = store.read(cx).prefs.branches_start_open;
                if this.branches_start_open != start_open {
                    this.branches_start_open = start_open;
                    this.flipped_branches.clear();
                    this.flipped_mesh_branches.clear();
                    this.click_opened = None;
                }
                cx.notify()
            }),
            cx.subscribe(&store, |this, _, event, _| match event {
                StoreEvent::SelectionChanged => this.reveal_selection = true,
                StoreEvent::Loaded(_) | StoreEvent::MeshRead(_) => this.hold_still = true,
                // A renamed branch stays open or folded as it was.
                StoreEvent::Renamed(project, from, to) => {
                    let sets = [
                        &mut this.flipped_branches,
                        &mut this.unfolded_branches,
                        &mut this.folded_stacks,
                    ];
                    for set in sets {
                        if set.remove(&(project.clone(), from.clone())) {
                            set.insert((project.clone(), to.clone()));
                        }
                    }
                    if (this.open_upstream).remove(&(project.clone(), Some(from.clone()))) {
                        (this.open_upstream).insert((project.clone(), Some(to.clone())));
                    }
                    if let Some(ClickOpened::Here((p, r))) = &mut this.click_opened
                        && p == project
                        && r == from
                    {
                        *r = to.clone();
                    }
                }
                _ => {}
            }),
            // Dragging the uncommitted changes carries the checked ones.
            cx.subscribe(&detail, |_, _, _: &ChecksChanged, cx| cx.notify()),
        ];
        let branches_start_open = store.read(cx).prefs.branches_start_open;
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
            flipped_branches: HashSet::new(),
            click_opened: None,
            open_merges: HashSet::new(),
            open_upstream: HashSet::new(),
            folded_checkouts: HashSet::new(),
            flipped_mesh_branches: HashSet::new(),
            branches_start_open,
            folded_worktrees: HashSet::new(),
            folded_stacks: HashSet::new(),
            filter,
            first_hit: None,
            projects_unfolded: false,
            pointer_over: false,
            watch_hovered: None,
            project_drop: None,
            hold_still: false,
            edge_scroll: 0.,
            edge_scrolling: None,
            _subscriptions: subscriptions,
        }
    }

    /// Rows of every card, with a gap between two cards. Expanded projects get a card
    /// each; the collapsed ones share one compact card at the end.
    fn build_rows(&self, cx: &App) -> Rows {
        let terms = self.filter_terms(cx);
        if !terms.is_empty() {
            return self.build_filtered_rows(&terms, cx);
        }
        let store = self.store.read(cx);
        let mut rows = Rows::default();
        for entry in self.watched(store) {
            rows.inset = 0.;
            if !rows.rows.is_empty() {
                rows.push(Row::Gap);
            }
            let id = &entry.id;
            rows.push(Row::Item(Selection::Project(id.clone())));
            if store.project_folded(id) {
                continue;
            }
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
            self.upstream_rows(store, workspace, id, None, &mut rows);
            // Nothing but the edit goes on while a commit is checked out, its files listed
            // where the uncommitted ones are.
            if workspace.editing.is_some() {
                rows.push(Row::Item(Selection::Uncommitted(id.clone())));
                continue;
            }
            // The main worktree's heading folds its branches away; with linked worktrees each is
            // a section too, and the branches step in under their headings. Alone, they don't:
            // the heading lines up with them, and they with the project.
            let headed = !workspace.worktrees.is_empty();
            let selection = store.selection.as_ref().filter(|s| s.project() == id);
            let main_selected = selection.is_some_and(|s| within(s, workspace.branches()));
            let main_open = self.worktree_open(id, None, main_selected);
            // Always listed, with nothing uncommitted too, so where to commit is always there.
            rows.push(Row::Item(Selection::Uncommitted(id.clone())));
            if headed {
                rows.open();
                rows.inset = NEST;
            }
            if main_open {
                // A stack being unapplied leaves at once.
                let stacks = (workspace.stacks.iter()).filter(|stack| {
                    !(stack.branches.iter()).any(|b| store.hides_branch(id, &b.ref_name))
                });
                for stack in stacks {
                    self.lane_rows(store, id, &stack.branches, &mut rows);
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
                // The inactive ones wait under a heading of their own, folded unless opened or
                // the selection's in one.
                let (inactive, active): (Vec<_>, Vec<_>) = (workspace.worktrees.iter())
                    .filter(|wt| !store.hides_worktree(id, &wt.name))
                    .partition(|wt| is_inactive(wt));
                for wt in active {
                    self.worktree_rows(store, id, wt, on_branches(wt), &mut rows);
                }
                if !inactive.is_empty() {
                    let inside = (inactive.iter()).any(|wt| heads(wt) || on_branches(wt));
                    rows.push(Row::Inactive(id.clone(), inactive.len(), inside));
                    if inside || store.prefs.shown_inactive.contains(id.as_ref()) {
                        for wt in inactive {
                            self.worktree_rows(store, id, wt, on_branches(wt), &mut rows);
                        }
                    }
                }
            }
            // Signed in to the mesh, its checkouts on the user's other machines, as worktrees.
            if store.mesh.connection().is_some() {
                self.mesh_rows(store, id, &mut rows);
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
        let selected = |entry: &ProjectEntry| {
            store.selection.as_ref() == Some(&Selection::Project(entry.id.clone()))
        };
        let mut listed: Vec<(&ProjectEntry, Option<i64>)> = (others.iter())
            .map(|entry| (*entry, last_active(entry)))
            .filter(|(entry, at)| {
                !prefs.active_projects_only || at.is_some_and(active_lately) || selected(entry)
            })
            .collect();
        // Stable, so projects active at the same moment stay by name.
        if prefs.projects_by_activity {
            listed.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
        }
        rows.push(Row::ProjectsHeading(title, listed.len(), others.len()));
        if prefs.projects_folded {
            return rows;
        }
        // Past the fold, but never past the selected project.
        let fold_at = FOLD_AFTER
            .max((listed.iter().position(|(entry, _)| selected(entry))).map_or(0, |at| at + 1));
        let shown = match self.projects_unfolded {
            true => listed.len(),
            false => fold_at,
        };
        rows.extend(
            (listed.iter().take(shown))
                .map(|(entry, _)| Row::Item(Selection::Project(entry.id.clone()))),
        );
        if listed.len() > shown {
            rows.push(Row::MoreProjects(listed.len() - shown));
        } else if self.projects_unfolded && listed.len() > fold_at {
            rows.push(Row::FewerProjects);
        }
        if listed.is_empty() && !others.is_empty() {
            rows.push(Row::Note("None active in the last 7 days".into(), false));
        }
        rows
    }

    /// The rows while the filter has words in it: every project, worktree, branch and commit
    /// that matches, under what holds it, folded or not. One matches when each word is in
    /// its name or in the name of what holds it, and one at least is in its own, so "shop
    /// cart" finds shop's cart branch, not all of shop. A branch matches by its name or its
    /// pull request's title, a commit by its title, a project not expanded by its name.
    fn build_filtered_rows(&self, terms: &[String], cx: &App) -> Rows {
        let store = self.store.read(cx);
        let hit = |held, own| matches(terms, held, own);
        let mut rows = Rows::default();
        for entry in self.watched(store) {
            let id = &entry.id;
            let start = rows.rows.len();
            rows.inset = 0.;
            if start > 0 {
                rows.push(Row::Gap);
            }
            let own = found(terms, &[&entry.project.name]);
            rows.push_found(Selection::Project(id.clone()), hit(0, own));
            let project_end = rows.rows.len();
            let workspace = (entry.workspace.as_ref()).filter(|ws| ws.editing.is_none());
            if let Some(workspace) = workspace {
                // The main worktree's branches under its heading when there are linked ones.
                let headed = !workspace.worktrees.is_empty();
                let main = rows.rows.len();
                if headed {
                    rows.push(Row::Item(Selection::Uncommitted(id.clone())));
                    rows.open();
                    rows.inset = NEST;
                }
                let heading_end = rows.rows.len();
                for stack in &workspace.stacks {
                    self.filtered_lane(store, id, &stack.branches, terms, own, &mut rows);
                }
                if headed {
                    rows.close();
                    rows.inset = 0.;
                    if rows.rows.len() == heading_end {
                        rows.truncate(main);
                    }
                }
                let worktrees =
                    (workspace.worktrees.iter()).filter(|wt| !store.hides_worktree(id, &wt.name));
                for wt in worktrees {
                    let wt_start = rows.rows.len();
                    let wt_own = found(terms, &[&wt.name]);
                    let item = Selection::Worktree(id.clone(), wt.name.clone());
                    rows.push_found(item, hit(own, wt_own));
                    rows.open();
                    rows.inset = NEST;
                    self.filtered_lane(store, id, &wt.branches, terms, own | wt_own, &mut rows);
                    rows.inset = 0.;
                    rows.close();
                    if rows.rows.len() == wt_start + 1 && !hit(own, wt_own) {
                        rows.truncate(wt_start);
                    }
                }
            }
            self.filtered_mesh_rows(store, id, terms, own, &mut rows);
            if rows.rows.len() == project_end && !hit(0, own) {
                rows.truncate(start);
            }
        }
        let others = (store.projects.iter()).filter(|entry| !entry.expanded);
        let total = others.clone().count();
        let listed: Vec<_> = others
            .filter(|entry| hit(0, found(terms, &[&entry.project.name])))
            .collect();
        rows.inset = 0.;
        if !listed.is_empty() {
            let title = match rows.rows.is_empty() {
                true => "All projects",
                false => "More projects",
            };
            if !rows.rows.is_empty() {
                rows.push(Row::Gap);
            }
            rows.push(Row::ProjectsHeading(title, listed.len(), total));
            for entry in listed {
                rows.push_found(Selection::Project(entry.id.clone()), true);
            }
        }
        if rows.rows.is_empty() {
            rows.push(Row::Note("Nothing matches".into(), false));
        }
        rows
    }

    /// [`Self::lane_rows`] under the filter, with `held` the words found in what holds the
    /// lane: the branches that match or have commits that do, with those commits, under
    /// their stack's caption when there are two or more.
    fn filtered_lane(
        &self,
        store: &Store,
        id: &ProjectId,
        branches: &[Branch],
        terms: &[String],
        held: u64,
        rows: &mut Rows,
    ) {
        // A stack being unapplied leaves at once.
        if (branches.iter()).any(|b| store.hides_branch(id, &b.ref_name)) {
            return;
        }
        let branches = &*store.after_deletes(id, branches);
        let (start, base) = (rows.rows.len(), rows.inset);
        let stacked = branches.len() > 1;
        if let [_, .., bottom] = branches {
            rows.push(Row::Item(Selection::Stack(
                id.clone(),
                bottom.ref_name.clone(),
            )));
            rows.open();
            rows.inset = base + STACK_STEP;
        }
        let caption_end = rows.rows.len();
        for branch in branches {
            let own = found(
                terms,
                &[store.name_of(id, branch), store.label_of(id, branch)],
            );
            let commits: Vec<_> = (branch.commits.iter())
                .filter(|commit| {
                    !store.hides_commit(id, &commit.key)
                        && matches(terms, held | own, found(terms, &[&commit.title]))
                })
                .collect();
            let branch_hit = matches(terms, held, own);
            if !branch_hit && commits.is_empty() {
                continue;
            }
            rows.push_found(
                Selection::Branch(id.clone(), branch.ref_name.clone()),
                branch_hit,
            );
            rows.open();
            for commit in commits {
                rows.push_found(Selection::Commit(id.clone(), commit.id), true);
            }
            rows.close();
        }
        if stacked {
            rows.close();
        }
        rows.inset = base;
        if rows.rows.len() == caption_end {
            rows.truncate(start);
        }
    }

    /// A stack's or a linked worktree's branches: over two or more a caption, which folds
    /// them away unless the selection's in them, and they step in under it; then each branch
    /// with its commits, up to the fold, unless it's closed.
    fn lane_rows(&self, store: &Store, id: &ProjectId, branches: &[Branch], rows: &mut Rows) {
        let branches = &*store.after_deletes(id, branches);
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
            let upstream = Some(branch.ref_name.as_str());
            let closed = !branch.commits.is_empty()
                && !self.branch_open(&key)
                && selected_at.is_none()
                && !Self::upstream_selected(store, id, upstream);
            rows.push(Row::Item(Selection::Branch(key.0, key.1)));
            if closed {
                continue;
            }
            rows.open();
            if let Some(workspace) = store.workspace(id) {
                self.upstream_rows(store, workspace, id, upstream, rows);
            }
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

    /// The words the filter looks for, up to as many as [`found`] can tell apart.
    fn filter_terms(&self, cx: &App) -> Vec<String> {
        (self.filter.read(cx).text().split_whitespace())
            .take(64)
            .map(str::to_lowercase)
            .collect()
    }

    /// Give the rows the keys, the selection drawn solid again.
    fn focus_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// The filter's words changed: its rows show from the top, and once it's empty, the
    /// selection shows where it is in the whole tree.
    fn filter_changed(&mut self, cx: &mut Context<Self>) {
        self.list.scroll_to(ListOffset::default());
        if self.filter.read(cx).text().trim().is_empty() {
            self.reveal_selection = true;
        }
        cx.notify();
    }

    fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            let again = store.selection.as_ref() == Some(&selection);
            store.select(Some(selection), cx);
            if again {
                cx.emit(StoreEvent::Reopened);
            }
        });
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
        // From off the list, a step forward starts at the top and one back at the bottom.
        let at = match current.and_then(|s| items.iter().position(|item| *item == s)) {
            Some(ix) => ix as isize,
            None if delta > 0 => -1,
            None => items.len() as isize,
        };
        let next = (at + delta).clamp(0, items.len() as isize - 1) as usize;
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
            (None, true) => items.iter().find(heads),
            (None, false) => items.iter().rev().find(heads),
        };
        if let Some(next) = next.map(|item| (*item).clone()) {
            self.select(next, window, cx);
        }
    }

    /// Whether `key`'s branch shows its commits: as Settings start branches, unless folded or
    /// opened since.
    fn branch_open(&self, key: &(ProjectId, String)) -> bool {
        self.branches_start_open != self.flipped_branches.contains(key)
    }

    fn set_branch_open(&mut self, key: (ProjectId, String), open: bool) {
        // By hand, it stays as it's left.
        if matches!(&self.click_opened, Some(ClickOpened::Here(opened)) if *opened == key) {
            self.click_opened = None;
        }
        if self.branch_open(&key) != open {
            toggle(&mut self.flipped_branches, key);
        }
    }

    /// Open a branch's commits for a plain click on its row, and fold the ones the last
    /// click opened: a click looks inside a branch, without the list growing by a branch
    /// each time.
    fn open_on_click(&mut self, branch: ClickOpened, cx: &mut Context<Self>) {
        if let Some(previous) = self.click_opened.take_if(|previous| *previous != branch) {
            match previous {
                ClickOpened::Here(key) => self.set_branch_open(key, false),
                ClickOpened::Elsewhere(key) => {
                    if self.mesh_branch_open(&key) {
                        toggle(&mut self.flipped_mesh_branches, key);
                    }
                }
                ClickOpened::Merge(_, id) => {
                    self.open_merges.remove(&id);
                }
            }
        }
        match &branch {
            ClickOpened::Here(key) if !self.branch_open(key) => {
                self.set_branch_open(key.clone(), true);
                self.click_opened = Some(branch);
            }
            ClickOpened::Elsewhere(key) if !self.mesh_branch_open(key) => {
                toggle(&mut self.flipped_mesh_branches, key.clone());
                self.click_opened = Some(branch);
            }
            ClickOpened::Merge(project, id) if self.open_merges.insert(*id) => {
                (self.store).update(cx, |store, cx| store.load_merged(project, *id, cx));
                self.click_opened = Some(branch);
            }
            _ => {}
        }
        cx.notify();
    }

    /// Whether one of the commits only `ref_name`'s remote has, or with `None` the target's
    /// new ones, is selected.
    fn upstream_selected(store: &Store, id: &ProjectId, ref_name: Option<&str>) -> bool {
        matches!(&store.selection, Some(Selection::Upstream(p, r, _))
            if p == id && r.as_deref() == ref_name)
    }

    /// Show the commits remote merge `id` brought in, or fold them, by hand: it stays as it's
    /// left. Folding them over the one selected selects the merge.
    fn toggle_merge(
        &mut self,
        project: &ProjectId,
        ref_name: Option<String>,
        id: CommitId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.click_opened == Some(ClickOpened::Merge(project.clone(), id)) {
            self.click_opened = None;
        }
        if self.open_merges.insert(id) {
            (self.store).update(cx, |store, cx| store.load_merged(project, id, cx));
            return cx.notify();
        }
        self.open_merges.remove(&id);
        let store = self.store.read(cx);
        let inside = matches!(&store.selection, Some(Selection::Upstream(p, r, c))
            if p == project && store.merged_upstream(p, r.as_deref(), *c).is_some_and(|(m, _)| m == id));
        match inside {
            true => self.select(
                Selection::Upstream(project.clone(), ref_name, id),
                window,
                cx,
            ),
            false => cx.notify(),
        }
    }

    /// Whether the commits only `ref_name`'s remote has, or with `None` the target's new
    /// ones, are listed under their heading: once it's opened, and while one is selected.
    fn upstream_open(&self, store: &Store, id: &ProjectId, ref_name: Option<&str>) -> bool {
        self.open_upstream
            .contains(&(id.clone(), ref_name.map(str::to_owned)))
            || Self::upstream_selected(store, id, ref_name)
    }

    /// The heading over the commits only `ref_name`'s remote has, or with `None` the target's
    /// new ones, and when it's open each of them, newest first, as `but status` lists them;
    /// for the target, then how many more its page lists.
    fn upstream_rows(
        &self,
        store: &Store,
        workspace: &Workspace,
        id: &ProjectId,
        ref_name: Option<&str>,
        rows: &mut Rows,
    ) {
        let commits = workspace.upstream(ref_name);
        if commits.is_empty() {
            return;
        }
        let owner = ref_name.map(str::to_owned);
        rows.push(Row::UpstreamHeading(id.clone(), owner.clone()));
        if !self.upstream_open(store, id, ref_name) {
            return;
        }
        // Guides from the heading's chevron and an open merge's, as from a branch's.
        let (heading, merge) = upstream_chevrons(ref_name);
        rows.open_at(heading);
        let selected = match &store.selection {
            Some(Selection::Upstream(p, r, c)) if p == id && *r == owner => Some(*c),
            _ => None,
        };
        for commit in commits {
            rows.push(Row::Item(Selection::Upstream(
                id.clone(),
                owner.clone(),
                commit.id,
            )));
            let Some((merged, _)) = (store.merged.get(&commit.id)).and_then(|m| m.as_ref()) else {
                continue;
            };
            let holds = selected.is_some_and(|c| merged.iter().any(|m| m.id == c));
            if commit.merge && (self.open_merges.contains(&commit.id) || holds) {
                rows.open_at(merge);
                rows.extend(merged.iter().map(|merged| {
                    Row::Item(Selection::Upstream(id.clone(), owner.clone(), merged.id))
                }));
                rows.close();
            }
        }
        let behind = (workspace.target.as_ref()).filter(|_| owner.is_none());
        let more = behind.map_or(0, |target| target.behind.saturating_sub(commits.len()));
        if more > 0 {
            rows.push(Row::UpstreamMore(id.clone(), more));
        }
        rows.close();
    }

    fn expand(&mut self, _: &Expand, _: &mut Window, cx: &mut Context<Self>) {
        match self.store.read(cx).selection.clone() {
            // A folded card opens; an unwatched project is watched.
            Some(Selection::Project(id)) => self.store.update(cx, |store, cx| {
                match store.prefs.folded_projects.contains(id.as_ref()) {
                    true => store.set_project_folded(&id, false, cx),
                    false => store.set_expanded(&id, true, cx),
                }
            }),
            Some(Selection::Branch(project, ref_name)) => {
                self.set_branch_open((project, ref_name), true);
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
            Some(Selection::Mesh(project, id, pick)) => self.expand_mesh(project, id, pick, cx),
            _ => {}
        }
    }

    /// Step out the way a tree does: a commit to its branch, an open branch, stack or worktree
    /// closed, a branch up to its stack or worktree, and anything else up to its project.
    fn collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let selection = store.selection.clone();
        // A commit a remote's merge brought in goes up to the merge.
        if let Some(Selection::Upstream(project, ref_name, id)) = &selection
            && let Some((merge, _)) = store.merged_upstream(project, ref_name.as_deref(), *id)
        {
            let up = Selection::Upstream(project.clone(), ref_name.clone(), merge);
            return self.select(up, window, cx);
        }
        let closes = matches!(&selection, Some(item @ Selection::Branch(project, ref_name))
            if store.branch_for(item).is_some_and(|branch| !branch.commits.is_empty())
                && self.branch_open(&(project.clone(), ref_name.clone())));
        let folds = (self.rows.iter())
            .position(|row| matches!(row, Row::Item(item) if Some(item) == selection.as_ref()))
            .is_some_and(|ix| self.section_open(ix));
        match selection {
            // An open card folds; a folded one stops being watched.
            Some(Selection::Project(id)) => self.store.update(cx, |store, cx| {
                let open = store.project(&id).is_some_and(|p| p.expanded)
                    && !store.prefs.folded_projects.contains(id.as_ref());
                match open {
                    true => store.set_project_folded(&id, true, cx),
                    false => store.set_expanded(&id, false, cx),
                }
            }),
            Some(Selection::Branch(project, ref_name)) if closes => {
                self.set_branch_open((project, ref_name), false);
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
            // The target's new commits go up to the project, below.
            Some(Selection::Upstream(project, Some(ref_name), _)) => {
                self.select(Selection::Branch(project, ref_name), window, cx)
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
            Some(Selection::Mesh(project, id, pick)) => {
                self.collapse_mesh(project, id, pick, window, cx)
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
            None | Some(Row::Gap) => return div().h(px(12.)).into_any_element(),
            Some(Row::Item(item)) => self.render_item(ix, item, inset, window, theme, cx),
            Some(Row::MeshHeading(id)) => self.mesh_heading(id, theme, cx),
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
            // The drop zone is padded, not margined: a margin collapses through the card's
            // ground.
            Some(Row::NewBranch(project)) => {
                let project = project.clone();
                let zone = drop_zone(("new-branch-drop", ix), "New branch", theme)
                    .h(px(30.))
                    .on_drag_move(cx.listener(Self::over_zone))
                    .on_drop(cx.listener(move |this, _: &Cut, _, cx| {
                        // A row still lit says where the pill told the drop goes, as when the
                        // zone slid under a pointer that didn't move.
                        let store = this.store.read(cx);
                        if let Some(Cut::Commit(from, ids)) = &store.dragging
                            && *from == project
                            && store.drop_at.is_none()
                        {
                            let (from, ids) = (from.clone(), ids.clone());
                            this.store
                                .update(cx, |store, cx| store.move_to_new_branch(&from, ids, cx));
                        }
                    }));
                div().py(px(2.)).child(zone).into_any_element()
            }
            Some(Row::UpstreamHeading(project, ref_name)) => {
                self.upstream_heading(ix, project, ref_name.as_deref(), theme, cx)
            }
            Some(Row::UpstreamMore(project, more)) => {
                let page = Selection::Project(project.clone());
                fold_row(("upstream-more", ix), Icon::Kebab, 41., 63., theme)
                    .child(format!("{more} more on its page"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.select(page.clone(), window, cx)
                    }))
                    .into_any_element()
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
            Some(Row::Inactive(project, n, inside)) => {
                self.inactive_heading(project, *n, *inside, theme, cx)
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
        let guides = (self.guides.get(ix).into_iter().flatten()).map(|at| guide(*at, theme));
        // A watched project's card, or the one after them, takes a dragged project card.
        let slot = match &self.rows[ix] {
            _ if self.store.read(cx).prefs.watched_order != WatchedOrder::Manual => None,
            Row::Item(Selection::Project(id))
                if self.store.read(cx).project(id).is_some_and(|p| p.expanded) =>
            {
                Some(CardSlot::Before(id.clone()))
            }
            Row::ProjectsHeading(..) => Some(CardSlot::End),
            _ => None,
        };
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
            .when_some(slot.filter(|_| first), |el, slot| {
                let lit = self.project_drop.as_ref() == Some(&slot);
                el.relative()
                    .child(self.card_drop(slot, cx))
                    // In the middle of the gap between the cards; over the first card's top
                    // edge, which has no gap above it in the list.
                    .when(lit, |el| {
                        let line = insertion_line(true, theme.pop).right(px(10.));
                        el.child(line.when(ix > 0, |line| line.top(px(-6.))))
                    })
            })
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
            (
                Some(Cut::Commit(p, ids) | Cut::CherryPick(p, ids)),
                Selection::Commit(project, commit),
            ) => p == project && ids.contains(commit),
            _ => false,
        };
        let style = RowStyle {
            theme,
            selected,
            // Not while typing in the filter, where the keys go to it.
            solid: selected
                && self.focus.contains_focused(window, cx)
                && !self.filter.focus_handle(cx).is_focused(window),
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
                let tip = Some("Branches not applied in your workspace");
                Some(self.page_row(item, Icon::Branch, "Branches", tip, style, cx))
            }
            Selection::History(_) => {
                let label = "Operations history";
                Some(self.page_row(item, Icon::Clock, label, None, style, cx))
            }
            Selection::Stack(id, bottom) => self.stack_row(ix, item, id, bottom, style, cx),
            Selection::Mesh(..) => self.mesh_row(ix, item, style, cx),
            Selection::Upstream(id, ref_name, commit) => {
                self.upstream_row(item, id, ref_name.as_deref(), commit, style, cx)
            }
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
        let folded = self.store.read(cx).prefs.projects_folded;
        let fold = h_flex()
            .id("projects-fold")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .gap(px(6.))
            .when(total > 0, |el| {
                el.child(
                    small_icon(match folded {
                        true => Icon::ChevronRight,
                        false => Icon::ChevronDown,
                    })
                    .text_color(theme.text_3),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    (this.store).update(cx, |store, cx| {
                        store.set_prefs(|prefs| prefs.projects_folded = !prefs.projects_folded, cx)
                    })
                }))
            })
            .child(section_label(title, theme).truncate())
            .when(total > 0, |el| {
                el.child(faint(count, theme).text_size(px(12.)))
            });
        row.child(fold)
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
        // A watched project whose card is folded to this row.
        let card_folded = expanded && store.project_folded(id);
        let manual = store.prefs.watched_order == WatchedOrder::Manual;
        let hovered = self.watch_hovered.as_ref() == Some(id);
        // Folded, the row sums up the whole card; open, it leaves out what the rows under it
        // say: their uncommitted files, unpushed commits and worktrees.
        let summary = workspace.filter(|ws| ws.editing.is_none()).map(|ws| {
            let folded = |n: usize| if expanded && !card_folded { 0 } else { n };
            let in_worktrees = ws.worktrees.iter().map(|w| w.uncommitted.len());
            (
                folded(ws.uncommitted.len() + in_worktrees.sum::<usize>()),
                folded(ws.unpushed_commits()),
                ws.target.as_ref(),
                folded(ws.worktrees.len()),
            )
        });
        let unread = workspace.is_some_and(|ws| {
            ws.commits().any(|commit| store.is_unread(id, commit))
                || ws
                    .remote_commits()
                    .any(|commit| store.is_unread(id, commit))
        });
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
                        // Every project wears one, the glyph's outline until one's picked, so
                        // their names line up. In an icon's 16 points, an emoji a little smaller
                        // than the text.
                        .child(
                            div()
                                .flex_none()
                                .w(px(16.))
                                .flex()
                                .justify_center()
                                .text_size(px(13.))
                                .child(match ProjectIcon::parse(store.project_icon(id)) {
                                    // Unpicked, it's quiet as the eye beside it, and takes the
                                    // row's ink when it's selected.
                                    ProjectIcon::Glyph(Colour { fill: None, .. }) => {
                                        icon(Icon::Repo)
                                            .size(px(16.))
                                            .text_color(style.muted())
                                            .into_any_element()
                                    }
                                    picked => picked.render(px(16.), theme),
                                }),
                        ),
                )
                // However narrow the navigator, the name stays: what the row sums up gives way
                // first, a whole item at a time from its end, the age before the counts. Each
                // item that doesn't fit wraps to a second line, which is cut off.
                .child(
                    div()
                        .flex_grow(1.)
                        .min_w(px(0.))
                        .truncate()
                        .when(expanded, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                        .child(entry.project.name.clone()),
                )
                .child(
                    h_flex()
                        .flex_shrink(1000.)
                        .min_w(px(0.))
                        .h(px(16.))
                        .flex_wrap()
                        .overflow_hidden()
                        .gap(px(8.))
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
                                let tip =
                                    format!("{} behind {}", plural(t.behind, "commit"), t.name);
                                // Each merge counts once, as the project's page lists them.
                                let tip = match t.commits_behind > t.behind {
                                    true => format!(
                                        "{tip} ({} with what they merged)",
                                        t.commits_behind
                                    ),
                                    false => tip,
                                };
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
                        }),
                )
                // Only a watched project's: an unwatched one updates from its page or its menu,
                // which keeps the list of the others quiet.
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
                // A watched project's card folds to this row, still watched.
                .when(expanded, |el| {
                    let project = id.clone();
                    let step_color = style.quiet();
                    el.child(
                        div()
                            .id(ElementId::Name(format!("fold-project-{id}").into()))
                            .flex_none()
                            .size(px(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .hover(|el| el.bg(theme.hover()))
                            .child(
                                small_icon(match card_folded {
                                    true => Icon::ChevronRight,
                                    false => Icon::ChevronDown,
                                })
                                .text_color(step_color),
                            )
                            .tooltip(Tooltip::text(match card_folded {
                                true => "Show the project",
                                false => "Fold the project, still watching it",
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.store.update(cx, |store, cx| {
                                    store.set_project_folded(&project, !card_folded, cx)
                                });
                            })),
                    )
                })
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    // A watched project's card folds, as a folder does; another starts being
                    // watched.
                    if event.click_count() == 2 {
                        this.store.update(cx, |store, cx| match expanded {
                            true => store.set_project_folded(&toggle_id, !card_folded, cx),
                            false => store.set_expanded(&toggle_id, true, cx),
                        });
                    }
                }))
                // Only the manual order moves by hand.
                .when(expanded && manual, |el| self.reorderable(el, id, entry, cx))
                .into_any_element(),
        )
    }

    /// While something dragged is near the list's top or bottom edge, or past it, scroll the
    /// list that way, faster the closer it gets, so it can reach rows out of view.
    fn scroll_near_edge(
        &mut self,
        at: Point<Pixels>,
        list: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        const EDGE: f32 = 48.;
        const FASTEST: f32 = 16.;
        let closeness = |gap: Pixels| 1. - (f32::from(gap) / EDGE).max(0.);
        let (from_top, from_bottom) = (at.y - list.top(), list.bottom() - at.y);
        self.edge_scroll = match at.x >= list.left() && at.x <= list.right() {
            false => 0.,
            true if from_top < px(EDGE) => -FASTEST * closeness(from_top),
            true if from_bottom < px(EDGE) => FASTEST * closeness(from_bottom),
            true => 0.,
        };
        if self.edge_scroll == 0. || self.edge_scrolling.is_some() {
            return;
        }
        self.edge_scrolling = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let step = this.update_in(cx, |this, window, cx| {
                    let speed = this.edge_scroll;
                    if speed == 0. || !cx.has_active_drag() {
                        return false;
                    }
                    let offset = -this.list.scroll_px_offset_for_scrollbar().y;
                    let room = match speed < 0. {
                        true => offset > px(0.),
                        false => offset < this.list.max_offset_for_scrollbar().y,
                    };
                    if room {
                        this.list.scroll_by(px(speed));
                        cx.notify();
                        // The rows moved under a pointer that stays put: once they're drawn
                        // where they are now, have the drag land again where it is, as moving
                        // the pointer would. A next frame's callbacks run before it's drawn,
                        // so the frame after the one that draws them.
                        window.on_next_frame(|window, _| {
                            window.on_next_frame(|window, cx| {
                                let event = MouseMoveEvent {
                                    position: window.mouse_position(),
                                    pressed_button: Some(MouseButton::Left),
                                    modifiers: window.modifiers(),
                                };
                                window.dispatch_event(PlatformInput::MouseMove(event), cx);
                            })
                        });
                    }
                    true
                });
                if !step.unwrap_or(false) {
                    break;
                }
            }
            (this.update(cx, |this, _| this.edge_scrolling = None)).ok();
        }));
    }

    /// Let a watched project's heading drag its card among the others (see [`Self::card_drop`]).
    fn reorderable(
        &self,
        el: Stateful<Div>,
        id: &ProjectId,
        entry: &ProjectEntry,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let (name, navigator) = (entry.project.name.clone(), cx.entity().downgrade());
        el.on_drag(ProjectDrag(id.clone()), move |_, offset, _, cx| {
            let pill = cx.new(|_| ProjectPill {
                name: name.clone().into(),
                offset,
            });
            // A drop anywhere else, or none, leaves the last line drawn otherwise.
            let navigator = navigator.clone();
            cx.observe_release(&pill, move |_, cx| {
                (navigator.update(cx, |this, cx| {
                    this.project_drop = None;
                    cx.notify()
                }))
                .ok();
            })
            .detach();
            pill
        })
    }

    /// Where a dragged project card goes before the card that `slot`'s heading starts: over
    /// the heading and the gap above the card, laid over both. It takes the drop, and says
    /// where it would land only when the card would move.
    fn card_drop(&self, slot: CardSlot, cx: &Context<Self>) -> Div {
        let (over, onto) = (slot.clone(), slot);
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(-10.))
            .bottom_0()
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<ProjectDrag>, _, cx| {
                    let next = if event.bounds.contains(&event.event.position) {
                        let moves = slot_moves(this.store.read(cx), &event.drag(cx).0, &over);
                        moves.then(|| over.clone())
                    } else if this.project_drop.as_ref() == Some(&over) {
                        None
                    } else {
                        return;
                    };
                    if this.project_drop != next {
                        this.project_drop = next;
                        cx.notify();
                    }
                }),
            )
            .on_drop(cx.listener(move |this, drag: &ProjectDrag, _, cx| {
                let landed = this.project_drop.take().is_some();
                let before = match &onto {
                    CardSlot::Before(id) => Some(id),
                    CardSlot::End => None,
                };
                if landed {
                    (this.store).update(cx, |store, cx| store.move_watched(&drag.0, before, cx));
                }
                cx.notify();
            }))
    }

    /// A quiet row at the end of a project that opens one of its pages.
    fn page_row(
        &self,
        item: &Selection,
        glyph: Icon,
        label: &'static str,
        tip: Option<&'static str>,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        style
            .row(format!("{label}-{}", item.project()), 6.)
            .text_color(style.muted())
            .child(icon(glyph).text_color(style.muted()))
            .child(div().flex_1().child(label))
            .when_some(tip, |el, tip| el.tooltip(Tooltip::text(tip)))
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
        let uncommitted = store.uncommitted_shown(id, name.as_deref(), uncommitted);
        let branches = section_branches(workspace, name.as_ref());
        let unpushed_n = unpushed(branches.iter().copied());
        let unread = (branches.iter()).any(|branch| store.branch_unread(id, branch));
        // Open even when folded while the selection's in it.
        let open = self.section_open(ix);
        // Written to in the last two minutes, likely by the agent working in it.
        let active = worktree.is_some_and(|wt| wt.last_activity().is_some_and(just_now));
        let toggle = {
            let (project, name) = (id.clone(), name.clone());
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_section(&project, name.clone(), open, window, cx)
            }
        };
        let folds = !branches.is_empty();
        let step = self.fold_step(
            ("worktree-fold", ix),
            folds.then_some(open),
            style.quiet(),
            cx,
            toggle.clone(),
        );
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
            // Its own width, so the quiet "uncommitted" gives way before the name does.
            .child(
                div()
                    .id(("worktree-name", ix))
                    .flex_grow(1.)
                    .min_w(px(0.))
                    .truncate()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .tooltip(Tooltip::text(path.display().to_string()))
                    .child(label),
            )
            // As a branch says it's empty, the quietest meta ahead of the line counts. Laid
            // out from the right, so in a narrow navigator it's what wraps out of sight first.
            .child(
                h_flex()
                    .flex_row_reverse()
                    .flex_wrap()
                    .flex_shrink(1000.)
                    .min_w(px(0.))
                    .h(px(16.))
                    .overflow_hidden()
                    .gap(px(8.))
                    .children(
                        change_counts(&uncommitted, style)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev(),
                    )
                    .when(!uncommitted.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(px(11.))
                                .text_color(style.quiet())
                                .child("uncommitted"),
                        )
                    }),
            )
            .when(!open, |el| {
                el.children(style.unpushed(unpushed_n))
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
            .when(folds, |el| el.on_click(double_click(toggle, cx)));
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
        // Find the stack by its bottom branch as drawn, after pending deletes.
        let branches = (store.workspace(id)?.lanes())
            .map(|lane| store.after_deletes(id, lane))
            .find(|lane| lane.last().is_some_and(|branch| branch.ref_name == bottom))?;
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
        let toggle = {
            let (project, bottom) = (id.clone(), bottom.to_owned());
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_stack(&project, &bottom, open, window, cx)
            }
        };
        let step = self.fold_step(
            ("stack-fold", ix),
            Some(open),
            style.quiet(),
            cx,
            toggle.clone(),
        );
        // As ui-react's StackCaption: quiet, so it doesn't read as one more branch, with
        // "Stack" a step above the rest.
        let (word, quiet) = match style.solid {
            true => (theme.text_inverted, style.muted()),
            false => (theme.text_2, theme.text_3),
        };
        Some(
            style
                .row(format!("stack-{id}-{bottom}"), 6.)
                .aria_label(label)
                .aria_expanded(open)
                .child(step)
                .child(icon(Icon::Stack).text_color(quiet))
                .child(
                    h_flex()
                        .id(("stack-name", ix))
                        .flex_1()
                        .min_w(px(0.))
                        .gap(px(6.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .tooltip(Tooltip::text(format!("Stack: {}", names.join(" ← "))))
                        .child(count_badge(branches.len(), theme).when(style.solid, |el| {
                            el.bg(theme.text_inverted.opacity(0.15)).text_color(quiet)
                        }))
                        .child(div().flex_none().text_color(word).child("Stack"))
                        // Folded, its top branch, the one that gets pushed, tells it from
                        // another stack as tall, and gives way first; open, that row is right
                        // under it.
                        .children(branches.first().filter(|_| !open).map(|top| {
                            div()
                                .min_w(px(0.))
                                .truncate()
                                .text_color(quiet)
                                .child(store.label_of(id, top).to_owned())
                        })),
                )
                .when(!open, |el| {
                    el.children(style.unpushed(unpushed_n))
                        .children(latest.map(|at| {
                            div()
                                .flex_none()
                                .text_size(px(11.))
                                .text_color(style.muted())
                                .child(relative_time(at))
                        }))
                })
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .on_click(double_click(toggle, cx))
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
            (Some(review), _) => review::state_glyph(store.review_state(id, review), theme),
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
        let clicked = ClickOpened::Here(key.clone());
        let closed = !self.branch_open(&key) && !branch.commits.is_empty();
        // A selected commit keeps its branch open, as does one only its remote has.
        let holds_selected = matches!(&store.selection, Some(Selection::Commit(p, c))
            if p == id && branch.commits.iter().any(|commit| commit.id == *c))
            || Self::upstream_selected(store, id, Some(ref_name));
        let open = !closed || holds_selected;
        // As ui-react's SidebarRow: a chevron in a step of its own folds the commits away; a
        // branch without any has a dot there, keeping it in line with those that fold.
        let toggle = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            this.set_branch_open(key.clone(), !open);
            // Folding it over its selected commit takes the selection up to it, as ← does.
            match open && holds_selected {
                true => {
                    let (project, ref_name) = key.clone();
                    this.select(Selection::Branch(project, ref_name), window, cx)
                }
                false => cx.notify(),
            }
        };
        let folds = !branch.commits.is_empty();
        let step = self.fold_step(
            ElementId::Name(format!("fold-{id}-{ref_name}").into()),
            folds.then_some(open),
            style.quiet(),
            cx,
            toggle.clone(),
        );
        // An unread commit marks its branch with a dot too, folded or not.
        let unread = store.branch_unread(id, branch);
        let review = (branch.review.as_ref()).map(|r| (r, store.review_state(id, r)));
        Some(
            style
                .row(format!("branch-{id}-{ref_name}"), 6.)
                .aria_label(format!("Branch {name}"))
                .child(step)
                .child(icon(glyph).text_color(style.meta(glyph_color)))
                .child(
                    branch_name(name, review, &style)
                        .id(ElementId::Name(format!("name-{id}-{ref_name}").into()))
                        .flex_1()
                        .when_some(tip, |el, tip| el.tooltip(Tooltip::text(tip))),
                )
                // As ui-react's empty MetaCount, the quietest meta, ahead of any counts.
                .when(branch.commits.is_empty(), |el| {
                    let quiet = style.quiet();
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
                // Folded, how many only its remote has; open, its On origin/… row says.
                .when(!open, |el| {
                    el.children(meta_count(
                        Icon::ArrowDown,
                        branch.incoming.len(),
                        style.meta(theme.warn),
                        format!(
                            "{} only on {}",
                            plural(branch.incoming.len(), "commit"),
                            branch.remote.as_deref().unwrap_or("the remote")
                        ),
                    ))
                })
                .children(style.unpushed(unpushed))
                .when(store.pushing(id, ref_name), |el| {
                    el.child(faint("pushing", theme))
                })
                // Folded, how many commits it holds; open, they're listed under it.
                .when(!open, |el| {
                    let n = branch.commits.len();
                    el.children(meta_count(
                        Icon::Commit,
                        n,
                        style.muted(),
                        plural(n, "commit"),
                    ))
                })
                .when(unread, |el| el.child(dot(style.meta(theme.pop))))
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                // A click looks inside, opening its commits; the chevron, ← and → fold them.
                .when(folds, |el| el.on_click(open_on_click(clicked, cx)))
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

    /// A commit only a remote has, among its branch's commits or under the project: a ↓
    /// where a commit has its dot, and its title quieter unless it's unread, as only a
    /// branch's are. It isn't dragged or edited here.
    fn upstream_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        ref_name: Option<&str>,
        commit_id: &CommitId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let workspace = store.workspace(id)?;
        let listed = (workspace.upstream(ref_name).iter()).find(|c| c.id == *commit_id);
        // One a merge listed brought in sits a step in under it.
        let (title, time, merge, nested) = match listed {
            Some(commit) => (commit.title.clone(), commit.time, commit.merge, false),
            None => {
                let (_, commit) = store.merged_upstream(id, ref_name, *commit_id)?;
                (commit.title.clone(), commit.time, false, true)
            }
        };
        let title: SharedString = title.into();
        let unread = listed.is_some_and(|commit| ref_name.is_some() && store.is_unread(id, commit));
        let warn = style.meta(style.theme.warn);
        // A merge folds the commits it brought in behind a chevron in its ↓'s place.
        let glyph = match merge {
            false => small_icon(Icon::ArrowDown)
                .text_color(warn)
                .into_any_element(),
            true => {
                let open = self.open_merges.contains(commit_id)
                    || matches!(&store.selection, Some(Selection::Upstream(_, _, c))
                        if store.merged_upstream(id, ref_name, *c).is_some_and(|(m, _)| m == *commit_id));
                let (project, owner, merge_id) =
                    (id.clone(), ref_name.map(str::to_owned), *commit_id);
                div()
                    .id(ElementId::Name(
                        format!("merge-fold-{}", commit_id.0).into(),
                    ))
                    .flex_none()
                    .size(px(16.))
                    .mx(px(-2.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .hover(|el| el.bg(style.theme.hover()))
                    .child(
                        small_icon(match open {
                            true => Icon::ChevronDown,
                            false => Icon::ChevronRight,
                        })
                        .text_color(warn),
                    )
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        this.toggle_merge(&project, owner.clone(), merge_id, window, cx)
                    }))
                    .into_any_element()
            }
        };
        Some(
            style
                // Under its heading's words, as a branch's commits are under its name.
                .row(
                    format!("upstream-{id}-{}", commit_id.0),
                    upstream_indent(ref_name) + if nested { STACK_STEP } else { 0. },
                )
                .gap(px(10.))
                .aria_label(title.clone())
                .child(glyph)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .map(|el| match unread {
                            true => el.font_weight(gpui::FontWeight::SEMIBOLD),
                            false => el.text_color(style.muted()),
                        })
                        .child(title),
                )
                .when(unread, |el| el.child(dot(style.meta(style.theme.pop))))
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(style.muted())
                        .child(relative_time(time)),
                )
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                // A click on a merge opens the commits it brought in, as on a branch.
                .when(merge, |el| {
                    let clicked = ClickOpened::Merge(id.clone(), *commit_id);
                    el.on_click(open_on_click(clicked, cx))
                })
                .into_any_element(),
        )
    }

    /// Over `project`'s `n` inactive worktrees, as Network worktrees is over its checkouts
    /// elsewhere: a click lists or folds them, and folded, it says how many. The selection in
    /// one keeps them listed, and folding them over it takes it up to the project.
    fn inactive_heading(
        &self,
        project: &ProjectId,
        n: usize,
        inside: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let key = project.to_string();
        let open = inside || self.store.read(cx).prefs.shown_inactive.contains(&key);
        let project = project.clone();
        h_flex()
            .id(ElementId::Name(
                format!("inactive-heading-{project}").into(),
            ))
            .h(px(ROW_HEIGHT))
            .mx(px(3.))
            .pl(px(14.))
            .pr(px(12.))
            .gap(px(6.))
            .child(step(Some(open), theme.text_3))
            .child(small_icon(Icon::Clock).text_color(theme.text_3))
            .child(section_label("Inactive worktrees", theme))
            .when(!open, |el| el.child(faint(n.to_string(), theme)))
            .tooltip(Tooltip::text(
                "Nothing uncommitted, and no commits or checkouts in 12 hours",
            ))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let key = key.clone();
                this.store.update(cx, |store, cx| {
                    store.set_prefs(
                        |prefs| {
                            match open {
                                true => prefs.shown_inactive.remove(&key),
                                false => prefs.shown_inactive.insert(key),
                            };
                        },
                        cx,
                    )
                });
                match open && inside {
                    true => this.select(Selection::Project(project.clone()), window, cx),
                    false => cx.notify(),
                }
            }))
            .into_any_element()
    }

    /// Over the commits only branch `ref_name`'s remote has, or with `None` the target's new
    /// ones, a section heading as Network worktrees has: where they are, how many while
    /// folded and whether any is unread, which a click lists or folds, and for a branch, Pull.
    fn upstream_heading(
        &self,
        ix: usize,
        project: &ProjectId,
        ref_name: Option<&str>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let workspace = store.workspace(project);
        let open = self.upstream_open(store, project, ref_name);
        let (remote, count) = match ref_name {
            Some(ref_name) => {
                let branch = workspace.and_then(|ws| ws.find_branch(ref_name));
                (
                    branch.and_then(|b| b.remote.clone()),
                    branch.map_or(0, |b| b.incoming.len()),
                )
            }
            None => {
                let target = workspace.and_then(|ws| ws.target.as_ref());
                (
                    target.map(|t| t.name.clone()),
                    target.map_or(0, |t| t.behind),
                )
            }
        };
        // Only a branch's are marked read.
        let unread = (ref_name.zip(workspace)).is_some_and(|(ref_name, ws)| {
            (ws.upstream(Some(ref_name)).iter()).any(|commit| store.is_unread(project, commit))
        });
        let pull = (ref_name.filter(|r| workspace.is_some_and(|ws| ws.can_pull(r))))
            .map(|r| (project.clone(), r.to_owned()));
        let key = (project.clone(), ref_name.map(str::to_owned));
        let hover = theme.hover();
        h_flex()
            .id(("upstream-heading", ix))
            .h(px(ROW_HEIGHT))
            .mx(px(3.))
            // Its chevron under a branch's commit glyphs, or the project's worktree chevrons.
            .pl(px(upstream_chevrons(ref_name).0 - 10.))
            .pr(px(8.))
            .gap(px(6.))
            .rounded(px(4.))
            .hover(move |el| el.bg(hover))
            .child(step(Some(open), theme.text_3))
            .child(
                section_label(
                    match remote {
                        Some(remote) => format!("On {remote}"),
                        None => "On the remote".to_owned(),
                    },
                    theme,
                )
                .flex_1()
                .min_w(px(0.))
                .truncate(),
            )
            .when(!open, |el| {
                let tip = plural(count, "commit");
                el.children(meta_count(Icon::ArrowDown, count, theme.warn, tip))
                    .when(unread, |el| el.child(dot(theme.pop)))
            })
            .children(pull.map(|(project, ref_name)| {
                button(ElementId::Name(format!("pull-{ix}").into()), "Pull")
                    .small()
                    .style(ButtonStyle::Gray)
                    .tooltip("Bring them in, with this branch's own commits on top")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let ref_name = ref_name.clone();
                        (this.store).update(cx, |store, cx| {
                            store.pull_remote(&project, ref_name, None, cx)
                        })
                    }))
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let (project, ref_name) = key.clone();
                let selected =
                    Self::upstream_selected(this.store.read(cx), &project, ref_name.as_deref());
                match open {
                    true => this.open_upstream.remove(&key),
                    false => this.open_upstream.insert(key.clone()),
                };
                // Folding them over the selected one takes the selection up, as a branch's
                // chevron does.
                match (open && selected, ref_name) {
                    (true, Some(ref_name)) => {
                        this.select(Selection::Branch(project, ref_name), window, cx)
                    }
                    (true, None) => this.select(Selection::Project(project), window, cx),
                    (false, _) => cx.notify(),
                }
            }))
            .into_any_element()
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
            // it a drop lands, and the one it leaves lets go of it. What's dragged is the
            // store's copy, which follows it as agents rewrite it; GPUI's stays as it began.
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<Cut>, window, cx| {
                    let (at, bounds) = (event.event.position, event.bounds);
                    let store = this.store.read(cx);
                    let next = if bounds.contains(&at) {
                        let y = (at.y - bounds.top()) / bounds.size.height;
                        (store.dragging.as_ref()).and_then(|payload| {
                            let place = place_at(payload, &move_target, y, window.modifiers());
                            Transfer::resolve(payload, &move_target, place, store)
                                .map(|_| (move_target.clone(), place))
                        })
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
            .can_drop(move |_, _, cx| {
                let store = store.read(cx);
                (store.dragging.as_ref()).is_some_and(|payload| {
                    (store.drop_at.as_ref()).is_some_and(|(at, place)| {
                        *at == can_drop_target
                            && Transfer::resolve(payload, at, *place, store).is_some()
                    })
                })
            })
            .on_drop(cx.listener(move |this, _: &Cut, window, cx| {
                let store = this.store.read(cx);
                let (Some(payload), Some((_, place))) =
                    (store.dragging.clone(), store.drop_at.clone())
                else {
                    return;
                };
                (this.app)
                    .update(cx, |app, cx| {
                        app.transfer(&payload, &target, place, window, cx)
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
    crate::ui::now() - at <= ACTIVE_WITHIN.as_secs() as i64
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
        && (worktree.updated_at)
            .is_some_and(|at| crate::ui::now() - at > INACTIVE_AFTER.as_secs() as i64)
}

/// A branch as the navigator names it, in semibold as but.dev's: its pull request's title once
/// the forge cache has one, toned by the request's state, or else `name`. Drawn from what the
/// last load read, so no row waits on the network, and offline the cached title stays.
fn branch_name(name: &str, review: Option<(&Review, ReviewState)>, style: &RowStyle) -> Div {
    let theme = style.theme;
    let el = div()
        .min_w(px(0.))
        .truncate()
        .font_weight(gpui::FontWeight::SEMIBOLD);
    match review.filter(|(review, _)| !review.title.is_empty()) {
        Some((review, state)) => el
            .text_color(match style.solid {
                true if state == ReviewState::Open => style.meta(theme.text_1),
                true => style.muted(),
                false => review::title_color(state, theme),
            })
            .child(review.title.clone()),
        None => el.child(name.to_owned()),
    }
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
        // A branch's heading: its lower half puts the dragged branch below it, its upper half
        // stacks it on top.
        (Cut::Branch(..), Selection::Branch(..)) if y > 0.5 => Place::Below,
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
            first_hit,
            ..
        } = self.build_rows(cx);
        self.insets = insets;
        self.guides = guides;
        self.sync_list(rows, window, cx);
        let theme = Theme::get(cx).clone();
        self.first_hit = first_hit;
        let store = self.store.read(cx);
        let empty = store.projects_loaded && store.projects.is_empty();
        let filter_focused = self.filter.focus_handle(cx).is_focused(window);
        let filtering = !self.filter.read(cx).text().is_empty();

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
                field(&self.filter, filter_focused, &theme)
                    .flex_1()
                    .min_w(px(0.))
                    .py(px(3.)),
            )
            .when(filtering, |el| {
                el.child(
                    icon_button("clear-filter", Icon::Cross)
                        .small()
                        .tooltip("Clear the filter")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.filter.update(cx, |input, cx| input.set_text("", cx));
                            this.filter_changed(cx);
                        })),
                )
            })
            .children(self.update_button(cx))
            .child(self.view_button(cx))
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
            .on_action(cx.listener(|this, _: &FocusFilter, window, cx| {
                this.filter
                    .update(cx, |input, cx| input.select_all_text(cx));
                window.focus(&this.filter.focus_handle(cx), cx);
                cx.notify();
            }))
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
                    el.child(
                        scrolled(rows, "navigator-scrollbar", scroll, &theme)
                            .on_drag_move(cx.listener(|this, event: &DragMoveEvent<Cut>, w, cx| {
                                this.scroll_near_edge(event.event.position, event.bounds, w, cx)
                            }))
                            .on_drag_move(cx.listener(
                                |this, event: &DragMoveEvent<ProjectDrag>, w, cx| {
                                    this.scroll_near_edge(event.event.position, event.bounds, w, cx)
                                },
                            )),
                    )
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

/// Which of `terms`, lowercase, any of `texts` holds, as bits by the term's place.
fn found(terms: &[String], texts: &[&str]) -> u64 {
    (terms.iter().enumerate())
        .filter(|(_, term)| texts.iter().any(|text| holds(text, term)))
        .fold(0, |bits, (ix, _)| bits | 1 << ix)
}

/// Whether a row whose own text has the terms in `own`, under rows with those in `held`,
/// matches: every term found, and one at least in its own text.
fn matches(terms: &[String], held: u64, own: u64) -> bool {
    own != 0 && held | own == u64::MAX >> (64 - terms.len())
}

/// Whether `text` holds `term` (lowercase), in any case; without allocating for ASCII text,
/// as most names and titles are, since the filter looks through every commit on each key.
fn holds(text: &str, term: &str) -> bool {
    match text.is_ascii() {
        true => (text.as_bytes().windows(term.len()))
            .any(|window| window.eq_ignore_ascii_case(term.as_bytes())),
        false => text.to_lowercase().contains(term),
    }
}

/// A watched project's card being dragged, by its heading.
struct ProjectDrag(ProjectId);

/// Where a dragged project card goes: before a watched project's card, or after them all.
#[derive(Clone, PartialEq)]
enum CardSlot {
    Before(ProjectId),
    End,
}

/// Whether dropping watched project `id` at `slot` moves its card: not right before or after
/// itself, where it already is.
fn slot_moves(store: &Store, id: &ProjectId, slot: &CardSlot) -> bool {
    let watched: Vec<&ProjectId> = (store.projects.iter())
        .filter(|entry| entry.expanded)
        .map(|entry| &entry.id)
        .collect();
    let Some(from) = watched.iter().position(|other| *other == id) else {
        return false;
    };
    let to = match slot {
        CardSlot::Before(before) => watched.iter().position(|other| *other == before),
        CardSlot::End => Some(watched.len()),
    };
    to.is_some_and(|to| to != from && to != from + 1)
}

/// What a dragged project card shows under the pointer.
struct ProjectPill {
    name: SharedString,
    offset: Point<Pixels>,
}

impl Render for ProjectPill {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        drag_pill(
            self.offset,
            Icon::Eye,
            self.name.clone(),
            None,
            Theme::get(cx),
        )
    }
}

#[cfg(test)]
mod filter_tests {
    use super::{found, holds, matches};

    #[test]
    fn a_row_matches_with_what_holds_it() {
        let terms = ["shop".to_owned(), "cart".to_owned()];
        let project = found(&terms, &["shop"]);
        assert!(
            !matches(&terms, 0, project),
            "the project alone lacks \"cart\""
        );
        let branch = found(&terms, &["feature/Cart"]);
        assert!(
            matches(&terms, project, branch),
            "the branch has \"cart\", in any case, and its project \"shop\""
        );
        let commit = found(&terms, &["Keep items in order"]);
        assert!(
            !matches(&terms, project | branch, commit),
            "a commit needs a word of its own, not only its branch's"
        );
        assert!(
            holds("Über die Karte", "über"),
            "non-ASCII text folds case too"
        );
    }
}
