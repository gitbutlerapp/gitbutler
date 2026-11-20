//! The left column: every project as a card, each listing its uncommitted changes,
//! stacks, branches and commits as one tree of rows.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use but_tty::Status;
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, ElementId, Entity, FocusHandle, Focusable,
    Hsla, ListAlignment, ListOffset, ListState, Modifiers, MouseButton, MouseDownEvent,
    PathBuilder, Pixels, Point, Role, SharedString, Stateful, Subscription, Task, Window, actions,
    canvas, div, fill, list, point, prelude::*, relative,
};

use crate::app::{AddProject, AppHandle, UpdateFromTarget};
use crate::backend::rebuild::Rebuild;
use crate::backend::upstream::UpdateScope;
use crate::backend::{
    Branch, Checkout, Commit, CommitId, CommitState, FileChange, FullName, PushState, Review,
    ReviewState, Target, Workspace,
};
use crate::detail::{ChecksChanged, Detail, oldest_conflict};
use crate::overlays::{MenuItem, keys_at};
use crate::state::{
    Part, Prefs, ProjectId, Section, Selection, Store, StoreEvent, Tile, WatchedOrder,
};
use crate::terminal::TerminalId;
use crate::theme::{Hue, Theme, tokens};
use crate::transfer::{
    Changes, Dragged, Move, Op, PageDrag, drag_preview, page_preview, row_preview,
};
use crate::ui::emoji::ProjectIcon;
use crate::ui::fold::{self, Folds, Reorder, clipped, curtain, inert, on_top, reordered, shifted};
use crate::ui::scrollbar::{Scroll, scrollbar, scrolled_in_gap};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    BLINK, Button, ButtonStyle, HEADER_HEIGHT, Icon, Tooltip, WINDOW_CONTROLS_WIDTH, above_drag,
    blink_once, button, commit_title, count_badge, dp, faint, field, fixed, fold_button, fold_row,
    glow_once, guide, h_flex, icon, icon_at, icon_button, list_row, meta_count, plural, pulsing,
    review, section_label, small_icon, spinning, unread_line, v_flex,
};

mod account;
mod cursor;
mod drop;
mod frame;
mod menu;
pub(crate) use menu::{
    ArchiveWorktree, OpenProjectSettings, OpenRepository, Reveal, StopWatching, UnarchiveWorktree,
    Watch,
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
mod tabs;
mod terminals;
use cursor::row_actions;
use drop::on_any_drag_move;
use frame::{Built, RowsIn};
pub(crate) use rows::{lists_changes, worktrees_listed};
pub(crate) use tabs::NavigatorTabs;
pub(crate) use terminals::TerminalOverview;

use crate::ui::Text;
use moving::Mark;
use rows::{
    CARD_EDGE, CARD_PADDING, COMMIT_INDENT, Drawer, Fold, HEADING_HEIGHT, ROW_LEFT, Row, RowKey,
    caption, card_ends, checkout_holds, in_checkout, in_inactive, inactive_open, span_height,
};

/// How far apart the cards are, as the detail's are: but-ui's `gap/stack`, the one gap between
/// cards stacked in a column on every surface.
pub(super) const CARD_GAP: f32 = tokens::GAP_STACK;

/// How wide and tall the map of a row's tiles is, and the gap between them there.
const TILE_MAP: (f32, f32) = (20., 14.);
const TILE_MAP_GAP: f32 = 1.5;

/// How far the cards sit in from the list's sides, as far as the detail's cards are from the
/// window's edges.
const CARD_MARGIN: f32 = CARD_GAP;

/// How far they sit in from its side beside the detail: with the navigator's edge, the whole
/// gap to the detail's cards, as wide as the gaps between those, which the detail's cards sit
/// right against, so the gap's divider and the list's scrollbar are the navigator's own.
const CARD_MARGIN_BESIDE: f32 = CARD_GAP - 1.;

const LIST_TOP: f32 = 0.;

/// How many rows above the last one the list drew last frame draws the rows sliding into view
/// from past its bottom, so it's drawn though the rows moved since.
const ABOVE_LAST_LISTED: usize = 2;

/// How tall the bar of a card at the navigator's foot is, the drawer's level with the commit
/// footer beside it, and the space under the drawer's rows.
pub(super) const CARD_BAR_HEIGHT: f32 = crate::detail::FOOTER_HEIGHT;
const DRAWER_PADDING: f32 = 6.;

/// How far that bar sits in from its card's edges, as a row from a card's.
pub(super) const CARD_BAR_INSET: f32 = 3.;

/// How tall the drawer's card is around its rows: its bar, and its edges.
const DRAWER_CHROME: f32 = CARD_BAR_HEIGHT + 2. * CARD_BAR_INSET + 2.;

/// How far the rows under a row sit in from it: its 16-point icon and the gap after it, so
/// their icons line up under its name.
const STEP: f32 = 16. + 8.;

/// The group every item row is, so a fold's chevron shows in place of its icon while the
/// pointer is over the row (see [`Navigator::fold`]).
const ROW_GROUP: &str = "navigator-row";

/// The eye of a project the app watches, or struck through for one it doesn't.
fn watch_glyph(watched: bool) -> gpui::Svg {
    let eye = match watched {
        true => Icon::Eye,
        false => Icon::EyeClosed,
    };
    crate::ui::icon_at(eye, 14.)
}

actions!(
    navigator,
    [
        SelectFirst,
        SelectLast,
        PreviousSection,
        NextSection,
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

/// How an item row draws: `solid` is the selection while the navigator has focus, `taken`
/// is the row being moved, `held` a copy of what's being moved, or a terminal
/// going where it'd land itself, `mark` the colour taken or held rows are marked in, and `inset`
/// is how far in its section sets it.
#[derive(Clone, Copy)]
pub(crate) struct RowStyle<'a> {
    theme: &'a Theme,
    selected: bool,
    solid: bool,
    taken: bool,
    held: bool,
    mark: Hue,
    inset: f32,
    /// The rows above and below are picked with it: see [`crate::ui::joined`].
    joined: (bool, bool),
    /// The rows above and below are outlined as it is, moving with it, so a run of
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
    Copied,
    /// Where the move under way would land, as a click it can't use reminds the user of it.
    Move,
}

/// How far a branch's remote's commits sit in: a step in from their heading, which is under
/// the branch's commit glyphs.
const UPSTREAM_INDENT: f32 = COMMIT_INDENT + STEP;

/// Where the middle of a branch's remote's heading's glyph sits: that of a 12-point glyph
/// under the branch's commit glyphs, as a commit's is.
const UPSTREAM_GUIDE: f32 = ROW_LEFT + COMMIT_INDENT + 6.;

/// Where the middle of a remote merge's glyph sits, a step in from its heading's.
const MERGE_GUIDE: f32 = UPSTREAM_GUIDE + STEP;

/// What a checkout's row and tab call the main worktree.
pub(super) const MAIN_WORKTREE: &str = "Main worktree";

/// The bar atop a card at the navigator's foot, the projects' drawer's and the terminal
/// overview's: while it folds, its chevron, up while it's `open` is false and turning down as
/// it opens up into the column, then its `title` and `count`. The caller adds its buttons and,
/// while it folds, what a click on it does. `selected` draws it as a row with the keys on it,
/// `solid` as one in a list that has them.
pub(super) fn card_bar(
    id: &'static str,
    title: &'static str,
    count: Option<String>,
    open: Option<bool>,
    selected: bool,
    solid: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let (ink, muted) = match solid {
        true => (theme.text_inverted, theme.muted_on_solid),
        false => (theme.text_3, theme.text_3),
    };
    let fold = h_flex()
        .flex_1()
        .min_w_0()
        .h_full()
        .gap(dp(6.))
        .children(
            open.map(|open| fold::turned(format!("{id}-fold"), open, Icon::ChevronUp, PI, muted)),
        )
        .child(section_label(title, theme).text_color(ink).truncate())
        .children(count.map(|count| faint(count, theme).text_12().text_color(muted)));
    // In from its card's edges as a row is, its hover rounded as one's, so it stays inside the
    // card's corners.
    h_flex()
        .id(id)
        .role(Role::ListItem)
        .aria_label(title)
        .aria_selected(selected)
        .flex_none()
        .h(dp(CARD_BAR_HEIGHT))
        .m(dp(CARD_BAR_INSET))
        .rounded(dp(theme.shape.control))
        .pl(dp(10.))
        .pr(dp(6.))
        .gap(dp(2.))
        .when(selected, |el| el.bg(theme.selected_unfocused))
        .when(solid, |el| el.bg(theme.fill_gray))
        .child(fold)
        .when_some(open, |el, open| {
            el.aria_expanded(open)
                .cursor_pointer()
                .when(!selected, |el| el.hover(|el| el.bg(theme.hover)))
        })
}

/// What the projects not watched go by: all of them, `alone`, with none watched.
pub(super) fn drawer_title(alone: bool) -> &'static str {
    match alone {
        true => "All projects",
        false => "More projects",
    }
}

/// The icon project `id` wears in its row and on its tab, in 16 points: the one picked, else the
/// glyph's outline in `color`.
pub(super) fn project_glyph(
    store: &Store,
    id: &ProjectId,
    color: Hsla,
    theme: &Theme,
) -> AnyElement {
    match ProjectIcon::parse(store.project_icon(id)) {
        ProjectIcon::Glyph => icon_at(Icon::Repo, 16.)
            .text_color(color)
            .into_any_element(),
        picked => picked.render(16., theme),
    }
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
        let on = store.prefs.click_opens_branch && store.moving.is_none();
        if on && event.click_count() == 1 && !held.secondary() && !held.shift && !this.filtering(cx)
        {
            this.click_just_opened = !this.is_open(&branch);
            this.open_on_click(branch.clone(), cx)
        }
    })
}

impl<'a> RowStyle<'a> {
    /// A row set in by `inset` that nothing selects, picks or marks.
    pub(crate) fn plain(theme: &'a Theme, inset: f32) -> Self {
        RowStyle {
            theme,
            selected: false,
            solid: false,
            taken: false,
            held: false,
            mark: Hue::Pop,
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
            .group(ROW_GROUP)
            .when_some(self.blink, |el, n| {
                let shape = div().absolute().inset_0().rounded(dp(theme.shape.control));
                el.relative()
                    .child(blink_once(("blink", n), shape, theme.hue(self.mark)))
            })
            // A pop wash, as a commit that just showed up glows: quieter than a drop's outline.
            .when(self.goes_here, |el| {
                let wash = div().absolute().inset_0().rounded(dp(theme.shape.control));
                el.relative().child(wash.bg(theme.wash(Hue::Pop)))
            })
            .mx(dp(3.))
            .pl(dp(8. + indent + self.inset))
            .when(self.solid, |el| {
                el.bg(theme.fill_gray).text_color(theme.text_inverted)
            })
            .when(self.taken, |el| {
                (el.opacity(0.5).border_1().border_dashed()).border_color(theme.hue(self.mark))
            })
            // As the copies of what's moving are drawn where it would land.
            .when(self.held, |el| {
                el.bg(theme.wash(self.mark))
                    .border_1()
                    .border_color(theme.hue(self.mark))
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
            self.theme.muted_on_solid
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
    /// count is blue, where Push takes its place (see `Navigator::push_step`).
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
            self.meta(self.theme.pop_mark)
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
            (Icon::Warning, theme.warn_mark)
        } else {
            match commit.state {
                CommitState::Local => (Icon::Commit, theme.commit_local),
                CommitState::Pushed => (Icon::CommitFill, theme.commit_remote),
                CommitState::Integrated => (Icon::CommitFill, theme.commit_integrated),
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
        let glyph = small_icon(Icon::Commit).text_color(self.meta(self.theme.commit_local));
        self.commit_line(id, COMMIT_INDENT, glyph, title, false, None)
    }
}

pub struct Navigator {
    app: AppHandle,
    store: Entity<Store>,
    detail: Entity<Detail>,
    focus: FocusHandle,
    /// The rows as last built, and what's worked out from them: see [`Built`].
    built: Built,
    /// How tall a row is, in design pixels, as the density draws it: the theme's control
    /// height, taken as the navigator draws, so its row maths and its rows agree.
    row: f32,
    folds: Folds<Fold>,
    /// The rows sliding to where a reorder put them.
    reorder: Reorder<RowKey>,
    /// Rows a move just took away, drawn where they were while the rows close over them.
    stand_ins: Vec<moving::StandIn>,
    /// The terminal sliding from one card to another, which floats over the cards on its way
    /// rather than taking a slice of the card it's going to with it.
    crossing: Option<RowKey>,
    /// Whether each fold the rows were last built through showed what it holds, to tell
    /// which opened or shut since.
    folds_seen: HashMap<Fold, bool>,
    pub(crate) list: ListState,
    /// The list the drawer draws the projects not watched in.
    pub(crate) drawer_list: ListState,
    /// The room above the drawer at the list's end, and its card's height, as last worked out:
    /// see [`Self::fit_footer`].
    footer: Cell<(f32, f32)>,
    /// How tall it's drawn in this frame, which the app sets once the window's laid out and
    /// before this draws, and how tall it was when the list was last laid out: see
    /// [`Self::fit_footer`].
    pub(crate) drawn_height: Cell<Pixels>,
    listed_height: Cell<Pixels>,
    /// The keys stepped onto the drawer's bar, which folds but is nothing to select, so the
    /// selection and its page stay as they were: see [`cursor::Step`].
    on_drawer_bar: bool,
    /// The selection changed, or where it'd be moved: scroll to it once the rows are up to
    /// date, to the copies of commits being moved where they'd land rather than to them.
    reveal_selection: bool,
    /// An amend just started: the changes going in are scrolled to as well, and first, so
    /// where they are and where they go both show.
    reveal_source: bool,
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
    /// Worktree headings and stack captions showing only themselves, by their row.
    folded: HashSet<Selection>,
    /// Checkouts on other machines showing only their heading, by the mesh's id.
    folded_checkouts: HashSet<(ProjectId, Arc<str>)>,
    /// The commits only a branch's remote has listed under their heading, by the branch's
    /// full ref. They start folded to the heading.
    open_upstream: HashSet<(ProjectId, FullName)>,
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
    pointer_over: bool,
    /// What the card under a tab shows, while the rows it holds stand in for the navigator's
    /// own: being drawn, or the keys stepping and folding them, as they do the navigator's.
    scope: Option<tabs::TabKey>,
    /// The selection the rows being drawn show in place of the store's, for a card on its way
    /// out, which stays as it was left.
    scoped_selection: Option<Option<Selection>>,
    /// The tabs across the window's top have the keys, which stay there as rows are selected.
    strip_has_keys: bool,
    /// What takes the keys for the rows across the top, which is the tabs.
    strip_keys: Option<FocusHandle>,
    /// The tab the card's down from, whose rows a drag out of it goes by, and where the first of
    /// them was last drawn.
    card_key: Option<tabs::TabKey>,
    card_top: Rc<Cell<Option<Pixels>>>,
    /// What has the keys as those rows are drawn, where a press on them leaves them: the
    /// navigator isn't drawn to take them.
    scoped_keys: Option<FocusHandle>,
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
    /// The project whose card is held, kept once it's let go of until the cards stop sliding,
    /// so it stays over the cards it passes going back.
    held_project: Option<ProjectId>,
    /// The linked worktree being dragged by its heading, by its project and name, which goes
    /// where it's let go, but over its own rows where it was, where what it carries lands.
    dragged_worktree: Option<(ProjectId, bstr::BString)>,
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
                if this.branches_start_open == prefs.branches_start_folded {
                    this.branches_start_open = !prefs.branches_start_folded;
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
                    if (this.folded).remove(&Selection::Stack(project.clone(), from.clone())) {
                        (this.folded).insert(Selection::Stack(project.clone(), to.clone()));
                    }
                    let here = |ref_name: &Option<FullName>| {
                        ClickOpened::Here((project.clone(), ref_name.clone()))
                    };
                    if this.flipped.remove(&here(from)) {
                        this.flipped.insert(here(&to));
                    }
                    if let (Some(from), Some(to)) = (from, &to)
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
            (!prefs.branches_start_folded, prefs.all_branch_commits);
        Self {
            app,
            store,
            detail,
            focus: cx.focus_handle(),
            built: Built::default(),
            row: Theme::get(cx).sizes.control,
            folds: Folds::default(),
            reorder: Reorder::default(),
            stand_ins: Vec::new(),
            crossing: None,
            folds_seen: HashMap::new(),
            // Its last item, after the rows, is the drawer: see `drawer_footer`.
            list: ListState::new(1, ListAlignment::Top, fixed(56.)),
            drawer_list: ListState::new(0, ListAlignment::Top, fixed(56.)),
            footer: Cell::new((0., 0.)),
            drawn_height: Cell::new(Pixels::ZERO),
            listed_height: Cell::new(Pixels::ZERO),
            on_drawer_bar: false,
            reveal_selection: false,
            reveal_source: false,
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
            folded: HashSet::new(),
            filter,
            first_hit: None,
            pointer_over: false,
            scope: None,
            scoped_selection: None,
            strip_has_keys: false,
            strip_keys: None,
            card_key: None,
            card_top: Rc::default(),
            scoped_keys: None,
            floating: false,
            detail_shown: Cell::new(1.),
            held_order: None,
            watch_hovered: None,
            held_project: None,
            dragged_worktree: None,
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

    /// Whether `heading`, a worktree's or a stack's, shows the rows under it: it isn't folded,
    /// or the selection is in them (`selected`).
    fn shows_under(&self, heading: &Selection, selected: bool) -> bool {
        selected || !self.folded.contains(heading)
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

    /// A row's `icon`, which turns into its fold's chevron, as [`fold_button`], while the
    /// pointer is over the row, as Notion's sidebar does, so rows that fold and rows that
    /// don't line up. A `selected` row shows the chevron too, so the keys can see what folds.
    /// With nothing to fold (`None`), or while the filter lists what it finds, which folds
    /// don't hide, it stays the icon.
    #[expect(clippy::too_many_arguments)]
    fn fold(
        &self,
        id: impl Into<ElementId>,
        icon: impl IntoElement,
        open: Option<bool>,
        selected: bool,
        color: Hsla,
        cx: &Context<Self>,
        toggle: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let Some(open) = open.filter(|_| !self.filtering(cx)) else {
            return icon.into_any_element();
        };
        let chevron = fold_button(id, Some(open), color, self.pressed_keys(), cx, toggle)
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .when(!selected, |el| {
                el.opacity(0.).group_hover(ROW_GROUP, |el| el.opacity(1.))
            });
        let icon = div().child(icon).map(|el| match selected {
            true => el.opacity(0.),
            false => el.group_hover(ROW_GROUP, |el| el.opacity(0.)),
        });
        div()
            .relative()
            .flex_none()
            .child(icon)
            .child(chevron)
            .into_any_element()
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
        self.take_keys(window, cx);
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
        match (self.strip_has_keys, self.takes_keys(cx)) {
            (true, _) => {}
            (false, true) => window.focus(&self.focus, cx),
            // What the filter found, the keys go on from there, as they do here.
            (false, false) if self.filter_has_keys(window, cx) => self.take_keys(window, cx),
            (false, false) => (self.app).update_later(window, cx, |app, window, cx| {
                app.focus_page_files(window, cx);
            }),
        }
        // Up to a section's heading, what the selection held open stays open, as a tree's
        // folder does over the row it was opened to show.
        if let Selection::Section(project, section) = &selection
            && !self.filtering(cx)
            && self.heading_open(project, section, cx)
            && !self.listed_by_hand(project, section, cx)
        {
            self.set_heading_open(project, section, true, cx);
        }
        self.store.update(cx, |store, cx| {
            store.select_by_hand(selection, cx);
        });
    }

    /// Draw the navigator as a card out over the detail, or beside it again.
    pub(crate) fn set_floating(&mut self, floating: bool, cx: &mut Context<Navigator>) {
        if self.floating != floating {
            self.floating = floating;
            cx.notify();
        }
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

    fn set_drawer_folded(&mut self, folded: bool, cx: &mut Context<Navigator>) {
        if self.store.read(cx).prefs.projects_folded != folded {
            (self.store).update(cx, |store, cx| {
                store.set_prefs(|p| p.projects_folded = folded, cx)
            });
        }
    }

    fn toggle_drawer(&mut self, cx: &mut Context<Navigator>) {
        let folded = self.store.read(cx).prefs.projects_folded;
        self.set_drawer_folded(!folded, cx);
    }

    /// The commits of `project` that can be picked along with commit `id`, as shown: its
    /// stack's, or linked worktree's, not those folded away.
    fn commit_order(&self, project: &ProjectId, id: CommitId, cx: &App) -> Vec<CommitId> {
        let lane = (self.store.read(cx).workspace(project))
            .and_then(|ws| ws.lane_commits(id))
            .unwrap_or_default();
        (self.built.rows.iter())
            .filter_map(|row| match row {
                Row::Item(Selection::Commit(p, commit)) if p == project => Some(*commit),
                _ => None,
            })
            .filter(|commit| lane.contains(commit))
            .collect()
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

    /// Whether one of the commits only `ref_name`'s remote has is selected.
    fn upstream_selected(store: &Store, id: &ProjectId, ref_name: &FullName) -> bool {
        matches!(&store.selection, Some(Selection::Upstream(p, r, _)) if p == id && r == ref_name)
    }

    /// Show the commits remote merge `id` brought in, which load as they open, or fold them,
    /// by hand: it stays as it's left. Folding them over the one selected selects the merge.
    fn toggle_merge(
        &mut self,
        project: &ProjectId,
        ref_name: FullName,
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
            if p == project && store.merged_upstream(p, r, *c).is_some_and(|(m, _)| m == id));
        match inside {
            true => self.select(
                Selection::Upstream(project.clone(), ref_name, id),
                window,
                cx,
            ),
            false => cx.notify(),
        }
    }

    /// Whether the commits only `ref_name`'s remote has are listed under their heading: once
    /// it's opened, and while one is selected.
    fn upstream_open(&self, store: &Store, id: &ProjectId, ref_name: &FullName) -> bool {
        self.open_upstream.contains(&(id.clone(), ref_name.clone()))
            || Self::upstream_selected(store, id, ref_name)
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

    /// What the rows' keys do besides going through them and folding them ([`row_actions`]):
    /// moving and squashing what's selected, and the filter.
    pub(super) fn key_actions<E>(&self, el: E, cx: &mut Context<Self>) -> E
    where
        E: InteractiveElement + gpui::prelude::FluentBuilder,
    {
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
        el.on_action(cx.listener(|this, _: &FocusFilter, window, cx| {
            this.filter
                .update(cx, |input, cx| input.select_all_text(cx));
            window.focus(&this.filter.focus_handle(cx), cx);
            cx.notify();
        }))
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
    }

    /// What the rows are the rows of across the top: the tab the selection's in, as what the
    /// keys do goes no further than the card under it.
    fn top_scope(&self, cx: &App) -> Option<tabs::TabKey> {
        let store = self.store.read(cx);
        if store.prefs.navigator_place != crate::state::NavigatorPlace::Top {
            return None;
        }
        (self.card_key.clone())
            .filter(|_| store.dragging.is_some())
            .or_else(|| NavigatorTabs::active_tab(store))
    }

    /// What the rows are the rows of: the card being drawn, else across the top the tab the
    /// selection's in, else the cards here (`None`).
    fn rows_scope(&self, cx: &App) -> Option<tabs::TabKey> {
        self.scope.clone().or_else(|| self.top_scope(cx))
    }

    fn set_card_key(&mut self, key: Option<tabs::TabKey>) {
        // A card coming shows its folds as they are, not moving from how it last showed them.
        if self.card_key != key {
            self.folds_seen.clear();
        }
        self.card_key = key;
    }

    /// Where the card's first row is drawn, which says so while it draws.
    pub(super) fn card_top(&self) -> Rc<Cell<Option<Pixels>>> {
        self.card_top.clone()
    }

    /// What's dragged over the card under a tab goes by its rows, as it does by the list here.
    pub(super) fn takes_drags<E>(&self, el: E, cx: &mut Context<Self>) -> E
    where
        E: InteractiveElement + StatefulInteractiveElement,
    {
        el.on_drag_move(cx.listener(Self::follow_dragged))
            .on_drop(cx.listener(Self::drop_dragged))
    }

    /// The filter, in the header here and over the rows in the card under a tab. Its clear button
    /// is inside it, so typing the first letter doesn't narrow it.
    pub(super) fn filter_field(
        &self,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let focused = self.filter.focus_handle(cx).is_focused(window);
        let filtering = !self.filter.read(cx).text().is_empty();
        above_drag(
            // Edged as the cards are, but while it has the keys.
            field(&self.filter, focused, theme)
                .when(!focused, |el| el.border_color(theme.border_3)),
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
        })
        .into_any_element()
    }

    /// Whether the filter has the keys, which across the top it has for as long as it shows.
    pub(super) fn filter_has_keys(&self, window: &Window, cx: &App) -> bool {
        self.filter.focus_handle(cx).is_focused(window)
    }

    /// Empty the filter, as it can't be seen once it's left across the top.
    pub(super) fn clear_filter(&mut self, cx: &mut Context<Self>) {
        if self.filtering(cx) {
            self.filter.update(cx, |input, cx| input.set_text("", cx));
            self.filter_changed(cx);
        }
    }

    /// `el` over the rows it's among, those of a higher `rank` over the rest.
    fn over_rows(&self, el: impl IntoElement, rank: usize) -> AnyElement {
        on_top(el).priority(self.layer(rank)).into_any_element()
    }

    /// The layer `rank` draws in over the rows, which in the card under a tab is over that card.
    fn layer(&self, rank: usize) -> usize {
        match self.scope {
            Some(_) => tabs::CARD_ROWS_LAYER + rank,
            None => rank,
        }
    }

    /// Have what takes the keys for the rows take them.
    pub(super) fn take_keys(&self, window: &mut Window, cx: &mut App) {
        let keys = match self.takes_keys(cx) {
            true => self.focus.clone(),
            false => (self.strip_keys.clone()).unwrap_or_else(|| self.focus.clone()),
        };
        window.focus(&keys, cx);
    }

    pub(crate) fn set_strip_keys(&mut self, keys: FocusHandle) {
        self.strip_keys = Some(keys);
    }

    /// Whether something under way needs the rows in sight, which across the top keeps the card
    /// down: a row's name being edited, something being placed or dragged but a project or a
    /// worktree, which go along the tabs, or the filter being typed in.
    pub(super) fn holds_card(&self, window: &Window, cx: &App) -> bool {
        let store = self.store.read(cx);
        self.renaming.is_some()
            || (store.moving.is_some() && tabs::moving_tab(store).is_none())
            || store.dragging.is_some()
            || self.filter_has_keys(window, cx)
    }

    /// Where `item`'s row, in the cards or the drawer, was last drawn while it shows.
    pub(crate) fn row_bounds(&self, item: &Selection) -> Option<Bounds<Pixels>> {
        let at = |rows: &[Row]| {
            rows.iter()
                .position(|row| matches!(row, Row::Item(it) if it == item))
        };
        match (at(&self.built.rows), at(&self.built.drawer.rows)) {
            (Some(ix), _) => crate::ui::shown_row(&self.list, ix),
            (None, Some(ix)) => crate::ui::shown_row(&self.drawer_list, ix),
            (None, None) => None,
        }
    }

    /// Whether `item` has a row in the cards or the drawer.
    fn listed(&self, item: &Selection) -> bool {
        (self.built.rows.iter().chain(&self.built.drawer.rows))
            .any(|row| matches!(row, Row::Item(it) if it == item))
    }

    /// Scroll the first row `wanted` finds into view, in the cards or else the drawer, rows
    /// `row` tall until drawn, saying whether it found one.
    fn scroll_to_row(&self, wanted: impl Fn(&Row) -> bool, row: Pixels) -> bool {
        let at = |rows: &[Row]| rows.iter().position(&wanted);
        match (at(&self.built.rows), at(&self.built.drawer.rows)) {
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
        if self.built.rows.is_empty() {
            return;
        }
        let (spacer, _) = self.footer.get();
        let scale = f32::from(row) / self.row;
        let above = spacer + CARD_MARGIN + DRAWER_CHROME;
        self.reveal_in_footer(fixed(above * scale) + row * ix as f32, row);
    }

    /// Scroll what's `top` down the list's last item, `height` tall, into the column.
    fn reveal_in_footer(&self, top: Pixels, height: Pixels) {
        let footer = self.built.rows.len();
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
        let row = self.row;
        let sized = |keys: &[Option<RowKey>], rows: &[Row]| -> Vec<(Option<RowKey>, f32)> {
            keys.iter()
                .cloned()
                .zip(rows.iter().map(|r| r.height(row)))
                .collect()
        };
        let (old, new) = (
            sized(&self.built.row_keys, &self.built.rows),
            sized(keys, rows),
        );
        let mut offsets = match every {
            true => shifted(&old, &new),
            false => reordered(&old, &new),
        };
        let top = |rows: &[Row], ix: usize| span_height(rows, 0..ix, row);
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
                Some(left @ RowKey::Left(own)) if at(&self.built.row_keys, left).is_none() => {
                    at(&self.built.row_keys, own).map(|was| (left.clone(), was))
                }
                Some(RowKey::Left(own)) if at(keys, own).is_none() => {
                    at(&self.built.row_keys, own).map(|was| ((**own).clone(), was))
                }
                Some(RowKey::Left(_)) | None => None,
                Some(own) => {
                    let left = RowKey::Left(Box::new(own.clone()));
                    let was = at(&self.built.row_keys, own);
                    let from_left = match rows.get(is) {
                        Some(Row::Item(_)) => !landing,
                        // Moved instead of squashed, its copy shows where it'd land at once,
                        // as the squash's outline went, rather than coming out from its row.
                        Some(Row::Ghost(_))
                            if was.is_some_and(|was| {
                                matches!(self.built.rows.get(was), Some(Row::Item(_)))
                            }) =>
                        {
                            offsets.remove(own);
                            false
                        }
                        _ => was.is_none(),
                    };
                    (from_left.then(|| at(&self.built.row_keys, &left)).flatten())
                        .map(|was| (own.clone(), was))
                }
            };
            if let Some((key, was)) = from {
                offsets.insert(key, top(&self.built.rows, was) - top(rows, is));
            }
        }
        if let Some((old_ix, new_ix)) = held.filter(|_| !offsets.is_empty()) {
            let scrolled = top(rows, new_ix) - top(&self.built.rows, old_ix);
            let before: HashSet<&RowKey> = self.built.row_keys.iter().flatten().collect();
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
        (self.list.logical_scroll_top().item_ix..self.built.rows.len())
            .map_while(|ix| Some((ix, self.list.bounds_for_item(ix)?)))
            .find(|(_, bounds)| bounds.contains(&pointer))
            .map(|(ix, bounds)| (ix, bounds.top() - top))
    }

    /// The row at `ix` of the list. The list draws only the rows laid out in view, so while
    /// rows slide, those sliding into view from past its top or bottom are drawn by the first
    /// row it draws and by one near its last ([`Self::slide_edges`]).
    fn render_row(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        if ix == self.built.rows.len() {
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
                true => (ABOVE_LAST_LISTED + 3) as f32 * self.row,
                false => 0.,
            };
        let height = |ix: usize| self.built.rows.get(ix).map_or(0., |r| r.height(self.row));
        let mut far = 0.;
        let rows = match down {
            true => {
                let mut to = ix + 1;
                while to < self.built.rows.len() && far < reach {
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
        // Only the navigator's own rows slide, which across the top are those of the tab the
        // selection's in.
        let sliding =
            self.reorder.any() && (self.scope.is_none() || self.scope == self.top_scope(cx));
        let inset = self.built.insets.get(ix).copied().unwrap_or_default();
        // A row a fold is moving shows as far as the fold's open: inside its card, so the
        // card's edges stay where they are, and taking no clicks while it goes. Under the
        // curtain, it isn't drawn at all.
        let folding = (self.built.moving.get(ix).and_then(Option::as_ref)).and_then(|m| {
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
        let el = match self.built.rows.get(ix) {
            None | Some(Row::Gap) => return div().h(dp(CARD_GAP)).into_any_element(),
            _ if hidden => div().into_any_element(),
            Some(Row::Item(_)) => self.render_item(&self.built.rows, ix, inset, window, theme, cx),
            // A dashed line, as but-ui-react's WorktreeDivider.
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
            Some(Row::Pending(title, slow, _)) => h_flex()
                .min_h(dp(theme.sizes.control))
                .mx(dp(3.))
                .pl(dp(8. + COMMIT_INDENT))
                .pr(dp(12.))
                .gap(dp(10.))
                .when(*slow, |el| el.opacity(0.5))
                .child(small_icon(Icon::Commit).text_color(theme.commit_local))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(title.is_empty(), |el| el.text_color(theme.text_2))
                        .child(commit_title(title)),
                )
                .when(*slow, |el| {
                    el.child(div().text_11().text_color(theme.text_2).child("…"))
                })
                .into_any_element(),
            Some(Row::NewBranch(_)) => self.new_branch_zone(ix, theme),
            Some(Row::Ghost(item)) => {
                // Copies of commits moving together are outlined as one.
                let ghost = |ix: usize| matches!(self.built.rows.get(ix), Some(Row::Ghost(_)));
                let with = (ix > 0 && ghost(ix - 1), ghost(ix + 1));
                let blink = self.blinking(Blink::Move);
                moving::ghost_row(item, inset, with, blink, self.store.read(cx), theme)
            }
            Some(Row::Between(_)) => div().into_any_element(),
            Some(Row::GhostBranch(project, at)) => {
                let mark = theme.hue(moving::mark_of(self.store.read(cx), project));
                moving::ghost_branch_row(at, inset, mark, self.blinking(Blink::Move), theme)
            }
            Some(Row::NewCommit(project)) => {
                let blink = self.blinking(Blink::Move);
                moving::new_commit_row(project, inset, blink, self.store.read(cx), theme)
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
            self.built.rows.get(ix),
            Some(Row::Item(_) | Row::Ghost(..) | Row::GhostBranch(..) | Row::NewCommit(_))
        );
        let el = match inset > 0. && !full_width {
            true => div().pl(dp(inset)).child(el).into_any_element(),
            false => el,
        };
        let (first, last) = card_ends(&self.built.rows, ix);
        // A guide for each row it's under, joining those of the rows above and below into
        // one line, as but.dev's sidebar draws them.
        let guides =
            || (self.built.guides.get(ix).into_iter().flatten()).map(|at| guide(*at, theme));
        // The card dragged by its heading is edged as what's moving is where it would land.
        let held = (self.built.held_card.as_ref()).is_some_and(|card| card.contains(&ix));
        let held_worktree =
            (self.built.held_worktree.as_ref()).is_some_and(|rows| rows.contains(&ix));
        let focused =
            (self.built.focused_card.borrow().as_ref()).is_some_and(|card| card.contains(&ix));
        let edge = match (held, focused) {
            (true, _) => theme.pop,
            (false, true) => theme.focus_edge(),
            (false, false) => theme.border_3,
        };
        // How far row `ix` is drawn from where it's laid out: as far as a reorder slid it, but
        // not while a fold moves it, which draws it as far as the fold's open.
        let drawn_at = |ix: usize| match self.built.moving.get(ix).and_then(Option::as_ref) {
            Some(m) if self.folds.openness(&m.fold).is_some() => 0.,
            _ => (self.built.row_keys.get(ix).and_then(Option::as_ref))
                .map_or(0., |key| self.reorder.offset(key)),
        };
        // How far the card after row `ix`, its card's last, is drawn from where it's laid out.
        let next_card_at = |ix: usize| match self.built.rows.get(ix + 1) {
            Some(Row::Gap) => Some(drawn_at(ix + 2)),
            _ => None,
        };
        let crossing_at = |ix: usize| {
            self.crossing.is_some()
                && self.built.row_keys.get(ix).and_then(Option::as_ref) == self.crossing.as_ref()
        };
        // A terminal crossing to another card leaves its slice where it's laid out, the card it's
        // going to closing around that, and floats over the cards. Last in its card, the slice
        // carries the card's bottom edge, so it keeps its distance from the card after it.
        let slid_at = |ix: usize| match crossing_at(ix) {
            true => next_card_at(ix).unwrap_or_default(),
            false => drawn_at(ix),
        };
        // A row slides with its slice of the card, so the card's edges follow its rows. Its
        // slice also stretches over the room between it and where it's laid out, as far as its
        // card goes, and down to the next row of its card where that's drawn further down, so
        // the card holds together as they slide, even where the list doesn't draw the row that
        // would cover it, past its top or bottom, and never covers the card beside it as cards
        // trade places. While rows slide, what's in every row is drawn over all that, so no
        // stretch covers it, on the card's ground so the rows it passes don't show through,
        // the copies of commits being moved over the rest, inside the list. A card's last row
        // carries its bottom edge, so as rows of the card trade places, the edge stays where the
        // card ends, on a stretch of its own.
        let slid = slid_at(ix);
        let crossing = crossing_at(ix);
        let below = match last {
            // The room a terminal leaves closes as the card after it, or the list's end, comes.
            true if self.crossing.is_some() => {
                (next_card_at(ix).unwrap_or_default() - slid).max(0.)
            }
            true if sliding => {
                let card = (0..ix)
                    .rev()
                    .map_while(|above| match self.built.rows.get(above) {
                        Some(Row::Gap) | None => None,
                        Some(_) => Some(slid_at(above)),
                    });
                past_last_row(card, slid)
            }
            true => 0.,
            false => {
                let room = card_beyond(&self.built.rows, ix, true, -slid, self.row);
                (slid_at(ix + 1) - slid).max(room).max(0.)
            }
        };
        let above = match first {
            true => 0.,
            false => card_beyond(&self.built.rows, ix, false, slid, self.row).max(0.),
        };
        // What's in the card being dragged goes over everything, as it goes over the cards it
        // passes (see below), and so do the rows of a linked worktree being moved, over those of
        // the others it passes, and a terminal being moved, selected or not; the copies of
        // what's moving over the rest, and the selected row, which moves, over the rows it
        // passes. The new branch the copies would land in shows from under what slides off it,
        // rather than over what's about to.
        let store = self.store.read(cx);
        let selection = store.selection.as_ref();
        let over_rank = match self.built.rows.get(ix) {
            _ if held || held_worktree || crossing => 3,
            Some(Row::Item(item)) if matches!(moving::mark(store, item), Some(Mark::Held(_))) => 3,
            Some(Row::Ghost(..) | Row::NewCommit(_)) => 2,
            Some(Row::Item(item)) if selection == Some(item) => 1,
            _ => 0,
        };
        // What a move just took away from under this row stays there, moving with it, while
        // the rows after it close over it: only while rows slide, as only then do they draw
        // over it.
        let key = self.built.row_keys.get(ix).and_then(Option::as_ref);
        let stand_ins = (self.stand_ins.iter())
            .filter(|stand_in| sliding && key == Some(&stand_in.under))
            .map(|stand_in| {
                let margins = match self.scope {
                    Some(_) => (0., 0.),
                    None => self.card_margins(),
                };
                moving::stand_in(stand_in, margins, self.layer(0), self.store.read(cx), theme)
            });
        // What's being moved has its copy drawn over it while the move is at where it is, over
        // everything, as the copies elsewhere are, sliding out to them and back by its key, as
        // the copy keyed so would. Copies of commits moving together are outlined as one.
        let copy = match (self.built.rows.get(ix), key) {
            (Some(Row::Item(item)), Some(RowKey::Left(own))) => {
                let store = self.store.read(cx);
                let copied =
                    |item: &Selection| matches!(moving::mark(store, item), Some(Mark::Copied(_)));
                let copied_at = |ix: usize| matches!(self.built.rows.get(ix), Some(Row::Item(other)) if copied(other));
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
                    self.over_rows(copy, 2)
                })
            }
            _ => None,
        };
        // Under a tab, the card they're in has edges of its own.
        let edged = self.scope.is_none();
        let card = div()
            .relative()
            .bg(theme.bg_1)
            .border_color(edge)
            .when(edged, |el| el.border_l_1().border_r_1())
            .when(first && edged, |el| {
                el.border_t_1()
                    .rounded_t(dp(theme.shape.card))
                    .pt(dp(CARD_PADDING))
            })
            .when(last && edged, |el| {
                el.border_b_1()
                    .rounded_b(dp(theme.shape.card))
                    .pb(dp(CARD_PADDING))
            })
            .map(|card| match folding {
                Some((height, closing)) => card.child(clipped(
                    div().relative().child(el).children(guides()),
                    dp(height),
                    closing,
                )),
                None if sliding => {
                    // Rounded at a card's ends, inside its corners, which square it'd cover a
                    // sliver of until the rows stop.
                    let corner = match edged {
                        true => 2.,
                        false => theme.shape.popup - CARD_PADDING,
                    };
                    let over = (div().relative().bg(theme.bg_1).child(el))
                        .when(first, |el| el.rounded_t(dp(corner)))
                        .when(last, |el| el.rounded_b(dp(corner)))
                        .when(crossing, |el| el.top(dp(drawn_at(ix) - slid)));
                    let over = self.over_rows(over.children(guides()), over_rank);
                    card.children(guides()).child(over)
                }
                None => card.child(el).children(guides()),
            })
            .children(copy);
        let (left, right) = match edged {
            true => self.card_margins(),
            false => (0., 0.),
        };
        let stretched = |height: f32| {
            div()
                .absolute()
                .left(dp(left))
                .right(dp(right))
                .h(dp(height))
                .bg(theme.bg_1)
                .border_color(edge)
                .when(edged, |el| el.border_l_1().border_r_1())
                .children(guides())
        };
        let below = (below > 0.).then(|| match last {
            // Over the last row's own edge and corners, which are no longer the card's, and tall
            // enough for its own corners to stay as round as it comes to nothing.
            true => stretched(below + 2. * theme.shape.card)
                .bottom(dp(-below))
                .when(edged, |el| el.border_b_1().rounded_b(dp(theme.shape.card))),
            false => stretched(below).top(relative(1.)),
        });
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
        let row = match held && sliding {
            true => self.over_rows(row, 2),
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
        let shown = (self.scoped_selection.clone()).unwrap_or_else(|| store.selection.clone());
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
        let taken_by = |item: &Selection| {
            matches!(
                moving::mark(store, item),
                Some(Mark::Taken(_) | Mark::Copied(_))
            )
        };
        let taken = taken_by(item);
        // What a squash goes into, or the branch dragged files start a commit in, is outlined
        // over it in the colour of what's done, or where it started around the rows held there,
        // the outline sliding in from where it was before, its edges apart.
        let slid = (
            self.reorder.offset(&RowKey::SquashTop),
            self.reorder.offset(&RowKey::SquashBottom),
        );
        let outlined = (self.squash_outline.as_ref()).filter(|outline| outline.row == *item);
        let held = matches!(marked, Some(Mark::Held(_)));
        // Moving together, next to each other, they're outlined as one.
        let outlined_at = |ix: usize| match rows.get(ix) {
            Some(Row::Item(other)) if taken => taken_by(other),
            Some(Row::Item(other)) if held => {
                matches!(moving::mark(store, other), Some(Mark::Held(_)))
            }
            _ => false,
        };
        let outlined_with = (ix > 0 && outlined_at(ix - 1), outlined_at(ix + 1));
        // Not while something's dragged, or the selection outshines what shows where it lands.
        let solid = selected
            && store.dragging.is_none()
            && !self.on_drawer_bar
            && self.rows_have_keys(window, cx);
        let style = RowStyle {
            theme,
            selected,
            solid,
            taken,
            held,
            // In the colour of what's being done with it, as the TUI's modes have their own.
            mark: match marked {
                Some(Mark::Taken(op) | Mark::Copied(op) | Mark::Held(op)) => op.hue(),
                None => Hue::Pop,
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
            Selection::Pending(id, op, _) => self.pending_row(item, id, *op, style, cx),
            Selection::Terminal(_, id) => self.terminal_row(item, *id, style, cx),
            Selection::Stack(id, bottom) => {
                self.stack_row(ix, item, id, bottom.as_ref(), style, cx)
            }
            Selection::Mesh(..) => self.mesh_row(ix, item, style, cx),
            Selection::Upstream(id, ref_name, commit) => {
                self.upstream_row(item, id, ref_name, commit, style, cx)
            }
            Selection::Section(id, Section::Upstream(ref_name)) => {
                self.upstream_heading(ix, id, ref_name, style, cx)
            }
            Selection::Section(id, Section::Inactive) => {
                Some(self.inactive_heading(ix, id, style, cx))
            }
            Selection::Section(id, Section::Mesh) => Some(self.mesh_heading(ix, id, style, cx)),
        };
        let Some(el) = el else {
            return div().into_any_element();
        };
        let el = self.with_tiles(el, item, style, cx);
        let el = self.row_menu(el, item.clone(), cx);
        match outlined {
            Some(outline) => {
                let blink = self.blinking(Blink::Move);
                let hue = moving::mark_of(store, item.project());
                // Around the stack a dragged branch goes in, it says it makes one or joins it.
                let label = (moving::stack_label(store, item))
                    .map(|label| moving::outline_label(label, self.row, hue, theme));
                let outline = moving::squash_outline(outline.edges(slid), blink, hue, theme);
                let outline = self.over_rows(outline.children(label), 2);
                div().relative().child(el).child(outline).into_any_element()
            }
            None => el,
        }
    }

    /// Row `el` of `item`, beside a small map of its tiles while it has others than its page,
    /// which is the brightest: shaded with the row while it's selected, solid as it is, and
    /// naming them in its tooltip. A click on it is one on the row. Before the map goes the
    /// status of the terminals in the row's tilings, which a terminal's row shows itself.
    fn with_tiles(
        &self,
        el: AnyElement,
        item: &Selection,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (theme, selected, solid) = (style.theme, style.selected, style.solid);
        let store = self.store.read(cx);
        let tiles = store.tiles_of(item);
        let status = match item {
            Selection::Terminal(..) => None,
            _ => store.row_status(item),
        };
        let mark = status.and_then(|status| status_mark(status, !store.hidden(), style));
        let mapped = tiles.len() >= 2;
        if !mapped && mark.is_none() {
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
                .bg(ink)
                .when(matches!(tile, Tile::Pinned(_)), |el| el.opacity(0.4))
        });
        let names = (tiles.iter()).filter_map(|(tile, _)| match tile {
            Tile::Home => None,
            Tile::Pinned(pinned) => Some(crate::app::tile_label(store, pinned).1),
        });
        let names = names.collect::<Vec<_>>().join("\n");
        let map = h_flex()
            .id(SharedString::from(format!("tiles-{item:?}")))
            .flex_none()
            .mr(dp(3.))
            .px(dp(7.))
            .gap(dp(6.))
            .rounded(dp(theme.shape.control))
            .map(|el| match (selected, solid) {
                (true, true) => el.bg(theme.fill_gray),
                (true, false) => el.bg(theme.selected_unfocused),
                (false, _) => el
                    .bg(theme.hover)
                    .hover(|el| el.bg(theme.selected_unfocused)),
            })
            .children(mark)
            .when(mapped, |el| {
                el.child(
                    div()
                        .relative()
                        .w(dp(TILE_MAP.0))
                        .h(dp(TILE_MAP.1))
                        .children(cells),
                )
                .tooltip(Tooltip::text(names))
            })
            .map(|el| self.selectable(el, item, cx))
            .map(|el| self.page_drag(el, item, cx));
        // As tall as the row, which a watched project's heading is more than the others.
        h_flex()
            .w_full()
            .items_stretch()
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
                window.focus(self.pressed_keys(), cx);
            }
            Selection::Commit(project, id) if self.store.read(cx).among_picks(project, *id) => {
                window.focus(self.pressed_keys(), cx)
            }
            _ => self.select(item.clone(), window, cx),
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
            .rounded(dp(theme.shape.card))
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
        let full = match self.built.drawer.rows.len() {
            0 => 0.,
            n => n as f32 * self.row + DRAWER_PADDING,
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
    /// is. Gives how tall the list is in this frame.
    fn fit_footer(&self, window: &mut Window, cx: &mut Context<Self>) -> f32 {
        let scale = Theme::get(cx).scale;
        let view = self.list.viewport_bounds();
        // The list is as tall as it was last laid out, give or take as much as the navigator
        // has grown since, as the overview under it slides.
        let drawn = self.drawn_height.get();
        let grown = drawn - self.listed_height.replace(drawn);
        let column = f32::from(view.size.height + grown) / scale;
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
                let above = (self.list.bounds_for_item(self.built.rows.len()))
                    .map_or(past, |footer| footer.top() - view.top());
                self.list.scroll_by(past.min(above).max(fixed(0.)));
            }
            self.footer.set((spacer, height));
            let at = self.built.rows.len();
            self.list.remeasure_items(at..at + 1);
            // After this draw, as a notify during it draws nothing.
            cx.defer_in(window, |_, _, cx| cx.notify());
        }
        column
    }

    /// Whether the navigator has the keys, but not while typing in the filter, where they go
    /// to it.
    fn rows_have_keys(&self, window: &Window, cx: &App) -> bool {
        if self.scope.is_some() {
            // A card on its way out has left them behind.
            return self.strip_has_keys && self.scoped_selection.is_none();
        }
        keys_at(window, cx).is_some_and(|keys| {
            self.focus.contains(&keys, window) && keys != self.filter.focus_handle(cx)
        })
    }

    /// Note the card the selection's in while the navigator has the keys, but in its filter,
    /// for its rows to edge it as the tile with the keys is.
    fn find_focused_card(&self, window: &Window, cx: &App) {
        let store = self.store.read(cx);
        let keys = store.prefs.shows(Part::FocusOutline)
            && !self.on_drawer_bar
            && self.rows_have_keys(window, cx);
        let selection = store.selection.as_ref();
        let at = selection.filter(|_| keys).and_then(|selection| {
            (self.built.rows.iter())
                .position(|row| matches!(row, Row::Item(item) if item == selection))
        });
        let card = at.map(|at| {
            let gap = |ix: &usize| matches!(self.built.rows.get(*ix), Some(Row::Gap));
            let start = (0..at).rev().find(gap).map_or(0, |ix| ix + 1);
            let end = (at..self.built.rows.len())
                .find(gap)
                .unwrap_or(self.built.rows.len());
            start..end
        });
        *self.built.focused_card.borrow_mut() = card;
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
        self.built.rows.iter().enumerate().map(|(ix, row)| {
            let moving = self.built.moving.get(ix).and_then(Option::as_ref);
            let open = moving.and_then(|m| Some((m, self.folds.openness(&m.fold)?)));
            let height = match open {
                Some((m, open)) => curtain(open, m.extent, m.top, m.height),
                None => row.height(self.row),
            };
            (row, height)
        })
    }

    /// Where the list's top and bottom edges cut a card off as it's scrolled in this frame,
    /// the drawer's too: there the card's edge is drawn along the list's, rounded as its own,
    /// so it keeps its shape. GPUI cuts what the list draws to a rectangle, so the ground the
    /// cards are on is painted past the corners. The list is `height` tall in this frame, as
    /// [`Self::fit_footer`] works out.
    fn card_caps(&self, height: f32, theme: &Theme, cx: &App) -> Vec<Div> {
        let scale = Theme::get(cx).scale;
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
        let focused = (self.built.focused_card.borrow().as_ref()).map(|rows| {
            (self.built.rows.iter().take(rows.start))
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
            let sliver = shows < 2. * theme.shape.card;
            let radius = if sliver { shows / 2. } else { theme.shape.card };
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
                (false, true) => (
                    edge.border_t_1().rounded_t(dp(theme.shape.card)),
                    shows - theme.shape.card,
                ),
                (false, false) => (
                    edge.border_b_1().rounded_b(dp(theme.shape.card)),
                    shows - theme.shape.card,
                ),
            };
            let edge = edge.when(!sliver, |el| el.h(dp(2. * theme.shape.card)));
            let band = div()
                .absolute()
                .left_0()
                .right_0()
                .h(dp(shown.min(2. * theme.shape.card)))
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
                        let wide = theme.scaled(theme.shape.card);
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
        let Drawer { listed, total, .. } = self.built.drawer;
        let title = drawer_title(alone);
        let count = match listed == total {
            true => total.to_string(),
            false => format!("{listed} of {total}"),
        };
        let toggles = self.drawer_toggles(cx);
        let selected = self.on_drawer_bar;
        let solid = selected && self.rows_have_keys(window, cx);
        let open = toggles.then(|| !self.store.read(cx).prefs.projects_folded);
        let count = (total > 0).then_some(count);
        card_bar("drawer-bar", title, count, open, selected, solid, theme)
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
                el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_drawer(cx)))
            })
    }

    /// Row `ix` of the drawer, set in as a card's rows are.
    fn render_drawer_row(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let el = match self.built.drawer.rows.get(ix) {
            Some(Row::Note(text, is_error)) => note_row(text, *is_error, theme),
            _ => self.render_item(&self.built.drawer.rows, ix, 0., window, theme, cx),
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
        // Folded, the row sums up the whole card; open, it leaves out what the rows under it
        // say: their uncommitted files and unpushed commits. How far behind the target it is
        // shows either way, even while a commit is being edited.
        let summary = workspace.filter(|ws| ws.editing.is_none()).map(|ws| {
            let folded = |n: usize| if expanded && !card_folded { 0 } else { n };
            let shown = |checkout, files| store.uncommitted_shown(id, &checkout, files).len();
            let in_worktrees = (ws.worktrees.iter())
                .map(|w| shown(Checkout::Linked(w.name.clone()), &w.uncommitted));
            (
                folded(shown(Checkout::Main, &ws.uncommitted) + in_worktrees.sum::<usize>()),
                folded(ws.unpushed_commits()),
            )
        });
        let target = workspace.and_then(|ws| ws.target.as_ref());
        let unread = workspace.is_some_and(|ws| {
            ws.commits().any(|commit| store.is_unread(id, commit))
                || ws
                    .remote_commits()
                    .any(|commit| store.is_unread(id, commit))
        });
        let needs_update = target.is_some_and(|t| t.needs_update());
        // Every project wears one, the glyph's outline until one's picked, so their names line
        // up, over the icons of the rows under it. In an icon's 16 points, an emoji a little
        // smaller than the text. A watched project's card folds from it, still watched.
        let glyph = div()
            .flex_none()
            .w(dp(16.))
            .flex()
            .justify_center()
            .text_13()
            .child(project_glyph(store, id, style.muted(), theme));
        let glyph = self.fold(
            ElementId::Name(format!("fold-project-{id}").into()),
            glyph,
            expanded.then_some(!card_folded),
            style.selected,
            style.quiet(),
            cx,
            {
                let id = id.clone();
                move |this, _, cx| this.toggle_project(&id, cx)
            },
        );
        // Whether the app watches the project, as a watched one is loaded and kept current,
        // and under the pointer what a click makes it. Named by what it does, so its tooltip
        // goes with the click. Its click doesn't select the row, and while something is being
        // moved it's only part of the row, which a press puts the move at.
        let moving = store.moving.is_some();
        let eye = div()
            .id(ElementId::Name(
                format!("{}-{id}", if expanded { "unwatch" } else { "watch" }).into(),
            ))
            .flex_none()
            .size(dp(20.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(dp(theme.shape.control))
            .child(
                watch_glyph(expanded != hovered).text_color(match (hovered, style.solid) {
                    (false, _) => style.muted(),
                    (true, true) => theme.text_inverted,
                    (true, false) => theme.text_1,
                }),
            )
            .when(!moving, |el| {
                let id = id.clone();
                el.hover(|el| el.bg(theme.hover))
                    // A button, which leaves the keys where they were.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        // The row moves to the other card, from under the pointer.
                        this.watch_hovered = None;
                        this.store.update(cx, |store, cx| {
                            let expanded = store.project(&id).is_some_and(|p| p.expanded);
                            store.set_expanded(&id, !expanded, cx)
                        });
                    }))
            })
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
            }));
        Some(
            style
                .row(format!("project-{id}"), 6.)
                .aria_label(entry.project.name.clone())
                .aria_expanded(expanded)
                .h(dp(if expanded {
                    HEADING_HEIGHT
                } else {
                    theme.sizes.control
                }))
                .when(unread, |el| el.relative().child(style.unread_line(false)))
                .child(glyph)
                // However narrow the navigator, the name stays: what the row sums up gives way
                // first, a whole item at a time from its end. Each item that doesn't fit wraps to
                // a second line, which is cut off.
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
                        .when_some(summary, |el, (files, unpushed)| {
                            el.children(style.files(files))
                                .children(style.unpushed(unpushed))
                        })
                        .children(target.and_then(|t| {
                            let behind = plural(t.behind, "commit");
                            let tip = format!("{behind} behind {}{}", t.name, with_merged(t));
                            meta_count(Icon::ArrowDown, t.behind, style.meta(theme.warn), tip)
                        }))
                        .when(entry.is_loading(), |el| {
                            el.child(small_icon(Icon::Clock).text_color(theme.text_3))
                        }),
                )
                // Only a watched project's: an unwatched one updates from its page or its menu,
                // which keeps the list of the others quiet.
                .when(expanded && needs_update, |el| {
                    el.child(self.target_update_button(id, style.solid, cx))
                })
                .child(eye)
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
        let status = store.row_status(item);
        let field = self.rename_field(item, style.theme, cx);
        let renaming = field.is_some();
        let row = style
            .row(format!("terminal-{}", id.0), 6.)
            .child(icon(Icon::TextCode).text_color(style.muted()))
            .child(match field {
                Some(field) => field,
                None => (div().flex_1().min_w_0().truncate().child(title)).into_any_element(),
            })
            .children(status.and_then(|status| status_mark(status, !store.hidden(), style)))
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
            format!(" in {}", checkout.linked().unwrap_or(MAIN_WORKTREE.into()))
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
                MAIN_WORKTREE.to_owned(),
                &entry.project.path,
                &workspace.uncommitted,
            ),
        };
        // The checkout a client `cargo run` started was built from.
        let running = cx
            .try_global::<Rebuild>()
            .is_some_and(|r| r.checkout() == path);
        let uncommitted = store.uncommitted_shown(id, checkout, uncommitted);
        let branches: Vec<_> = workspace.branches_in(checkout).collect();
        let unpushed_n = unpushed(branches.iter().copied());
        let unread = (branches.iter()).any(|branch| store.branch_unread(id, branch));
        // Open even when folded while the selection's in it.
        let open = self.section_open(ix);
        let folds = checkout_holds(store, workspace, id, checkout);
        let toggle = {
            let (project, checkout) = (id.clone(), checkout.clone());
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_section(&project, checkout.clone(), open, window, cx)
            }
        };
        let glyph = self.fold(
            ("worktree-fold", ix),
            worktree_icon(checkout.is_linked(), style),
            folds.then_some(open),
            style.selected,
            style.quiet(),
            cx,
            toggle.clone(),
        );
        let row = style
            .row(
                format!("worktree-{id}-{}", checkout.linked().unwrap_or_default()),
                6.,
            )
            .aria_label(label.clone())
            .when(folds, |el| el.aria_expanded(open))
            .when(!open && unread, |el| {
                el.relative().child(style.unread_line(false))
            })
            .child(glyph)
            .child(
                div()
                    .id(("worktree-name", ix))
                    .flex_grow(1.)
                    .min_w_0()
                    .truncate()
                    .child(label.clone()),
            )
            .when(!open, |el| {
                el.children(change_counts(tally(&uncommitted), style))
                    .children(style.unpushed(unpushed_n))
            })
            .when(running, |el| el.child(running_mark(ix, style)))
            .map(|el| self.selectable(el, item, cx))
            .on_click(double_click(toggle, cx));
        // A linked worktree drags itself, its changes going with it.
        let row = match checkout.linked() {
            Some(name) => self.worktree_drag(row, item, name, name.to_string().into(), cx),
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
            let changes = Changes {
                from: None,
                worktree: checkout,
                files: files.into(),
            };
            (Dragged::from((id.clone(), changes)), label)
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

    /// Fold `checkout`'s branches, or unfold them, as [`Self::fold_heading`] does.
    fn toggle_section(
        &mut self,
        project: &ProjectId,
        checkout: Checkout,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let workspace = store.workspace(project);
        if open && !workspace.is_some_and(|ws| checkout_holds(store, ws, project, &checkout)) {
            return;
        }
        let inside = (store.selection.as_ref().zip(workspace))
            .is_some_and(|(s, ws)| s.project() == project && in_checkout(store, s, ws, &checkout));
        let heading = Selection::Checkout(project.clone(), checkout);
        self.fold_heading(heading, open, inside, window, cx)
    }

    /// Fold the rows under `heading` away, `open` as they are, or unfold them. Folding them
    /// over the selection, `inside` them, takes it up to the heading, as a tree does.
    fn fold_heading(
        &mut self,
        heading: Selection,
        open: bool,
        inside: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !open {
            self.folded.remove(&heading);
            return cx.notify();
        }
        self.folded.insert(heading.clone());
        match inside {
            true => self.select(heading, window, cx),
            false => cx.notify(),
        }
    }

    /// Whether row `ix` heads rows stepped in under it that show: an open worktree or stack.
    fn section_open(&self, ix: usize) -> bool {
        let next = self.built.insets.get(ix + 1);
        (next.zip(self.built.insets.get(ix))).is_some_and(|(next, this)| next > this)
    }

    /// Over a stack of two or more branches, as ui-react's StackCaption: its glyph, which folds
    /// them away as a worktree's heading does, how many, and "Stack". Selecting it shows what
    /// its branches change together. Folded, it names its top branch, the one that gets pushed,
    /// and says how many commits they have to push and whether any is unread.
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
        let branches = store.after_deletes(id, &store.lane_for(item)?.0.branches);
        // Open even when folded while the selection's in it.
        let open = self.section_open(ix);
        let unread = (branches.iter()).any(|branch| store.branch_unread(id, branch));
        let toggle = {
            let caption = item.clone();
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_stack(caption.clone(), open, window, cx)
            }
        };
        // Quiet, so it doesn't read as one more branch, with "Stack" a step above the rest.
        let (word, quiet) = match style.solid {
            true => (theme.text_inverted, style.muted()),
            false => (theme.text_2, theme.text_3),
        };
        let glyph = self.fold(
            ("stack-fold", ix),
            icon(Icon::Stack).text_color(quiet),
            Some(open),
            style.selected,
            style.quiet(),
            cx,
            toggle.clone(),
        );
        let label = h_flex()
            .id(("stack-name", ix))
            .flex_grow(1.)
            .min_w_0()
            .gap(dp(6.))
            .child(count_badge(branches.len(), theme).when(style.solid, |el| {
                el.bg(theme.inverse_soft).text_color(quiet)
            }))
            .child(div().flex_none().text_color(word).child("Stack"))
            .children(branches.first().filter(|_| !open).map(|top| {
                (div().min_w_0().truncate())
                    .text_color(quiet)
                    .child(store.label_of(id, top).to_owned())
            }));
        let bottom_name = bottom.map(ToString::to_string).unwrap_or_default();
        let row = style
            .row(format!("stack-{id}-{bottom_name}"), 6.)
            .aria_label(format!("Stack of {}", branches.len()))
            .aria_expanded(open)
            .when(!open && unread, |el| {
                el.relative().child(style.unread_line(false))
            })
            .child(glyph)
            .child(label)
            .when(!open, |el| {
                el.children(style.unpushed(unpushed(branches.iter())))
            })
            .map(|el| self.selectable(el, item, cx))
            .on_click(double_click(toggle, cx));
        Some(row.into_any_element())
    }

    /// Fold the stack under `caption` away, or unfold it, as [`Self::fold_heading`] does.
    fn toggle_stack(
        &mut self,
        caption: Selection,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let inside = (store.selection.as_ref().zip(store.lane_for(&caption))).is_some_and(
            |(s, (lane, _))| {
                s.project() == caption.project() && lane.branches.iter().any(|b| s.on(b))
            },
        );
        self.fold_heading(caption, open, inside, window, cx)
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
        let key = (id.clone(), ref_name.cloned());
        let clicked = ClickOpened::Here(key.clone());
        let closed = !self.is_open(&clicked) && !branch.commits.is_empty();
        // What's selected under it keeps it open.
        let holds_selected =
            (store.selection.as_ref()).is_some_and(|s| s.project() == id && s.under(branch));
        let open = !closed || holds_selected;
        // Its icon turns into the chevron that folds the commits away.
        let toggle = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            this.set_open(ClickOpened::Here(key.clone()), !open, cx);
            // Folding it over its selected commit takes the selection up to it, as ⇧← does.
            if open && holds_selected {
                let (project, ref_name) = key.clone();
                this.select(Selection::Branch(project, ref_name), window, cx)
            }
        };
        let folds = !branch.commits.is_empty();
        let glyph = match icon(glyph).text_color(glyph_color) {
            glyph if publishing => pulsing(glyph).into_any_element(),
            glyph => glyph.into_any_element(),
        };
        let glyph = self.fold(
            ElementId::Name(format!("fold-{id}-{row_id}").into()),
            glyph,
            folds.then_some(open),
            style.selected,
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
                .child(glyph)
                .child(match rename {
                    Some(field) => field,
                    None => branch_name(name, review, &style)
                        .id(ElementId::Name(format!("name-{id}-{row_id}").into()))
                        .flex_1()
                        .into_any_element(),
                })
                // CI only while it runs or after it failed: passing is the usual state. A mark,
                // not a button: the branch's page shows the checks.
                .when_some(
                    checks.filter(|c| c.running > 0 || c.failed > 0),
                    |el, checks| {
                        let (glyph, color, text) = review::checks_summary(checks, theme);
                        let color = style.meta(color);
                        let tip = SharedString::from(text);
                        el.child(
                            div()
                                .id(ElementId::Name(format!("checks-{row_id}").into()))
                                .flex_none()
                                .child(small_icon(glyph).text_color(color))
                                .aria_label(tip.clone())
                                .tooltip(Tooltip::text(tip)),
                        )
                    },
                )
                // That someone wrote a brief for it, quietly; its page has it to read.
                .when(
                    store.briefs
                        && store
                            .brief_at(id, &branch.name)
                            .is_some_and(|known| known.found.is_some()),
                    |el| {
                        let color = style.meta(style.muted());
                        el.child(
                            div()
                                .id(ElementId::Name(format!("brief-{row_id}").into()))
                                .flex_none()
                                .child(small_icon(Icon::Docs).text_color(color))
                                .aria_label("Has a brief")
                                .tooltip(Tooltip::text("Has a brief")),
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
                // Folded, who and how many only its remote has; open, its New on origin row says.
                .when(!open, |el| {
                    let n = branch.incoming.len();
                    let remote = branch.remote.as_deref().unwrap_or("the remote");
                    let tip = format!("{} only on {remote}", plural(n, "commit"));
                    let people = branch
                        .incoming
                        .iter()
                        .map(|c| (c.author.as_str(), c.email.as_str()));
                    el.children(authors_stack(people, theme))
                        .children(self.update_step(id, ref_name, n, tip, style, cx))
                })
                .children(self.push_step(id, ref_name, unpushed_n, style, cx))
                // Its step, after the counts: the first conflict to resolve, unless a linked
                // worktree resolves it. Conflicts block Push, so it's the only one.
                .children(conflict.map(|commit| {
                    let project = id.clone();
                    let resolve = button(
                        ElementId::Name(format!("resolve-{id}-{row_id}").into()),
                        "Resolve",
                    )
                    .small()
                    .inverted(style.solid)
                    .tooltip("Resolve the first conflicted commit");
                    self.row_step(resolve, cx, move |this, window, cx| {
                        let project = project.clone();
                        this.app.update_later(window, cx, move |app, window, cx| {
                            app.resolve_commit(project, commit, window, cx)
                        })
                    })
                }))
                .map(|el| self.selectable(el, item, cx))
                // A click looks inside, opening its commits; the chevron, a double-click, ⇧←
                // and ⇧→ fold them.
                .when(folds, |el| el.on_click(click_opens(clicked, toggle, cx)))
                .map(|el| match ref_name.filter(|_| self.drags(cx)) {
                    Some(ref_name) => {
                        let payload = Dragged {
                            project: id.clone(),
                            what: crate::transfer::Moving::Branch(ref_name.clone()),
                        };
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

    /// Push, in place of the ↑ count of the branch's `n` unpushed commits: the number and its
    /// glyph in a blue button, which pushes them without selecting the branch and pulses while
    /// they go. While they can't go, as with conflicts, it's the plain count, saying why.
    fn push_step(
        &self,
        project: &ProjectId,
        ref_name: Option<&FullName>,
        n: usize,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let ws = store.workspace(project)?;
        if let Some(why) = ws.push_blocked(ref_name) {
            let color = style.meta(style.theme.blue);
            return meta_count(Icon::ArrowUp, n, color, why.to_owned())
                .map(IntoElement::into_any_element);
        }
        let ref_name = ref_name?;
        if n == 0 {
            return None;
        }
        let pushing = store.pushing(project, ref_name);
        let push = button(
            ElementId::Name(format!("push-{project}-{}", ref_name.as_bstr()).into()),
            n.to_string(),
        )
        .icon_end(Icon::ArrowUp)
        .style(ButtonStyle::Info)
        .small()
        .inverted(style.solid)
        .tooltip(ws.push_tooltip(ref_name))
        .disabled_because(pushing.then_some("Pushing"));
        let pushed = (project.clone(), ref_name.clone());
        let step = self.row_step(push, cx, move |this, window, cx| {
            let (project, ref_name) = pushed.clone();
            this.app.update_later(window, cx, move |app, window, cx| {
                app.push_branch(project, Some(ref_name), window, cx)
            });
        });
        Some(match pushing {
            true => pulsing(step).into_any_element(),
            false => step.into_any_element(),
        })
    }

    /// Update, in place of the ↓ count of the `n` commits only `ref_name`'s remote has: the
    /// number and its glyph in an amber button, which brings them in and leaves the selection
    /// where it is. When they can't come in, it's the plain count, saying `tip`.
    fn update_step(
        &self,
        project: &ProjectId,
        ref_name: Option<&FullName>,
        n: usize,
        tip: String,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let warn = style.meta(style.theme.warn);
        let ws = self.store.read(cx).workspace(project);
        let Some(ref_name) = ref_name.filter(|r| ws.is_some_and(|ws| ws.can_pull(r))) else {
            return meta_count(Icon::ArrowDown, n, warn, tip).map(IntoElement::into_any_element);
        };
        if n == 0 {
            return None;
        }
        let remote = (ws.and_then(|ws| ws.find_branch(Some(ref_name))))
            .and_then(|branch| branch.remote.clone())
            .unwrap_or_else(|| "the remote".to_owned());
        let update = button(
            ElementId::Name(format!("update-{project}-{}", ref_name.as_bstr()).into()),
            n.to_string(),
        )
        .icon_end(Icon::ArrowDown)
        .style(ButtonStyle::Warning)
        .small()
        .inverted(style.solid)
        .tooltip(format!("Update with {} from {remote}", plural(n, "commit")));
        let (project, ref_name) = (project.clone(), ref_name.clone());
        let step = self.row_step(update, cx, move |this, _, cx| {
            let ref_name = ref_name.clone();
            (this.store).update(cx, |store, cx| {
                store.pull_remote(&project, ref_name, None, cx)
            })
        });
        Some(step.into_any_element())
    }

    /// `step` as a row's next step: pressing it does `press` and leaves its row as it was,
    /// unselected and unfolded. While something is being moved it's only part of its row, which
    /// a press puts the move at, so it does nothing of its own then.
    fn row_step(
        &self,
        step: Button,
        cx: &Context<Self>,
        press: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Div {
        let moving = self.store.read(cx).moving.is_some();
        div()
            .flex_none()
            .child(match moving {
                true => step,
                false => {
                    step.on_click(cx.listener(move |this, _, window, cx| press(this, window, cx)))
                }
            })
            .when(!moving, |el| {
                el.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            })
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
        let payload = Dragged {
            project: id.clone(),
            what: crate::transfer::Moving::Commits(moved),
        };
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

    /// The commit `project`'s operation `op` is making, till the reload after it shows it made:
    /// as made at once, a local commit with the title it'll have, dimmed, with an ellipsis, only
    /// once it's taking a while. It isn't dragged, edited or split, as it's not there yet.
    fn pending_row(
        &self,
        item: &Selection,
        project: &ProjectId,
        op: usize,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let (title, slow) = self.store.read(cx).pending_commit(project, op)?;
        let glyph = small_icon(Icon::Commit).text_color(style.meta(style.theme.commit_local));
        let ellipsis = div().text_11().text_color(style.muted()).child("…");
        Some(
            style
                .commit_line(
                    format!("pending-{project}-{op}"),
                    COMMIT_INDENT,
                    glyph,
                    title,
                    false,
                    None,
                )
                .when(slow, |el| el.opacity(0.5).child(ellipsis))
                .map(|el| self.selectable(el, item, cx))
                .into_any_element(),
        )
    }

    /// A commit only a branch's remote has, among the branch's commits: a ↓ where a commit has
    /// its glyph, and a hairline while it's unread. It isn't dragged or edited here.
    fn upstream_row(
        &self,
        item: &Selection,
        id: &ProjectId,
        ref_name: &FullName,
        commit_id: &CommitId,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let workspace = store.workspace(id)?;
        let listed = (workspace.upstream(ref_name).iter()).find(|c| c.id == *commit_id);
        // One a merge listed brought in sits a step in under it.
        let (title, merge, nested, author, email) = match listed {
            Some(commit) => (
                commit.title.clone(),
                commit.merge,
                false,
                commit.author.clone(),
                commit.email.clone(),
            ),
            None => {
                let (_, commit) = store.merged_upstream(id, ref_name, *commit_id)?;
                (
                    commit.title.clone(),
                    false,
                    true,
                    commit.author.clone(),
                    commit.email.clone(),
                )
            }
        };
        let title: SharedString = title.into();
        let unread = listed.is_some_and(|commit| store.is_unread(id, commit));
        let warn = style.meta(style.theme.commit_upstream);
        // A merge folds the commits it brought in on a double-click, or by the chevron its ↓
        // turns into.
        let toggle = merge.then(|| {
            let (project, owner, merge_id) = (id.clone(), ref_name.clone(), *commit_id);
            move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                this.toggle_merge(&project, owner.clone(), merge_id, window, cx)
            }
        });
        // A commit's glyph by its state, as every commit's is: upstream, or a merge among them.
        let arrow = small_icon(match merge {
            true => Icon::BranchMerge,
            false => Icon::CommitFill,
        })
        .text_color(warn);
        let merge_open = self.open_merges.contains(commit_id)
            || matches!(&store.selection, Some(Selection::Upstream(_, _, c))
                if store.merged_upstream(id, ref_name, *c).is_some_and(|(m, _)| m == *commit_id));
        let glyph = match toggle.clone() {
            None => arrow.into_any_element(),
            Some(toggle) => {
                let fold = ElementId::Name(format!("merge-fold-{}", commit_id.0).into());
                self.fold(
                    fold,
                    arrow,
                    Some(merge_open),
                    style.selected,
                    warn,
                    cx,
                    toggle,
                )
            }
        };
        // Who it's from at its end; a folded merge, who the commits it brought in are from, once
        // they're read. Open, its commits carry the faces.
        let end = match merge {
            false => {
                Some(crate::ui::person::author_avatar(&author, &email, 14., style.theme).mx(dp(3.)))
            }
            true if !merge_open => store.merged.get(commit_id).and_then(|read| {
                let (merged, _) = read.as_ref()?;
                let people = merged.iter().map(|c| (c.author.as_str(), c.email.as_str()));
                authors_stack(people, style.theme)
            }),
            true => None,
        };
        Some(
            style
                // Under its heading's words, as a branch's commits are under its name.
                .commit_line(
                    format!("upstream-{id}-{}", commit_id.0),
                    UPSTREAM_INDENT + if nested { STEP } else { 0. },
                    glyph,
                    title,
                    unread,
                    None,
                )
                .children(end)
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

    fn heading_open(&self, project: &ProjectId, section: &Section, cx: &App) -> bool {
        let store = self.store.read(cx);
        match section {
            Section::Upstream(ref_name) => self.upstream_open(store, project, ref_name),
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
            Section::Upstream(ref_name) => Self::upstream_selected(store, &project, ref_name),
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
        .style(ButtonStyle::Outline)
        .inverted(inverted)
        .disabled_because(updating.then_some("Updating already"))
        .tooltip_for(
            "Rebase your branches and worktrees onto the target's new commits",
            &UpdateFromTarget,
        );
        let step = self.row_step(update, cx, move |this, window, cx| {
            let project = project.clone();
            this.app.update_later(window, cx, move |app, _, cx| {
                app.update_from_target(project, UpdateScope::All, cx)
            });
        });
        // Pulsing while it updates, rather than a longer label moving what's beside it.
        div().child(match updating {
            true => pulsing(step).into_any_element(),
            false => step.into_any_element(),
        })
    }

    /// Over the commits only branch `ref_name`'s remote has, as but-ui-react's UpstreamItem:
    /// "New on origin", the whole ref on hover, who they're from and how many while folded, and
    /// Update. The branch's own row says when any is unread.
    fn upstream_heading(
        &self,
        ix: usize,
        project: &ProjectId,
        ref_name: &FullName,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let workspace = store.workspace(project);
        let can_pull = workspace.is_some_and(|ws| ws.can_pull(ref_name));
        let open = self.upstream_open(store, project, ref_name);
        let (_, count, tip) = upstream_label(workspace, ref_name);
        let branch = workspace.and_then(|ws| ws.find_branch(Some(ref_name)));
        let full = branch.and_then(|b| b.remote.clone());
        // The remote alone, as but-ui-react's UpstreamItem names it; the whole ref on hover.
        let label = match full.as_deref().and_then(|full| full.split_once('/')) {
            Some((remote, _)) => format!("New on {remote}"),
            None => "New on the remote".to_owned(),
        };
        let people = branch.map(|b| {
            b.incoming
                .iter()
                .map(|c| (c.author.as_str(), c.email.as_str()))
        });
        let authors = (!open).then(|| people.and_then(|people| authors_stack(people, style.theme)));
        let section = Section::Upstream(ref_name.clone());
        let row = self.section_heading(ix, project, &section, label, style, cx);
        let row = row
            .when_some(full, |el, full| el.tooltip(Tooltip::text(full)))
            .children(authors.flatten())
            .pr(dp(8.))
            // Folded, how many. While they can be pulled it shows open too, as what pulls them.
            .when(!open || can_pull, |el| {
                el.children(self.update_step(project, Some(ref_name), count, tip, style, cx))
            });
        Some(row.into_any_element())
    }

    /// The heading over `section` of `project`'s card: its glyph, which turns into its fold's
    /// chevron, and `label`, quiet, the glyph in a 16-point slot as a row's icon is (see
    /// [`RowStyle::row`]). Selecting it shows what it folds on its page; the chevron, a
    /// double-click or Enter lists or folds them.
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
            // A branch's remote's under its commit glyphs, its label under their titles.
            Section::Upstream(_) => (UPSTREAM_GUIDE - 8. - ROW_LEFT, Icon::ArrowInBox),
            Section::Inactive => (6., Icon::Clock),
            Section::Mesh => (6., Icon::Globe),
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
        // The upstream heading's glyph takes its commits' colour, so the two change together.
        let ink = match section {
            Section::Upstream(_) => style.meta(style.theme.commit_upstream),
            Section::Inactive | Section::Mesh => quiet,
        };
        let glyph =
            (div().w(dp(16.)).flex().justify_center()).child(small_icon(glyph).text_color(ink));
        (style.row(format!("section-heading-{ix}"), indent))
            .pr(dp(12.))
            .aria_label(label.clone())
            .aria_expanded(open)
            .child(self.fold(
                ("section-fold", ix),
                glyph,
                Some(open),
                style.selected,
                quiet,
                cx,
                toggle.clone(),
            ))
            .child(
                (section_label(label, style.theme).text_color(match section {
                    // A heading of its own rows, as but-ui-react's UpstreamItem, not a group's.
                    Section::Upstream(_) => style.meta(style.theme.text_1),
                    Section::Inactive | Section::Mesh => quiet,
                }))
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
        .min_h(dp(theme.sizes.control))
        .px(dp(12.))
        .py(dp(4.))
        .gap(dp(6.))
        .text_12()
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

/// How far a card goes on below its last row, drawn `slid` from where it's laid out, given how
/// far the rows `above` it are: down to where the card ends, while that row's above its place
/// and another's below its own, as rows trading places leave the card as long as it was. Else
/// nothing, as a card moving whole or a row coming in moves the edge with the last row.
fn past_last_row(mut above: impl Iterator<Item = f32>, slid: f32) -> f32 {
    match slid < 0. && above.any(|above| above > 0.) {
        true => -slid,
        false => 0.,
    }
}

/// How far the rest of row `ix`'s card goes on from it, down or else up, up to `room`, short of
/// the card's last or first row: a stretch over those would show its square corners outside
/// their round ones, which they're drawn with anyway, by the list or by [`Navigator::slid_in`].
fn card_beyond(rows: &[Row], ix: usize, down: bool, room: f32, row: f32) -> f32 {
    let (before, after) = rows.split_at(ix.min(rows.len()));
    let far = |rest: &mut dyn Iterator<Item = &Row>| {
        let mut rest = rest.take_while(|row| !matches!(row, Row::Gap)).peekable();
        let mut far = 0.;
        while let Some(next) = rest.next() {
            if far >= room || rest.peek().is_none() {
                break;
            }
            far += next.height(row);
        }
        f32::min(far, room)
    };
    match down {
        true => far(&mut after.iter().skip(1)),
        false => far(&mut before.iter().rev()),
    }
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

/// What ⇧← goes up to from `selection`: a branch in a stack its stack's caption, and a stack,
/// a branch alone, a terminal or uncommitted changes its worktree's heading, else the project's
/// row standing in for it. Not while a commit is edited, which lists its files without one.
fn parent(store: &Store, selection: &Selection) -> Option<Selection> {
    let project = selection.project();
    let workspace = store.workspace(project)?;
    if workspace.editing.is_some() {
        return None;
    }
    if let Selection::Branch(_, ref_name) = selection
        && let Some((lane, _)) = workspace.lane_of(ref_name.as_ref())
        && let Some(caption) = caption(project, &store.after_deletes(project, &lane.branches))
    {
        return Some(caption);
    }
    match selection {
        Selection::Branch(..)
        | Selection::Stack(..)
        | Selection::Terminal(..)
        | Selection::Uncommitted(..) => {
            let checkout = store.checkout_of(selection)?;
            Some(match checkout.is_linked() || store.main_headed(project) {
                true => Selection::Checkout(project.clone(), checkout),
                false => Selection::Project(project.clone()),
            })
        }
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

fn tally(files: &[FileChange]) -> Tally {
    let conflicted = files.iter().filter(|f| f.conflicted).count();
    // A conflicted file's lines are its markers, which the file list doesn't count either.
    let lines = (files.iter().filter(|f| !f.conflicted))
        .filter_map(|f| f.lines)
        .fold((0, 0), |(a, r), (fa, fr)| (a + fa, r + fr));
    (files.len(), conflicted, lines)
}

/// What a terminal's program is at, in colour only where the user is wanted, turning while
/// it works and `seen`.
pub(crate) fn status_mark(status: &Status, seen: bool, style: RowStyle) -> Option<Stateful<Div>> {
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
            glyph(Icon::Warning, style.meta(theme.warn_mark)),
            said(reason, "Waiting for you"),
        ),
        Status::Done => (
            glyph(Icon::Tick, style.meta(theme.safe_mark)),
            "Finished".into(),
        ),
        Status::Failed(reason) => (
            glyph(Icon::Cross, style.meta(theme.danger_mark)),
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
                    .child(crate::ui::diff_stats(added, removed, style.solid, theme))
                    .tooltip(Tooltip::text(tip)),
            )
        }
    };
    conflicts.into_iter().chain(all)
}

/// A flask at the end of a worktree heading, on the checkout the running client was built from.
fn running_mark(ix: usize, style: RowStyle) -> impl IntoElement {
    div()
        .id(("worktree-running", ix))
        .flex_none()
        .child(small_icon(Icon::Lab).text_color(style.muted()))
        .tooltip(Tooltip::text(
            "The app you're using was built from this checkout",
        ))
}

/// A worktree heading's folder, a linked one's doubled.
fn worktree_icon(linked: bool, style: RowStyle) -> gpui::Svg {
    let glyph = match linked {
        false => Icon::Folder,
        true => Icon::FolderCopy,
    };
    icon(glyph).text_color(style.muted())
}

/// What the heading over the commits only `ref_name`'s remote has says: where they are, how
/// many, and that count in words.
pub(crate) fn upstream_label(
    workspace: Option<&Workspace>,
    ref_name: &FullName,
) -> (String, usize, String) {
    let branch = workspace.and_then(|ws| ws.find_branch(Some(ref_name)));
    let count = branch.map_or(0, |b| b.incoming.len());
    let label = match branch.and_then(|b| b.remote.as_ref()) {
        Some(remote) => format!("On {remote}"),
        None => "On the remote".to_owned(),
    };
    (label, count, plural(count, "commit"))
}

/// Who `people` (name, email) are, each once, as overlapping pictures: the first three, as
/// but-ui-react's rows stack them on a folded row. Set in 3 points, as one picture is.
fn authors_stack<'a>(
    people: impl Iterator<Item = (&'a str, &'a str)>,
    theme: &Theme,
) -> Option<Div> {
    let mut shown: Vec<(&str, &str)> = Vec::new();
    for (name, email) in people {
        if shown.len() == 3 {
            break;
        }
        if !shown
            .iter()
            .any(|(_, known)| known.eq_ignore_ascii_case(email))
        {
            shown.push((name, email));
        }
    }
    (!shown.is_empty()).then(|| {
        h_flex()
            .flex_none()
            .px(dp(3.))
            .children(shown.into_iter().enumerate().map(|(ix, (name, email))| {
                crate::ui::person::author_avatar(name, email, 14., theme)
                    .when(ix > 0, |el| el.ml(dp(-4.)))
            }))
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
                true if matches!(state, ReviewState::Open | ReviewState::Draft) => {
                    style.meta(theme.text_1)
                }
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

/// Whether `row` shows where what's moving goes: commits being moved show where they'd land, if
/// that's elsewhere, and what a squash goes into, or the branch dragged files start a commit
/// in, is outlined.
fn shows_where_it_lands(store: &Store, row: &Row) -> bool {
    match row {
        Row::Ghost(..) | Row::GhostBranch(..) | Row::NewCommit(_) => true,
        Row::Item(item) => moving::lands_on(store, item),
        _ => false,
    }
}

impl Render for Navigator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // After the rows are up to date, as a reorder of them starts sliding there, and nothing
        // else may draw the next frame, which what waits for the list's height needs too.
        if self.prepare(RowsIn::List, window, cx) || !self.list_drawn() {
            window.request_animation_frame();
        }
        let theme = Theme::get(cx).clone();
        let store = self.store.read(cx);
        let empty = store.projects_loaded && store.projects.is_empty();
        let detail_shown = self.detail_shown.get();
        let top_left = crate::menu_bar::top_left_room(window, &store.prefs, &theme);
        // Level with the location bar: the same height, from the window's top, and no rule
        // along its bottom as the cards scroll under it, as the bar has none either.
        let header = h_flex()
            .h(dp(HEADER_HEIGHT))
            .flex_none()
            // Room for macOS's traffic lights and the menu bar's button; the window's buttons are
            // at its other end. A card out over the detail has them out of its way, and isn't the
            // window's title bar.
            .map(|el| match top_left.filter(|_| !self.floating) {
                Some(room) => el.pl(room),
                None => el.pl(dp(8.)),
            })
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
            .child(self.filter_field(window, &theme, cx))
            .children(self.update_button(cx).map(|el| above_drag(div().child(el))))
            .child(above_drag(div().child(self.view_button(cx))))
            .child(above_drag(div().child(self.settings_button(&theme, cx))));
        let store = self.store.read(cx);
        let moving = store.moving.is_some();
        let squashing =
            (store.moving.as_ref()).is_some_and(|moving| matches!(moving.op, Op::Squash(_)));

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
            .on_drag_move(cx.listener(Self::follow_dragged))
            .map(|el| {
                on_any_drag_move(el, cx, |this, at, bounds, _, cx| {
                    this.drag_moved(bounds.contains(&at), cx)
                })
            })
            // One for each kind of drag: GPUI panics on a second.
            .on_drop(cx.listener(Self::drop_project))
            .on_drop(cx.listener(Self::drop_dragged))
            .on_drop(cx.listener(Self::drop_terminal))
            .track_focus(&self.focus)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.pointer_over = *hovered;
                // GPUI calls a drag's start a leave, and the drop has to land where it's aimed:
                // the drag's moves say where it is meanwhile (see `drag_moved`).
                this.hold_order(*hovered || cx.has_active_drag(), cx);
                cx.notify();
            }))
            .map(|el| row_actions(el, cx, Navigator::act_on_rows))
            .map(|el| self.key_actions(el, cx))
            .size_full()
            .relative()
            // Out over the detail, it's in a popover's card, which has the edge, with the
            // terminal overview under it. GPUI cuts what's inside to a rectangle, so the drawer
            // along its bottom rounds its own corners.
            .map(|el| match self.floating {
                true => el.rounded(dp(theme.shape.popup)),
                // On the detail's ground, which its cards edge, nothing to edge against, but the
                // room an edge took stays, so what's against the right doesn't move. The column
                // it's in paints the ground, under the line along its edge.
                false => el.border_r_1().border_color(gpui::transparent_black()),
            })
            .child(header)
            .map(|el| {
                // With no projects, the welcome beside it says what to do.
                if empty {
                    el
                } else if self.built.rows.is_empty() {
                    el.child(self.drawer(window, &theme, cx))
                } else {
                    let rows = list(
                        self.list.clone(),
                        cx.processor(|this, ix: usize, window, cx| this.render_row(ix, window, cx)),
                    )
                    .size_full()
                    .pt(dp(LIST_TOP));
                    let height = self.fit_footer(window, cx);
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
                        .children(self.card_caps(height, &theme, cx))
                        .children(self.drag_divider(&theme, cx))
                        // Ending level with the detail's cards, so what scrolls past is cut
                        // where they end.
                        .mb(dp(CARD_GAP))
                        .map(|el| on_any_drag_move(el, cx, Navigator::scroll_near_edge)),
                    )
                }
            })
        // A card out over the detail goes back once the pointer leaves it, so it's resized
        // only beside the detail, once it's all there.
    }
}

#[cfg(test)]
mod tests {
    use super::past_last_row;

    #[test]
    fn a_card_moving_whole_keeps_its_edge_on_its_last_row() {
        assert_eq!(
            past_last_row([-60., -60.].into_iter(), -60.),
            0.,
            "every row slides as far, so the edge goes with them"
        );
    }

    #[test]
    fn a_row_landing_last_leaves_the_edge_where_the_card_ends() {
        assert_eq!(
            past_last_row([28., 28.].into_iter(), -56.),
            56.,
            "the rows trade places, so the card stays as long as it was"
        );
    }

    #[test]
    fn a_row_rising_from_last_leaves_the_edge_where_the_card_ends() {
        assert_eq!(
            past_last_row([-28., 56.].into_iter(), -28.),
            28.,
            "the row on its way up is two rows over the last"
        );
    }

    #[test]
    fn a_row_coming_in_moves_the_edge_with_the_last_row() {
        assert_eq!(
            past_last_row([0., 0.].into_iter(), -28.),
            0.,
            "the card grows as the rows below it make room"
        );
    }

    #[test]
    fn a_row_taken_away_moves_the_edge_with_the_last_row() {
        assert_eq!(
            past_last_row([28., 0.].into_iter(), 28.),
            0.,
            "the card shrinks as the rows close up"
        );
    }
}
