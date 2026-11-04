//! The left column: every project as a card, each listing its uncommitted changes,
//! branches and commits as one tree of rows.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use but_tty::Status;
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, DragMoveEvent, ElementId, Entity,
    FocusHandle, Focusable, Hsla, ListAlignment, ListOffset, ListState, Modifiers, MouseButton,
    MouseDownEvent, PathBuilder, Pixels, Point, Role, ScrollWheelEvent, SharedString, Stateful,
    Subscription, Task, Window, actions, canvas, div, fill, list, point, prelude::*, relative,
};

use crate::app::{
    AddProject, AppHandle, Collapse, Confirm, Expand, SelectNext, SelectPrevious, ToggleFold,
    UpdateFromTarget,
};
use crate::backend::upstream::UpdateScope;
use crate::backend::{
    Branch, Checkout, Commit, CommitId, CommitState, FileChange, FullName, PushState, Review,
    ReviewState, Target, Workspace,
};
use crate::clock;
use crate::detail::{ChecksChanged, Detail, oldest_conflict};
use crate::overlays::{MenuItem, keys_at};
use crate::state::{
    Axis, Cut, DividerDrag, Pane, Prefs, ProjectId, Section, Selection, Store, StoreEvent, Tile,
    WatchedOrder,
};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::transfer::{
    Move, Op, PageDrag, ProjectDrag, Stop, TerminalDrag, drag_preview, page_preview, row_preview,
};
use crate::ui::emoji::{Colour, ProjectIcon};
use crate::ui::fold::{self, Folds, Reorder, clipped, curtain, inert, on_top, reordered, shifted};
use crate::ui::scrollbar::{Scroll, scrollbar, scrolled_in_gap};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    BLINK, ButtonStyle, HEADER_HEIGHT, Icon, POPOVER_RADIUS, ROW_HEIGHT, TILE_MARGIN, Tooltip,
    WINDOW_CONTROLS_WIDTH, above_drag, ago, blink_once, button, commit_title, dot, dp, faint,
    field, fixed, fold_button, fold_glyph, fold_row, gap_divider, glow_once, guide, h_flex, icon,
    icon_button, list_row, mark_dragged, meta_count, plural, pulsing, relative_time, review,
    section_label, small_icon, spinning, unread_line, v_flex,
};

mod account;
mod drop;
mod menu;
pub(crate) use menu::{
    ArchiveWorktree, CloseTerminal, DeleteWorktree, OpenProjectSettings, Reveal, StopWatching,
    UnarchiveWorktree, Watch,
};
pub(crate) use order::{
    ShowActiveProjects, ShowAllProjects, SortProjectsByActivity, SortProjectsByName,
};
mod mesh;
pub(crate) use mesh::{mesh_choices, mesh_heading_choice};
mod moving;
mod order;
mod rename;
mod rows;
pub(crate) use rows::{lists_changes, worktrees_listed};

use moving::Mark;
use rows::{
    CARD_EDGE, CARD_PADDING, COMMIT_INDENT, Drawer, Fold, GUIDE_AT, HEADING_HEIGHT, Moving,
    ROW_LEFT, Row, RowKey, Rows, in_checkout, in_inactive, inactive_open, last_active, row_keys,
    span_height,
};

/// The room a pressable mark has around it for its tint under the pointer. A negative margin
/// to keep it in place would wrap it off a project's summary.
const MARK_PAD: f32 = 3.;

/// How far apart the cards are, as the detail's are.
pub(super) const CARD_GAP: f32 = 8.;

/// How wide and tall the map of a row's tiles is, and the gap between them there.
const TILE_MAP: (f32, f32) = (20., 14.);
const TILE_MAP_GAP: f32 = 1.5;

/// How far the cards sit in from the list's sides, as far as the detail's cards are from the
/// window's edges.
const CARD_MARGIN: f32 = 8.;

/// How far they sit in from its side beside the detail: with the navigator's edge, the whole
/// gap to the detail's cards, as wide as the gaps between those, which the detail's cards sit
/// right against, so the gap's divider and the list's scrollbar are the navigator's own.
const CARD_MARGIN_BESIDE: f32 = 7.;

/// How far below its top the list lays out its first row.
const LIST_TOP: f32 = 0.;

/// How many rows above the last one the list drew last frame draws the rows sliding into view
/// from past its bottom, so it's drawn though the rows moved since.
const ABOVE_LAST_LISTED: usize = 2;

/// How tall the drawer's handle is, level with the commit footer beside it, and the space
/// under its rows.
const DRAWER_BAR_HEIGHT: f32 = crate::detail::FOOTER_HEIGHT;
const DRAWER_PADDING: f32 = 6.;

/// How far the drawer's bar sits in from its card's edges, as a row from a card's.
const DRAWER_INSET: f32 = 3.;

/// How tall the drawer's card is around its rows: its bar, and its edges.
const DRAWER_CHROME: f32 = DRAWER_BAR_HEIGHT + 2. * DRAWER_INSET + 2.;

/// How far a worktree's rows sit in from its heading, as ui-react's SidebarRow steps them:
/// their chevrons' middle under the heading's 16-point glyph's, the chevron's step, 14, the
/// gap, 8, and half the glyph past the chevron's half.
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
/// the row waiting for ⌘V or being moved, `held` a copy of what's being moved, or a terminal
/// going where it'd land itself, `mark` the colour cut or held rows are marked in, and `inset`
/// is how far in its section sets it.
#[derive(Clone, Copy)]
struct RowStyle<'a> {
    theme: &'a Theme,
    selected: bool,
    solid: bool,
    cut: bool,
    held: bool,
    mark: Hsla,
    inset: f32,
    /// The rows above and below are picked with it: see [`crate::ui::joined`].
    joined: (bool, bool),
    /// The rows above and below are outlined as it is, cut or moving with it, so a run of
    /// them is outlined as one, as a branch is with its commits.
    outlined_with: (bool, bool),
    /// A commit that just showed up, not selected: see [`crate::ui::glow_once`].
    glow: bool,
    /// The selection just copied, or where a move would land as a click it can't use reminds
    /// the user of it, by which blink it is, in `mark`: see [`crate::ui::blink_once`].
    blink: Option<usize>,
    /// Where Go to anything goes: see [`Navigator::going_to`].
    goes_here: bool,
}

/// What a row's blink is of: see [`Navigator::blinked`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Blink {
    /// The selection, just copied.
    Copied,
    /// Where the move under way would land, as a click it can't use reminds the user of it.
    Move,
}

/// Where a step of the keys lands.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// The item at this index of those listed.
    Item(usize),
    /// The drawer's bar, between the cards' rows and its own.
    Bar,
}

/// Where a step of `delta` goes from item `at` of `items`, or from the drawer's bar while
/// `on_bar`, with `bar` how many items are above the bar, if it's a stop. From off the list, a
/// step forward starts at the top and one back at the bottom. Past an end it's `None`, rather
/// than picking the row again, which would take its page back to how it opens on every repeat
/// of a held key.
fn step_to(
    at: Option<usize>,
    items: usize,
    bar: Option<usize>,
    on_bar: bool,
    delta: isize,
) -> Option<Step> {
    let stops = (items + usize::from(bar.is_some())) as isize;
    let from = match (bar.filter(|_| on_bar), at) {
        (Some(bar), _) => bar as isize,
        (None, Some(ix)) if bar.is_some_and(|bar| ix >= bar) => ix as isize + 1,
        (None, Some(ix)) => ix as isize,
        (None, None) if delta > 0 => -1,
        (None, None) => stops,
    };
    let next = (from + delta).clamp(0, stops - 1);
    if next == from {
        return None;
    }
    let next = next as usize;
    Some(match bar {
        Some(bar) if next == bar => Step::Bar,
        Some(bar) if next > bar => Step::Item(next - 1),
        Some(_) | None => Step::Item(next),
    })
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
/// does any while the filter lists what it finds, or while something is being moved, which
/// the click puts there.
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
        let store = this.store.read(cx);
        let on = store.prefs.open_branch_on_click && store.moving.is_none();
        if on && event.click_count() == 1 && !held.secondary() && !held.shift && !this.filtering(cx)
        {
            this.click_just_opened = !this.is_open(&branch);
            this.open_on_click(branch.clone(), cx)
        }
    })
}

impl<'a> RowStyle<'a> {
    /// A row set in by `inset` that nothing selects, picks or marks.
    fn plain(theme: &'a Theme, inset: f32) -> Self {
        RowStyle {
            theme,
            selected: false,
            solid: false,
            cut: false,
            held: false,
            mark: theme.pop,
            inset,
            joined: (false, false),
            outlined_with: (false, false),
            glow: false,
            blink: None,
            goes_here: false,
        }
    }

    fn row(&self, id: String, indent: f32) -> Stateful<Div> {
        let theme = self.theme;
        list_row(ElementId::Name(id.into()), self.selected, theme)
            .when_some(self.blink, |el, n| {
                let shape = div().absolute().inset_0().rounded(dp(4.));
                el.relative()
                    .child(blink_once(("blink", n), shape, self.mark))
            })
            // A pop wash, as a commit that just showed up glows: quieter than a drop's outline.
            .when(self.goes_here, |el| {
                let wash = div().absolute().inset_0().rounded(dp(4.));
                el.relative().child(wash.bg(theme.pop.opacity(0.1)))
            })
            .mx(dp(3.))
            .pl(dp(8. + indent + self.inset))
            .when(self.solid, |el| {
                el.bg(theme.fill_gray).text_color(theme.text_inverted)
            })
            .when(self.cut, |el| {
                (el.opacity(0.5).border_1().border_dashed()).border_color(self.mark)
            })
            // As the copies of what's moving are drawn where it would land.
            .when(self.held, |el| {
                el.bg(self.mark.opacity(0.14))
                    .border_1()
                    .border_color(self.mark)
            })
            .map(|el| {
                let (above, below) = self.outlined_with;
                el.when(above, |el| el.border_t_0().rounded_t_none())
                    .when(below, |el| el.border_b_0().rounded_b_none())
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

    /// How many of a row's commits aren't pushed, when any aren't, quiet: only a branch's own
    /// count pushes, in blue (see `Navigator::push_count`).
    fn unpushed(&self, n: usize) -> Option<Stateful<Div>> {
        meta_count(Icon::ArrowUp, n, self.muted(), plural(n, "unpushed commit"))
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

    /// An unread row's hairline: a commit's own in pop, and in faint ink, which still reads at
    /// 3:1, on a row that only holds unread commits, so the commits stand out.
    fn unread_line(&self, own: bool) -> Div {
        unread_line(if own {
            self.meta(self.theme.pop)
        } else {
            self.quiet()
        })
    }

    /// A commit's row, `indent` in: `glyph` and its title, or `rename`, the field it's being
    /// edited in, with a hairline while it's unread. Only a missing title is quieter.
    fn commit_line(
        &self,
        id: String,
        indent: f32,
        glyph: impl IntoElement,
        title: SharedString,
        unread: bool,
        rename: Option<AnyElement>,
    ) -> Stateful<Div> {
        self.row(id, indent)
            .gap(dp(10.))
            .aria_label(title.clone())
            // Under its row's id, which goes by the commit.
            .when(self.glow, |el| {
                el.relative().child(glow_once("glow", self.theme))
            })
            .when(unread, |el| el.relative().child(self.unread_line(true)))
            .child(glyph)
            .child(match rename {
                Some(field) => field,
                None => div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(title.is_empty(), |el| el.text_color(self.muted()))
                    .child(commit_title(&title))
                    .into_any_element(),
            })
    }

    /// `commit` of `project` as its row draws it, `id` the row's: its glyph by its state, and
    /// its title as last reworded, or `rename`, the field it's being edited in.
    fn commit(
        &self,
        id: String,
        project: &ProjectId,
        commit: &Commit,
        store: &Store,
        rename: Option<AnyElement>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let (glyph, color) = if commit.conflicted {
            (Icon::Warning, theme.warn)
        } else {
            match commit.state {
                CommitState::Local => (Icon::Commit, theme.blue),
                CommitState::Pushed => (Icon::CommitFill, self.muted()),
                CommitState::Integrated => (Icon::CommitFill, theme.purple),
            }
        };
        let glyph = small_icon(glyph).text_color(self.meta(color));
        let (title, _) = store.message_of(project, commit);
        let unread = store.is_unread(project, commit);
        self.commit_line(
            id,
            COMMIT_INDENT,
            glyph,
            title.to_owned().into(),
            unread,
            rename,
        )
    }

    /// The copy of a commit titled `title` from outside the workspace a cherry-pick would
    /// make, `id` the row's: a local commit.
    fn copy(&self, id: String, title: SharedString) -> Stateful<Div> {
        let glyph = small_icon(Icon::Commit).text_color(self.meta(self.theme.blue));
        self.commit_line(id, COMMIT_INDENT, glyph, title, false, None)
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
    /// Each row's place in the fold it moves with, while one does.
    moving: Vec<Option<Moving>>,
    /// The folds opening or closing now.
    folds: Folds<Fold>,
    /// The rows sliding to where a reorder put them, and each row's key, to tell which.
    reorder: Reorder<RowKey>,
    row_keys: Vec<Option<RowKey>>,
    /// Rows a move just took away, drawn where they were while the rows close over them.
    stand_ins: Vec<moving::StandIn>,
    /// Whether each fold the rows were last built through showed what it holds, to tell
    /// which opened or shut since.
    folds_seen: HashMap<Fold, bool>,
    pub(crate) list: ListState,
    /// The projects not watched as last drawn, and the list the drawer draws them in.
    drawer: Drawer,
    pub(crate) drawer_list: ListState,
    /// The room above the drawer at the list's end, and its card's height, as last worked out:
    /// see [`Self::fit_footer`].
    footer: Cell<(f32, f32)>,
    /// The keys stepped onto the drawer's bar, which folds but is nothing to select, so the
    /// selection and its page stay as they were: see [`Step`].
    on_drawer_bar: bool,
    /// The selection changed, or where it'd be moved: scroll to it once the rows are up to
    /// date, to the copies of commits being moved where they'd land rather than to them.
    reveal_selection: bool,
    /// What the move shown, or landing, did when the rows were last brought up to date.
    shown_op: Option<Op>,
    /// The move shown then was one Return landed, until the reload shows where it went, which
    /// takes what it moved from where it was: not a cherry-pick.
    shown_landing: bool,
    /// Where a squash's outline was when the rows were last brought up to date: it slides from
    /// there to what it goes into next.
    squash_outline: Option<moving::SquashOutline>,
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
    /// Drawn out over the detail while it's closed, as a card of its own, which the app
    /// sizes, rather than beside it.
    floating: bool,
    /// How much of the detail beside it shows, from 0 hidden to 1, as it slides: the app sets
    /// it before this draws. Hidden, the navigator takes the window, with nothing beside it
    /// to edge or resize it against.
    pub detail_shown: Cell<f32>,
    /// The watched projects' order by activity as it was when the pointer came over the
    /// list, kept until it leaves, so a project an agent wakes doesn't move the row about to
    /// be clicked.
    held_order: Option<Vec<ProjectId>>,
    /// The project whose watch button the pointer is on, which shows what a click makes it.
    watch_hovered: Option<ProjectId>,
    /// The rows of the card being moved, dragged by its heading or with the keys, as last
    /// drawn, which show it held.
    held_card: Option<Range<usize>>,
    /// The rows of the card the selection's in, while the navigator has the keys, which edge
    /// it as the tile with them is: see [`Self::find_focused_card`].
    focused_card: std::cell::RefCell<Option<Range<usize>>>,
    /// The project whose card is held, kept once it's let go of until the cards stop sliding,
    /// so it stays over the cards it passes going back.
    held_project: Option<ProjectId>,
    /// The linked worktree being dragged by its heading, by its project and name, which goes
    /// where it's let go, but over its own rows where it was, where what it carries lands.
    dragged_worktree: Option<(ProjectId, bstr::BString)>,
    /// The rows of the linked worktree being moved, dragged by its heading or with the keys, as
    /// last drawn, which go over the rows they pass.
    held_worktree: Option<Range<usize>>,
    /// The linked worktree whose rows are held, by its project and name, kept once it's let go
    /// of until the rows stop sliding, so it stays over the rows it passes going back.
    held_worktree_name: Option<(ProjectId, bstr::BString)>,
    /// While rows slide, the first row the list draws and one near its last, which draw the
    /// rows sliding into view from past its top and its bottom (see [`Self::render_row`]).
    slide_edges: Option<(usize, usize)>,
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
    /// How many times a row has blinked, and the last blink: what it was of, and when.
    blinked: (usize, Option<(Blink, Instant)>),
    /// Where the choice Go to anything has selected goes, while it's open.
    going_to: Option<Selection>,
    /// The row outlined for [`Self::going_to`]: its own, or while that's folded away, the one
    /// holding it. The list scrolls to show it as it changes.
    going_row: Option<Selection>,
    /// Where the list and the drawer were scrolled to before Go to anything scrolled them,
    /// which closing it without going anywhere puts back.
    scrolled_from: Option<(ListOffset, ListOffset)>,
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
            // The commit being placed is titled as its message is written, which notifies
            // nothing else.
            cx.subscribe(&detail.read(cx).message_editor(), |this, _, event, cx| {
                let placing = matches!(
                    this.store.read(cx).moving,
                    Some(Move { op: Op::Commit, .. })
                );
                if matches!(event, InputEvent::Changed) && placing {
                    cx.notify()
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
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::SelectionChanged => this.reveal(cx),
                StoreEvent::MoveToward(item) => this.click_move(item, window, cx),
                StoreEvent::Loaded(_) | StoreEvent::MeshRead(_) => this.hold_still = true,
                // A renamed branch stays open or folded as it was.
                StoreEvent::Renamed(project, from, to) => {
                    let to = Some(to.clone());
                    if this.flipped_folds.remove(&(project.clone(), from.clone())) {
                        this.flipped_folds.insert((project.clone(), to.clone()));
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
            moving: Vec::new(),
            folds: Folds::default(),
            reorder: Reorder::default(),
            row_keys: Vec::new(),
            stand_ins: Vec::new(),
            folds_seen: HashMap::new(),
            // Its last item, after the rows, is the drawer: see `drawer_footer`.
            list: ListState::new(1, ListAlignment::Top, fixed(56.)),
            drawer: Drawer::default(),
            drawer_list: ListState::new(0, ListAlignment::Top, fixed(56.)),
            footer: Cell::new((0., 0.)),
            on_drawer_bar: false,
            reveal_selection: false,
            shown_op: None,
            shown_landing: false,
            squash_outline: None,
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
            filter,
            first_hit: None,
            pointer_over: false,
            floating: false,
            detail_shown: Cell::new(1.),
            held_order: None,
            watch_hovered: None,
            held_card: None,
            focused_card: Default::default(),
            held_project: None,
            dragged_worktree: None,
            held_worktree: None,
            held_worktree_name: None,
            slide_edges: None,
            hold_still: false,
            edge_scroll: 0.,
            edge_scrolling: None,
            renaming: None,
            blinked: (0, None),
            going_to: None,
            going_row: None,
            scrolled_from: None,
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
            true => fold_glyph(id, Some(true), color).into_any_element(),
            false => fold_button(id, open, color, cx, toggle).into_any_element(),
        }
    }

    /// Whether `event` is a double-click that acts on its row, as a folder opens: not when
    /// its first click opened a branch.
    fn double_clicked(&mut self, event: &ClickEvent) -> bool {
        let just_opened = std::mem::take(&mut self.click_just_opened);
        event.click_count() == 2 && !just_opened
    }

    /// Hold the watched projects' order by activity as it is now (`held`), so rows don't move
    /// under the pointer, or let it go on.
    fn hold_order(&mut self, held: bool, cx: &Context<Self>) {
        match held {
            false => self.held_order = None,
            true if self.held_order.is_none() => {
                let watched = self.watched(self.store.read(cx));
                self.held_order = Some(watched.into_iter().map(|e| e.id.clone()).collect());
            }
            true => {}
        }
    }

    /// A drag moved, `over` the navigator or not. GPUI counts nothing as hovered while dragging,
    /// so no leave comes for a drag let go elsewhere, as on a terminal, and the order by
    /// activity would stay held: it's let go as the drag leaves, and held again as it comes back.
    fn drag_moved(&mut self, over: bool, cx: &mut Context<Self>) {
        if over != self.held_order.is_some() {
            self.hold_order(over, cx);
            cx.notify();
        }
    }

    /// Give the rows the keys, the selection drawn solid again.
    fn focus_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Blink the selection's row once, as it was just copied.
    pub(crate) fn blink_selection(&mut self, cx: &mut Context<Self>) {
        self.blink(Blink::Copied, cx)
    }

    /// Blink where the move under way would land, as a click it can't use reminds the user
    /// it's still on.
    pub(super) fn nudge_move(&mut self, cx: &mut Context<Self>) {
        self.blink(Blink::Move, cx)
    }

    // GPUI draws the blink itself, at the real time, so it's timed by that.
    fn blink(&mut self, of: Blink, cx: &mut Context<Self>) {
        self.blinked = (self.blinked.0 + 1, Some((of, Instant::now())));
        cx.notify();
    }

    /// Which blink `of` is in, if one is blinking, as [`crate::ui::blink_once`] needs it.
    fn blinking(&self, of: Blink) -> Option<usize> {
        let (n, last) = self.blinked;
        last.is_some_and(|(was, at)| was == of && at.elapsed() < BLINK)
            .then_some(n)
    }

    /// Outline the row where the choice Go to anything has selected goes, `target`, scrolling
    /// to show it, or with `None` no row.
    pub(crate) fn go_toward(&mut self, target: Option<Selection>, cx: &mut Context<Self>) {
        if self.going_to == target {
            return;
        }
        if target.is_some() && self.scrolled_from.is_none() {
            let lists = (&self.list, &self.drawer_list);
            self.scrolled_from = Some((lists.0.logical_scroll_top(), lists.1.logical_scroll_top()));
        }
        self.going_to = target;
        cx.notify();
    }

    /// Go to anything closed: no row is outlined, and when it went nowhere (`back`), the lists
    /// scroll back to where they were.
    pub(crate) fn stop_going(&mut self, back: bool, cx: &mut Context<Self>) {
        self.go_toward(None, cx);
        if let Some((list, drawer)) = self.scrolled_from.take()
            && back
        {
            self.list.scroll_to(list);
            self.drawer_list.scroll_to(drawer);
            cx.notify();
        }
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
        self.leave_drawer_bar(cx);
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
        self.leave_drawer_bar(cx);
        // While something is being moved the selection stays on it, which goes toward what's
        // asked for instead, before the folds below follow a selection that doesn't change.
        if self.store.read(cx).moving.is_some() {
            return self.click_move(&selection, window, cx);
        }
        window.focus(&self.focus, cx);
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
    }

    /// Draw the navigator as a card out over the detail, or beside it again.
    pub(crate) fn set_floating(&mut self, floating: bool, cx: &mut Context<Navigator>) {
        if self.floating != floating {
            self.floating = floating;
            cx.notify();
        }
    }

    /// The rows that can be selected, in order: the cards', then the drawer's, but not
    /// those a fold closing takes away.
    fn items(&self) -> Vec<&Selection> {
        self.staying(&self.rows, &self.moving, &self.drawer.rows)
    }

    /// The items of `rows`, which move as `moving` says, then of `drawer`, without those on
    /// their way out.
    fn staying<'a>(
        &self,
        rows: &'a [Row],
        moving: &[Option<Moving>],
        drawer: &'a [Row],
    ) -> Vec<&'a Selection> {
        let closing = |m: &Option<Moving>| m.as_ref().is_some_and(|m| self.folds.closing(&m.fold));
        let cards = (rows.iter().zip(moving)).filter(|(_, m)| !closing(m));
        let drawer = drawer.iter().filter(|_| !self.folds.closing(&Fold::Drawer));
        items(cards.map(|(row, _)| row).chain(drawer))
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
        // While commits move, the keys step through where they can land.
        if self.store.read(cx).moving.is_some() {
            return self.step_move(delta, cx);
        }
        // ↓ onto a fold of commits shows the rest and goes on into them, as a feed loads more:
        // the fold is no stop of its own.
        let unfolds = delta > 0 && !self.on_drawer_bar && self.unfold_after_selection(cx);
        let built = unfolds.then(|| self.build_rows(cx));
        let items = match &built {
            Some(built) => self.staying(&built.rows, &built.moving, &built.drawer.rows),
            None => self.items(),
        };
        if items.is_empty() {
            return;
        }
        let current = self.store.read(cx).selection.as_ref().map(selected_row);
        let at = (current.as_ref()).and_then(|s| items.iter().position(|item| *item == s));
        let bar = self.drawer_toggles(cx).then(|| match &built {
            Some(built) => self.staying(&built.rows, &built.moving, &[]).len(),
            None => self.staying(&self.rows, &self.moving, &[]).len(),
        });
        let next = match step_to(at, items.len(), bar, self.on_drawer_bar, delta) {
            Some(Step::Bar) => return self.enter_drawer_bar(cx),
            Some(Step::Item(next)) => next,
            None => return,
        };
        let Some(next) = items.get(next).map(|item| (*item).clone()) else {
            return;
        };
        // Back on the row it left, whose page stays as it was, not back to how it opens.
        if self.on_drawer_bar && current.as_ref() == Some(&next) {
            return self.leave_drawer_bar(cx);
        }
        self.select(next, window, cx);
    }

    /// Whether the drawer's bar opens and closes it, and so is a stop for the keys: with no
    /// cards it's always open, and while the filter lists what it finds, they show as they
    /// are, as cards don't fold.
    fn drawer_toggles(&self, cx: &App) -> bool {
        self.drawer.total > 0 && !self.rows.is_empty() && !self.filtering(cx)
    }

    /// Give the drawer's bar the keys, in view.
    fn enter_drawer_bar(&mut self, cx: &mut Context<Navigator>) {
        self.on_drawer_bar = true;
        let theme = Theme::get(cx);
        let (spacer, _) = self.footer.get();
        let top = theme.scaled(spacer + CARD_MARGIN);
        self.reveal_in_footer(top, theme.scaled(DRAWER_CHROME));
        cx.notify();
    }

    /// Give the keys back to the selection's row, if the drawer's bar has them.
    fn leave_drawer_bar(&mut self, cx: &mut Context<Navigator>) {
        if std::mem::take(&mut self.on_drawer_bar) {
            cx.notify();
        }
    }

    /// Open the drawer, or fold it.
    fn set_drawer_folded(&mut self, folded: bool, cx: &mut Context<Navigator>) {
        if self.store.read(cx).prefs.projects_folded != folded {
            (self.store).update(cx, |store, cx| {
                store.set_prefs(|p| p.projects_folded = folded, cx)
            });
        }
    }

    /// Fold the drawer if it's open, or open it.
    fn toggle_drawer(&mut self, cx: &mut Context<Navigator>) {
        let folded = self.store.read(cx).prefs.projects_folded;
        self.set_drawer_folded(!folded, cx);
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
        // What's moving was picked when the move started.
        if store.moving.is_some() {
            return;
        }
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
        if self.store.read(cx).moving.is_some() {
            return self.jump_move(forward, cx);
        }
        let items = self.items();
        let current = self.store.read(cx).selection.as_ref().map(selected_row);
        let at = current.and_then(|s| items.iter().position(|item| **item == s));
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
        if self.on_drawer_bar {
            return self.set_drawer_folded(false, cx);
        }
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
            Some(Selection::Checkout(project, checkout)) => {
                self.folded_worktrees.remove(&(project, checkout));
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

    /// Step out the way a tree does: a commit to its branch, an open branch or worktree
    /// closed, a branch up to its worktree, and anything else up to its project.
    /// While the filter lists what it finds, nothing folds, so ⇧← only goes up.
    fn collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_drawer_bar {
            return self.set_drawer_folded(true, cx);
        }
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
            Some(Selection::Checkout(project, checkout)) if folds => {
                self.toggle_section(&project, checkout, true, window, cx)
            }
            Some(Selection::Checkout(project, Checkout::Linked(name)))
                if (store.workspace(&project))
                    .and_then(|ws| ws.find_worktree(name.as_ref()))
                    .is_some_and(|wt| wt.is_inactive(store.prefs.inactive_after)) =>
            {
                let heading = Selection::Section(project, Section::Inactive);
                self.select(heading, window, cx)
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

    /// Return in GitButler's keymap: as in the TUI's, except that uncommitted changes start a
    /// commit, placed as c places it.
    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        let selection = self.store.read(cx).selection.clone();
        match selection {
            Some(Selection::Uncommitted(..)) if !self.on_drawer_bar => {
                window.dispatch_action(Box::new(crate::app::StartCommit), cx)
            }
            _ => self.toggle_fold(&ToggleFold, window, cx),
        }
    }

    /// Go into the selected terminal, to type in, once the update running now is over.
    fn enter_terminal(&self, window: &mut Window, cx: &mut Context<Self>) {
        (self.app).update_later(window, cx, |app, window, cx| app.enter_terminal(window, cx));
    }

    /// Return in the TUI's keymap: fold or unfold the rows the selected one heads, as its
    /// chevron does, go into a terminal, or start watching a project. Any other row that heads
    /// none stays as it is, as every row does under the filter.
    fn toggle_fold(&mut self, _: &ToggleFold, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_drawer_bar {
            return self.toggle_drawer(cx);
        }
        let Some(selection) = self.store.read(cx).selection.clone() else {
            return;
        };
        if let Selection::Terminal(..) = selection {
            return self.enter_terminal(window, cx);
        }
        if let Selection::Project(id) = &selection
            && (self.store.read(cx).project(id)).is_some_and(|entry| !entry.expanded)
        {
            return self.toggle_project(id, cx);
        }
        let fold = Fold::Under(selection.clone());
        let Some(open) = (self.build_rows(cx).folds.into_iter())
            .find_map(|(f, open, _)| (f == fold).then_some(open))
        else {
            return;
        };
        match selection {
            Selection::Project(id) => self.toggle_project(&id, cx),
            Selection::Section(project, section) => {
                self.toggle_heading(project, section, open, window, cx)
            }
            Selection::Branch(project, ref_name) => {
                self.set_open(ClickOpened::Here((project, ref_name)), !open, cx)
            }
            Selection::Checkout(project, checkout) => {
                self.toggle_section(&project, checkout, open, window, cx)
            }
            Selection::Mesh(project, id, pick) => {
                self.toggle_mesh(project, id, pick, open, window, cx)
            }
            Selection::Upstream(project, ref_name, id) => {
                self.toggle_merge(&project, ref_name, id, window, cx)
            }
            Selection::Commit(..)
            | Selection::Uncommitted(..)
            | Selection::Stack(..)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Graph(_)
            | Selection::Terminal(..) => {}
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

    /// Set moving the folds `seen` building the rows that opened or shut since they were last
    /// built, saying whether any did. Only one whose rows start on screen moves: elsewhere it
    /// would move only what's in view. Under the filter, which lists what it finds whatever's
    /// folded, none do.
    fn follow_folds(&mut self, seen: &[(Fold, bool, Option<usize>)], cx: &App) -> bool {
        if self.filtering(cx) {
            self.folds_seen.clear();
            self.folds.stop_all();
            return false;
        }
        // Where the rows above them were drawn, which is where they'll be.
        let list = &self.list;
        let view = list.viewport_bounds();
        let on_screen = |above: Option<usize>| {
            above.is_none_or(|ix| {
                (list.bounds_for_item(ix))
                    .is_some_and(|row| row.bottom() > view.top() && row.bottom() < view.bottom())
            })
        };
        let (mut changed, mut started) = (seen.len() != self.folds_seen.len(), false);
        for (fold, open, above) in seen {
            match self.folds_seen.get(fold) {
                Some(was) if was == open => {}
                Some(_) => {
                    changed = true;
                    if on_screen(*above) {
                        self.folds.start(fold.clone(), *open, cx);
                        started = true;
                    }
                }
                None => changed = true,
            }
        }
        if changed {
            self.folds_seen = (seen.iter())
                .map(|(fold, open, _)| (fold.clone(), *open))
                .collect();
        }
        started
    }

    /// Have each moving fold move only as far as the list shows it, from the row above it as
    /// last drawn: the rows past the list's bottom go or come at once, so what's seen moves
    /// all the way. Here, as a row can't ask the list while it draws.
    fn fit_folds(&mut self, cx: &App) {
        if !self.folds.any() {
            return;
        }
        let (list, scale) = (&self.list, Theme::get(cx).scale);
        let view = list.viewport_bounds();
        let room = |above: Option<usize>| {
            (above.and_then(|ix| list.bounds_for_item(ix)))
                .map(|row| f32::from(view.bottom() - row.bottom()).max(0.) / scale)
        };
        let mut rooms = HashMap::new();
        for m in self.moving.iter_mut().flatten() {
            let room = *rooms.entry(m.above).or_insert_with(|| room(m.above));
            if let Some(room) = room {
                m.extent = m.extent.min(room);
            }
        }
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
        if splice(&self.drawer_list, &self.drawer.rows, &drawer.rows, 0).is_some() {
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
                splice(&self.list, old_above, new_above, 0).is_some() || below.is_some()
            }
            None => {
                let top = self.list.logical_scroll_top();
                let spliced = splice(&self.list, &self.rows, &rows, 0);
                // Cards or linked worktrees trading places, as one dragged by its heading does,
                // leave the list where it is, rather than at the first row they moved, where a
                // splice of the list's top would.
                if let Some(old) = &spliced
                    && old.contains(&top.item_ix)
                    && headings_reordered(&self.rows, &rows)
                {
                    let theme = Theme::get(cx);
                    (self.list).scroll_to(as_deep(&self.rows, &rows, old.start, top, theme));
                }
                spliced.is_some()
            }
        };
        if at_top && changed {
            self.list.scroll_to(ListOffset::default());
        }
        // While something's being moved or landing, and as that ends, every row slides from
        // where it was, so the room the copies take or leave opens and closes rather than jumps.
        let store = self.store.read(cx);
        let op = (store.moving.as_ref().or_else(|| store.landing())).map(|moving| moving.op);
        let was = std::mem::replace(&mut self.shown_op, op);
        let every = op.is_some() || was.is_some();
        // A squash just ended: landed, what it took goes.
        let ended = matches!(was, Some(Op::Squash(_))) && store.moving.is_none();
        // A move Return landed, until the reload after it, and as that shows where it went. Not
        // a cherry-pick, whose commits stay where they are, as when a move ends without landing,
        // while the copies shown give way to the new commits in their place.
        let leaving = (store.landing()).is_some_and(|landing| landing.op != Op::CherryPick);
        let landing = std::mem::replace(&mut self.shown_landing, leaving) || leaving;
        // The outline of what a squash goes into slides there from where it was before, first
        // from around what it's of where it started, its edges coming together as it leaves
        // that, and going apart as it grows back around it.
        let home = (store.moving.as_ref()).is_some_and(|moving| {
            matches!(moving.at, Stop::Into(_)) && !moving.shows_landing(store)
        });
        let outline = moving::SquashOutline::of(&rows, store, home);
        if let (Some(was), Some(is)) = (&self.squash_outline, &outline)
            && was.row != is.row
        {
            let (top, bottom) = is.slide(was);
            let offsets = HashMap::from([(RowKey::SquashTop, top), (RowKey::SquashBottom, bottom)]);
            self.reorder.start(offsets, cx);
        }
        self.squash_outline = outline;
        // While a move's shown, the rows' keys change with it where the rows don't, as what's
        // moving is keyed apart as the move starts and ends.
        let reordered = self.rows != rows;
        let keys = (reordered || every)
            .then(|| row_keys(&rows, self.store.read(cx)))
            .filter(|keys| reordered || *keys != self.row_keys);
        if let Some(keys) = keys {
            let held = anchor.map(|(old_ix, new_ix, _)| (old_ix, new_ix));
            // Those still being closed over stay while the rows they're under do, as when the
            // reload after a move lands partway.
            let mut stand_ins = std::mem::take(&mut self.stand_ins);
            stand_ins.retain(|stand_in| keys.iter().flatten().any(|key| *key == stand_in.under));
            let store = self.store.read(cx);
            let taken = self.stand_ins(&keys, was, (ended, landing), clock::now(), store);
            stand_ins.extend(taken);
            self.stand_ins = stand_ins;
            let offsets = self.slides(
                &rows,
                &keys,
                held,
                every || changes_came_or_went(&self.rows, &rows),
                landing,
            );
            self.reorder.start(offsets, cx);
            self.row_keys = keys;
        }
        self.rows = rows;
        // The bar goes with the rows that made it a stop, or once something moves.
        self.on_drawer_bar &= self.drawer_toggles(cx) && self.store.read(cx).moving.is_none();
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
        // Commits being moved show where they'd land, if that's elsewhere, and what a squash
        // goes into is outlined.
        let ghost = |row: &Row| match row {
            Row::Ghost(..) | Row::GhostBranch(..) | Row::NewCommit(_) => true,
            Row::Item(item) => moving::squashed_into(store, item),
            _ => false,
        };
        if std::mem::take(&mut self.reveal_selection)
            && let Some(selection) = store.selection.as_ref().map(selected_row)
            && !self.scroll_to_row(ghost, row)
            && !self.scroll_to_row(|row| matches!(row, Row::Item(it) if *it == selection), row)
        {
            // A project just added has its rows once it's loaded.
            self.reveal_selection =
                (store.project(selection.project())).is_some_and(|entry| entry.is_loading())
        }
        // Up from where Go to anything goes, as ⇧← steps, to the first row listed.
        let going_row = (self.going_to.clone()).and_then(|to| {
            std::iter::successors(Some(to), |item| holder(store, item))
                .find(|item| self.listed(item))
        });
        if going_row != self.going_row {
            if let Some(item) = &going_row {
                self.scroll_to_row(|row| matches!(row, Row::Item(it) if it == item), row);
            }
            self.going_row = going_row;
        }
    }

    /// Where `item`'s row, in the cards or the drawer, was last drawn while it shows.
    pub(crate) fn row_bounds(&self, item: &Selection) -> Option<Bounds<Pixels>> {
        let at = |rows: &[Row]| {
            rows.iter()
                .position(|row| matches!(row, Row::Item(it) if it == item))
        };
        match (at(&self.rows), at(&self.drawer.rows)) {
            (Some(ix), _) => crate::ui::shown_row(&self.list, ix),
            (None, Some(ix)) => crate::ui::shown_row(&self.drawer_list, ix),
            (None, None) => None,
        }
    }

    /// Whether `item` has a row in the cards or the drawer.
    fn listed(&self, item: &Selection) -> bool {
        (self.rows.iter().chain(&self.drawer.rows))
            .any(|row| matches!(row, Row::Item(it) if it == item))
    }

    /// Scroll the first row `wanted` finds into view, in the cards or else the drawer, rows
    /// `row` tall until drawn, saying whether it found one.
    fn scroll_to_row(&self, wanted: impl Fn(&Row) -> bool, row: Pixels) -> bool {
        let at = |rows: &[Row]| rows.iter().position(&wanted);
        match (at(&self.rows), at(&self.drawer.rows)) {
            (Some(ix), _) => {
                // Rows not drawn since the list's width changed, as zooming changes it,
                // count as one row tall rather than none, so the reveal lands on it.
                self.list.clone().with_uniform_item_height(row);
                self.list.scroll_to_reveal_item(ix);
                true
            }
            (None, Some(ix)) => {
                self.drawer_list.scroll_to_reveal_item(ix);
                self.reveal_in_drawer(ix, row);
                true
            }
            (None, None) => false,
        }
    }

    /// Scroll the drawer's row `ix`, `row` tall, into the column: the list does, as the drawer's
    /// as tall as its rows under the cards.
    fn reveal_in_drawer(&self, ix: usize, row: Pixels) {
        if self.rows.is_empty() {
            return;
        }
        let (spacer, _) = self.footer.get();
        let scale = f32::from(row) / ROW_HEIGHT;
        let above = spacer + CARD_MARGIN + DRAWER_CHROME;
        self.reveal_in_footer(fixed(above * scale) + row * ix as f32, row);
    }

    /// Scroll what's `top` down the list's last item, `height` tall, into the column.
    fn reveal_in_footer(&self, top: Pixels, height: Pixels) {
        let footer = self.rows.len();
        let Some(bounds) = self.list.bounds_for_item(footer) else {
            return self.list.scroll_to(ListOffset {
                item_ix: footer,
                offset_in_item: top,
            });
        };
        let view = self.list.viewport_bounds();
        let (from, to) = (bounds.top() + top, bounds.top() + top + height);
        if to > view.bottom() {
            self.list.scroll_by(to - view.bottom());
        } else if from < view.top() {
            self.list.scroll_by(from - view.top());
        }
    }

    /// How far each of the rows slides as the list goes from its rows to `rows`, keyed by `keys`,
    /// from where it was on screen: those a reorder moved, or with `every`, all of them, as
    /// [`shifted`] says. With the row under the pointer `held` still, from its index before to
    /// after, as a load holds it, the list scrolls by as far as that row moved, so the rows on
    /// screen slide that far too, but only with a reorder.
    fn slides(
        &self,
        rows: &[Row],
        keys: &[Option<RowKey>],
        held: Option<(usize, usize)>,
        every: bool,
        landing: bool,
    ) -> HashMap<RowKey, f32> {
        let sized = |keys: &[Option<RowKey>], rows: &[Row]| -> Vec<(Option<RowKey>, f32)> {
            keys.iter()
                .cloned()
                .zip(rows.iter().map(Row::height))
                .collect()
        };
        let (old, new) = (sized(&self.row_keys, &self.rows), sized(keys, rows));
        let mut offsets = match every {
            true => shifted(&old, &new),
            false => reordered(&old, &new),
        };
        let top = |rows: &[Row], ix: usize| span_height(rows, 0..ix);
        // The row of what's moving is keyed apart while it moves, as its copy takes its key,
        // but they're the same thing: as a move starts, the row stays where it was under its
        // own key; the copy comes out from it rather than showing up in place, and slides back
        // over it, drawn over it by that key there, rather than leaving from where it was. As
        // the move ends, or squashes instead, the row takes its own key back where it is,
        // rather than coming from where its copy was, unless it's `landing` there.
        let at = |keys: &[Option<RowKey>], key: &RowKey| {
            (keys.iter()).position(|other| other.as_ref() == Some(key))
        };
        for (is, key) in keys.iter().enumerate() {
            let from = match key {
                Some(left @ RowKey::Left(own)) if at(&self.row_keys, left).is_none() => {
                    at(&self.row_keys, own).map(|was| (left.clone(), was))
                }
                Some(RowKey::Left(own)) if at(keys, own).is_none() => {
                    at(&self.row_keys, own).map(|was| ((**own).clone(), was))
                }
                Some(RowKey::Left(_)) | None => None,
                Some(own) => {
                    let left = RowKey::Left(Box::new(own.clone()));
                    let was = at(&self.row_keys, own);
                    let from_left = match rows.get(is) {
                        Some(Row::Item(_)) => !landing,
                        // Moved instead of squashed, its copy shows where it'd land at once,
                        // as the squash's outline went, rather than coming out from its row.
                        Some(Row::Ghost(_))
                            if was.is_some_and(|was| {
                                matches!(self.rows.get(was), Some(Row::Item(_)))
                            }) =>
                        {
                            offsets.remove(own);
                            false
                        }
                        _ => was.is_none(),
                    };
                    (from_left.then(|| at(&self.row_keys, &left)).flatten())
                        .map(|was| (own.clone(), was))
                }
            };
            if let Some((key, was)) = from {
                offsets.insert(key, top(&self.rows, was) - top(rows, is));
            }
        }
        if let Some((old_ix, new_ix)) = held.filter(|_| !offsets.is_empty()) {
            let scrolled = top(rows, new_ix) - top(&self.rows, old_ix);
            let before: HashSet<&RowKey> = self.row_keys.iter().flatten().collect();
            for key in keys.iter().flatten().filter(|key| before.contains(key)) {
                *offsets.entry(key.clone()).or_default() += scrolled;
            }
        }
        offsets
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

    /// The row at `ix` of the list. The list draws only the rows laid out in view, so while
    /// rows slide, those sliding into view from past its top or bottom are drawn by the first
    /// row it draws and by one near its last ([`Self::slide_edges`]).
    fn render_row(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        if ix == self.rows.len() {
            return self.drawer_footer(window, Theme::get(cx), cx);
        }
        let row = self.row_in_card(ix, window, cx);
        let Some((first, below)) = self.slide_edges else {
            return row;
        };
        let above = (ix == first).then(|| self.slid_in(ix, false, window, cx));
        let below = (ix == below).then(|| self.slid_in(ix, true, window, cx));
        if above.is_none() && below.is_none() {
            return row;
        }
        (div().relative().child(row))
            .children(above)
            .children(below)
            .into_any_element()
    }

    /// The rows after row `ix`, down or else up, that a slide may bring into view from where
    /// the list draws none, stacked on from `ix`'s edge as the list lays them out, each drawn
    /// only if the list doesn't draw it after all ([`fold::unlisted`]). Down, they start a few
    /// rows above the list's last, to be sure `ix` is drawn, so they go that much further.
    fn slid_in(&self, ix: usize, down: bool, window: &Window, cx: &Context<Self>) -> AnyElement {
        let reach = self.reorder.furthest()
            + match down {
                true => (ABOVE_LAST_LISTED + 3) as f32 * ROW_HEIGHT,
                false => 0.,
            };
        let height = |ix: usize| self.rows.get(ix).map_or(0., Row::height);
        let mut far = 0.;
        let rows = match down {
            true => {
                let mut to = ix + 1;
                while to < self.rows.len() && far < reach {
                    far += height(to);
                    to += 1;
                }
                ix + 1..to
            }
            false => {
                let mut from = ix;
                while from > 0 && far < reach {
                    from -= 1;
                    far += height(from);
                }
                from..ix
            }
        };
        let inset = Theme::get(cx).scaled(LIST_TOP);
        let rows = rows.map(|ix| {
            let row = self.row_in_card(ix, window, cx);
            fold::unlisted(self.list.clone(), inset, ix, row)
        });
        let stack = v_flex().absolute().left_0().right_0().children(rows);
        on_top(match down {
            true => stack.top(relative(1.)),
            false => stack.bottom(relative(1.)),
        })
        .into_any_element()
    }

    /// How far the cards sit in from the list's left and right: less on the right beside the
    /// detail, whose cards they're as far from as those are from each other.
    fn card_margins(&self) -> (f32, f32) {
        match !self.floating && self.detail_shown.get() > 0. {
            true => (CARD_MARGIN, CARD_MARGIN_BESIDE),
            false => (CARD_MARGIN, CARD_MARGIN),
        }
    }

    /// The row at `ix` of the list, drawn inside its card.
    fn row_in_card(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let inset = self.insets.get(ix).copied().unwrap_or_default();
        // A row a fold is moving shows as far as the fold's open: inside its card, so the
        // card's edges stay where they are, and taking no clicks while it goes. Under the
        // curtain, it isn't drawn at all.
        let folding = (self.moving.get(ix).and_then(Option::as_ref)).and_then(|m| {
            let open = self.folds.openness(&m.fold)?;
            Some((
                curtain(open, m.extent, m.top, m.height),
                self.folds.closing(&m.fold),
            ))
        });
        let hidden = folding.is_some_and(|(height, _)| height <= 0.);
        // A fold row at the end of commits: its glyph under theirs, its words under their
        // titles, past the glyph and the commit row's gap.
        let under_commits = |id: &'static str, glyph| {
            let at = ROW_LEFT + COMMIT_INDENT;
            fold_row((id, ix), glyph, at, at + 12. + 10., theme)
        };
        let el = match self.rows.get(ix) {
            None | Some(Row::Gap) => return div().h(dp(CARD_GAP)).into_any_element(),
            _ if hidden => div().into_any_element(),
            Some(Row::Item(_)) => self.render_item(&self.rows, ix, inset, window, theme, cx),
            // A dashed line, as ui-react's WorktreeDivider. Last in its card, the card's edge
            // rules it off, so it keeps only its room, which a worktree dragged counts.
            Some(Row::WorktreeDivider) => {
                let last = matches!(self.rows.get(ix + 1), None | Some(Row::Gap));
                h_flex()
                    .h(dp(12.))
                    .px(dp(12.))
                    .when(!last, |el| {
                        el.child(
                            div()
                                .flex_1()
                                .border_t_1()
                                .border_dashed()
                                .border_color(theme.border_3),
                        )
                    })
                    .into_any_element()
            }
            Some(Row::Note(text, is_error)) => note_row(text, *is_error, theme),
            // As made at once, set in as a commit row is, a stand-in title quiet as one's;
            // dimmed, with an ellipsis, only once it's taking a while.
            Some(Row::Pending(title, slow, _)) => h_flex()
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
            Some(Row::Ghost(item)) => {
                // Copies of commits moving together are outlined as one.
                let ghost = |ix: usize| matches!(self.rows.get(ix), Some(Row::Ghost(_)));
                let with = (ix > 0 && ghost(ix - 1), ghost(ix + 1));
                let blink = self.blinking(Blink::Move);
                moving::ghost_row(item, inset, with, blink, self.store.read(cx), theme)
            }
            // A line from under the branches' glyphs, clear of their commits' guides, drawn as
            // the guides are where a worktree's is dashed, so it reads as part of the worktree
            // rather than the end of it.
            Some(Row::Between(_)) => h_flex()
                .h(dp(12.))
                .pl(dp(ROW_LEFT + inset))
                .pr(dp(12.))
                .child(div().flex_1().h(fixed(1.)).bg(theme.text_2.opacity(0.2)))
                .into_any_element(),
            Some(Row::GhostBranch(project, at)) => {
                let mark = moving::mark_of(self.store.read(cx), project, theme);
                moving::ghost_branch_row(at, inset, mark, self.blinking(Blink::Move), theme)
            }
            Some(Row::NewCommit(project)) => {
                let blink = self.blinking(Blink::Move);
                moving::new_commit_row(project, inset, blink, self.store.read(cx), theme)
            }
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
        // Items and the copies of what's moving step in by their style, keeping the highlight
        // full width; the rest as a whole.
        let full_width = matches!(
            self.rows.get(ix),
            Some(Row::Item(_) | Row::Ghost(..) | Row::GhostBranch(..) | Row::NewCommit(_))
        );
        let el = match inset > 0. && !full_width {
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
        let guides = || (self.guides.get(ix).into_iter().flatten()).map(|at| guide(*at, theme));
        // The card dragged by its heading is edged as what's moving is where it would land.
        let held = (self.held_card.as_ref()).is_some_and(|card| card.contains(&ix));
        let held_worktree = (self.held_worktree.as_ref()).is_some_and(|rows| rows.contains(&ix));
        let focused = (self.focused_card.borrow().as_ref()).is_some_and(|card| card.contains(&ix));
        let edge = match (held, focused) {
            (true, _) => theme.pop,
            (false, true) => theme.focus_edge(),
            (false, false) => theme.border_3,
        };
        // How far row `ix` is drawn from where it's laid out: as far as a reorder slid it, but
        // not while a fold moves it, which draws it as far as the fold's open.
        let slid_at = |ix: usize| match self.moving.get(ix).and_then(Option::as_ref) {
            Some(m) if self.folds.openness(&m.fold).is_some() => 0.,
            _ => (self.row_keys.get(ix).and_then(Option::as_ref))
                .map_or(0., |key| self.reorder.offset(key)),
        };
        // A row slides with its slice of the card, so the card's edges follow its rows. Its
        // slice also stretches over the room between it and where it's laid out, as far as its
        // card goes, and down to the next row of its card where that's drawn further down, so
        // the card holds together as they slide, even where the list doesn't draw the row that
        // would cover it, past its top or bottom, and never covers the card beside it as cards
        // trade places. While rows slide, what's in every row is drawn over all that, so no
        // stretch covers it, on the card's ground so the rows it passes don't show through,
        // the copies of commits being moved over the rest, inside the list.
        let slid = slid_at(ix);
        let below = match last {
            true => 0.,
            false => {
                let room = card_beyond(&self.rows, ix, true, -slid);
                (slid_at(ix + 1) - slid).max(room).max(0.)
            }
        };
        let above = match first {
            true => 0.,
            false => card_beyond(&self.rows, ix, false, slid).max(0.),
        };
        // What's in the card being dragged goes over everything, as it goes over the cards it
        // passes (see below), and so do the rows of a linked worktree being moved, over those of
        // the others it passes; the copies of what's moving over the rest, and the selected row,
        // which moves, over the rows it passes. The new branch the copies would land in shows
        // from under what slides off it, rather than over what's about to.
        let selection = self.store.read(cx).selection.as_ref();
        let over_rank = match self.rows.get(ix) {
            _ if held || held_worktree => 3,
            Some(Row::Ghost(..) | Row::NewCommit(_)) => 2,
            Some(Row::Item(item)) if selection == Some(item) => 1,
            _ => 0,
        };
        // What a move just took away from under this row stays there, moving with it, while
        // the rows after it close over it: only while rows slide, as only then do they draw
        // over it.
        let key = self.row_keys.get(ix).and_then(Option::as_ref);
        let stand_ins = (self.stand_ins.iter())
            .filter(|stand_in| self.reorder.any() && key == Some(&stand_in.under))
            .map(|stand_in| {
                moving::stand_in(stand_in, self.card_margins(), self.store.read(cx), theme)
            });
        // What's being moved has its copy drawn over it while the move is at where it is, over
        // everything, as the copies elsewhere are, sliding out to them and back by its key, as
        // the copy keyed so would. Copies of commits moving together are outlined as one.
        let copy = match (self.rows.get(ix), key) {
            (Some(Row::Item(item)), Some(RowKey::Left(own))) => {
                let store = self.store.read(cx);
                let copied =
                    |item: &Selection| matches!(moving::mark(store, item), Some(Mark::Copied(_)));
                let copied_at = |ix: usize| matches!(self.rows.get(ix), Some(Row::Item(other)) if copied(other));
                copied(item).then(|| {
                    let with = (ix > 0 && copied_at(ix - 1), copied_at(ix + 1));
                    let copy = div()
                        .absolute()
                        .top(dp(self.reorder.offset(own) - slid_at(ix)))
                        .left_0()
                        .right_0()
                        .bg(theme.bg_1)
                        .child(moving::ghost_row(
                            item,
                            inset,
                            with,
                            self.blinking(Blink::Move),
                            store,
                            theme,
                        ))
                        .children(guides());
                    on_top(copy).priority(2)
                })
            }
            _ => None,
        };
        let card = div()
            .relative()
            .bg(theme.bg_1)
            .border_color(edge)
            .border_l_1()
            .border_r_1()
            .when(first, |el| {
                el.border_t_1().rounded_t(dp(6.)).pt(dp(CARD_PADDING))
            })
            .when(last, |el| {
                el.border_b_1().rounded_b(dp(6.)).pb(dp(CARD_PADDING))
            })
            .map(|card| match folding {
                Some((height, closing)) => card.child(clipped(
                    div().relative().child(el).children(guides()),
                    dp(height),
                    closing,
                )),
                None if self.reorder.any() => {
                    // Rounded at a card's ends, inside its corners, which square it'd cover a
                    // sliver of until the rows stop.
                    let over = (div().relative().bg(theme.bg_1).child(el))
                        .when(first, |el| el.rounded_t(dp(2.)))
                        .when(last, |el| el.rounded_b(dp(2.)));
                    let over = on_top(over.children(guides())).priority(over_rank);
                    card.children(guides()).child(over)
                }
                None => card.child(el).children(guides()),
            })
            .children(copy);
        let (left, right) = self.card_margins();
        let stretched = |height: f32| {
            div()
                .absolute()
                .left(dp(left))
                .right(dp(right))
                .h(dp(height))
                .bg(theme.bg_1)
                .border_color(edge)
                .border_l_1()
                .border_r_1()
                .children(guides())
        };
        let below = (below > 0.).then(|| stretched(below).top(relative(1.)));
        let above = (above > 0.).then(|| stretched(above).top(dp(-above)));
        // Padded, not margined: a list lays its rows out from their left edge.
        let row = div()
            .relative()
            .top(dp(slid))
            .pl(dp(left))
            .pr(dp(right))
            .child(card)
            .children(below)
            .children(above)
            .children(stand_ins);
        // The card being dragged goes over the cards it passes as they trade places, edges and
        // all, so it never runs into them, and what's in its rows over its slices.
        let row = match held && self.reorder.any() {
            true => on_top(row).priority(2).into_any_element(),
            false => row.into_any_element(),
        };
        // The list lays out the row it's given where it goes, so what slides is inside it.
        div().child(row).into_any_element()
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
        let shown = store.selection.as_ref().map(selected_row);
        let lit = |item: &Selection| {
            shown.as_ref() == Some(item)
                || matches!(item, Selection::Commit(project, id) if store.is_picked(project, *id))
        };
        // What's being moved shows where the move is, rather than that it's selected. A linked
        // worktree dragged by its heading shows held the whole drag, over its own rows too,
        // where it isn't moving.
        let dragged = matches!(item, Selection::Checkout(project, Checkout::Linked(name))
            if (self.dragged_worktree.as_ref()).is_some_and(|(p, n)| p == project && n == name));
        let marked = moving::mark(store, item).or_else(|| dragged.then_some(Mark::Held(Op::Move)));
        let selected = lit(item) && marked.is_none();
        // Picked commits next to each other join up.
        let lit_at = |ix: usize| matches!(rows.get(ix), Some(Row::Item(other)) if lit(other));
        let joined = match selected && !store.picked.is_empty() {
            true => (ix > 0 && lit_at(ix - 1), lit_at(ix + 1)),
            false => (false, false),
        };
        let cut_of = |item: &Selection| {
            let cut = match (&store.cut, item) {
                (Some(Cut::Branch(p, r)), Selection::Branch(project, ref_name)) => {
                    p == project && ref_name.as_ref() == Some(r)
                }
                (Some(Cut::Commit(p, ids)), Selection::Commit(project, commit)) => {
                    p == project && ids.contains(commit)
                }
                _ => false,
            };
            cut || matches!(
                moving::mark(store, item),
                Some(Mark::Cut(_) | Mark::Copied(_))
            )
        };
        let cut = cut_of(item);
        // What a squash goes into is outlined over it, or where it started around the rows
        // held there, the outline sliding in from where it was before, its edges apart.
        let slid = (
            self.reorder.offset(&RowKey::SquashTop),
            self.reorder.offset(&RowKey::SquashBottom),
        );
        let outlined = (self.squash_outline.as_ref()).filter(|outline| outline.row == *item);
        let held = matches!(marked, Some(Mark::Held(_)));
        // Cut or moving together, next to each other, they're outlined as one.
        let outlined_at = |ix: usize| match rows.get(ix) {
            Some(Row::Item(other)) if cut => cut_of(other),
            Some(Row::Item(other)) if held => {
                matches!(moving::mark(store, other), Some(Mark::Held(_)))
            }
            _ => false,
        };
        let outlined_with = (ix > 0 && outlined_at(ix - 1), outlined_at(ix + 1));
        let solid = selected && !self.on_drawer_bar && self.rows_have_keys(window, cx);
        let style = RowStyle {
            theme,
            selected,
            solid,
            cut,
            held,
            // In the colour of what's being done with it, as the TUI's modes have their own.
            mark: match marked {
                Some(Mark::Cut(op) | Mark::Copied(op) | Mark::Held(op)) => op.colour(theme),
                None => theme.pop,
            },
            inset,
            joined,
            outlined_with,
            glow: !selected && matches!(item, Selection::Commit(_, commit) if store.glows(*commit)),
            // The selection alone, which is what's copied, not the commits picked with it. A
            // project's card, a linked worktree and a terminal being moved go where they'd
            // land themselves, so they blink as where they'd land does.
            blink: match moving::goes_itself(store, item) {
                true => self.blinking(Blink::Move),
                false => self
                    .blinking(Blink::Copied)
                    .filter(|_| store.selection.as_ref() == Some(item)),
            },
            goes_here: self.going_row.as_ref() == Some(item),
        };
        let el = match item {
            Selection::Project(id) => self.project_row(item, id, style, cx),
            Selection::Uncommitted(id, Checkout::Main)
                if store.workspace(id).is_some_and(|ws| ws.editing.is_some()) =>
            {
                Some(self.editing_row(item, id, style, cx))
            }
            Selection::Checkout(_, checkout) => {
                self.worktree_heading(ix, item, checkout, style, cx)
            }
            Selection::Uncommitted(..) => self.changes_row(item, style, cx),
            Selection::Branch(id, ref_name) => {
                self.branch_row(item, id, ref_name.as_ref(), style, cx)
            }
            Selection::Commit(id, commit_id) => self.commit_row(item, id, commit_id, style, cx),
            Selection::Terminal(_, id) => self.terminal_row(item, *id, style, cx),
            // The rows list a stack's branches, not the stack, and a project's pages are tabs
            // of its own page, which its card opens.
            Selection::Stack(..)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Graph(_) => None,
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
        let Some(el) = el else {
            return div().into_any_element();
        };
        let el = self.with_tiles(el, item, (selected, solid), theme, cx);
        let el = self.drop_target(el, item.clone(), window, theme, cx);
        match outlined {
            Some(outline) => {
                let blink = self.blinking(Blink::Move);
                let outline = moving::squash_outline(outline.edges(slid), blink, theme);
                div().relative().child(el).child(outline).into_any_element()
            }
            None => el,
        }
    }

    /// Row `el` of `item`, beside a small map of its tiles while it has others than its page,
    /// which is the brightest: shaded with the row while it's `selected`, solid as it is, and
    /// naming them in its tooltip. A click on it is one on the row.
    fn with_tiles(
        &self,
        el: AnyElement,
        item: &Selection,
        (selected, solid): (bool, bool),
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let tiles = store.tiles_of(item);
        if tiles.len() < 2 {
            return el;
        }
        let ink = match selected && solid {
            true => theme.text_inverted,
            false => theme.text_2,
        };
        let cells = tiles.iter().map(|(tile, area)| {
            div()
                .absolute()
                .left(dp(area.left * TILE_MAP.0 + TILE_MAP_GAP / 2.))
                .top(dp(area.top * TILE_MAP.1 + TILE_MAP_GAP / 2.))
                .w(dp((area.width * TILE_MAP.0 - TILE_MAP_GAP).max(1.)))
                .h(dp((area.height * TILE_MAP.1 - TILE_MAP_GAP).max(1.)))
                .rounded(dp(1.5))
                .bg(match tile {
                    Tile::Home => ink,
                    Tile::Pinned(_) => ink.opacity(0.4),
                })
        });
        let names = (tiles.iter()).filter_map(|(tile, _)| match tile {
            Tile::Home => None,
            Tile::Pinned(pinned) => Some(crate::app::tile_label(store, pinned).1),
        });
        let names = names.collect::<Vec<_>>().join("\n");
        let map = h_flex()
            .id(SharedString::from(format!("tiles-{item:?}")))
            .flex_none()
            .h(dp(ROW_HEIGHT))
            .mr(dp(3.))
            .px(dp(7.))
            .rounded(dp(4.))
            .map(|el| match (selected, solid) {
                (true, true) => el.bg(theme.fill_gray),
                (true, false) => el.bg(theme.selected_unfocused()),
                (false, _) => el
                    .bg(theme.hover())
                    .hover(|el| el.bg(theme.selected_unfocused())),
            })
            .child(
                div()
                    .relative()
                    .w(dp(TILE_MAP.0))
                    .h(dp(TILE_MAP.1))
                    .children(cells),
            )
            .tooltip(Tooltip::text(names))
            .map(|el| self.selectable(el, item, cx))
            .map(|el| self.page_drag(el, item, cx));
        h_flex()
            .w_full()
            .child(div().flex_grow(1.).min_w(dp(120.)).child(el))
            .child(map)
            .into_any_element()
    }

    /// Whether a press on a row can start dragging it: not while something is being moved,
    /// which a press puts at the row instead ([`Self::click_move`]), but the drag of a card, a
    /// linked worktree or a terminal moving it, which keeps on.
    fn drags(&self, cx: &App) -> bool {
        self.store.read(cx).moving.is_none() || cx.has_active_drag()
    }

    /// Let a click on row `el` select `item` ([`Self::select_row`]) once it lets go, so the
    /// tiles and a terminal among them stay as they are for a row dragged over them. While
    /// something is being moved, the press puts it at the row instead ([`Self::click_move`]),
    /// ⌘ and ⇧ with it too, which pick nothing then.
    fn selectable(&self, el: Stateful<Div>, item: &Selection, cx: &Context<Self>) -> Stateful<Div> {
        let (pressed, clicked) = (item.clone(), item.clone());
        el.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                if this.store.read(cx).moving.is_some() {
                    this.click_move(&pressed, window, cx)
                }
            }),
        )
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            if this.store.read(cx).moving.is_none() {
                this.select_row(&clicked, event.modifiers(), window, cx)
            }
        }))
    }

    /// Let row `el` of `item`, with nothing else to drag it for, be dragged to land as a tile.
    fn page_drag(&self, el: Stateful<Div>, item: &Selection, cx: &Context<Self>) -> Stateful<Div> {
        if !self.drags(cx) {
            return el;
        }
        let store = self.store.read(cx);
        let (kind, label) = crate::app::tile_label(store, &store.row_key(item));
        let preview = page_preview(self.store.clone(), kind, label);
        el.on_drag(
            PageDrag,
            row_preview(self.store.clone(), item.clone(), preview),
        )
    }

    /// A row `item` clicked with `held` down: ⌘ (ctrl off macOS) or ⇧ picks commits along with
    /// the selected one. A click on one of several picked commits selects just it (see
    /// [`Self::commit_row`]), and dragging it takes them all.
    fn select_row(
        &mut self,
        item: &Selection,
        held: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match item {
            Selection::Commit(project, id) if held.secondary() || held.shift => {
                let range = held.shift.then(|| self.commit_order(project, *id, cx));
                self.store
                    .update(cx, |store, cx| store.pick(project, *id, range, cx));
                window.focus(&self.focus, cx);
            }
            Selection::Commit(project, id) if self.store.read(cx).among_picks(project, *id) => {
                window.focus(&self.focus, cx)
            }
            _ => {
                self.select(item.clone(), window, cx);
                // A row with tiles gives the keys back to the one that had them last.
                self.app.update_later(window, cx, |app, window, cx| {
                    app.focus_last_tile(window, cx);
                });
            }
        }
    }

    /// The projects not watched, in a drawer under the cards as tall as its rows, which the
    /// list scrolls. With no cards it's the column, open, and scrolls past that.
    fn drawer(&self, window: &Window, theme: &Theme, cx: &Context<Self>) -> impl IntoElement {
        let (left, right) = self.card_margins();
        // The column's only card, filling it, as far from its edges as cards are apart.
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .pl(dp(left))
            .pr(dp(right))
            .py(dp(CARD_MARGIN))
            .child(self.drawer_card(true, window, theme, cx))
    }

    /// The drawer's card: its bar, and under it its rows as far as it's open.
    fn drawer_card(&self, alone: bool, window: &Window, theme: &Theme, cx: &Context<Self>) -> Div {
        let rows = list(
            self.drawer_list.clone(),
            cx.processor(|this, ix: usize, window, cx| this.render_drawer_row(ix, window, cx)),
        )
        .size_full();
        let scroll = Scroll::List(self.drawer_list.clone());
        let rows_height = self.drawer_rows_height();
        let closing = self.folds.closing(&Fold::Drawer);
        v_flex()
            .relative()
            .size_full()
            .bg(theme.bg_1)
            .border_1()
            .border_color(theme.border_3)
            .rounded(dp(6.))
            .child(self.drawer_bar(alone, window, theme, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .when(rows_height > 0., |el| el.pb(dp(DRAWER_PADDING)))
                    .child(rows)
                    .when(alone, |el| {
                        el.children(scrollbar("drawer-scrollbar", scroll, theme))
                    })
                    .map(|el| inert(el, closing)),
            )
    }

    /// How tall the drawer's rows are: each one row tall, and opening or closing, as tall as
    /// it's open.
    fn drawer_rows_height(&self) -> f32 {
        let full = match self.drawer.rows.len() {
            0 => 0.,
            n => n as f32 * ROW_HEIGHT + DRAWER_PADDING,
        };
        full * self.folds.openness(&Fold::Drawer).unwrap_or(1.)
    }

    /// The list's last item, after the cards: the drawer's card, as far from them as they are
    /// apart, and while they're short of the column, pushed down to its bottom by the room
    /// they leave, as [`Self::fit_footer`] works out.
    fn drawer_footer(&self, window: &Window, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let (left, right) = self.card_margins();
        let (spacer, height) = self.footer.get();
        div()
            .pl(dp(left))
            .pr(dp(right))
            .pt(dp(spacer + CARD_MARGIN))
            .child(
                div()
                    .h(dp(height))
                    .child(self.drawer_card(false, window, theme, cx)),
            )
            .into_any_element()
    }

    /// Work out how tall the drawer's card is and the room above it, before the list lays it
    /// out: as tall as it's open, and below the cards, or at the column's bottom while they're short of it, as far as they end in this
    /// frame, folds and all, so it moves with them. The list keeps an item's height, so it
    /// measures this one again when it changes, and the navigator draws again, cached as it
    /// is.
    fn fit_footer(&self, window: &mut Window, cx: &mut Context<Self>) {
        let scale = Theme::get(cx).scale;
        let view = self.list.viewport_bounds();
        let column = f32::from(view.size.height) / scale;
        let height = DRAWER_CHROME + self.drawer_rows_height();
        // Where the cards end in this frame, which the list only knows once it's drawn it.
        let room = column - self.cards_height();
        let spacer = (room - height - CARD_MARGIN).max(0.);
        let (was_spacer, was) = self.footer.get();
        if (was_spacer, was) != (spacer, height) {
            // Opening, past the room the cards leave, the list scrolls as far as it grows, so it
            // opens up into the column, as the drawer at the column's bottom did. Only once
            // there's no room left, where the room it took would cancel out but for rounding,
            // and until its bar's at the column's top, so it opens under that.
            let opening =
                self.folds.openness(&Fold::Drawer).is_some() && !self.folds.closing(&Fold::Drawer);
            let past = (height - was) - (was_spacer - spacer);
            if opening && spacer == 0. && past > 0. {
                let past = Theme::get(cx).scaled(past);
                let above = (self.list.bounds_for_item(self.rows.len()))
                    .map_or(past, |footer| footer.top() - view.top());
                self.list.scroll_by(past.min(above).max(fixed(0.)));
            }
            self.footer.set((spacer, height));
            let at = self.rows.len();
            self.list.remeasure_items(at..at + 1);
            // After this draw, as a notify during it draws nothing.
            cx.defer_in(window, |_, _, cx| cx.notify());
        }
    }

    /// Whether the navigator has the keys, but not while typing in the filter, where they go
    /// to it.
    fn rows_have_keys(&self, window: &Window, cx: &App) -> bool {
        keys_at(window, cx).is_some_and(|keys| {
            self.focus.contains(&keys, window) && keys != self.filter.focus_handle(cx)
        })
    }

    /// Note the card the selection's in while the navigator has the keys, but in its filter,
    /// for its rows to edge it as the tile with the keys is.
    fn find_focused_card(&self, window: &Window, cx: &App) {
        let keys = !self.on_drawer_bar && self.rows_have_keys(window, cx);
        let selection = self.store.read(cx).selection.as_ref();
        let at = selection.filter(|_| keys).and_then(|selection| {
            (self.rows.iter()).position(|row| matches!(row, Row::Item(item) if item == selection))
        });
        let card = at.map(|at| {
            let gap = |ix: &usize| matches!(self.rows.get(*ix), Some(Row::Gap));
            let start = (0..at).rev().find(gap).map_or(0, |ix| ix + 1);
            let end = (at..self.rows.len()).find(gap).unwrap_or(self.rows.len());
            start..end
        });
        *self.focused_card.borrow_mut() = card;
    }

    /// How tall the cards are drawn in this frame, from the list's top: each row as tall as it
    /// is, or one a fold is moving as far as the fold's open, as `row_in_card` draws them, and
    /// the first card's top edge and the last's bottom, which no gap between two counts.
    fn cards_height(&self) -> f32 {
        let rows = self.drawn_heights().map(|(_, height)| height);
        LIST_TOP + 2. * CARD_EDGE + rows.sum::<f32>()
    }

    /// Each row, and how tall it's drawn in this frame: as tall as it is, or one a fold is
    /// moving as far as the fold's open.
    fn drawn_heights(&self) -> impl Iterator<Item = (&Row, f32)> {
        self.rows.iter().enumerate().map(|(ix, row)| {
            let moving = self.moving.get(ix).and_then(Option::as_ref);
            let open = moving.and_then(|m| Some((m, self.folds.openness(&m.fold)?)));
            let height = match open {
                Some((m, open)) => curtain(open, m.extent, m.top, m.height),
                None => row.height(),
            };
            (row, height)
        })
    }

    /// Where the list's top and bottom edges cut a card off as it's scrolled in this frame,
    /// the drawer's too: there the card's edge is drawn along the list's, rounded as its own,
    /// so it keeps its shape. GPUI cuts what the list draws to a rectangle, so the ground the
    /// cards are on is painted past the corners.
    fn card_caps(&self, theme: &Theme, cx: &App) -> Vec<Div> {
        const RADIUS: f32 = 6.;
        let scale = Theme::get(cx).scale;
        let height = f32::from(self.list.viewport_bounds().size.height) / scale;
        let top = -f32::from(self.list.scroll_px_offset_for_scrollbar().y) / scale;
        // Each card's top and bottom, from the list's top, as `cards_height` counts them.
        let mut cards = Vec::new();
        let (mut start, mut y) = (LIST_TOP, LIST_TOP + CARD_EDGE);
        for (row, drawn) in self.drawn_heights() {
            if matches!(row, Row::Gap) {
                cards.push((start, y + CARD_EDGE));
                start = y + drawn - CARD_EDGE;
            }
            y += drawn;
        }
        cards.push((start, y + CARD_EDGE));
        let (spacer, drawer) = self.footer.get();
        let drawer_top = y + CARD_EDGE + spacer + CARD_MARGIN;
        cards.push((drawer_top, drawer_top + drawer));
        // How much of the card an edge cuts shows on the list's side of it, if it cuts one past
        // its edge.
        let cut = |at: f32, at_top: bool| {
            (cards.iter())
                .position(|&(card_top, bottom)| at > card_top + 1. && at < bottom - 1.)
                .and_then(|ix| Some((ix, *cards.get(ix)?)))
                .map(|(ix, (card_top, bottom))| {
                    (ix, if at_top { bottom - at } else { at - card_top })
                })
        };
        // Which card the selection's, while it's edged, counting them as `cards` does.
        let focused = (self.focused_card.borrow().as_ref()).map(|rows| {
            (self.rows.iter().take(rows.start))
                .filter(|row| matches!(row, Row::Gap))
                .count()
        });
        let (left, right) = self.card_margins();
        let ground = theme.bg_2;
        let cap = |at_top: bool, (card, shows): (usize, f32)| {
            let line = match focused == Some(card) {
                true => theme.focus_edge(),
                false => theme.border_3,
            };
            // Too little of the card shows for a corner at each of its ends: it's drawn whole,
            // as a rounded box is that's shorter than two corners, over its own corners.
            let sliver = shows < 2. * RADIUS;
            let radius = if sliver { shows / 2. } else { RADIUS };
            let edge = div()
                .absolute()
                .left(dp(left))
                .right(dp(right))
                .border_color(line)
                .border_l_1()
                .border_r_1();
            // Otherwise its edge along the list's, two corners tall, as GPUI rounds a box no
            // further than half its height, cut where the card's own corners start.
            let (edge, shown) = match (sliver, at_top) {
                (true, _) => (
                    (edge.h(dp(shows)).border_t_1().border_b_1()).rounded(dp(radius)),
                    shows,
                ),
                (false, true) => (edge.border_t_1().rounded_t(dp(RADIUS)), shows - RADIUS),
                (false, false) => (edge.border_b_1().rounded_b(dp(RADIUS)), shows - RADIUS),
            };
            let edge = edge.when(!sliver, |el| el.h(dp(2. * RADIUS)));
            let band = div()
                .absolute()
                .left_0()
                .right_0()
                .h(dp(shown.min(2. * RADIUS)))
                .overflow_hidden();
            let edge = match at_top {
                true => band.top_0().child(edge.top_0()),
                false => band.bottom_0().child(edge.bottom_0()),
            };
            // The ground past its corners, painted as their shape: GPUI rounds only a border's
            // outer edge, so no border's inner one could cover them.
            let corners = canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let theme = Theme::get(cx);
                    let (r, out) = (theme.scaled(radius), theme.scaled(1.));
                    let (x0, x1) = (
                        bounds.left() + theme.scaled(left),
                        bounds.right() - theme.scaled(right),
                    );
                    // The list's edge, and the way into it.
                    let (y, toward) = match at_top {
                        true => (bounds.top(), r),
                        false => (bounds.bottom(), -r),
                    };
                    let mut ends = vec![(y, toward)];
                    if sliver {
                        // The card's own end, its corners under the card's ground, rounder
                        // than the sliver's.
                        let end = y + toward / r * theme.scaled(shows);
                        let (top, bottom) = (y.min(end), y.max(end));
                        let wide = theme.scaled(RADIUS);
                        for x in [x0, x1 - wide] {
                            let under =
                                Bounds::from_corners(point(x, top), point(x + wide, bottom));
                            window.paint_quad(fill(under, theme.bg_1));
                        }
                        ends.push((end, -toward));
                    }
                    // A quarter circle's control points, as a cubic Bézier draws it.
                    let k = r * 0.5523;
                    for (y, toward) in ends {
                        for (x, inward) in [(x0, r), (x1, -r)] {
                            let mut path = PathBuilder::fill();
                            let side = |dx: Pixels, dy: Pixels| point(x + dx, y + dy);
                            let (outward, beyond) = (-inward / r * out, -toward / r * out);
                            path.move_to(side(outward, toward));
                            path.line_to(side(outward, beyond));
                            path.line_to(side(inward, beyond));
                            path.line_to(side(inward, Pixels::ZERO));
                            path.cubic_bezier_to(
                                side(Pixels::ZERO, toward),
                                side(inward - inward / r * k, Pixels::ZERO),
                                side(Pixels::ZERO, toward - toward / r * k),
                            );
                            path.close();
                            if let Ok(path) = path.build() {
                                window.paint_path(path, ground);
                            }
                        }
                    }
                },
            )
            .absolute()
            .inset_0();
            div().absolute().inset_0().child(corners).child(edge)
        };
        let ends = [(cut(top, true), true), (cut(top + height, false), false)];
        (ends.into_iter())
            .filter_map(|(cut, at_top)| Some(cap(at_top, cut?)))
            .collect()
    }

    /// The drawer's handle: a click opens or closes it, and it says how many projects it lists
    /// of how many there are, and has ways to add, order and find them. With the keys on it,
    /// it's drawn as a selected row is.
    fn drawer_bar(
        &self,
        alone: bool,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let Drawer { listed, total, .. } = self.drawer;
        let title = match alone {
            true => "All projects",
            false => "More projects",
        };
        let count = match listed == total {
            true => total.to_string(),
            false => format!("{listed} of {total}"),
        };
        let toggles = self.drawer_toggles(cx);
        let selected = self.on_drawer_bar;
        let solid = selected && self.rows_have_keys(window, cx);
        let (ink, muted) = match solid {
            true => (theme.text_inverted, theme.muted_on_solid()),
            false => (theme.text_3, theme.text_3),
        };
        // Up while it's folded, turning down as it opens.
        let open = !self.store.read(cx).prefs.projects_folded;
        let chevron = fold::turned("drawer-fold", open, Icon::ChevronUp, PI, muted);
        // Its chevron over the projects' eyes.
        let fold = h_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap(dp(6.))
            .when(toggles, |el| el.child(chevron))
            .child(section_label(title, theme).text_color(ink).truncate())
            .when(total > 0, |el| {
                el.child(faint(count, theme).text_size(dp(12.)).text_color(muted))
            });
        // In from its card's edges as a row is, its hover rounded as one's, so it stays inside
        // the card's corners.
        h_flex()
            .id("drawer-bar")
            .role(Role::ListItem)
            .aria_label(title)
            .aria_selected(selected)
            .flex_none()
            .h(dp(DRAWER_BAR_HEIGHT))
            .m(dp(DRAWER_INSET))
            .rounded(dp(4.))
            .pl(dp(10.))
            .pr(dp(6.))
            .gap(dp(2.))
            .when(selected, |el| el.bg(theme.selected_unfocused()))
            .when(solid, |el| el.bg(theme.fill_gray))
            .child(fold)
            .child(
                icon_button("add-project", Icon::Plus)
                    .small()
                    .inverted(solid)
                    .tooltip("Add project")
                    .dispatch(&AddProject),
            )
            .when(total > 0, |el| {
                el.child(
                    icon_button("project-view-options", Icon::Kebab)
                        .small()
                        .inverted(solid)
                        .tooltip("Sort and show")
                        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                            this.open_menu(event.position(), |this, cx| this.view_options(cx), cx)
                        })),
                )
            })
            .when(toggles, |el| {
                el.aria_expanded(open)
                    .cursor_pointer()
                    .when(!selected, |el| el.hover(|el| el.bg(theme.hover())))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_drawer(cx)))
            })
    }

    /// Row `ix` of the drawer, set in as a card's rows are.
    fn render_drawer_row(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let el = match self.drawer.rows.get(ix) {
            Some(Row::Note(text, is_error)) => note_row(text, *is_error, theme),
            _ => self.render_item(&self.drawer.rows, ix, 0., window, theme, cx),
        };
        div().child(el).into_any_element()
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
        // say: their uncommitted files and unpushed commits.
        let summary = workspace.filter(|ws| ws.editing.is_none()).map(|ws| {
            let folded = |n: usize| if expanded && !card_folded { 0 } else { n };
            let shown = |checkout, files| store.uncommitted_shown(id, &checkout, files).len();
            let in_worktrees = (ws.worktrees.iter())
                .map(|w| shown(Checkout::Linked(w.name.clone()), &w.uncommitted));
            (
                folded(shown(Checkout::Main, &ws.uncommitted) + in_worktrees.sum::<usize>()),
                folded(ws.unpushed_commits()),
                ws.target.as_ref().filter(|_| !new_on),
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
            (workspace.and_then(|ws| ws.target.as_ref())).is_some_and(|t| t.needs_update());
        // The eye and the card's chevron, whose clicks don't select the row. While something is
        // being moved, the eye is only part of the row, which a press puts the move at; the
        // chevron folds the card still.
        let moving = self.store.read(cx).moving.is_some();
        let button =
            |name: &str, folds: bool, click: fn(&mut Self, &ProjectId, &mut Context<Self>)| {
                let id = id.clone();
                div()
                    .id(ElementId::Name(format!("{name}-{id}").into()))
                    .flex_none()
                    .size(dp(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(dp(4.))
                    .when(folds || !moving, |el| {
                        el.hover(|el| el.bg(theme.hover()))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                click(this, &id, cx)
                            }))
                    })
            };
        Some(
            style
                .row(format!("project-{id}"), 0.)
                .aria_label(entry.project.name.clone())
                .aria_expanded(expanded)
                .h(dp(if expanded { HEADING_HEIGHT } else { ROW_HEIGHT }))
                .pl(dp(11.))
                .when(unread, |el| el.relative().child(style.unread_line(false)))
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
                                false,
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
                        .when_some(summary, |el, (files, unpushed, target)| {
                            let warn = style.meta(theme.warn);
                            el.children(style.files(files))
                                .children(style.unpushed(unpushed))
                                .children(target.and_then(|t| {
                                    let behind = plural(t.behind, "commit");
                                    let tip =
                                        format!("{behind} behind {}{}", t.name, with_merged(t));
                                    meta_count(Icon::ArrowDown, t.behind, warn, tip)
                                }))
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
                        button("fold-project", true, |this, id, cx| {
                            this.toggle_project(id, cx)
                        })
                        .child(fold_glyph("fold", Some(!card_folded), style.quiet()))
                        .tooltip(Tooltip::text(
                            match (filtering, card_folded) {
                                (true, _) => "Clear the filter to fold the project",
                                (false, true) => "Show the project",
                                (false, false) => "Fold the project, still watching it",
                            },
                        )),
                    )
                })
                .map(|el| self.selectable(el, item, cx))
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
                // Only the manual order moves cards by hand.
                .map(|el| {
                    let name = SharedString::from(entry.project.name.clone());
                    self.project_drag(el, item, name, expanded && manual, cx)
                })
                .into_any_element(),
        )
    }

    /// A terminal open on the worktree it's under, by the name the user gave it, else the title
    /// its shell did.
    fn terminal_row(
        &self,
        item: &Selection,
        id: TerminalId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let title = store.terminals.title(id)?.clone();
        let status = store.terminals.status(id)?;
        let field = self.rename_field(item, style.theme, cx);
        let renaming = field.is_some();
        let row = style
            .row(format!("terminal-{}", id.0), 6.)
            .child(icon(Icon::TextCode).text_color(style.muted()))
            .child(match field {
                Some(field) => field,
                None => (div().flex_1().min_w_0().truncate().child(title)).into_any_element(),
            })
            .children(status_mark(status, !store.hidden(), style))
            .when(!renaming, |el| self.terminal_drag(el, item, id, cx))
            .map(|el| self.selectable(el, item, cx));
        Some(row.into_any_element())
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
            .map(|el| self.selectable(el, item, cx))
            .map(|el| self.page_drag(el, item, cx))
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
                format!(" at {}", store.short_reference(item.project(), *id))
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

    /// A worktree's heading, which selects its overview. Its chevron or a double-click folds
    /// its uncommitted changes and branches away; folded, it counts the changes and the
    /// branches' unpushed commits.
    fn worktree_heading(
        &self,
        ix: usize,
        item: &Selection,
        checkout: &Checkout,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let id = item.project();
        let (entry, workspace) = (store.project(id)?, store.workspace(id)?);
        let worktree = (checkout.linked()).and_then(|name| workspace.find_worktree(name));
        let (label, path, uncommitted) = match worktree {
            Some(wt) => (wt.name.to_string(), &wt.path, &wt.uncommitted),
            None => (
                "Main worktree".to_owned(),
                &entry.project.path,
                &workspace.uncommitted,
            ),
        };
        let uncommitted = store.uncommitted_shown(id, checkout, uncommitted);
        let branches: Vec<_> = workspace.branches_in(checkout).collect();
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
        // Its uncommitted changes are always listed under it, so it always folds.
        let step = self.fold(
            ("worktree-fold", ix),
            Some(open),
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
            .when(!open && unread, |el| {
                el.relative().child(style.unread_line(false))
            })
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
                    .child(label.clone()),
            )
            .when(!open, |el| {
                el.children(change_counts(tally(&uncommitted), style))
                    .children(style.unpushed(unpushed_n))
            })
            .map(|el| self.selectable(el, item, cx))
            .on_click(double_click(toggle, cx));
        // A linked worktree drags itself, its changes going with it.
        let row = match checkout.linked() {
            Some(name) => {
                let payload = Cut::Worktree(id.clone(), name.to_owned());
                self.worktree_drag(row, item, name, payload, name.to_string().into(), cx)
            }
            // The main worktree drags its folder, for a terminal to paste.
            None => self.project_drag(row, item, label.into(), false, cx),
        };
        Some(row.into_any_element())
    }

    /// A worktree's uncommitted changes, first under its heading, which select their files and
    /// diff as [`Selection::Uncommitted`]. Listed only while there are some.
    fn changes_row(
        &self,
        item: &Selection,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let id = item.project();
        let workspace = store.workspace(id)?;
        let checkout = store.checkout_of(item)?;
        let uncommitted = match checkout.linked() {
            Some(name) => &workspace.find_worktree(name)?.uncommitted,
            None => &workspace.uncommitted,
        };
        let uncommitted = store.uncommitted_shown(id, &checkout, uncommitted);
        let label = "Uncommitted changes";
        let row = style
            .row(
                format!("changes-{id}-{}", checkout.linked().unwrap_or_default()),
                6.,
            )
            .aria_label(label)
            .child(icon(Icon::FileDiff).text_color(style.muted()))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .children(change_counts(tally(&uncommitted), style))
            .map(|el| self.selectable(el, item, cx));
        // The checked files while they're shown, else all of them, but not conflicted ones:
        // with only those, there's nothing to drag.
        let files = self.detail.read(cx).files_to_move(id, checkout.clone(), cx);
        let dragged = (!files.is_empty()).then(|| {
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
            (payload, label)
        });
        let row = match dragged.filter(|_| self.drags(cx)) {
            Some((payload, label)) => {
                let preview = drag_preview(self.store.clone(), label);
                row.on_drag(
                    payload,
                    row_preview(self.store.clone(), item.clone(), preview),
                )
            }
            None => self.page_drag(row, item, cx),
        };
        Some(row.into_any_element())
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
            .is_some_and(|(s, ws)| s.project() == project && in_checkout(store, s, ws, &key.1));
        let heading = Selection::Checkout(project.clone(), key.1.clone());
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

    fn branch_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        ref_name: Option<&FullName>,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let moving = store.moving.is_some();
        let row_id = ref_name.map(ToString::to_string).unwrap_or_default();
        let theme = style.theme;
        let branch = store.branch_for(item)?;
        let name = store.name_of(id, branch);
        let unpushed_n = unpushed(std::iter::once(branch));
        let conflicted = branch.commits.iter().filter(|c| c.conflicted).count();
        // The conflict to resolve first, as its page's Resolve finds it.
        let conflict = (ref_name.zip(store.workspace(id)))
            .and_then(|(r, ws)| oldest_conflict(ws, r))
            .map(|(_, commit)| commit.id);
        // Its pull request shows as it's asked for, pulsing until the forge has it.
        let publishing =
            branch.review.is_none() && ref_name.is_some_and(|r| store.publishing(id, r));
        let state = match (&branch.review, branch.push) {
            (Some(review), _) => Some(store.review_state(id, review)),
            (None, _) if publishing => Some(ReviewState::Open),
            // Merged without a pull request, as one that merged with one shows.
            (None, PushState::Integrated) => Some(ReviewState::Merged),
            (None, _) => None,
        };
        let (glyph, glyph_color) = match state {
            Some(state) => review::state_glyph(state, theme),
            None => (Icon::Branch, style.muted()),
        };
        let glyph_color = match state {
            Some(ReviewState::Merged) => review::merged_color(style.solid, theme),
            _ => style.meta(glyph_color),
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
        // An unread commit marks its branch too, folded or not.
        let unread = store.branch_unread(id, branch);
        let review = (branch.review.as_ref()).map(|r| (r, store.review_state(id, r)));
        let rename = self.rename_field(item, theme, cx);
        Some(
            style
                .row(format!("branch-{id}-{row_id}"), 6.)
                .aria_label(format!("Branch {name}"))
                .when(unread, |el| el.relative().child(style.unread_line(false)))
                .child(step)
                .child(match icon(glyph).text_color(glyph_color) {
                    glyph if publishing => pulsing(glyph).into_any_element(),
                    glyph => glyph.into_any_element(),
                })
                .child(match rename {
                    Some(field) => field,
                    None => branch_name(name, review, &style)
                        .id(ElementId::Name(format!("name-{id}-{row_id}").into()))
                        .flex_1()
                        .tooltip(Tooltip::text(tip))
                        .into_any_element(),
                })
                // CI only while it runs or after it failed: passing is the usual state. Pressed,
                // the branch's pull request shows, its checks asked for again.
                .when_some(
                    checks.filter(|c| c.running > 0 || c.failed > 0),
                    |el, checks| {
                        let (glyph, color, text) = review::checks_summary(checks, theme);
                        let color = style.meta(color);
                        let shown = Selection::Branch(id.clone(), ref_name.cloned());
                        let tip = SharedString::from(format!("{text}, click to see the checks"));
                        let mark = div()
                            .id(ElementId::Name(format!("checks-{row_id}").into()))
                            .child(small_icon(glyph).text_color(color))
                            .aria_label(tip.clone())
                            .tooltip(Tooltip::text(tip));
                        el.child(pressable(
                            mark,
                            color,
                            moving,
                            cx.listener(move |this, _, window, cx| {
                                this.select(shown.clone(), window, cx);
                                (this.detail).update(cx, |detail, cx| {
                                    detail.set_review_tab(true, window, cx)
                                });
                                (this.store)
                                    .update(cx, |store, _| store.refresh_forge(shown.project()))
                            }),
                        ))
                    },
                )
                // So conflicts show while the commits are hidden too. Pressed, the first to
                // resolve is up, unless a linked worktree resolves it.
                .children({
                    let warn = style.meta(theme.warn);
                    let tip = plural(conflicted, "conflicted commit");
                    let count = |tip| meta_count(Icon::Warning, conflicted, warn, tip);
                    match conflict {
                        None => count(tip),
                        Some(commit) => count(tip + ", click to resolve").map(|count| {
                            let project = id.clone();
                            let resolve = cx.listener(move |this, _, window, cx| {
                                let project = project.clone();
                                this.app.update_later(window, cx, move |app, window, cx| {
                                    app.resolve_commit(project, commit, window, cx)
                                })
                            });
                            pressable(count, warn, moving, resolve)
                        }),
                    }
                })
                // Folded, how many only its remote has; open, its On origin/… row says.
                .when(!open, |el| {
                    let n = branch.incoming.len();
                    let remote = branch.remote.as_deref().unwrap_or("the remote");
                    let tip = format!("{} only on {remote}", plural(n, "commit"));
                    el.children(self.pull_count(id, ref_name, n, tip, style, cx))
                })
                .children(self.push_count(id, ref_name, unpushed_n, style, cx))
                .map(|el| self.selectable(el, item, cx))
                // A click looks inside, opening its commits; the chevron, a double-click, ⇧←
                // and ⇧→ fold them.
                .when(folds, |el| el.on_click(click_opens(clicked, toggle, cx)))
                .map(|el| match ref_name.filter(|_| self.drags(cx)) {
                    Some(ref_name) => {
                        let payload = Cut::Branch(id.clone(), ref_name.clone());
                        let label = store.label_of(id, branch).to_owned();
                        let preview = drag_preview(self.store.clone(), SharedString::from(label));
                        el.on_drag(
                            payload,
                            row_preview(self.store.clone(), item.clone(), preview),
                        )
                    }
                    None => self.page_drag(el, item, cx),
                })
                .into_any_element(),
        )
    }

    /// How many of the branch's `n` commits aren't pushed, pressed to push them without
    /// selecting the branch. It pulses while they go; while they can't, its tooltip says why.
    fn push_count(
        &self,
        project: &ProjectId,
        ref_name: Option<&FullName>,
        n: usize,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let ws = store.workspace(project)?;
        let pushing = ref_name.is_some_and(|r| store.pushing(project, r));
        let blocked = ws.push_blocked(ref_name);
        let tip = match blocked {
            _ if pushing => "Pushing".to_owned(),
            Some(why) => why.to_owned(),
            None => ws.push_tooltip(ref_name?),
        };
        let color = style.meta(style.theme.blue);
        // Padded as pressing it pads it, so it holds still while it pushes.
        let count = meta_count(Icon::ArrowUp, n, color, tip)?.px(dp(MARK_PAD));
        if pushing {
            return Some(pulsing(count).into_any_element());
        }
        if blocked.is_some() {
            return Some(count.into_any_element());
        }
        let pushed = (project.clone(), ref_name.cloned());
        let push = cx.listener(move |this, _, window, cx| {
            let (project, ref_name) = pushed.clone();
            this.app.update_later(window, cx, move |app, window, cx| {
                app.push_branch(project, ref_name, window, cx)
            });
        });
        let moving = self.store.read(cx).moving.is_some();
        Some(pressable(count, color, moving, push).into_any_element())
    }

    /// How many of `n` commits only `ref_name`'s remote has, pressed to pull them when it can,
    /// which leaves the selection where it is. When it can't, it says `tip`.
    fn pull_count(
        &self,
        project: &ProjectId,
        ref_name: Option<&FullName>,
        n: usize,
        tip: String,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<Stateful<Div>> {
        let warn = style.meta(style.theme.warn);
        let ws = self.store.read(cx).workspace(project);
        let Some(ref_name) = ref_name.filter(|r| ws.is_some_and(|ws| ws.can_pull(r))) else {
            return meta_count(Icon::ArrowDown, n, warn, tip);
        };
        let count = meta_count(
            Icon::ArrowDown,
            n,
            warn,
            format!("Pull {}", plural(n, "commit")),
        )?;
        let (project, ref_name) = (project.clone(), ref_name.clone());
        let pull = cx.listener(move |this, _, _, cx| {
            let ref_name = ref_name.clone();
            (this.store).update(cx, |store, cx| {
                store.pull_remote(&project, ref_name, None, cx)
            })
        });
        let moving = self.store.read(cx).moving.is_some();
        Some(pressable(count, warn, moving, pull))
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
        let (_, commit) = store.find_commit(id, *commit_id)?;
        // A picked commit takes the other picks along.
        let moved = store.with_picks(id, *commit_id);
        let several = moved.len() > 1;
        let dragged: SharedString = match several {
            false => store.message_of(id, commit).0.to_owned().into(),
            true => format!("{} commits", moved.len()).into(),
        };
        let payload = Cut::Commit(id.clone(), moved);
        Some(
            style
                .commit(
                    format!("commit-{id}-{}", commit_id.0),
                    id,
                    commit,
                    store,
                    self.rename_field(item, style.theme, cx),
                )
                // Selected but on its way out.
                .when(store.hides_commit(id, &commit.key), |el| el.opacity(0.5))
                .map(|el| self.selectable(el, item, cx))
                .when(several, |el| {
                    let item = item.clone();
                    el.on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        let held = event.modifiers();
                        let moving = this.store.read(cx).moving.is_some();
                        if !moving && !held.secondary() && !held.shift {
                            this.select(item.clone(), window, cx)
                        }
                    }))
                })
                .when(self.drags(cx), |el| {
                    let preview = drag_preview(self.store.clone(), dragged);
                    el.on_drag(
                        payload,
                        row_preview(self.store.clone(), item.clone(), preview),
                    )
                })
                .into_any_element(),
        )
    }

    /// A commit only a remote has, among its branch's commits or under the project: a ↓
    /// where a commit has its glyph, and a hairline while it's unread, as only a branch's get.
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
                    None,
                )
                .map(|el| self.selectable(el, item, cx))
                .map(|el| self.page_drag(el, item, cx))
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
        let after = store.prefs.inactive_after;
        let label = "Inactive worktrees";
        self.section_heading(ix, project, &Section::Inactive, label, style, cx)
            .when(!open, |el| {
                el.child(faint(inactive.len().to_string(), style.theme).text_color(style.quiet()))
            })
            .tooltip(move |window, cx| Tooltip::text(after.rule())(window, cx))
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

    /// Update, which rebases `project`'s branches and worktrees onto its target's new commits;
    /// `inverted` on a solid selection. Pressing it leaves the selection where it is.
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
            "Rebase your branches and worktrees onto the target's new commits",
            &UpdateFromTarget,
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            // Only part of its row while something is being moved, which the press put there.
            if this.store.read(cx).moving.is_some() {
                return;
            }
            let project = project.clone();
            this.app.update_later(window, cx, move |app, _, cx| {
                app.update_from_target(project, UpdateScope::All, cx)
            });
        }));
        let moving = self.store.read(cx).moving.is_some();
        div()
            .when(!moving, |el| {
                el.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            })
            // Pulsing while it updates, rather than a longer label moving what's beside it.
            .child(match updating {
                true => pulsing(div().child(update)).into_any_element(),
                false => update.into_any_element(),
            })
    }

    /// Over the commits only branch `ref_name`'s remote has, or with `None` the target's new
    /// ones, a section heading as Network worktrees has: where they are, how many while
    /// folded, and for a branch, Pull. The branch's own row says when any is unread.
    fn upstream_heading(
        &self,
        ix: usize,
        project: &ProjectId,
        ref_name: Option<&FullName>,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let workspace = store.workspace(project);
        let open = self.upstream_open(store, project, ref_name);
        let target = workspace.and_then(|ws| ws.target.as_ref());
        let (label, count, tip) = upstream_label(workspace, ref_name);
        let update = ref_name.is_none() && target.is_some_and(|t| t.needs_update());
        let can_pull = (ref_name.zip(workspace)).is_some_and(|(r, ws)| ws.can_pull(r));
        let section = Section::Upstream(ref_name.cloned());
        let row = self.section_heading(ix, project, &section, label, style, cx);
        let row = row
            .pr(dp(8.))
            // Folded, how many. While they can be pulled it shows open too, as what pulls them.
            .when(!open || can_pull, |el| {
                el.children(self.pull_count(project, ref_name, count, tip, style, cx))
            })
            .when(update, |el| {
                el.child(self.target_update_button(project, style.solid, cx))
            });
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
            .map(|el| self.selectable(el, &item, cx))
            .map(|el| self.page_drag(el, &item, cx))
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
/// start and end with, which it measures again. The rows of `old` they were, if any changed.
fn splice(list: &ListState, old: &[Row], new: &[Row], at: usize) -> Option<Range<usize>> {
    let head = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let (old, new) = (
        old.get(head..).unwrap_or_default(),
        new.get(head..).unwrap_or_default(),
    );
    let tail = (old.iter().rev().zip(new.iter().rev()))
        .take_while(|(a, b)| a == b)
        .count();
    let changed = old.len() > tail || new.len() > tail;
    let spliced = at + head..at + head + old.len() - tail;
    changed.then(|| {
        list.splice(spliced.clone(), new.len() - tail);
        spliced
    })
}

/// How far the rest of row `ix`'s card goes on from it, down or else up, up to `room`, short of
/// the card's last or first row: a stretch over those would show its square corners outside
/// their round ones, which they're drawn with anyway, by the list or by [`Navigator::slid_in`].
fn card_beyond(rows: &[Row], ix: usize, down: bool, room: f32) -> f32 {
    let (before, after) = rows.split_at(ix.min(rows.len()));
    let far = |rest: &mut dyn Iterator<Item = &Row>| {
        let mut rest = rest.take_while(|row| !matches!(row, Row::Gap)).peekable();
        let mut far = 0.;
        while let Some(row) = rest.next() {
            if far >= room || rest.peek().is_none() {
                break;
            }
            far += row.height();
        }
        f32::min(far, room)
    };
    match down {
        true => far(&mut after.iter().skip(1)),
        false => far(&mut before.iter().rev()),
    }
}

/// Whether `old` and `new` list the same cards, and the same linked worktrees in them, in
/// another order.
fn headings_reordered(old: &[Row], new: &[Row]) -> bool {
    fn headings(rows: &[Row]) -> Vec<&Selection> {
        (rows.iter())
            .filter_map(|row| match row {
                Row::Item(
                    heading @ (Selection::Project(_) | Selection::Checkout(_, Checkout::Linked(_))),
                ) => Some(heading),
                _ => None,
            })
            .collect()
    }
    let (old, new) = (headings(old), headings(new));
    old != new && old.len() == new.len() && old.iter().all(|heading| new.contains(heading))
}

/// Where the list's `top` in `old` goes in `new`, their rows the same up to `from`: as far
/// down past `from`, as the rows count their heights ([`span_height`]).
fn as_deep(old: &[Row], new: &[Row], from: usize, top: ListOffset, theme: &Theme) -> ListOffset {
    let depth = span_height(old, from..top.item_ix) + f32::from(top.offset_in_item) / theme.scale;
    let mut above = 0.;
    for (ix, row) in new.iter().enumerate().skip(from) {
        let height = row.height();
        if above + height > depth {
            return ListOffset {
                item_ix: ix,
                offset_in_item: theme.scaled(depth - above),
            };
        }
        above += height;
    }
    ListOffset {
        item_ix: new.len(),
        offset_in_item: Pixels::ZERO,
    }
}

/// Whether going from `old` to `new` only lists or unlists checkouts' uncommitted changes.
fn changes_came_or_went(old: &[Row], new: &[Row]) -> bool {
    fn others(rows: &[Row]) -> impl Iterator<Item = &Row> {
        (rows.iter()).filter(|row| !matches!(row, Row::Item(Selection::Uncommitted(..))))
    }
    old.len() != new.len() && others(old).eq(others(new))
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

/// The row standing for `selection`: its own, or for a project's pages, which are tabs of its
/// page, its card.
fn selected_row(selection: &Selection) -> Selection {
    match selection {
        Selection::Branches(project) | Selection::History(project) | Selection::Graph(project) => {
            Selection::Project(project.clone())
        }
        other => other.clone(),
    }
}

/// Whether `new` stands where `old` did, to keep it under the pointer: the same row, or the
/// other side of a fold, as "Show fewer" becomes "N more".
fn same_place(old: &Row, new: &Row) -> bool {
    match (old, new) {
        (Row::Fewer(project, ref_name), Row::More(p, r, _)) => project == p && ref_name == r,
        _ => old == new,
    }
}

/// What ⇧← goes up to from `selection`, a branch, a terminal or uncommitted changes: its
/// worktree's heading. Not while a commit is edited, which lists its files without one.
fn parent(store: &Store, selection: &Selection) -> Option<Selection> {
    let project = selection.project();
    if store.workspace(project)?.editing.is_some() {
        return None;
    }
    match selection {
        Selection::Branch(..) | Selection::Terminal(..) | Selection::Uncommitted(..) => Some(
            Selection::Checkout(project.clone(), store.checkout_of(selection)?),
        ),
        _ => None,
    }
}

/// The row folding hides `item` under, as ⇧← goes up to it, up to its project's: a commit's
/// branch, an inactive worktree's heading, else as [`parent`] says.
fn holder(store: &Store, item: &Selection) -> Option<Selection> {
    let project = item.project().clone();
    let up = match item {
        Selection::Project(_) => return None,
        Selection::Commit(_, id) => (store.find_commit(&project, *id))
            .map(|(branch, _)| Selection::Branch(project.clone(), branch.ref_name.clone())),
        Selection::Checkout(_, Checkout::Linked(name))
            if (store.workspace(&project))
                .and_then(|ws| ws.find_worktree(name.as_ref()))
                .is_some_and(|wt| wt.is_inactive(store.prefs.inactive_after)) =>
        {
            Some(Selection::Section(project.clone(), Section::Inactive))
        }
        _ => parent(store, item),
    };
    Some(up.unwrap_or(Selection::Project(project)))
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
/// What a terminal's program is at, in colour only where the user is wanted, turning while
/// it works and `seen`.
fn status_mark(status: &Status, seen: bool, style: RowStyle) -> Option<Stateful<Div>> {
    let theme = style.theme;
    let said = |reason: &str, otherwise: &str| -> SharedString {
        match reason {
            "" => otherwise.to_owned().into(),
            reason => reason.to_owned().into(),
        }
    };
    let glyph = |glyph: Icon, color: Hsla| small_icon(glyph).text_color(color).into_any_element();
    let (drawn, tip) = match status {
        Status::Idle => return None,
        Status::Working => (
            spinning(style.muted(), seen).into_any_element(),
            "Working".into(),
        ),
        Status::Blocked(reason) => (
            glyph(Icon::Warning, style.meta(theme.warn)),
            said(reason, "Waiting for you"),
        ),
        Status::Done => (glyph(Icon::Tick, style.meta(theme.safe)), "Finished".into()),
        Status::Failed(reason) => (
            glyph(Icon::Cross, style.meta(theme.danger)),
            said(reason, "Failed"),
        ),
    };
    let mark = div()
        .id("terminal-status")
        .flex_none()
        .aria_label(tip.clone())
        .child(drawn)
        .tooltip(Tooltip::text(tip));
    Some(mark)
}

/// `mark` as a button, its tooltip read as its name: padded with `MARK_PAD` and tinted `color`
/// under the pointer. Pressing it does `press` and leaves its row as it was, unselected and
/// unfolded. While something is `moving` it's only part of its row, which a press puts the move
/// at, so it does nothing of its own then.
fn pressable(
    mark: Stateful<Div>,
    color: Hsla,
    moving: bool,
    press: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let mark = mark.px(dp(MARK_PAD)).rounded(dp(4.));
    if moving {
        return mark;
    }
    mark.role(Role::Button)
        .hover(move |el| el.bg(color.opacity(0.16)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            press(event, window, cx)
        })
}

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
                    .child(crate::ui::diff::stats(added, removed, style.solid, theme))
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

/// What the heading over the commits only `ref_name`'s remote has, or with `None` the
/// target's new ones, says: where they are, how many, and that count in words.
pub(crate) fn upstream_label(
    workspace: Option<&Workspace>,
    ref_name: Option<&FullName>,
) -> (String, usize, String) {
    match ref_name {
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
            let target = workspace.and_then(|ws| ws.target.as_ref());
            let count = target.map_or(0, |t| t.behind);
            let label = match target {
                Some(target) => format!("New on {}", target.name),
                None => "New on the target".to_owned(),
            };
            let tip = plural(count, "commit") + &target.map(with_merged).unwrap_or_default();
            (label, count, tip)
        }
    }
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

/// How many commits pushing `branches` sends.
fn unpushed<'a>(branches: impl Iterator<Item = &'a Branch>) -> usize {
    branches.map(Branch::unpushed_commits).sum()
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
        self.folds.frame();
        self.reorder.frame();
        let now = clock::now();
        self.stand_ins.retain_mut(|stand_in| stand_in.fade(now));
        let mut built = self.build_rows(cx);
        // A fold that just opened or shut is drawn moving from this frame on.
        if self.follow_folds(&built.folds, cx) {
            built = self.build_rows(cx);
        }
        let Rows {
            rows,
            insets,
            guides,
            moving,
            first_hit,
            drawer,
            ..
        } = built;
        self.moving = moving;
        // As the list drew it, before the rows change, which a reorder doesn't do by much.
        let inset = Theme::get(cx).scaled(LIST_TOP);
        let last_listed = (self.list.logical_scroll_top().item_ix..self.rows.len())
            .take_while(|ix| fold::listed(&self.list, inset, *ix))
            .last();
        self.sync_list(rows, drawer, window, cx);
        // After the rows, whose stand-ins are set in and guided as they were drawn.
        self.insets = insets;
        self.guides = guides;
        self.held_card = self.held_card(self.store.read(cx));
        self.held_worktree = self.held_worktree(self.store.read(cx));
        self.slide_edges = self.reorder.any().then(|| {
            let first = self.list.logical_scroll_top().item_ix;
            let below = last_listed.map_or(first, |last| last.saturating_sub(ABOVE_LAST_LISTED));
            (first, below.max(first))
        });
        self.fit_folds(cx);
        // After the rows are up to date, as a reorder of them starts sliding there, and nothing
        // else may draw the next frame.
        if self.folds.any() || self.reorder.any() {
            window.request_animation_frame();
        }
        let theme = Theme::get(cx).clone();
        self.first_hit = first_hit;
        let store = self.store.read(cx);
        let empty = store.projects_loaded && store.projects.is_empty();
        let filter_focused = self.filter.focus_handle(cx).is_focused(window);
        let filtering = !self.filter.read(cx).text().is_empty();
        let detail_shown = self.detail_shown.get();
        // Level with the location bar: the same height, from the window's top, and no rule
        // along its bottom as the cards scroll under it, as the bar has none either.
        let header = h_flex()
            .h(dp(HEADER_HEIGHT))
            .flex_none()
            // Room for macOS's traffic lights; elsewhere the window's buttons are at its other end.
            // A card out over the detail has them out of its way, and isn't the window's title bar.
            .map(
                |el| match crate::ui::has_traffic_lights(window) && !self.floating {
                    true => el.pl(fixed(84.)),
                    false => el.pl(dp(8.)),
                },
            )
            // Clear of the window's buttons, where the app draws them over its right end, as the
            // detail slides away from under them.
            .map(
                |el| match crate::ui::draws_window_controls(window) && detail_shown < 1. {
                    true => {
                        el.pr(theme.scaled(8.) + fixed(WINDOW_CONTROLS_WIDTH * (1. - detail_shown)))
                    }
                    false => el.pr(dp(8.)),
                },
            )
            .when(!self.floating, crate::ui::drags_window)
            .gap(dp(6.))
            // Its clear button inside it, so typing the first letter doesn't narrow it.
            .child(
                above_drag(
                    // Edged as the cards are, but while it has the keys.
                    field(&self.filter, filter_focused, &theme)
                        .when(!filter_focused, |el| el.border_color(theme.border_3)),
                )
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
        let store = self.store.read(cx);
        let moving = store.moving.is_some();
        let squashing =
            (store.moving.as_ref()).is_some_and(|moving| matches!(moving.op, Op::Squash(_)));
        // A commit or a named branch of a workspace, not one only a remote or another machine
        // has.
        let commits = match &store.selection {
            Some(Selection::Commit(project, id)) => store.find_commit(project, *id).is_some(),
            Some(Selection::Branch(project, ref_name @ Some(_))) => (store.workspace(project))
                .is_some_and(|ws| ws.find_branch(ref_name.as_ref()).is_some()),
            _ => false,
        };
        let movable = commits
            || match &store.selection {
                Some(Selection::Project(project)) => {
                    store.project(project).is_some_and(|p| p.expanded)
                }
                Some(Selection::Checkout(project, Checkout::Linked(name))) => (store
                    .workspace(project))
                .is_some_and(|ws| ws.find_worktree(name.as_ref()).is_some()),
                Some(Selection::Terminal(..)) => true,
                _ => false,
            };
        // Those, or a worktree's uncommitted changes.
        let squashable = commits
            || match &store.selection {
                Some(item @ Selection::Uncommitted(project, _)) => (store.checkout_of(item))
                    .and_then(|checkout| store.workspace(project)?.uncommitted_in(&checkout))
                    .is_some_and(|files| !files.is_empty()),
                _ => false,
            };
        // Commits or a branch being moved squash instead, and being squashed, move instead;
        // files only squash.
        let moves_commits = (store.moving.as_ref()).is_some_and(Move::of_commits);

        v_flex()
            .id("navigator")
            // While commits move, Return lands them. On the drawer's bar, the keys that act on
            // the selection are off, as it's left the row.
            .key_context(match (self.on_drawer_bar, moving, squashing) {
                (true, _, _) => "DrawerBar",
                (false, true, true) => "Navigator Moving Squashing",
                (false, true, false) => "Navigator Moving",
                (false, false, _) => "Navigator",
            })
            .on_drag_move(cx.listener(Self::follow_project_drag))
            .on_drag_move(cx.listener(Self::follow_worktree_drag))
            .on_drag_move(cx.listener(Self::follow_terminal_drag))
            .on_drag_move(cx.listener(|this, event: &DragMoveEvent<Cut>, _, cx| {
                this.drag_moved(event.bounds.contains(&event.event.position), cx)
            }))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<ProjectDrag>, _, cx| {
                    this.drag_moved(event.bounds.contains(&event.event.position), cx)
                }),
            )
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<TerminalDrag>, _, cx| {
                    this.drag_moved(event.bounds.contains(&event.event.position), cx)
                }),
            )
            .on_drag_move(cx.listener(|this, event: &DragMoveEvent<PageDrag>, _, cx| {
                this.drag_moved(event.bounds.contains(&event.event.position), cx)
            }))
            .on_drop(cx.listener(Self::drop_project))
            .on_drop(cx.listener(Self::drop_worktree))
            .on_drop(cx.listener(Self::drop_terminal))
            .track_focus(&self.focus)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.pointer_over = *hovered;
                // GPUI calls a drag's start a leave, and the drop has to land where it's aimed:
                // the drag's moves say where it is meanwhile (see `drag_moved`).
                this.hold_order(*hovered || cx.has_active_drag(), cx);
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
            .on_action(cx.listener(Self::toggle_fold))
            .when((movable && !moving) || (squashing && moves_commits), |el| {
                el.on_action(cx.listener(Self::start_move))
            })
            .when(
                (squashable && !moving) || (!squashing && moves_commits),
                |el| el.on_action(cx.listener(Self::start_squash)),
            )
            .when(commits && !moving, |el| {
                el.on_action(cx.listener(Self::start_squash_into))
            })
            .when(moving, |el| el.on_action(cx.listener(Self::move_here)))
            .when(squashing, |el| {
                el.on_action(cx.listener(Self::use_target_message))
            })
            .size_full()
            .relative()
            .bg(theme.bg_2)
            // Out over the detail, it's a popover's card. GPUI cuts what's inside to a
            // rectangle, so the drawer along its bottom rounds its own corners.
            .map(|el| match self.floating {
                true => (el.rounded(dp(POPOVER_RADIUS)).border_1()).border_color(theme.border_2),
                // On the detail's ground, which its cards edge, nothing to edge against, but the
                // room an edge took stays, so what's against the right doesn't move.
                false => el.border_r_1().border_color(gpui::transparent_black()),
            })
            .when(!self.floating && detail_shown >= 1., |el| {
                // The gap to the detail's cards, its edge included, drags the navigator's
                // width, its middle as far in as its edge is out. Before the list, under its
                // scrollbar's thumb, which shows in the gap while it scrolls.
                let which = DividerDrag::Navigator;
                el.child(
                    gap_divider(
                        "resize-navigator",
                        Axis::Across,
                        which.clone(),
                        &self.store,
                        &theme,
                        cx,
                    )
                    .top_0()
                    .bottom_0()
                    .right(fixed(-1.))
                    .w(dp(2. * TILE_MARGIN))
                    .on_drag(Pane::Navigator, mark_dragged(&self.store, which))
                    .on_click(cx.listener(
                        |this, event: &ClickEvent, _, cx| {
                            if event.click_count() == 2 {
                                (this.store).update(cx, |store, cx| {
                                    store.set_width(Pane::Navigator, None, cx)
                                })
                            }
                        },
                    )),
                )
            })
            .child(header)
            .map(|el| {
                // With no projects, the welcome beside it says what to do.
                if empty {
                    el
                } else if self.rows.is_empty() {
                    el.child(self.drawer(window, &theme, cx))
                } else {
                    let rows = list(
                        self.list.clone(),
                        cx.processor(|this, ix: usize, window, cx| this.render_row(ix, window, cx)),
                    )
                    .size_full()
                    .pt(dp(LIST_TOP));
                    self.fit_footer(window, cx);
                    self.find_focused_card(window, cx);
                    let scroll = Scroll::List(self.list.clone());
                    el.child(
                        // Its thumb in the middle of the gap to the detail's cards, beside it
                        // only while it scrolls.
                        scrolled_in_gap(
                            rows,
                            "navigator-scrollbar",
                            scroll,
                            theme.scaled(match self.card_margins().1 == CARD_MARGIN_BESIDE {
                                true => (CARD_MARGIN_BESIDE - 1.) / 2.,
                                false => CARD_MARGIN / 2.,
                            }),
                            &theme,
                        )
                        .children(self.card_caps(&theme, cx))
                        // Ending level with the detail's cards, so what scrolls past is cut
                        // where they end.
                        .mb(dp(CARD_GAP))
                        .on_drag_move(cx.listener(|this, event: &DragMoveEvent<Cut>, w, cx| {
                            this.scroll_near_edge(event.event.position, event.bounds, w, cx)
                        }))
                        .on_drag_move(cx.listener(
                            |this, event: &DragMoveEvent<ProjectDrag>, w, cx| {
                                this.scroll_near_edge(event.event.position, event.bounds, w, cx)
                            },
                        ))
                        .on_drag_move(cx.listener(
                            |this, event: &DragMoveEvent<TerminalDrag>, w, cx| {
                                this.scroll_near_edge(event.event.position, event.bounds, w, cx)
                            },
                        ))
                        .on_drag_move(cx.listener(
                            |this, event: &DragMoveEvent<PageDrag>, w, cx| {
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
        // A card out over the detail goes back once the pointer leaves it, so it's resized
        // only beside the detail, once it's all there.
    }
}

#[cfg(test)]
mod tests {
    use super::{Row, Step, headings_reordered, step_to};
    use crate::backend::Checkout;
    use crate::state::{ProjectId, Selection};

    #[test]
    fn a_step_down_from_the_last_card_row_lands_on_the_bar() {
        assert_eq!(
            step_to(Some(2), 3, Some(3), false, 1),
            Some(Step::Bar),
            "the folded drawer's bar comes after the cards' rows"
        );
    }

    #[test]
    fn a_step_down_from_the_folded_bar_stays_put() {
        assert_eq!(
            step_to(Some(2), 3, Some(3), true, 1),
            None,
            "a held key doesn't open the drawer"
        );
    }

    #[test]
    fn a_step_down_from_the_open_bar_goes_to_its_first_project() {
        assert_eq!(
            step_to(Some(2), 5, Some(3), true, 1),
            Some(Step::Item(3)),
            "the drawer's rows follow its bar"
        );
    }

    #[test]
    fn a_step_up_from_the_bar_goes_to_the_last_card_row() {
        assert_eq!(
            step_to(Some(4), 5, Some(3), true, -1),
            Some(Step::Item(2)),
            "from the bar, wherever the selection stayed"
        );
    }

    #[test]
    fn a_step_up_from_the_drawers_first_project_lands_on_the_bar() {
        assert_eq!(
            step_to(Some(3), 5, Some(3), false, -1),
            Some(Step::Bar),
            "the bar is above the drawer's rows"
        );
    }

    #[test]
    fn a_step_within_the_drawer_counts_the_bar_above_it() {
        assert_eq!(
            step_to(Some(3), 5, Some(3), false, 1),
            Some(Step::Item(4)),
            "the bar is a stop, not an item"
        );
    }

    #[test]
    fn selecting_the_last_lands_on_the_folded_bar() {
        assert_eq!(
            step_to(Some(0), 3, Some(3), false, isize::MAX / 2),
            Some(Step::Bar),
            "it's the last stop while the drawer's folded"
        );
    }

    #[test]
    fn selecting_the_last_passes_the_open_bar() {
        assert_eq!(
            step_to(Some(0), 5, Some(3), false, isize::MAX / 2),
            Some(Step::Item(4)),
            "the drawer's last project is the last stop"
        );
    }

    #[test]
    fn selecting_the_first_leaves_the_bar_for_the_top() {
        assert_eq!(
            step_to(Some(2), 3, Some(3), true, isize::MIN / 2),
            Some(Step::Item(0)),
            "from the bar too"
        );
    }

    #[test]
    fn without_a_bar_a_step_stops_at_the_last_item() {
        assert_eq!(
            step_to(Some(2), 3, None, false, 1),
            None,
            "a drawer that can't fold is no stop"
        );
    }

    #[test]
    fn a_bar_that_stopped_being_a_stop_steps_from_the_selection() {
        assert_eq!(
            step_to(Some(1), 3, None, true, 1),
            Some(Step::Item(2)),
            "the filter took the bar away before the rows were drawn"
        );
    }

    #[test]
    fn from_off_the_list_a_step_back_starts_at_the_bottom() {
        assert_eq!(
            step_to(None, 3, None, false, -1),
            Some(Step::Item(2)),
            "and forward, at the top"
        );
        assert_eq!(step_to(None, 3, None, false, 1), Some(Step::Item(0)));
    }

    #[test]
    fn cards_or_worktrees_trading_places_are_a_reorder() {
        let (a, b) = (ProjectId::from("a"), ProjectId::from("b"));
        let card = |id: &ProjectId| Row::Item(Selection::Project(id.clone()));
        let worktree = |name: &str| {
            Row::Item(Selection::Checkout(
                a.clone(),
                Checkout::Linked(name.into()),
            ))
        };
        let commit = || Row::Item(Selection::Uncommitted(a.clone(), Checkout::Main));
        let listed = [
            card(&a),
            worktree("x"),
            commit(),
            worktree("y"),
            Row::Gap,
            card(&b),
        ];
        let swapped = [
            card(&a),
            worktree("y"),
            worktree("x"),
            commit(),
            Row::Gap,
            card(&b),
        ];
        assert!(
            headings_reordered(&listed, &swapped),
            "worktrees trading places keep the list where it is, as cards do"
        );
        let cards = [
            card(&b),
            Row::Gap,
            card(&a),
            worktree("x"),
            commit(),
            worktree("y"),
        ];
        assert!(
            headings_reordered(&listed, &cards),
            "and cards trading places"
        );
        let gone = [card(&a), worktree("y"), Row::Gap, card(&b)];
        assert!(
            !headings_reordered(&listed, &gone),
            "a worktree going is no reorder"
        );
        assert!(
            !headings_reordered(&listed, &listed),
            "nor rows that stay in order"
        );
    }
}
