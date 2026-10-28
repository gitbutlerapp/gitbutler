//! The left column: every project as a card, each listing its uncommitted changes,
//! stacks, branches and commits as one tree of rows.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, Context, CursorStyle, Div, DragMoveEvent, ElementId, EmptyView,
    Entity, FocusHandle, Focusable, Hsla, ListAlignment, ListOffset, ListState, MouseButton,
    MouseDownEvent, Pixels, Point, ScrollWheelEvent, SharedString, Stateful, Subscription, Task,
    Window, actions, div, list, prelude::*, relative,
};

use crate::app::{
    AddProject, AppHandle, Collapse, Confirm, Expand, SelectNext, SelectPrevious, UpdateFromTarget,
};
use crate::backend::{
    Branch, Checkout, CommitId, CommitState, FileChange, FullName, PushState, Review, ReviewState,
    Target,
};
use crate::detail::{ChecksChanged, Detail};
use crate::overlays::MenuItem;
use crate::state::{
    Cut, Pane, Prefs, ProjectId, Section, Selection, Store, StoreEvent, WatchedOrder,
};
use crate::theme::Theme;
use crate::transfer::drag_preview;
use crate::ui::emoji::{Colour, ProjectIcon};
use crate::ui::scrollbar::{Scroll, scrollbar, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, ROW_HEIGHT, Tooltip, above_drag, ago, button, commit_title,
    count_badge, dot, dp, faint, field, fixed, fold_button, fold_glyph, fold_row, glow_once, guide,
    h_flex, icon, icon_button, list_row, meta_count, plural, pulsing, relative_time, resize_handle,
    review, section_label, small_icon, v_flex,
};

mod account;
mod drop;
mod menu;
mod mesh;
mod order;
mod rename;
mod rows;

use drop::insertion_line;
use order::{CardSlot, ProjectDrag};
use rows::{
    COMMIT_INDENT, Drawer, GUIDE_AT, ROW_LEFT, Row, Rows, in_inactive, inactive_open, last_active,
    within,
};

/// How tall the drawer's handle is, level with the commit footer beside it, and the space
/// under its rows.
const DRAWER_BAR_HEIGHT: f32 = crate::detail::FOOTER_HEIGHT;
const DRAWER_PADDING: f32 = 6.;

/// The most of the column the open drawer takes until it's dragged, and how little and how
/// much dragging it can make that.
const DRAWER_SHARE: f32 = 0.45;
const DRAWER_SHARES: (f32, f32) = (0.15, 0.85);

/// The open drawer's top edge being dragged, to resize it.
struct DrawerResize;

/// How far a worktree's or a stack's rows sit in from its heading, as ui-react's SidebarRow
/// steps them: their chevrons' middle under the heading's 16-point glyph's, the chevron's step,
/// 14, the gap, 8, and half the glyph past the chevron's half.
const STEP: f32 = 14. + 8. + 1.;

/// The eye of a project the app watches, or struck through for one it doesn't.
fn watch_glyph(watched: bool) -> gpui::Svg {
    let eye = match watched {
        true => Icon::Eye,
        false => Icon::EyeOff,
    };
    gpui::svg().path(eye.path()).size(dp(14.)).flex_none()
}

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

/// A project's branch by its full ref, `None` for an unnamed one.
type BranchKey = (ProjectId, Option<FullName>);

/// A branch, here or on another machine, whose commits a click on its row opens.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ClickOpened {
    Here(BranchKey),
    Elsewhere(crate::state::MeshBranch),
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
    /// A commit that just showed up, not selected: see [`crate::ui::glow_once`].
    glow: bool,
}

/// How far a remote's commits sit in: the target's as a branch's commits do, a branch's a
/// step in from their heading, which is under the branch's commit glyphs.
fn upstream_indent(ref_name: Option<&FullName>) -> f32 {
    COMMIT_INDENT + if ref_name.is_some() { STEP } else { 0. }
}

/// Where the middles of a remote's heading's chevron and of its merges' chevrons sit: the
/// target's as the worktree sections' headings have theirs, a branch's under its commit glyphs.
fn upstream_chevrons(ref_name: Option<&FullName>) -> (f32, f32) {
    // The middle of a 12-point glyph set in by `indent`, as a commit's is.
    let glyph = |indent: f32| ROW_LEFT + indent + 6.;
    let heading = match ref_name {
        Some(_) => glyph(COMMIT_INDENT),
        None => GUIDE_AT,
    };
    (heading, glyph(upstream_indent(ref_name)))
}

/// A foldable row's click handler: a double-click runs `toggle`, folding or unfolding the
/// row as its chevron does (see [`Navigator::double_clicked`]), unless the filter lists what
/// it finds.
fn double_click(
    toggle: impl Fn(&mut Navigator, &mut Window, &mut Context<Navigator>) + 'static,
    cx: &Context<Navigator>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |this, event: &ClickEvent, window, cx| {
        if this.double_clicked(event) && !this.filtering(cx) {
            toggle(this, window, cx)
        }
    })
}

/// A branch row's click handler: a double-click runs `toggle`, as [`double_click`] does.
/// With Open branches on click on in Settings, a plain click opens its commits, as
/// [`Navigator::open_on_click`] does; one with ⌘ or ⇧, picking commits, doesn't, and nor
/// does any while the filter lists what it finds.
fn click_opens(
    branch: ClickOpened,
    toggle: impl Fn(&mut Navigator, &mut Window, &mut Context<Navigator>) + 'static,
    cx: &Context<Navigator>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |this, event: &ClickEvent, window, cx| {
        if this.double_clicked(event) && !this.filtering(cx) {
            return toggle(this, window, cx);
        }
        let held = event.modifiers();
        let on = this.store.read(cx).prefs.open_branch_on_click;
        if on && event.click_count() == 1 && !held.secondary() && !held.shift && !this.filtering(cx)
        {
            this.click_just_opened = !this.is_open(&branch);
            this.open_on_click(branch.clone(), cx)
        }
    })
}

impl RowStyle<'_> {
    fn row(&self, id: String, indent: f32) -> Stateful<Div> {
        let theme = self.theme;
        list_row(ElementId::Name(id.into()), self.selected, theme)
            .mx(dp(3.))
            .pl(dp(8. + indent + self.inset))
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
            self.theme.muted_on_solid()
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

    /// How many commits a folded row holds, when it holds any.
    fn commits(&self, n: usize) -> Option<Stateful<Div>> {
        meta_count(Icon::Commit, n, self.muted(), plural(n, "commit"))
    }

    /// How many files are uncommitted, when any are.
    fn files(&self, n: usize) -> Option<Stateful<Div>> {
        let color = self.meta(self.theme.warn);
        meta_count(Icon::File, n, color, plural(n, "uncommitted file"))
    }

    /// Meaningful colours wash out on the dark selection ground; keep them legible.
    fn meta(&self, color: Hsla) -> Hsla {
        if self.solid {
            self.theme.text_inverted
        } else {
            color
        }
    }

    /// How long ago `at` (seconds since the epoch) was, at a row's end.
    fn age(&self, at: i64) -> Div {
        div()
            .flex_none()
            .text_size(dp(11.))
            .text_color(self.muted())
            .child(relative_time(at))
    }

    /// A commit's row, `indent` in: `glyph`, its title, and a dot while it's unread. The dot
    /// alone says it's unread; only a missing title is quieter.
    fn commit_line(
        &self,
        id: String,
        indent: f32,
        glyph: impl IntoElement,
        title: SharedString,
        unread: bool,
    ) -> Stateful<Div> {
        self.row(id, indent)
            .gap(dp(10.))
            .aria_label(title.clone())
            // Under its row's id, which goes by the commit.
            .when(self.glow, |el| {
                el.relative().child(glow_once("glow", self.theme))
            })
            .child(glyph)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(title.is_empty(), |el| el.text_color(self.muted()))
                    .child(commit_title(&title)),
            )
            .when(unread, |el| el.child(dot(self.meta(self.theme.pop))))
    }
}

pub struct Navigator {
    app: AppHandle,
    store: Entity<Store>,
    detail: Entity<Detail>,
    focus: FocusHandle,
    /// The rows as last drawn, which the list draws as they scroll into view.
    rows: Vec<Row>,
    /// How far in each row sits.
    insets: Vec<f32>,
    /// Where each row's guides run.
    guides: Vec<Vec<f32>>,
    pub(crate) list: ListState,
    /// The projects not watched as last drawn, and the list the drawer draws them in.
    drawer: Drawer,
    pub(crate) drawer_list: ListState,
    /// The selection changed: scroll to it once the rows are up to date.
    reveal_selection: bool,
    /// Branches showing all their commits, or the first few, against how Settings say open
    /// branches list them: the first few, unless they say all.
    flipped_folds: HashSet<BranchKey>,
    /// Whether open branches list all their commits, as the fold flips were made against.
    /// When Settings change it, the flips go as `flipped` does.
    all_branch_commits: bool,
    /// Branches, here or on other machines, folded or opened against how Settings say
    /// branches start: folded, unless they say open. A branch with the selected commit shows
    /// its commits either way.
    flipped: HashSet<ClickOpened>,
    /// Whether branches start open, as the flips were made against. When Settings change
    /// it, the flips go and every branch starts over.
    branches_start_open: bool,
    /// Checkouts showing only their heading.
    folded_worktrees: HashSet<(ProjectId, Checkout)>,
    /// Stacks showing only their caption, by their bottom branch.
    folded_stacks: HashSet<BranchKey>,
    /// Checkouts on other machines showing only their heading, by the mesh's id.
    folded_checkouts: HashSet<(ProjectId, Arc<str>)>,
    /// The commits only a remote has listed under their heading, a branch's (by its full
    /// ref) or with `None` the target's. They start folded to the heading.
    open_upstream: HashSet<(ProjectId, Option<FullName>)>,
    /// The branch a click on its row opened, which a click on another's folds again; one
    /// opened or folded by hand since stays as it's left.
    click_opened: Option<ClickOpened>,
    /// The last click on a row opened a branch, as Open branches on click does, so the
    /// double-click it began doesn't fold it again, nor a row that folding moved under the
    /// pointer.
    click_just_opened: bool,
    /// Merges only a remote has whose commits show under them.
    open_merges: HashSet<CommitId>,
    /// The filter in the header, over every project's rows.
    filter: Entity<TextInput>,
    /// The first item the filter found, which Enter in it selects.
    first_hit: Option<Selection>,
    /// Whether the pointer is over the navigator.
    pointer_over: bool,
    /// The watched projects' order by activity as it was when the pointer came over the
    /// list, kept until it leaves, so a project an agent wakes doesn't move the row about to
    /// be clicked.
    held_order: Option<Vec<ProjectId>>,
    /// Each project's linked worktrees in the order they had then, most recently used first,
    /// which an agent committing in one would change under the pointer as well.
    held_worktrees: HashMap<ProjectId, Vec<bstr::BString>>,
    /// The project whose watch button the pointer is on, which shows what a click makes it.
    watch_hovered: Option<ProjectId>,
    /// Where a dragged project card would go, while one is over a card's heading.
    project_drop: Option<CardSlot>,
    /// The row under the pointer holds still through the next change to the rows: one a
    /// workspace's load made, as an agent's commit, or a branch's "Show fewer" folding what's
    /// above it, which would leave the list scrolled past the fold. What else the user folds
    /// or does moves the rows as usual.
    hold_still: bool,
    /// How far a frame the list scrolls while something dragged is near its top or bottom
    /// edge, up when negative, and the task that scrolls it.
    edge_scroll: f32,
    edge_scrolling: Option<Task<()>>,
    /// The branch whose name is being edited in its row.
    renaming: Option<rename::Renaming>,
    _subscriptions: Vec<Subscription>,
}

impl Navigator {
    /// The menu `items` makes from this view, open at `at` once the update running now is over.
    fn open_menu(
        &self,
        at: Point<Pixels>,
        items: impl FnOnce(&mut Self, &mut Context<Self>) -> Vec<MenuItem> + 'static,
        cx: &mut Context<Self>,
    ) {
        self.app.open_menu(&cx.entity().downgrade(), at, items, cx);
    }

    pub fn new(
        app: AppHandle,
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
                let prefs = &store.read(cx).prefs;
                if this.branches_start_open != prefs.branches_start_open {
                    this.branches_start_open = prefs.branches_start_open;
                    this.flipped.clear();
                    this.click_opened = None;
                }
                if this.all_branch_commits != prefs.all_branch_commits {
                    this.all_branch_commits = prefs.all_branch_commits;
                    this.flipped_folds.clear();
                }
                cx.notify()
            }),
            cx.subscribe(&store, |this, _, event, cx| match event {
                StoreEvent::SelectionChanged => this.reveal(cx),
                StoreEvent::Loaded(_) | StoreEvent::MeshRead(_) => this.hold_still = true,
                // A renamed branch stays open or folded as it was.
                StoreEvent::Renamed(project, from, to) => {
                    let to = Some(to.clone());
                    for set in [&mut this.flipped_folds, &mut this.folded_stacks] {
                        if set.remove(&(project.clone(), from.clone())) {
                            set.insert((project.clone(), to.clone()));
                        }
                    }
                    let here = |ref_name: &Option<FullName>| {
                        ClickOpened::Here((project.clone(), ref_name.clone()))
                    };
                    if this.flipped.remove(&here(from)) {
                        this.flipped.insert(here(&to));
                    }
                    // `None` there is the target's, which no rename is of.
                    if from.is_some()
                        && (this.open_upstream).remove(&(project.clone(), from.clone()))
                    {
                        (this.open_upstream).insert((project.clone(), to.clone()));
                    }
                    if let Some(ClickOpened::Here((p, r))) = &mut this.click_opened
                        && p == project
                        && r == from
                    {
                        *r = to;
                    }
                }
                _ => {}
            }),
            // Dragging the uncommitted changes carries the checked ones.
            cx.subscribe(&detail, |_, _, _: &ChecksChanged, cx| cx.notify()),
        ];
        let prefs = &store.read(cx).prefs;
        let (branches_start_open, all_branch_commits) =
            (prefs.branches_start_open, prefs.all_branch_commits);
        Self {
            app,
            store,
            detail,
            focus: cx.focus_handle(),
            rows: Vec::new(),
            insets: Vec::new(),
            guides: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, fixed(56.)),
            drawer: Drawer::default(),
            drawer_list: ListState::new(0, ListAlignment::Top, fixed(56.)),
            reveal_selection: false,
            flipped_folds: HashSet::new(),
            all_branch_commits,
            flipped: HashSet::new(),
            click_opened: None,
            click_just_opened: false,
            open_merges: HashSet::new(),
            open_upstream: HashSet::new(),
            folded_checkouts: HashSet::new(),
            branches_start_open,
            folded_worktrees: HashSet::new(),
            folded_stacks: HashSet::new(),
            filter,
            first_hit: None,
            pointer_over: false,
            held_order: None,
            held_worktrees: HashMap::new(),
            watch_hovered: None,
            project_drop: None,
            hold_still: false,
            edge_scroll: 0.,
            edge_scrolling: None,
            renaming: None,
            _subscriptions: subscriptions,
        }
    }

    /// Whether `checkout` of `project` shows what's in it: it isn't folded, or `selected` is
    /// in it.
    fn worktree_open(&self, project: &ProjectId, checkout: Checkout, selected: bool) -> bool {
        selected || !self.folded_worktrees.contains(&(project.clone(), checkout))
    }

    /// The words the filter looks for, up to as many as [`found`] can tell apart.
    fn filter_terms(&self, cx: &App) -> Vec<String> {
        (self.filter.read(cx).text().split_whitespace())
            .take(64)
            .map(str::to_lowercase)
            .collect()
    }

    /// Whether the filter has words in it, so the rows are what it finds, whatever's folded.
    fn filtering(&self, cx: &App) -> bool {
        !self.filter.read(cx).text().trim().is_empty()
    }

    /// A row's fold, as [`fold_button`]. While the filter lists what it finds, which folds
    /// don't hide, it's drawn open and does nothing.
    fn fold(
        &self,
        id: impl Into<ElementId>,
        open: Option<bool>,
        color: Hsla,
        cx: &Context<Self>,
        toggle: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        match open.is_some() && self.filtering(cx) {
            true => fold_glyph(Some(true), color).into_any_element(),
            false => fold_button(id, open, color, cx, toggle).into_any_element(),
        }
    }

    /// Whether `event` is a double-click that acts on its row, as a folder opens: not when
    /// its first click opened a branch.
    fn double_clicked(&mut self, event: &ClickEvent) -> bool {
        let just_opened = std::mem::take(&mut self.click_just_opened);
        event.click_count() == 2 && !just_opened
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
            self.reveal(cx);
        }
        cx.notify();
    }

    /// Show the selection where it is once the rows are up to date, opening the drawer when
    /// it's one of its projects.
    fn reveal(&mut self, cx: &mut Context<Self>) {
        self.reveal_selection = true;
        let store = self.store.read(cx);
        let in_drawer = matches!(&store.selection, Some(Selection::Project(id))
            if store.project(id).is_some_and(|entry| !entry.expanded));
        if in_drawer && store.prefs.projects_folded {
            (self.store).update(cx, |store, cx| {
                store.set_prefs(|p| p.projects_folded = false, cx)
            });
        }
    }

    fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        // Up to a section's heading, what the selection held open stays open, as a tree's
        // folder does over the row it was opened to show.
        if let Selection::Section(project, section) = &selection
            && !self.filtering(cx)
            && self.heading_open(project, section, cx)
            && !self.listed_by_hand(project, section, cx)
        {
            self.set_heading_open(project, section, true, cx);
        }
        self.store
            .update(cx, |store, cx| store.reopen(selection, cx));
        window.focus(&self.focus, cx);
    }

    /// The rows that can be selected, in order: the cards', then the drawer's.
    fn items(&self) -> Vec<&Selection> {
        items(self.rows.iter().chain(&self.drawer.rows))
    }

    /// Space at the end of a commit's diff: on to the next unread commit, in any branch or
    /// project, going on from the top past the end, as a feed reader goes on to the next unread
    /// article, until there are none.
    pub fn read_on(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        if !matches!(store.selection, Some(Selection::Commit(..))) {
            return;
        }
        let items = self.items();
        let at = (store.selection.as_ref()).and_then(|s| items.iter().position(|item| *item == s));
        let unread = |item: &&&Selection| match item {
            Selection::Commit(project, id) => (store.workspace(project))
                .and_then(|ws| ws.commits().find(|commit| commit.id == *id))
                .is_some_and(|commit| store.is_unread(project, commit)),
            _ => false,
        };
        let after = at.map_or(0, |at| at + 1);
        let next = (items.iter().skip(after).chain(items.iter().take(after)))
            .find(unread)
            .map(|item| (*item).clone());
        match next {
            Some(next) => self.select(next, window, cx),
            None => crate::state::snack(&self.store, "All caught up", cx),
        }
    }

    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        // ↓ onto a fold of commits shows the rest and goes on into them, as a feed loads more:
        // the fold is no stop of its own.
        let built = (delta > 0 && self.unfold_after_selection(cx)).then(|| self.build_rows(cx));
        let items = match &built {
            Some(built) => items(built.rows.iter().chain(&built.drawer.rows)),
            None => self.items(),
        };
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
        // Past an end it stays put, rather than picking the row again, which would take its
        // page back to how it opens on every repeat of a held key.
        if next as isize == at {
            return;
        }
        let Some(next) = items.get(next).map(|item| (*item).clone()) else {
            return;
        };
        self.select(next, window, cx);
    }

    /// Unfold the branch's "N more" right under the selection, saying whether there was one.
    fn unfold_after_selection(&mut self, cx: &App) -> bool {
        let selection = self.store.read(cx).selection.as_ref();
        let at = (self.rows.iter())
            .position(|row| matches!(row, Row::Item(item) if Some(item) == selection));
        let Some(Row::More(project, ref_name, _)) = at.and_then(|at| self.rows.get(at + 1)) else {
            return false;
        };
        self.set_unfolded((project.clone(), ref_name.clone()), true);
        true
    }

    /// Step `delta` commits from the selected one, picking the run as ⇧-click does. Off a
    /// commit, it's a plain step.
    fn pick_step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let (project, id) = match &store.selection {
            Some(Selection::Commit(project, id)) => (project.clone(), *id),
            _ => return self.step(delta, window, cx),
        };
        let order = self.commit_order(&project, id, cx);
        let next = (order.iter().position(|commit| *commit == id))
            .and_then(|at| order.get(at.checked_add_signed(delta)?).copied());
        if let Some(next) = next {
            (self.store).update(cx, |store, cx| store.pick(&project, next, Some(order), cx));
        }
    }

    /// The commits of `project` that can be picked along with commit `id`, as shown: its
    /// stack's, or linked worktree's, not those folded away.
    fn commit_order(&self, project: &ProjectId, id: CommitId, cx: &App) -> Vec<CommitId> {
        let lane = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.lane_commits(id))
            .unwrap_or_default();
        (self.rows.iter())
            .filter_map(|row| match row {
                Row::Item(Selection::Commit(p, commit)) if p == project => Some(*commit),
                _ => None,
            })
            .filter(|commit| lane.contains(commit))
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
            (Some(at), true) => items.iter().skip(at + 1).find(heads),
            (Some(at), false) => items.iter().take(at).rev().find(heads),
            (None, true) => items.iter().find(heads),
            (None, false) => items.iter().rev().find(heads),
        };
        if let Some(next) = next.map(|item| (*item).clone()) {
            self.select(next, window, cx);
        }
    }

    /// Whether `branch` shows its commits: as Settings start branches, unless folded or
    /// opened since.
    fn is_open(&self, branch: &ClickOpened) -> bool {
        self.branches_start_open != self.flipped.contains(branch)
    }

    /// Show `branch`'s commits, or fold them, by hand: it stays as it's left.
    fn set_open(&mut self, branch: ClickOpened, open: bool, cx: &mut Context<Self>) {
        if self.click_opened.as_ref() == Some(&branch) {
            self.click_opened = None;
        }
        if self.is_open(&branch) == open {
            return;
        }
        if !self.flipped.remove(&branch) {
            self.flipped.insert(branch);
        }
        cx.notify();
    }

    /// Whether the branch at `key` lists all its commits: as Settings say, unless folded or
    /// unfolded since.
    fn unfolded(&self, key: &BranchKey) -> bool {
        self.all_branch_commits != self.flipped_folds.contains(key)
    }

    /// List all the commits of the branch at `key`, or only the first few, by hand.
    fn set_unfolded(&mut self, key: BranchKey, unfolded: bool) {
        match unfolded == self.all_branch_commits {
            true => self.flipped_folds.remove(&key),
            false => self.flipped_folds.insert(key),
        };
    }

    /// Open a branch's commits for a plain click on its row, and fold the ones the last
    /// click opened: a click looks inside a branch, without the list growing by a branch
    /// each time.
    fn open_on_click(&mut self, branch: ClickOpened, cx: &mut Context<Self>) {
        if let Some(previous) = self.click_opened.take_if(|previous| *previous != branch) {
            self.set_open(previous, false, cx);
        }
        if !self.is_open(&branch) {
            self.set_open(branch.clone(), true, cx);
            self.click_opened = Some(branch);
        }
    }

    /// Whether one of the commits only `ref_name`'s remote has, or with `None` the target's
    /// new ones, is selected.
    fn upstream_selected(store: &Store, id: &ProjectId, ref_name: Option<&FullName>) -> bool {
        matches!(&store.selection, Some(Selection::Upstream(p, r, _))
            if p == id && r.as_ref() == ref_name)
    }

    /// Show the commits remote merge `id` brought in, which load as they open, or fold them,
    /// by hand: it stays as it's left. Folding them over the one selected selects the merge.
    fn toggle_merge(
        &mut self,
        project: &ProjectId,
        ref_name: Option<FullName>,
        id: CommitId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open_merges.insert(id) {
            (self.store).update(cx, |store, cx| store.load_merged(project, id, cx));
            return cx.notify();
        }
        self.open_merges.remove(&id);
        let store = self.store.read(cx);
        let inside = matches!(&store.selection, Some(Selection::Upstream(p, r, c))
            if p == project && store.merged_upstream(p, r.as_ref(), *c).is_some_and(|(m, _)| m == id));
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
    fn upstream_open(&self, store: &Store, id: &ProjectId, ref_name: Option<&FullName>) -> bool {
        self.open_upstream
            .contains(&(id.clone(), ref_name.cloned()))
            || Self::upstream_selected(store, id, ref_name)
    }

    fn expand(&mut self, _: &Expand, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        match store.selection.clone() {
            // A folded card opens; an unwatched project is watched.
            Some(Selection::Project(id)) => self.store.update(cx, |store, cx| {
                let folded = store.project(&id).is_some_and(|p| p.expanded)
                    && store.prefs.folded_projects.contains(id.as_ref());
                match folded {
                    true => store.set_project_folded(&id, false, cx),
                    false => store.set_expanded(&id, true, cx),
                }
            }),
            Some(Selection::Branch(project, ref_name)) => {
                self.set_open(ClickOpened::Here((project, ref_name)), true, cx)
            }
            Some(Selection::Uncommitted(project)) => {
                self.folded_worktrees.remove(&(project, Checkout::Main));
                cx.notify();
            }
            Some(Selection::Worktree(project, name)) => {
                self.folded_worktrees
                    .remove(&(project, Checkout::Linked(name)));
                cx.notify();
            }
            Some(Selection::Stack(project, bottom)) => {
                self.folded_stacks.remove(&(project, bottom));
                cx.notify();
            }
            Some(Selection::Mesh(project, id, pick)) => self.expand_mesh(project, id, pick, cx),
            Some(Selection::Section(project, section)) => {
                if !self.heading_open(&project, &section, cx) {
                    self.toggle_heading(project, section, false, window, cx)
                }
            }
            // A remote's merge unfolds the commits it brought in.
            Some(Selection::Upstream(project, ref_name, id))
                if !self.open_merges.contains(&id)
                    && (store.workspace(&project))
                        .and_then(|ws| ws.upstream(ref_name.as_ref()).iter().find(|c| c.id == id))
                        .is_some_and(|commit| commit.merge) =>
            {
                self.toggle_merge(&project, ref_name, id, window, cx)
            }
            _ => {}
        }
    }

    /// Step out the way a tree does: a commit to its branch, an open branch, stack or worktree
    /// closed, a branch up to its stack or worktree, and anything else up to its project.
    /// While the filter lists what it finds, nothing folds, so ⇧← only goes up.
    fn collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        let filtering = self.filtering(cx);
        let store = self.store.read(cx);
        let selection = store.selection.clone();
        // A commit a remote's merge brought in goes up to the merge.
        if let Some(Selection::Upstream(project, ref_name, id)) = &selection
            && let Some((merge, _)) = store.merged_upstream(project, ref_name.as_ref(), *id)
        {
            let up = Selection::Upstream(project.clone(), ref_name.clone(), merge);
            return self.select(up, window, cx);
        }
        let closes = !filtering
            && matches!(&selection, Some(item @ Selection::Branch(project, ref_name))
                if store.branch_for(item).is_some_and(|branch| !branch.commits.is_empty())
                    && self.is_open(&ClickOpened::Here((project.clone(), ref_name.clone()))));
        let folds = !filtering
            && (self.rows.iter())
                .position(|row| matches!(row, Row::Item(item) if Some(item) == selection.as_ref()))
                .is_some_and(|ix| self.section_open(ix));
        match selection {
            Some(Selection::Project(_)) if filtering => {}
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
                self.set_open(ClickOpened::Here((project, ref_name)), false, cx)
            }
            Some(Selection::Commit(project, id)) => {
                let branch = store
                    .find_commit(&project, id)
                    .and_then(|(b, _)| b.ref_name.clone());
                if let Some(ref_name) = branch {
                    self.select(Selection::Branch(project, Some(ref_name)), window, cx);
                }
            }
            // An open merge folds; the target's new commits go up to the project, below.
            Some(Selection::Upstream(project, ref_name, id)) if self.open_merges.contains(&id) => {
                self.toggle_merge(&project, ref_name, id, window, cx)
            }
            Some(Selection::Upstream(project, ref_name, _)) => {
                let heading = Selection::Section(project, Section::Upstream(ref_name));
                self.select(heading, window, cx)
            }
            Some(Selection::Uncommitted(project)) if folds => {
                self.toggle_section(&project, Checkout::Main, true, window, cx)
            }
            Some(Selection::Worktree(project, name)) if folds => {
                self.toggle_section(&project, Checkout::Linked(name), true, window, cx)
            }
            Some(Selection::Worktree(project, name))
                if (store.workspace(&project))
                    .and_then(|ws| ws.find_worktree(name.as_ref()))
                    .is_some_and(|wt| wt.is_inactive()) =>
            {
                let heading = Selection::Section(project, Section::Inactive);
                self.select(heading, window, cx)
            }
            Some(Selection::Stack(project, bottom)) if folds => {
                self.toggle_stack(&project, bottom.as_ref(), true, window, cx)
            }
            Some(Selection::Mesh(project, id, pick)) => {
                self.collapse_mesh(project, id, pick, window, cx)
            }
            Some(Selection::Section(project, section)) if !filtering => {
                match self.heading_open(&project, &section, cx) {
                    true => self.toggle_heading(project, section, true, window, cx),
                    false => {
                        let up = match section {
                            Section::Upstream(Some(ref_name)) => {
                                Selection::Branch(project, Some(ref_name))
                            }
                            _ => Selection::Project(project),
                        };
                        self.select(up, window, cx)
                    }
                }
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
            Some(Selection::Project(id)) => self.toggle_project(&id, cx),
            Some(Selection::Section(project, section)) => {
                let open = self.heading_open(&project, &section, cx);
                self.toggle_heading(project, section, open, window, cx)
            }
            Some(Selection::Uncommitted(_) | Selection::Worktree(..)) => {
                self.detail
                    .update(cx, |detail, cx| detail.focus_message(window, cx));
            }
            _ => {}
        }
    }

    /// Enter or a double-click on project `id`, or its card's chevron: a watched project's
    /// card folds or unfolds, as a folder does, still watched, and another starts being
    /// watched. While the filter lists what it finds, cards don't fold.
    fn toggle_project(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        let filtering = self.filtering(cx);
        self.store.update(cx, |store, cx| {
            match store.project(id).is_some_and(|p| p.expanded) {
                true if filtering => {}
                true => store.set_project_folded(id, !store.project_folded(id), cx),
                false => store.set_expanded(id, true, cx),
            }
        })
    }

    /// Bring the lists up to date with `rows` and `drawer`, and show the selection if it just
    /// changed. Rows that didn't change keep their place on screen. When a load changed the
    /// cards' while the pointer is over them, so does the row under it: rows added above it,
    /// like an agent's new commit, push the rows above it up rather than it down.
    fn sync_list(&mut self, rows: Vec<Row>, drawer: Drawer, window: &Window, cx: &App) {
        // Rows not drawn yet count as one row tall, which a project's row is.
        let row = Theme::get(cx).scaled(ROW_HEIGHT);
        // At the very top a list stays there, showing what's added, as the diff does: rows
        // added above the first one count as added above the top, which would leave it
        // scrolled to their end, as the drawer's rows are when it opens.
        let drawer_at_top = at_top(&self.drawer_list);
        if splice(&self.drawer_list, &self.drawer.rows, &drawer.rows, 0) {
            self.drawer_list.clone().with_uniform_item_height(row);
            if drawer_at_top {
                self.drawer_list.scroll_to(ListOffset::default());
            }
        }
        self.drawer = drawer;
        // Scrolled, the row under the pointer stays where it is.
        let at_top = at_top(&self.list);
        let hovered = (std::mem::take(&mut self.hold_still) && !at_top)
            .then(|| self.hovered_row(window))
            .flatten();
        // The hovered row's index before and after, and how far below the top it is.
        let anchor = hovered.and_then(|(old_ix, offset)| {
            let old = self.rows.get(old_ix)?;
            let (new_ix, _) = (rows.iter().enumerate())
                .filter(|(_, row)| same_place(old, row))
                .min_by_key(|(ix, _)| ix.abs_diff(old_ix))?;
            Some((old_ix, new_ix, offset))
        });
        let halves = anchor.and_then(|(old_ix, new_ix, _)| {
            Some((
                self.rows.split_at_checked(old_ix)?,
                rows.split_at_checked(new_ix)?,
            ))
        });
        let changed = match halves {
            // Below the hovered row first, so the indexes above it still hold.
            Some(((old_above, old_below), (new_above, new_below))) => {
                let below = splice(&self.list, old_below, new_below, old_above.len());
                splice(&self.list, old_above, new_above, 0) || below
            }
            None => splice(&self.list, &self.rows, &rows, 0),
        };
        if at_top && changed {
            self.list.scroll_to(ListOffset::default());
        }
        self.rows = rows;
        if changed {
            // The clone shares the list's state.
            self.list.clone().with_uniform_item_height(row);
            if let Some((_, item_ix, offset)) = anchor {
                self.list.scroll_to(ListOffset {
                    item_ix,
                    offset_in_item: Pixels::ZERO,
                });
                self.list.scroll_by(-offset);
            }
        }
        let store = self.store.read(cx);
        if std::mem::take(&mut self.reveal_selection)
            && let Some(selection) = &store.selection
        {
            let at = |rows: &[Row]| {
                (rows.iter()).position(|row| matches!(row, Row::Item(item) if item == selection))
            };
            match (at(&self.rows), at(&self.drawer.rows)) {
                (Some(ix), _) => {
                    // Rows not drawn since the list's width changed, as zooming changes it,
                    // count as one row tall rather than none, so the reveal lands on it.
                    self.list.clone().with_uniform_item_height(row);
                    self.list.scroll_to_reveal_item(ix)
                }
                (None, Some(ix)) => self.drawer_list.scroll_to_reveal_item(ix),
                // A project just added has its rows once it's loaded.
                (None, None) => {
                    self.reveal_selection =
                        (store.project(selection.project())).is_some_and(|entry| entry.is_loading())
                }
            }
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
        // A fold row at the end of commits: its glyph under theirs, its words under their
        // titles, past the glyph and the commit row's gap.
        let under_commits = |id: &'static str, glyph| {
            let at = ROW_LEFT + COMMIT_INDENT;
            fold_row((id, ix), glyph, at, at + 12. + 10., theme)
        };
        let el = match self.rows.get(ix) {
            None | Some(Row::Gap) => return div().h(dp(12.)).into_any_element(),
            Some(Row::Item(_)) => self.render_item(&self.rows, ix, inset, window, theme, cx),
            // A dashed line, as ui-react's WorktreeDivider.
            Some(Row::WorktreeDivider) => h_flex()
                .h(dp(12.))
                .px(dp(12.))
                .child(
                    div()
                        .flex_1()
                        .border_t_1()
                        .border_dashed()
                        .border_color(theme.border_3),
                )
                .into_any_element(),
            Some(Row::Note(text, is_error)) => note_row(text, *is_error, theme),
            // As made at once, set in as a commit row is, a stand-in title quiet as one's;
            // dimmed, with an ellipsis, only once it's taking a while.
            Some(Row::Pending(title, slow)) => h_flex()
                .min_h(dp(ROW_HEIGHT))
                .mx(dp(3.))
                .pl(dp(8. + COMMIT_INDENT))
                .pr(dp(12.))
                .gap(dp(10.))
                .when(*slow, |el| el.opacity(0.5))
                .child(small_icon(Icon::Commit).text_color(theme.blue))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(title.is_empty(), |el| el.text_color(theme.text_2))
                        .child(commit_title(title)),
                )
                .when(*slow, |el| {
                    el.child(div().text_size(dp(11.)).text_color(theme.text_2).child("…"))
                })
                .into_any_element(),
            Some(Row::NewBranch(project)) => self.new_branch_zone(ix, project, theme, cx),
            Some(Row::UpstreamMore(project, more)) => {
                let page = Selection::Project(project.clone());
                under_commits("upstream-more", Icon::Kebab)
                    .child(format!("{more} more on its page"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.select(page.clone(), window, cx)
                    }))
                    .into_any_element()
            }
            Some(Row::RestsOn(item, checkout)) => {
                match self.rests_on_label(item, checkout.as_ref(), cx) {
                    Some(label) => {
                        let item = item.clone();
                        under_commits("rests-on", Icon::Branch)
                            .child(div().min_w_0().truncate().child(label))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.select(item.clone(), window, cx)
                            }))
                            .into_any_element()
                    }
                    None => div().into_any_element(),
                }
            }
            Some(Row::More(project, ref_name, hidden)) => {
                let key = (project.clone(), ref_name.clone());
                under_commits("more", Icon::Kebab)
                    .child(format!("{hidden} more"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_unfolded(key.clone(), true);
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Some(Row::Fewer(project, ref_name)) => {
                let key = (project.clone(), ref_name.clone());
                under_commits("fewer", Icon::ChevronUp)
                    .child("Show fewer")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_unfolded(key.clone(), false);
                        this.hold_still = true;
                        cx.notify();
                    }))
                    .into_any_element()
            }
        };
        // Items step in by their style, keeping the highlight full width; the rest as a whole.
        let el = match inset > 0. && !matches!(self.rows.get(ix), Some(Row::Item(_))) {
            true => div().pl(dp(inset)).child(el).into_any_element(),
            false => el,
        };
        let first = (ix.checked_sub(1).and_then(|ix| self.rows.get(ix)))
            .is_none_or(|prev| matches!(prev, Row::Gap));
        let last = self
            .rows
            .get(ix + 1)
            .is_none_or(|next| matches!(next, Row::Gap));
        // A guide for each row it's under, joining those of the rows above and below into
        // one line, as but.dev's sidebar draws them.
        let guides = (self.guides.get(ix).into_iter().flatten()).map(|at| guide(*at, theme));
        // A watched project's card takes a dragged project card before it.
        let slot = match self.rows.get(ix) {
            _ if self.store.read(cx).prefs.watched_order != WatchedOrder::Manual => None,
            Some(Row::Item(Selection::Project(id)))
                if self.store.read(cx).project(id).is_some_and(|p| p.expanded) =>
            {
                Some(CardSlot::Before(id.clone()))
            }
            _ => None,
        };
        // Padded, not margined: a list lays its rows out from their left edge.
        div()
            .px(dp(10.))
            .child(
                div()
                    .relative()
                    .bg(theme.bg_1)
                    .border_color(theme.border_3)
                    .border_l_1()
                    .border_r_1()
                    .when(first, |el| el.border_t_1().rounded_t(dp(6.)).pt(dp(3.)))
                    .when(last, |el| el.border_b_1().rounded_b(dp(6.)).pb(dp(3.)))
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
                        let line = insertion_line(true, theme.pop).right(dp(10.));
                        el.child(line.when(ix > 0, |line| line.top(dp(-6.))))
                    })
            })
            .into_any_element()
    }

    /// Item row `ix` of `rows`, the cards' or the drawer's.
    fn render_item(
        &self,
        rows: &[Row],
        ix: usize,
        inset: f32,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(Row::Item(item)) = rows.get(ix) else {
            return div().into_any_element();
        };
        let store = self.store.read(cx);
        let lit = |item: &Selection| {
            store.selection.as_ref() == Some(item)
                || matches!(item, Selection::Commit(project, id) if store.is_picked(project, *id))
        };
        let selected = lit(item);
        // Picked commits next to each other join up.
        let lit_at = |ix: usize| matches!(rows.get(ix), Some(Row::Item(other)) if lit(other));
        let joined = match selected && !store.picked.is_empty() {
            true => (ix > 0 && lit_at(ix - 1), lit_at(ix + 1)),
            false => (false, false),
        };
        let cut = match (&store.cut, item) {
            (Some(Cut::Branch(p, r)), Selection::Branch(project, ref_name)) => {
                p == project && ref_name.as_ref() == Some(r)
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
            glow: !selected && matches!(item, Selection::Commit(_, commit) if store.glows(*commit)),
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
            Selection::Branch(id, ref_name) => {
                self.branch_row(item, id, ref_name.as_ref(), style, cx)
            }
            Selection::Commit(id, commit_id) => self.commit_row(item, id, commit_id, style, cx),
            Selection::Branches(_) => {
                let tip = Some("Branches not applied");
                Some(self.page_row(item, Icon::Branch, "Branches", tip, style, cx))
            }
            Selection::History(_) => {
                let label = "Operations history";
                Some(self.page_row(item, Icon::Clock, label, None, style, cx))
            }
            Selection::Stack(id, bottom) => {
                self.stack_row(ix, item, id, bottom.as_ref(), style, cx)
            }
            Selection::Mesh(..) => self.mesh_row(ix, item, style, cx),
            Selection::Upstream(id, ref_name, commit) => {
                self.upstream_row(item, id, ref_name.as_ref(), commit, style, cx)
            }
            Selection::Section(id, Section::Upstream(ref_name)) => {
                self.upstream_heading(ix, id, ref_name.as_ref(), style, cx)
            }
            Selection::Section(id, Section::Inactive) => {
                Some(self.inactive_heading(ix, id, style, cx))
            }
            Selection::Section(id, Section::Mesh) => Some(self.mesh_heading(ix, id, style, cx)),
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
                    let range = shift.then(|| this.commit_order(project, *id, cx));
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

    /// The projects not watched, in a drawer under the cards that opens upwards to part of
    /// the column and scrolls past that. With no cards it's the column, open.
    fn drawer(&self, theme: &Theme, cx: &Context<Self>) -> impl IntoElement {
        let alone = self.rows.is_empty();
        let rows = list(
            self.drawer_list.clone(),
            cx.processor(|this, ix: usize, window, cx| this.render_drawer_row(ix, window, cx)),
        )
        .size_full();
        let scroll = Scroll::List(self.drawer_list.clone());
        let manual = self.store.read(cx).prefs.watched_order == WatchedOrder::Manual;
        let slot = (manual && !alone).then_some(CardSlot::End);
        let lit = slot.is_some() && self.project_drop == slot;
        let share = (self.store.read(cx).prefs.drawer_share).unwrap_or(DRAWER_SHARE);
        // Each of its rows is one row tall.
        let rows_height = match self.drawer.rows.len() {
            0 => 0.,
            n => n as f32 * ROW_HEIGHT + DRAWER_PADDING,
        };
        v_flex()
            .relative()
            .bg(theme.bg_1)
            .map(|el| match alone {
                true => el.flex_1().min_h_0(),
                false => (el.flex_none())
                    .h(dp(DRAWER_BAR_HEIGHT + rows_height))
                    .max_h(relative(share)),
            })
            .child(self.drawer_bar(alone, theme, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .when(rows_height > 0., |el| el.pb(dp(DRAWER_PADDING)))
                    .child(rows)
                    .children(scrollbar("drawer-scrollbar", scroll, theme)),
            )
            // After the cards, a dragged card goes last.
            .when_some(slot, |el, slot| el.child(self.card_drop(slot, cx)))
            .when(lit, |el| {
                el.child(insertion_line(true, theme.pop).right(dp(10.)).top(dp(-6.)))
            })
            .when(!alone && rows_height > 0., |el| {
                el.child(self.drawer_resize_handle(theme, cx))
            })
    }

    /// A strip along the open drawer's top edge: drag it to resize the drawer, as far as its
    /// rows go, double-click to put it back. The navigator listens with
    /// [`Self::resize_drawer`].
    fn drawer_resize_handle(&self, theme: &Theme, cx: &Context<Self>) -> impl IntoElement {
        div()
            .id("resize-drawer")
            .absolute()
            .left_0()
            .right_0()
            .top(dp(-2.))
            .h(dp(5.))
            .group("resize-drawer")
            .cursor(CursorStyle::ResizeUpDown)
            .block_mouse_except_scroll()
            // A line along the drawer's top edge, as a pane's handle marks its own.
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(dp(2.))
                    .h(fixed(1.))
                    .group_hover("resize-drawer", |el| el.bg(theme.border_1)),
            )
            .on_drag(DrawerResize, |_, _, _, cx| cx.new(|_| EmptyView))
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                if event.click_count() == 2 {
                    (this.store).update(cx, |store, cx| {
                        store.set_prefs(|p| p.drawer_share = None, cx)
                    })
                }
            }))
    }

    /// Resize the drawer while its top edge is dragged, as a share of the navigator's column,
    /// which the drawer ends at the bottom of.
    fn resize_drawer(
        &mut self,
        event: &DragMoveEvent<DrawerResize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = event.bounds;
        let share = (bounds.bottom() - event.event.position.y) / bounds.size.height;
        let share = share.clamp(DRAWER_SHARES.0, DRAWER_SHARES.1);
        (self.store).update(cx, |store, cx| {
            store.set_prefs(|p| p.drawer_share = Some(share), cx)
        });
    }

    /// The drawer's handle: a click opens or closes it, and it says how many projects it lists
    /// of how many there are, and has ways to add, order and find them.
    fn drawer_bar(&self, alone: bool, theme: &Theme, cx: &Context<Self>) -> impl IntoElement {
        let Drawer { listed, total, .. } = self.drawer;
        let title = match alone {
            true => "All projects",
            false => "More projects",
        };
        let count = match listed == total {
            true => total.to_string(),
            false => format!("{listed} of {total}"),
        };
        // While the filter lists what it finds, they show as they are, as cards don't fold.
        let toggles = total > 0 && !alone && !self.filtering(cx);
        let chevron = match self.store.read(cx).prefs.projects_folded {
            true => Icon::ChevronUp,
            false => Icon::ChevronDown,
        };
        // Its chevron over the projects' eyes.
        let fold = h_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap(dp(6.))
            .when(toggles, |el| {
                el.child(small_icon(chevron).text_color(theme.text_3))
            })
            .child(section_label(title, theme).truncate())
            .when(total > 0, |el| {
                el.child(faint(count, theme).text_size(dp(12.)))
            });
        h_flex()
            .id("drawer-bar")
            .flex_none()
            .h(dp(DRAWER_BAR_HEIGHT))
            .border_t_1()
            .border_color(theme.border_3)
            .pl(dp(22.))
            .pr(dp(15.))
            .gap(dp(2.))
            .child(fold)
            .child(
                icon_button("add-project", Icon::Plus)
                    .small()
                    .tooltip("Add project")
                    .dispatch(&AddProject),
            )
            .when(total > 0, |el| {
                el.child(
                    icon_button("project-view-options", Icon::Kebab)
                        .small()
                        .tooltip("Sort and show")
                        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                            this.open_menu(event.position(), |this, cx| this.view_options(cx), cx)
                        })),
                )
            })
            .when(toggles, |el| {
                el.cursor_pointer()
                    .hover(|el| el.bg(theme.hover()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        (this.store).update(cx, |store, cx| {
                            store.set_prefs(|p| p.projects_folded = !p.projects_folded, cx)
                        })
                    }))
            })
    }

    /// Row `ix` of the drawer, set in as a card's rows are.
    fn render_drawer_row(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let el = match self.drawer.rows.get(ix) {
            Some(Row::Note(text, is_error)) => note_row(text, *is_error, theme),
            _ => self.render_item(&self.drawer.rows, ix, 0., window, theme, cx),
        };
        div().px(dp(11.)).child(el).into_any_element()
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
        let expanded = entry.expanded;
        // A watched project whose card is folded to this row. The filter lists what it finds,
        // folded or not.
        let filtering = self.filtering(cx);
        let card_folded = expanded && store.project_folded(id) && !filtering;
        let manual = store.prefs.watched_order == WatchedOrder::Manual;
        let hovered = self.watch_hovered.as_ref() == Some(id);
        // Its target's New on … row, when listed, counts the new commits and has Update.
        let new_on = (self.rows.iter()).any(|row| {
            matches!(row, Row::Item(Selection::Section(project, Section::Upstream(None)))
                if project == id)
        });
        // Folded, the row sums up the whole card; open, it leaves out what the rows under it
        // say: their uncommitted files, unpushed commits and worktrees.
        let summary = workspace.filter(|ws| ws.editing.is_none()).map(|ws| {
            let folded = |n: usize| if expanded && !card_folded { 0 } else { n };
            let shown = |checkout, files| store.uncommitted_shown(id, &checkout, files).len();
            let in_worktrees = (ws.worktrees.iter())
                .map(|w| shown(Checkout::Linked(w.name.clone()), &w.uncommitted));
            (
                folded(shown(Checkout::Main, &ws.uncommitted) + in_worktrees.sum::<usize>()),
                folded(ws.unpushed_commits()),
                ws.target.as_ref().filter(|_| !new_on),
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
        // The eye and the card's chevron, whose clicks don't select the row.
        let button = |name: &str, click: fn(&mut Self, &ProjectId, &mut Context<Self>)| {
            let id = id.clone();
            div()
                .id(ElementId::Name(format!("{name}-{id}").into()))
                .flex_none()
                .size(dp(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(dp(4.))
                .hover(|el| el.bg(theme.hover()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    click(this, &id, cx)
                }))
        };
        Some(
            style
                .row(format!("project-{id}"), 0.)
                .aria_label(entry.project.name.clone())
                .aria_expanded(expanded)
                .h(dp(if expanded { 32. } else { ROW_HEIGHT }))
                .pl(dp(11.))
                // The eye over the chevrons of the rows below and the emoji, 5 points on,
                // over their icons, so the name sits over theirs.
                .child(
                    h_flex()
                        .flex_none()
                        .gap(dp(5.))
                        .child(
                            // Whether the app watches the project, as an open one is loaded and
                            // kept current, and under the pointer what a click makes it.
                            // Named by what it does, so its tooltip goes with the click.
                            button(
                                if expanded { "unwatch" } else { "watch" },
                                |this, id, cx| {
                                    // The row moves to the other card, from under the pointer.
                                    this.watch_hovered = None;
                                    this.store.update(cx, |store, cx| {
                                        let expanded =
                                            store.project(id).is_some_and(|p| p.expanded);
                                        store.set_expanded(id, !expanded, cx)
                                    });
                                },
                            )
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
                            })),
                        )
                        // Every project wears one, the glyph's outline until one's picked, so
                        // their names line up. In an icon's 16 points, an emoji a little smaller
                        // than the text.
                        .child(
                            div()
                                .flex_none()
                                .w(dp(16.))
                                .flex()
                                .justify_center()
                                .text_size(dp(13.))
                                .child(match ProjectIcon::parse(store.project_icon(id)) {
                                    // Unpicked, it's quiet as the eye beside it, and takes the
                                    // row's ink when it's selected.
                                    ProjectIcon::Glyph(Colour { fill: None, .. }) => {
                                        icon(Icon::Repo)
                                            .size(dp(16.))
                                            .text_color(style.muted())
                                            .into_any_element()
                                    }
                                    picked => picked.render(theme.scaled(16.), theme),
                                }),
                        ),
                )
                // However narrow the navigator, the name stays: what the row sums up gives way
                // first, a whole item at a time from its end, the age before the counts. Each
                // item that doesn't fit wraps to a second line, which is cut off.
                .child(
                    div()
                        .flex_grow(1.)
                        .min_w_0()
                        .truncate()
                        .child(entry.project.name.clone()),
                )
                .child(
                    h_flex()
                        .flex_shrink(1000.)
                        .min_w_0()
                        .h(dp(16.))
                        .flex_wrap()
                        .overflow_hidden()
                        .gap(dp(8.))
                        .when(unread, |el| el.child(dot(style.meta(theme.pop))))
                        .when_some(summary, |el, (files, unpushed, target, worktrees)| {
                            let warn = style.meta(theme.warn);
                            el.children(style.files(files))
                                .children(style.unpushed(unpushed))
                                .children(target.and_then(|t| {
                                    let behind = plural(t.behind, "commit");
                                    let tip =
                                        format!("{behind} behind {}{}", t.name, with_merged(t));
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
                        .when_some(activity, |el, at| {
                            el.child(
                                div()
                                    .id(ElementId::Name(format!("age-{id}").into()))
                                    .text_size(dp(11.))
                                    .text_color(style.muted())
                                    .tooltip(Tooltip::text(format!("Last active {}", ago(at))))
                                    .child(relative_time(at)),
                            )
                        }),
                )
                // Only a watched project's: an unwatched one updates from its page or its menu,
                // which keeps the list of the others quiet.
                .when(expanded && needs_update && !new_on, |el| {
                    el.child(self.target_update_button(id, style.solid, cx))
                })
                // A watched project's card folds to this row, still watched.
                .when(expanded, |el| {
                    el.child(
                        button("fold-project", |this, id, cx| this.toggle_project(id, cx))
                            .child(fold_glyph(Some(!card_folded), style.quiet()))
                            .tooltip(Tooltip::text(match (filtering, card_folded) {
                                (true, _) => "Clear the filter to fold the project",
                                (false, true) => "Show the project",
                                (false, false) => "Fold the project, still watching it",
                            })),
                    )
                })
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                // Under the filter too, where `toggle_project` leaves a card open but still
                // watches a project the filter found.
                .on_click({
                    let id = id.clone();
                    cx.listener(move |this, event: &ClickEvent, _, cx| {
                        if this.double_clicked(event) {
                            this.toggle_project(&id, cx)
                        }
                    })
                })
                // Only the manual order moves by hand.
                .when(expanded && manual, |el| self.reorderable(el, id, entry, cx))
                .into_any_element(),
        )
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
                    .min_w_0()
                    .truncate()
                    .child("Editing a commit"),
            )
            .children(change_counts(tally(files), style))
            .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
            .into_any_element()
    }

    fn rests_on_label(
        &self,
        item: &Selection,
        checkout: Option<&Checkout>,
        cx: &App,
    ) -> Option<String> {
        let store = self.store.read(cx);
        let (lane, at) = store.lane_for(item)?;
        let branch = lane.branches.get(at)?;
        let name = store.name_of(item.project(), branch);
        let below_tip = match item {
            Selection::Commit(_, id) if branch.commits.first()?.id != *id => {
                format!(" at {}", id.short())
            }
            _ => String::new(),
        };
        let checkout = checkout.map_or(String::new(), |checkout| {
            format!(
                " in {}",
                checkout.linked().unwrap_or("Main worktree".into())
            )
        });
        Some(format!("{name}{below_tip}{checkout}"))
    }

    /// A worktree's heading, which selects its uncommitted changes: the main one's for
    /// [`Selection::Uncommitted`], a linked one's for [`Selection::Worktree`]. With linked
    /// worktrees, its chevron or a double-click folds its branches; folded, it also counts
    /// their unpushed commits. Without, the main one's branches sit beside it, and still fold.
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
        let checkout = match item {
            Selection::Worktree(_, name) => Checkout::Linked(name.clone()),
            _ => Checkout::Main,
        };
        let worktree = (checkout.linked()).and_then(|name| workspace.find_worktree(name));
        let (label, path, uncommitted) = match worktree {
            Some(wt) => (wt.name.to_string(), &wt.path, &wt.uncommitted),
            None => (
                "Main worktree".to_owned(),
                &entry.project.path,
                &workspace.uncommitted,
            ),
        };
        let uncommitted = store.uncommitted_shown(id, &checkout, uncommitted);
        let branches: Vec<_> = workspace.branches_in(&checkout).collect();
        let unpushed_n = unpushed(branches.iter().copied());
        let unread = (branches.iter()).any(|branch| store.branch_unread(id, branch));
        // Open even when folded while the selection's in it.
        let open = self.section_open(ix);
        let toggle = {
            let (project, checkout) = (id.clone(), checkout.clone());
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_section(&project, checkout.clone(), open, window, cx)
            }
        };
        let lane = (checkout.linked()).and_then(|name| workspace.worktree_lane(name));
        let rests = lane.is_some_and(|lane| workspace.beneath(lane).next().is_some());
        let folds = !branches.is_empty() || rests;
        let step = self.fold(
            ("worktree-fold", ix),
            folds.then_some(open),
            style.quiet(),
            cx,
            toggle.clone(),
        );
        // In as far as a branch is, so its chevron and icon line up with theirs.
        let row = style
            .row(
                format!("worktree-{id}-{}", checkout.linked().unwrap_or_default()),
                6.,
            )
            .aria_label(label.clone())
            .aria_expanded(open)
            .child(step)
            .child(worktree_icon(
                checkout.is_linked(),
                !uncommitted.is_empty(),
                style,
            ))
            .child(
                div()
                    .id(("worktree-name", ix))
                    .flex_grow(1.)
                    .min_w_0()
                    .truncate()
                    .tooltip(Tooltip::text(path.display().to_string()))
                    .child(label),
            )
            .children(change_counts(tally(&uncommitted), style))
            .when(!open, |el| {
                el.children(style.unpushed(unpushed_n))
                    .when(unread, |el| el.child(dot(style.meta(theme.pop))))
            })
            .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
            .when(folds, |el| el.on_click(double_click(toggle, cx)));
        // The worktree's checked files while it's shown, else all of them, but not conflicted
        // ones: with only those, there's nothing to drag.
        let files = match uncommitted.is_empty() {
            true => Vec::new(),
            false => self.detail.read(cx).files_to_move(id, checkout.clone(), cx),
        };
        Some(
            row.when(!files.is_empty(), |el| {
                let label: SharedString = match files.as_slice() {
                    [file] => file.file.path.clone().into(),
                    files => format!("{} files", files.len()).into(),
                };
                let payload = Cut::Files {
                    project: id.clone(),
                    from: None,
                    worktree: checkout,
                    files: files.into(),
                };
                el.on_drag(payload, drag_preview(self.store.clone(), label))
            })
            .into_any_element(),
        )
    }

    /// Fold `checkout`'s branches, or unfold them. Folding them over the selection takes it
    /// up to the worktree's heading, as a tree does.
    fn toggle_section(
        &mut self,
        project: &ProjectId,
        checkout: Checkout,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = (project.clone(), checkout);
        if !open {
            self.folded_worktrees.remove(&key);
            return cx.notify();
        }
        let store = self.store.read(cx);
        let inside = (store.selection.as_ref())
            .zip(store.workspace(project))
            .is_some_and(|(s, ws)| s.project() == project && within(s, ws.branches_in(&key.1)));
        let heading = Selection::uncommitted_in(project.clone(), key.1.clone());
        self.folded_worktrees.insert(key);
        match inside {
            true => self.select(heading, window, cx),
            false => cx.notify(),
        }
    }

    /// Whether row `ix` heads rows stepped in under it that show: an open worktree or stack.
    fn section_open(&self, ix: usize) -> bool {
        let next = self.insets.get(ix + 1);
        (next.zip(self.insets.get(ix))).is_some_and(|(next, this)| next > this)
    }

    /// Over a stack of two or more branches, as ui-react's StackCaption: a chevron, its
    /// glyph and how many. Selecting it shows what its branches change together; its chevron,
    /// or a double-click, folds them away, and folded, it says how many commits they have to
    /// push, whether any is unread and when they last changed.
    fn stack_row(
        &self,
        ix: usize,
        item: &Selection,
        id: &ProjectId,
        bottom: Option<&FullName>,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        // Find the stack by its bottom branch as drawn, after pending deletes.
        let branches = (store.workspace(id)?.lanes.iter())
            .map(|lane| store.after_deletes(id, &lane.branches))
            .find(|lane| {
                lane.last()
                    .is_some_and(|branch| branch.ref_name.as_ref() == bottom)
            })?;
        // Open even when folded while the selection's in it.
        let open = self.section_open(ix);
        let names: Vec<&str> = (branches.iter())
            .map(|branch| store.label_of(id, branch))
            .collect();
        let unpushed_n = unpushed(branches.iter());
        let unread = (branches.iter()).any(|branch| store.branch_unread(id, branch));
        let latest = (branches.iter().flat_map(|branch| &branch.commits))
            .map(|commit| commit.time)
            .max();
        let label = format!("Stack of {}", branches.len());
        let toggle = {
            let (project, bottom) = (id.clone(), bottom.cloned());
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_stack(&project, bottom.as_ref(), open, window, cx)
            }
        };
        let step = self.fold(
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
                .row(
                    format!(
                        "stack-{id}-{}",
                        bottom.map(ToString::to_string).unwrap_or_default()
                    ),
                    6.,
                )
                .aria_label(label)
                .aria_expanded(open)
                .child(step)
                .child(icon(Icon::Stack).text_color(quiet))
                .child(
                    h_flex()
                        .id(("stack-name", ix))
                        .flex_1()
                        .min_w_0()
                        .gap(dp(6.))
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
                                .min_w_0()
                                .truncate()
                                .text_color(quiet)
                                .child(store.label_of(id, top).to_owned())
                        })),
                )
                .when(!open, |el| {
                    el.children(style.unpushed(unpushed_n))
                        .when(unread, |el| el.child(dot(style.meta(theme.pop))))
                        .children(latest.map(|at| style.age(at)))
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
        bottom: Option<&FullName>,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = (project.clone(), bottom.cloned());
        if !open {
            self.folded_stacks.remove(&key);
            return cx.notify();
        }
        let store = self.store.read(cx);
        let inside = (store.selection.as_ref())
            .zip(store.workspace(project).and_then(|ws| ws.lane(bottom)))
            .is_some_and(|(s, lane)| {
                s.project() == project && lane.branches.iter().any(|b| s.on(b))
            });
        self.folded_stacks.insert(key);
        match inside {
            true => self.select(
                Selection::Stack(project.clone(), bottom.cloned()),
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
        ref_name: Option<&FullName>,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let row_id = ref_name.map(ToString::to_string).unwrap_or_default();
        let theme = style.theme;
        let branch = store.branch_for(item)?;
        let name = store.name_of(id, branch);
        let unpushed_n = unpushed(std::iter::once(branch));
        let conflicted = branch.commits.iter().filter(|c| c.conflicted).count();
        // Its pull request shows as it's asked for, pulsing until the forge has it.
        let publishing =
            branch.review.is_none() && ref_name.is_some_and(|r| store.publishing(id, r));
        let (glyph, glyph_color) = match (&branch.review, branch.push) {
            (Some(review), _) => review::state_glyph(store.review_state(id, review), theme),
            (None, _) if publishing => review::state_glyph(ReviewState::Open, theme),
            // Landed without a pull request, as one that landed with one shows.
            (None, PushState::Integrated) => (Icon::BranchMerge, theme.purple),
            (None, _) => (Icon::Branch, style.muted()),
        };
        // Once a pull request merged, how its CI stands no longer matters.
        let checks = (branch.review.as_ref())
            .filter(|review| store.review_state(id, review) != ReviewState::Merged)
            .and_then(|review| review.checks);
        // A pull request's title names the row (see `branch_name`); its branch's name, number
        // and how CI stands are in the tooltip, and on the branch's page. Else its name is, in
        // full, as agents' branches can differ only at their end, past where the row cuts it.
        let tip = (branch.review.as_ref())
            .filter(|review| !review.title.is_empty())
            .map(|review| {
                let checks = checks
                    .map(|checks| format!(" · {}", review::checks_summary(checks, theme).2))
                    .unwrap_or_default();
                format!("{name} · #{}{checks}", review.number)
            })
            .unwrap_or_else(|| name.to_owned());
        let key = (id.clone(), ref_name.cloned());
        let clicked = ClickOpened::Here(key.clone());
        let closed = !self.is_open(&clicked) && !branch.commits.is_empty();
        // What's selected under it keeps it open.
        let holds_selected =
            (store.selection.as_ref()).is_some_and(|s| s.project() == id && s.under(branch));
        let open = !closed || holds_selected;
        // As ui-react's SidebarRow: a chevron in a step of its own folds the commits away; a
        // branch without any has a dot there, keeping it in line with those that fold.
        let toggle = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            this.set_open(ClickOpened::Here(key.clone()), !open, cx);
            // Folding it over its selected commit takes the selection up to it, as ⇧← does.
            if open && holds_selected {
                let (project, ref_name) = key.clone();
                this.select(Selection::Branch(project, ref_name), window, cx)
            }
        };
        let folds = !branch.commits.is_empty();
        let step = self.fold(
            ElementId::Name(format!("fold-{id}-{row_id}").into()),
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
                .row(format!("branch-{id}-{row_id}"), 6.)
                .aria_label(format!("Branch {name}"))
                .child(step)
                .child(match icon(glyph).text_color(style.meta(glyph_color)) {
                    glyph if publishing => pulsing(glyph).into_any_element(),
                    glyph => glyph.into_any_element(),
                })
                .child(match self.rename_field(id, ref_name, theme) {
                    Some(field) => field,
                    None => branch_name(name, review, &style)
                        .id(ElementId::Name(format!("name-{id}-{row_id}").into()))
                        .flex_1()
                        .tooltip(Tooltip::text(tip))
                        .into_any_element(),
                })
                // CI only while it runs or after it failed: passing is the usual state.
                .when_some(
                    checks.filter(|c| c.running > 0 || c.failed > 0),
                    |el, checks| {
                        let (glyph, color, text) = review::checks_summary(checks, theme);
                        let tip = format!("{text}, click to check again");
                        let (store, project) = (self.store.clone(), id.clone());
                        el.child(
                            div()
                                .id(ElementId::Name(format!("checks-{row_id}").into()))
                                .child(small_icon(glyph).text_color(style.meta(color)))
                                .tooltip(Tooltip::text(tip))
                                // Checking again doesn't select the branch or open it.
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    store.update(cx, |store, cx| {
                                        store.refresh_forge(&project);
                                        store.snack("Checking CI again", cx)
                                    })
                                }),
                        )
                    },
                )
                // So conflicts show while the commits are hidden too.
                .children(meta_count(
                    Icon::Warning,
                    conflicted,
                    style.meta(theme.warn),
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
                // While it pushes, its count pulses where it is.
                .children(style.unpushed(unpushed_n).map(|count| {
                    match ref_name.is_some_and(|r| store.pushing(id, r)) {
                        true => pulsing(count).into_any_element(),
                        false => count.into_any_element(),
                    }
                }))
                // Folded, how many commits it holds; open, they're listed under it.
                .when(!open, |el| el.children(style.commits(branch.commits.len())))
                .when(unread, |el| el.child(dot(style.meta(theme.pop))))
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                // A click looks inside, opening its commits; the chevron, a double-click, ⇧←
                // and ⇧→ fold them.
                .when(folds, |el| el.on_click(click_opens(clicked, toggle, cx)))
                .when_some(ref_name, |el, ref_name| {
                    let payload = Cut::Branch(id.clone(), ref_name.clone());
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
            (Icon::Warning, theme.warn)
        } else {
            match commit.state {
                CommitState::Local => (Icon::Commit, theme.blue),
                CommitState::Pushed => (Icon::CommitFill, style.muted()),
                CommitState::Integrated => (Icon::CommitFill, theme.purple),
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
        Some(
            style
                .commit_line(
                    format!("commit-{id}-{}", commit_id.0),
                    COMMIT_INDENT,
                    small_icon(glyph).text_color(style.meta(color)),
                    label,
                    store.is_unread(id, commit),
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
    /// where a commit has its glyph, and a dot while it's unread, as only a branch's get.
    /// It isn't dragged or edited here.
    fn upstream_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        ref_name: Option<&FullName>,
        commit_id: &CommitId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let workspace = store.workspace(id)?;
        let listed = (workspace.upstream(ref_name).iter()).find(|c| c.id == *commit_id);
        // One a merge listed brought in sits a step in under it.
        let (title, merge, nested) = match listed {
            Some(commit) => (commit.title.clone(), commit.merge, false),
            None => {
                let (_, commit) = store.merged_upstream(id, ref_name, *commit_id)?;
                (commit.title.clone(), false, true)
            }
        };
        let title: SharedString = title.into();
        let unread = listed.is_some_and(|commit| ref_name.is_some() && store.is_unread(id, commit));
        let warn = style.meta(style.theme.warn);
        // A merge folds the commits it brought in on a double-click, or by a chevron in its
        // ↓'s place, as wide as the ↓ in the row's flow so its title lines up with theirs.
        let toggle = merge.then(|| {
            let (project, owner, merge_id) = (id.clone(), ref_name.cloned(), *commit_id);
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_merge(&project, owner.clone(), merge_id, window, cx)
            }
        });
        let glyph = match toggle.clone() {
            None => small_icon(Icon::ArrowDown)
                .text_color(warn)
                .into_any_element(),
            Some(toggle) => {
                let open = self.open_merges.contains(commit_id)
                    || matches!(&store.selection, Some(Selection::Upstream(_, _, c))
                        if store.merged_upstream(id, ref_name, *c).is_some_and(|(m, _)| m == *commit_id));
                let fold = ElementId::Name(format!("merge-fold-{}", commit_id.0).into());
                fold_button(fold, Some(open), warn, cx, toggle)
                    .mx(fixed(-1.))
                    .into_any_element()
            }
        };
        Some(
            style
                // Under its heading's words, as a branch's commits are under its name.
                .commit_line(
                    format!("upstream-{id}-{}", commit_id.0),
                    upstream_indent(ref_name) + if nested { STEP } else { 0. },
                    glyph,
                    title,
                    unread,
                )
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .when_some(toggle, |el, toggle| el.on_click(double_click(toggle, cx)))
                .into_any_element(),
        )
    }

    /// Over `project`'s inactive worktrees, as Network worktrees is over its checkouts
    /// elsewhere: folded, it says how many. The selection in one keeps them listed.
    fn inactive_heading(
        &self,
        ix: usize,
        project: &ProjectId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let inactive = store.inactive_worktrees(project);
        let open = inactive_open(store, project, &inactive);
        let label = "Inactive worktrees";
        self.section_heading(ix, project, &Section::Inactive, label, style, cx)
            .when(!open, |el| {
                el.child(faint(inactive.len().to_string(), style.theme).text_color(style.quiet()))
            })
            .tooltip(Tooltip::text(
                "Nothing uncommitted, and no commits or checkouts in 12 hours",
            ))
            .into_any_element()
    }

    /// Whether the rows under a section's heading are listed.
    fn heading_open(&self, project: &ProjectId, section: &Section, cx: &App) -> bool {
        let store = self.store.read(cx);
        match section {
            Section::Upstream(ref_name) => self.upstream_open(store, project, ref_name.as_ref()),
            Section::Inactive => inactive_open(store, project, &store.inactive_worktrees(project)),
            Section::Mesh => self.mesh_open(project, cx),
        }
    }

    /// Fold the rows under a section's heading, or with `open` false list them. Folding them
    /// over the selection takes it up to the heading, as a tree does. Under the filter, which
    /// shows what it finds, it does nothing.
    fn toggle_heading(
        &mut self,
        project: ProjectId,
        section: Section,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.filtering(cx) {
            return;
        }
        let store = self.store.read(cx);
        let inside = match &section {
            Section::Upstream(ref_name) => {
                Self::upstream_selected(store, &project, ref_name.as_ref())
            }
            Section::Inactive => in_inactive(store, &project, &store.inactive_worktrees(&project)),
            Section::Mesh => {
                matches!(&store.selection, Some(Selection::Mesh(p, ..)) if *p == project)
            }
        };
        // Up to the heading first, which keeps open what the selection held open, then fold.
        if open && inside {
            self.select(
                Selection::Section(project.clone(), section.clone()),
                window,
                cx,
            );
        }
        self.set_heading_open(&project, &section, !open, cx);
    }

    /// Whether the rows under a section's heading were listed by hand, not only while the
    /// selection is in them.
    fn listed_by_hand(&self, project: &ProjectId, section: &Section, cx: &App) -> bool {
        let prefs = &self.store.read(cx).prefs;
        match section {
            Section::Upstream(ref_name) => {
                (self.open_upstream).contains(&(project.clone(), ref_name.clone()))
            }
            Section::Inactive => prefs.shown_inactive.contains(project.as_ref()),
            Section::Mesh => prefs.shown_mesh.contains(project.as_ref()),
        }
    }

    /// List the rows under a section's heading, or fold them away, by hand: they stay as
    /// left, the inactive worktrees and the checkouts elsewhere for the next launch too.
    fn set_heading_open(
        &mut self,
        project: &ProjectId,
        section: &Section,
        open: bool,
        cx: &mut Context<Self>,
    ) {
        let shown: fn(&mut Prefs) -> &mut HashSet<String> = match section {
            Section::Upstream(ref_name) => {
                let key = (project.clone(), ref_name.clone());
                match open {
                    true => self.open_upstream.insert(key),
                    false => self.open_upstream.remove(&key),
                };
                return cx.notify();
            }
            Section::Inactive => |p| &mut p.shown_inactive,
            Section::Mesh => |p| &mut p.shown_mesh,
        };
        let key = project.to_string();
        self.store.update(cx, |store, cx| {
            store.set_prefs(
                |prefs| {
                    match open {
                        true => shown(prefs).insert(key),
                        false => shown(prefs).remove(&key),
                    };
                },
                cx,
            )
        });
    }

    /// Update, which rebases `project`'s branches onto its target's new commits; `inverted`
    /// on a solid selection. Pressing it leaves the selection where it is.
    fn target_update_button(&self, project: &ProjectId, inverted: bool, cx: &Context<Self>) -> Div {
        let updating = self.store.read(cx).updating(project);
        let project = project.clone();
        let update = button(
            ElementId::Name(format!("update-{project}").into()),
            "Update",
        )
        .small()
        .style(ButtonStyle::Gray)
        .inverted(inverted)
        .disabled_because(updating.then_some("Updating already"))
        .tooltip_for(
            "Rebase your branches onto the target's new commits",
            &UpdateFromTarget,
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            let project = project.clone();
            this.app.update_later(window, cx, move |app, _, cx| {
                app.update_from_target(project, cx)
            });
        }));
        div()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // Pulsing while it updates, rather than a longer label moving what's beside it.
            .child(match updating {
                true => pulsing(div().child(update)).into_any_element(),
                false => update.into_any_element(),
            })
    }

    /// Over the commits only branch `ref_name`'s remote has, or with `None` the target's new
    /// ones, a section heading as Network worktrees has: where they are, how many while
    /// folded and whether any is unread, and for a branch, Pull.
    fn upstream_heading(
        &self,
        ix: usize,
        project: &ProjectId,
        ref_name: Option<&FullName>,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let theme = style.theme;
        let workspace = store.workspace(project);
        let open = self.upstream_open(store, project, ref_name);
        let target = workspace.and_then(|ws| ws.target.as_ref());
        let (label, count, tip) = match ref_name {
            Some(ref_name) => {
                let branch = workspace.and_then(|ws| ws.find_branch(Some(ref_name)));
                let count = branch.map_or(0, |b| b.incoming.len());
                let label = match branch.and_then(|b| b.remote.as_ref()) {
                    Some(remote) => format!("On {remote}"),
                    None => "On the remote".to_owned(),
                };
                (label, count, plural(count, "commit"))
            }
            None => {
                let count = target.map_or(0, |t| t.behind);
                let label = match target {
                    Some(target) => format!("New on {}", target.name),
                    None => "New on the target".to_owned(),
                };
                let tip = plural(count, "commit") + &target.map(with_merged).unwrap_or_default();
                (label, count, tip)
            }
        };
        let update = ref_name.is_none() && target.is_some_and(|t| t.needs_update);
        // Only a branch's are marked read.
        let unread = (ref_name.zip(workspace)).is_some_and(|(ref_name, ws)| {
            (ws.upstream(Some(ref_name)).iter()).any(|commit| store.is_unread(project, commit))
        });
        let pull = (ref_name.filter(|r| workspace.is_some_and(|ws| ws.can_pull(r))))
            .map(|r| (project.clone(), r.clone()));
        let section = Section::Upstream(ref_name.cloned());
        let row = self.section_heading(ix, project, &section, label, style, cx);
        let row = row
            .pr(dp(8.))
            .when(!open, |el| {
                el.children(meta_count(
                    Icon::ArrowDown,
                    count,
                    style.meta(theme.warn),
                    tip,
                ))
                .when(unread, |el| el.child(dot(style.meta(theme.pop))))
            })
            .when(update, |el| {
                el.child(self.target_update_button(project, style.solid, cx))
            })
            .children(pull.map(|(project, ref_name)| {
                // Pressing it leaves the selection where it is, as Update does.
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        button(ElementId::Name(format!("pull-{ix}").into()), "Pull")
                            .small()
                            .style(ButtonStyle::Gray)
                            .inverted(style.solid)
                            .tooltip("Bring them in, with this branch's own commits on top")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let ref_name = ref_name.clone();
                                (this.store).update(cx, |store, cx| {
                                    store.pull_remote(&project, ref_name, None, cx)
                                })
                            })),
                    )
            }));
        Some(row.into_any_element())
    }

    /// The heading over `section` of `project`'s card: its fold, its glyph and `label`,
    /// quiet, its fold set in as a row's would be (see [`RowStyle::row`]). Selecting it shows
    /// what it folds on its page; its chevron, a double-click or Enter lists or folds them.
    fn section_heading(
        &self,
        ix: usize,
        project: &ProjectId,
        section: &Section,
        label: impl Into<SharedString>,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let label = label.into();
        let (indent, glyph) = match section {
            // A branch's under its commit glyphs, the target's as Network worktrees has its own.
            Section::Upstream(ref_name) => (
                upstream_chevrons(ref_name.as_ref()).0 - 7. - ROW_LEFT,
                ref_name.is_none().then_some(Icon::ArrowLineDown),
            ),
            Section::Inactive => (6., Some(Icon::Clock)),
            Section::Mesh => (6., Some(Icon::Globe)),
        };
        let open = self.heading_open(project, section, cx);
        let item = Selection::Section(project.clone(), section.clone());
        let toggle = {
            let (project, section) = (project.clone(), section.clone());
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_heading(project.clone(), section.clone(), open, window, cx)
            }
        };
        let quiet = style.quiet();
        (style.row(format!("section-heading-{ix}"), indent))
            .gap(dp(6.))
            .pr(dp(12.))
            .aria_label(label.clone())
            .aria_expanded(open)
            .child(self.fold(("section-fold", ix), Some(open), quiet, cx, toggle.clone()))
            .children(glyph.map(|glyph| small_icon(glyph).text_color(quiet)))
            .child(
                (section_label(label, style.theme).text_color(quiet))
                    .flex_1()
                    .min_w_0()
                    .truncate(),
            )
            .on_mouse_down(MouseButton::Left, self.select_on_press(&item, cx))
            .on_click(double_click(toggle, cx))
    }
}

/// A row of words in place of others: why a project has none, or what's loading.
fn note_row(text: &SharedString, is_error: bool, theme: &Theme) -> AnyElement {
    h_flex()
        .min_h(dp(ROW_HEIGHT))
        .px(dp(12.))
        .py(dp(4.))
        .gap(dp(6.))
        .text_size(dp(12.))
        .text_color(if is_error { theme.danger } else { theme.text_2 })
        .when(is_error, |el| {
            el.child(small_icon(Icon::Warning).text_color(theme.danger))
        })
        // A long reason, as a failing filter's command twice over, stays on the project's
        // page, and its menu copies it.
        .child(
            (div().flex_1().min_w_0())
                .line_clamp(4)
                .text_ellipsis()
                .child(text.clone()),
        )
        .into_any_element()
}

/// Tell `list` its rows from `at` changed from `old` to `new`: the ones between what both
/// start and end with, which it measures again. Whether any did.
fn splice(list: &ListState, old: &[Row], new: &[Row], at: usize) -> bool {
    let head = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let (old, new) = (
        old.get(head..).unwrap_or_default(),
        new.get(head..).unwrap_or_default(),
    );
    let tail = (old.iter().rev().zip(new.iter().rev()))
        .take_while(|(a, b)| a == b)
        .count();
    let changed = old.len() > tail || new.len() > tail;
    if changed {
        list.splice(at + head..at + head + old.len() - tail, new.len() - tail);
    }
    changed
}

/// Whether `list` shows its first row from its top.
fn at_top(list: &ListState) -> bool {
    let top = list.logical_scroll_top();
    top.item_ix == 0 && top.offset_in_item == Pixels::ZERO
}

/// The rows of `rows` that can be selected, in order.
fn items<'a>(rows: impl IntoIterator<Item = &'a Row>) -> Vec<&'a Selection> {
    (rows.into_iter())
        .filter_map(|row| match row {
            Row::Item(item) => Some(item),
            _ => None,
        })
        .collect()
}

/// Whether `new` stands where `old` did, to keep it under the pointer: the same row, or the
/// other side of a fold, as "Show fewer" becomes "N more".
fn same_place(old: &Row, new: &Row) -> bool {
    match (old, new) {
        (Row::Fewer(project, ref_name), Row::More(p, r, _)) => project == p && ref_name == r,
        _ => old == new,
    }
}

/// What ⇧← goes up to from `selection`, a branch or a stack: a branch in a stack to the stack,
/// and a stack, or a branch alone, to its worktree's heading.
fn parent(store: &Store, selection: &Selection) -> Option<Selection> {
    let project = selection.project().clone();
    let workspace = store.workspace(&project)?;
    let bottom = match selection {
        Selection::Branch(_, ref_name) => {
            let (lane, _) = workspace.lane_of(ref_name.as_ref())?;
            match &lane.branches[..] {
                [_, .., bottom] => {
                    return Some(Selection::Stack(project, bottom.ref_name.clone()));
                }
                _ => ref_name.clone(),
            }
        }
        Selection::Stack(_, bottom) => bottom.clone(),
        _ => return None,
    };
    Some(
        match store.worktree_for(&Selection::Branch(project.clone(), bottom)) {
            Some(worktree) => Selection::Worktree(project, worktree.name.clone()),
            None => Selection::Uncommitted(project),
        },
    )
}

/// How many uncommitted files a checkout has, how many of them a merge left conflicted, and
/// the lines the rest add and remove, as [`change_counts`] takes them.
type Tally = (usize, usize, (u32, u32));

/// The [`Tally`] of a worktree's uncommitted `files`.
fn tally(files: &[FileChange]) -> Tally {
    let conflicted = files.iter().filter(|f| f.conflicted).count();
    // A conflicted file's lines are its markers, which the file list doesn't count either.
    let lines = (files.iter().filter(|f| !f.conflicted))
        .filter_map(|f| f.lines)
        .fold((0, 0), |(a, r), (fa, fr)| (a + fa, r + fr));
    (files.len(), conflicted, lines)
}

/// What a checkout's uncommitted files count: what a merge left conflicted, as an update can,
/// first, then the lines they add and remove, or with none counted, how many there are.
fn change_counts(
    (n, conflicted, (added, removed)): Tally,
    style: RowStyle,
) -> impl Iterator<Item = Stateful<Div>> {
    let theme = style.theme;
    let conflicts = meta_count(
        Icon::Warning,
        conflicted,
        style.meta(theme.warn),
        plural(conflicted, "conflicted file"),
    );
    let all = match added + removed {
        0 => style.files(n),
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

/// A worktree heading's folder, a linked one's doubled, with a dot in its corner while it has
/// uncommitted changes, as an edited document has.
fn worktree_icon(linked: bool, changed: bool, style: RowStyle) -> Div {
    let glyph = match (linked, changed) {
        (false, false) => Icon::Folder,
        (false, true) => Icon::FolderDot,
        (true, false) => Icon::FolderCopy,
        (true, true) => Icon::FolderCopyDot,
    };
    div()
        .relative()
        .flex_none()
        .child(icon(glyph).text_color(style.muted()))
        .when(changed, |el| {
            let color = style.meta(style.theme.warn);
            el.child(dot(color).absolute().top_0().right_0().size(dp(5.)))
        })
}

/// How many commits the target's merges brought in besides, as " (N counting merged
/// commits)" after its count, which counts each merge once as the project's page lists them,
/// or nothing when they brought in none.
pub(crate) fn with_merged(target: &Target) -> String {
    match target.commits_behind > target.behind {
        true => format!(" ({} counting merged commits)", target.commits_behind),
        false => String::new(),
    }
}

/// How many of `branches`' commits are only local.
fn unpushed<'a>(branches: impl Iterator<Item = &'a Branch>) -> usize {
    (branches.flat_map(|branch| &branch.commits))
        .filter(|commit| commit.state == CommitState::Local)
        .count()
}

/// A branch as the navigator names it: its pull request's title once the forge cache has one,
/// toned by the request's state, or else `name`. Drawn from what the last load read, so no row
/// waits on the network, and offline the cached title stays.
fn branch_name(name: &str, review: Option<(&Review, ReviewState)>, style: &RowStyle) -> Div {
    let theme = style.theme;
    let el = div().min_w_0().truncate();
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
            drawer,
            ..
        } = self.build_rows(cx);
        self.insets = insets;
        self.guides = guides;
        self.sync_list(rows, drawer, window, cx);
        let theme = Theme::get(cx).clone();
        self.first_hit = first_hit;
        let store = self.store.read(cx);
        let empty = store.projects_loaded && store.projects.is_empty();
        let filter_focused = self.filter.focus_handle(cx).is_focused(window);
        let filtering = !self.filter.read(cx).text().is_empty();

        // Once the cards scroll under it, a rule along its bottom, as but.dev draws one, and
        // level with the location bar's: the same height and border, from the window's top.
        let under = !at_top(&self.list);
        let header = h_flex()
            .h(dp(HEADER_HEIGHT))
            .flex_none()
            .border_b_1()
            .border_color(match under {
                true => theme.border_3,
                false => gpui::transparent_black(),
            })
            // Room for macOS's traffic lights; elsewhere the window's buttons are at its other end.
            .map(|el| match cfg!(target_os = "macos") {
                true => el.pl(fixed(84.)),
                false => el.pl(dp(8.)),
            })
            .pr(dp(8.))
            .map(crate::ui::drags_window)
            .gap(dp(6.))
            // Its clear button inside it, so typing the first letter doesn't narrow it.
            .child(
                above_drag(field(&self.filter, filter_focused, &theme))
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .py(dp(3.))
                    .when(filtering, |el| {
                        el.pr(dp(26.)).child(
                            h_flex().absolute().top_0().bottom_0().right(dp(2.)).child(
                                icon_button("clear-filter", Icon::Cross)
                                    .small()
                                    .tooltip("Clear the filter")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.filter.update(cx, |input, cx| input.set_text("", cx));
                                        this.filter_changed(cx);
                                    })),
                            ),
                        )
                    }),
            )
            .children(self.update_button(cx).map(|el| above_drag(div().child(el))))
            .child(above_drag(div().child(self.view_button(cx))))
            .child(above_drag(div().child(self.account(&theme, cx))));

        v_flex()
            .id("navigator")
            .key_context("Navigator")
            .on_drag_move(cx.listener(Self::resize_drawer))
            .track_focus(&self.focus)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.pointer_over = *hovered;
                // GPUI calls a drag's start a leave, and the drop has to land where it's aimed.
                if !*hovered && !cx.has_active_drag() {
                    this.held_order = None;
                    this.held_worktrees.clear();
                } else if this.held_order.is_none() {
                    let watched = this.watched(this.store.read(cx));
                    this.held_worktrees = (watched.iter())
                        .filter_map(|e| Some((e.id.clone(), e.workspace.as_ref()?)))
                        .map(|(id, ws)| (id, ws.worktrees.iter().map(|w| w.name.clone()).collect()))
                        .collect();
                    this.held_order = Some(watched.into_iter().map(|e| e.id.clone()).collect());
                }
                cx.notify();
            }))
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
                // With no projects, the welcome beside it says what to do.
                if empty {
                    el
                } else if self.rows.is_empty() {
                    el.child(self.drawer(&theme, cx))
                } else {
                    let rows = list(
                        self.list.clone(),
                        cx.processor(|this, ix: usize, window, cx| this.render_row(ix, window, cx)),
                    )
                    .size_full()
                    .pt(dp(2.))
                    .pb(dp(12.));
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
                    .child(self.drawer(&theme, cx))
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
