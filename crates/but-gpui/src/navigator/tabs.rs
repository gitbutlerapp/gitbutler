//! The navigator across the window's top, where Settings can put it to give its width back to
//! the panes: a tab for each project, the current one opened out with a tab for each of its
//! worktrees. A tab under the pointer drops a card holding the navigator's own rows for what it
//! names. The tab itself reaches down into the card, and moving to another tab slides both over,
//! the rows the card held sliding out as the new ones come in. The strip scrolls sideways as one;
//! nothing in it scrolls on its own. A project's tab, or a linked worktree's, drags along it to
//! another place, as moving it with the keys does.
//!
//! Tabs are placed here rather than by layout, from their labels' widths, so a frame knows where
//! each is, and where each will end up, without waiting for a layout to measure.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use but_tty::Status;
use gpui::{
    AnyElement, App, ClickEvent, Context, DispatchPhase, Div, DragMoveEvent, ElementId, Entity,
    FocusHandle, MouseExitEvent, MouseMoveEvent, Pixels, Point, Render, Role, ScrollHandle,
    SharedString, Stateful, Task, TextRun, Window, deferred, div, font, linear_color_stop,
    linear_gradient, point, prelude::*, relative,
};

use super::order::moving_worktree;
use super::rows::{CARD_PADDING, Rows, listed_with};
use super::{
    MAIN_WORKTREE, Navigator, Row, RowStyle, drawer_title, project_glyph, status_mark,
    worktree_icon, worktrees_listed,
};
use crate::app::{AppHandle, Cancel, Collapse, Expand, SelectNext, SelectPrevious, ToggleFold};
use crate::backend::Checkout;
use crate::clock;
use crate::overlays::OVERLAY;
use crate::state::{NavigatorPlace, ProjectId, Selection, Store, WatchedOrder};
use crate::theme::Theme;
use crate::transfer::{Dragged, Moving, Op, ProjectDrag, Slot};
use crate::ui::fold::{FOLD, Glide, Reorder, Slide, inert};
use crate::ui::{
    HEADER_HEIGHT, Icon, POPOVER_RADIUS, above_drag, dp, fixed, h_flex, icon, popover, v_flex,
};

/// How tall a tab's ground is, and how far down the strip it starts.
const TAB_HEIGHT: f32 = 28.;
const TAB_TOP: f32 = (HEADER_HEIGHT - TAB_HEIGHT) / 2.;
/// The room either side of a tab's icon and label, and between them.
const TAB_PADDING: f32 = 10.;
const TAB_ICON: f32 = 16.;
const ICON_GAP: f32 = 6.;
/// The widest a label draws, past which it's cut.
const LABEL_MOST: f32 = 180.;
/// How far apart a worktree's tabs are, and how far a project's tab is from the one before it.
const WORKTREE_GAP: f32 = 4.;
const PROJECT_GAP: f32 = 8.;
/// How far the current project's outline stands off its tabs.
const OUTLINE: f32 = 2.;
/// How far a selected tab's highlight is set in from its ground.
const SELECTION_INSET: f32 = 3.;
/// How far the strip's ends fade while there's more past them.
const FADE: f32 = 24.;
/// How wide the card under a tab is, and how far it stays from the window's edges.
const CARD_WIDTH: f32 = 340.;
const CARD_MARGIN: f32 = 8.;
/// What the card's drawn in: over what `ui::fold::on_top` draws, as a tile zoomed or dragged, and
/// under what covers the window, as a modal or a menu.
const CARD_LAYER: usize = OVERLAY - 5;
/// What's lifted out of the card's rows goes in the layers over it, by rank, under the overlays.
pub(super) const CARD_ROWS_LAYER: usize = CARD_LAYER + 1;
/// How much room the filter takes over the card's rows.
const FILTER_ROOM: f32 = 37.;
/// How far past the card its shadow reaches.
const CARD_SHADOW: f32 = 32.;
/// How much quicker than it opens the card fades in.
const CARD_FADES_IN: f32 = 3.;
/// How long what moves as the selection or the pointer goes from tab to tab takes, as a tile does:
/// it follows the pointer, so it can't be slower than it. All of it together, the card, the tabs
/// opening out and the scrolling, so a tab and the card hanging from it land at once.
const TAB_MOTION: Duration = Duration::from_millis(110);
/// How long the card waits on a tab the pointer crossed on its way down to the card.
const POINTER_REST: Duration = Duration::from_millis(80);
/// How long the pointer can stray before the card goes back.
const POINTER_GRACE: Duration = Duration::from_millis(220);
/// How far the pointer moves before that says which way it's going.
const POINTER_STRIDE: f32 = 6.;
/// The room a terminal's status takes in a tab, past its label.
const MARK_ROOM: f32 = 12. + ICON_GAP;

/// What a tab names: a watched project, one of its checkouts, or the projects not watched, which
/// share a tab as they share the drawer beside the panes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum TabKey {
    Project(ProjectId),
    Checkout(ProjectId, Checkout),
    More,
}

impl TabKey {
    /// The project the tab's in, if it's one's.
    fn project(&self) -> Option<&ProjectId> {
        match self {
            TabKey::Project(id) | TabKey::Checkout(id, _) => Some(id),
            TabKey::More => None,
        }
    }

    /// What a click on the tab selects, if it's anything: the projects not watched are only
    /// there to pick from.
    fn selection(&self) -> Option<Selection> {
        match self {
            TabKey::Project(id) => Some(Selection::Project(id.clone())),
            TabKey::Checkout(id, checkout) => {
                Some(Selection::Checkout(id.clone(), checkout.clone()))
            }
            TabKey::More => None,
        }
    }

    fn element_id(&self, prefix: &str) -> ElementId {
        ElementId::Name(match self {
            TabKey::More => format!("{prefix}-more").into(),
            TabKey::Project(id) => format!("{prefix}-{id}").into(),
            TabKey::Checkout(id, Checkout::Main) => format!("{prefix}-{id}-main").into(),
            TabKey::Checkout(id, Checkout::Linked(name)) => {
                format!("{prefix}-{id}-linked-{name}").into()
            }
        })
    }
}

/// A project's tabs as they're laid out: how wide its own is, and each of its worktrees', which
/// only a project that's opened out has.
struct Group {
    id: ProjectId,
    width: f32,
    worktrees: Vec<(Checkout, f32)>,
}

/// A tab where it's drawn, in the strip's content.
#[derive(Debug, Clone, PartialEq)]
struct Placed {
    key: TabKey,
    left: f32,
    width: f32,
}

/// A project opened out, or on its way there or back, in the strip's content: from where its
/// tab starts, through where its worktrees' start, how much of them `shown` and of `whole`.
#[derive(Debug, Clone, PartialEq)]
struct Opened {
    id: ProjectId,
    left: f32,
    worktrees_left: f32,
    shown: f32,
    whole: f32,
    share: f32,
}

#[derive(Debug, Default, PartialEq)]
struct Laid {
    tabs: Vec<Placed>,
    opened: Vec<Opened>,
    end: f32,
}

impl Laid {
    /// Where `key`'s tab is, or while that's gone, its project's.
    fn place(&self, key: &TabKey) -> Option<&Placed> {
        let at = |key: &TabKey| self.tabs.iter().find(|tab| tab.key == *key);
        at(key).or_else(|| match key {
            TabKey::Checkout(id, _) => at(&TabKey::Project(id.clone())),
            TabKey::Project(_) | TabKey::More => None,
        })
    }
}

/// Lay `groups` out left to right, each opened out as far as `share` says, then the tab of the
/// projects not watched, `more` wide. Worktrees slide out from behind their project's tab, so the
/// last comes first, and what's after them moves over to make room.
fn lay_out(groups: &[Group], more: f32, share: impl Fn(&ProjectId) -> f32) -> Laid {
    let mut laid = Laid::default();
    let mut at = 0.;
    for group in groups {
        let project = TabKey::Project(group.id.clone());
        laid.tabs.push(Placed {
            key: project,
            left: at,
            width: group.width,
        });
        let left = at;
        at += group.width;
        let share = share(&group.id);
        if share > 0. && !group.worktrees.is_empty() {
            let gaps = group.worktrees.len() as f32 * WORKTREE_GAP;
            let whole = group.worktrees.iter().map(|(_, w)| w).sum::<f32>() + gaps;
            let shown = whole * share;
            let mut tab = at + shown - whole + WORKTREE_GAP;
            for (checkout, width) in &group.worktrees {
                laid.tabs.push(Placed {
                    key: TabKey::Checkout(group.id.clone(), checkout.clone()),
                    left: tab,
                    width: *width,
                });
                tab += width + WORKTREE_GAP;
            }
            laid.opened.push(Opened {
                id: group.id.clone(),
                left,
                worktrees_left: at,
                shown,
                whole,
                share,
            });
            at += shown;
        }
        at += PROJECT_GAP;
    }
    laid.tabs.push(Placed {
        key: TabKey::More,
        left: at,
        width: more,
    });
    laid.end = at + more;
    laid
}

/// Whether a pointer that moved `by` is going down to the card under the tab it was on, rather
/// than along to the tab beside it, whose corner it cuts. The card is so close under the tabs
/// that the way to a row in it often goes further across than down, while a sweep along the tabs
/// is nearly flat. Wrongly taken for that, a sweep only waits a moment.
fn heading_down(by: Point<Pixels>) -> bool {
    by.y > Pixels::ZERO && by.y * 3. >= by.x.abs()
}

/// Where a tab `width` wide, dragged along the strip, goes among `others`, each by what it
/// names and how wide it is, laid out from `start` with `gap` between: the place where its
/// middle would be nearest `x`. Only the others set the places, as for a card dragged in the
/// rows, so where it's shown now never changes where it goes.
fn nearest_slot<T: Clone>(
    others: &[(T, f32)],
    start: f32,
    gap: f32,
    width: f32,
    x: f32,
) -> Slot<T> {
    let off = |left: f32| (left + width / 2. - x).abs();
    let (mut left, mut nearest) = (start, (f32::INFINITY, Slot::End));
    for (key, other) in others {
        if off(left) < nearest.0 {
            nearest = (off(left), Slot::Before(key.clone()));
        }
        left += other + gap;
    }
    match off(left) < nearest.0 {
        true => Slot::End,
        false => nearest.1,
    }
}

/// The tab after `from` in `order`, or before it. With `projects_only`, the next project's own,
/// from whichever of this project's tabs it's on.
fn stepped(
    order: &[TabKey],
    from: Option<&TabKey>,
    forward: bool,
    projects_only: bool,
) -> Option<TabKey> {
    // A worktree's tab can go from under the keys, as its project's last linked worktree does.
    let project = from.and_then(TabKey::project).cloned().map(TabKey::Project);
    let at = (order.iter().position(|tab| Some(tab) == from))
        .or_else(|| order.iter().position(|tab| Some(tab) == project.as_ref()))?;
    let wanted = |tab: &&TabKey| {
        !projects_only
            || (!matches!(tab, TabKey::Checkout(..))
                && tab.project() != from.and_then(TabKey::project))
    };
    match forward {
        true => order.iter().skip(at + 1).find(wanted),
        false => order.iter().take(at).rev().find(wanted),
    }
    .cloned()
}

/// The tab a move takes along the strip among the others, as it's moved: a watched project's,
/// or a linked worktree's among its project's.
pub(super) fn moving_tab(store: &Store) -> Option<TabKey> {
    let moving = store.moving.as_ref()?;
    let project = moving.project.clone();
    match &moving.what {
        Moving::Project(_) => Some(TabKey::Project(project)),
        Moving::Worktree { name, .. } => {
            Some(TabKey::Checkout(project, Checkout::Linked(name.clone())))
        }
        Moving::Commits(_)
        | Moving::Branch(_)
        | Moving::Terminal { .. }
        | Moving::Files(_)
        | Moving::Copies(_) => None,
    }
}

/// How far `key`'s tab is drawn from where it's laid out as `reorder` slides the tabs: a
/// worktree's as far as its project's, and as far again among its project's.
fn slid(reorder: &Reorder<TabKey>, key: &TabKey) -> f32 {
    let project = (key.project()).map_or(0., |id| reorder.offset(&TabKey::Project(id.clone())));
    match key {
        TabKey::Checkout(..) => project + reorder.offset(key),
        TabKey::Project(_) | TabKey::More => project,
    }
}

/// A project's worktrees sliding out or back.
fn opening_slide() -> Slide {
    Slide::lasting(TAB_MOTION)
}

/// How far to scroll, from `offset`, for `span` to show in `viewport`: not at all if it does, and
/// from its left if it's too wide to. Each span is `(left, right)`, the later ones winning.
fn scrolled_to(spans: &[(f32, f32)], viewport: f32, offset: f32, end: f32) -> f32 {
    let mut at = offset;
    for (left, right) in spans {
        if right - at > viewport {
            at = right - viewport;
        }
        if *left < at {
            at = *left;
        }
    }
    at.clamp(0., (end - viewport).max(0.))
}

/// What the card holds, or held as it slides out.
struct Pane {
    key: TabKey,
    /// Sliding across the card to `to`.
    x: Glide,
    to: f32,
    drawn_x: f32,
    /// How tall its rows are, in unscaled pixels, as last drawn.
    height: Rc<Cell<Option<f32>>>,
    /// Scrolls to the row the keys step to.
    scroll: ScrollHandle,
    /// The selection it was left with, sliding out, which it goes on showing.
    left_with: Option<Option<Selection>>,
}

impl Pane {
    fn new(key: TabKey) -> Self {
        Self {
            key,
            x: Glide::lasting(TAB_MOTION),
            to: 0.,
            drawn_x: 0.,
            height: Rc::default(),
            scroll: ScrollHandle::new(),
            left_with: None,
        }
    }

    /// Slide over to `to` from where it's drawn, or from `from`.
    fn slide(&mut self, from: Option<f32>, to: f32, cx: &App) {
        self.x.start(from.unwrap_or(self.drawn_x), cx);
        self.to = to;
    }
}

/// What a tab's terminals are at, the most urgent of them, as the tab shows it.
struct Mark {
    status: Status,
    shown: bool,
    slide: Slide,
}

/// A frame of the card: what's drawn of it, in the strip, the tab reaching down into it, in the
/// strip's content, and how far it's faded in, which they do as one.
struct DrawnCard {
    elements: [AnyElement; 3],
    left: f32,
    width: f32,
    fade: f32,
}

/// The card under a tab, and the tab reaching down into it.
struct Card {
    open: bool,
    slide: Slide,
    pane: Pane,
    /// What it held before, sliding out.
    leaving: Vec<Pane>,
    /// Going from tab to tab: how far along the window it is, how wide the tab it hangs from
    /// is, and how tall it is open.
    left: Glide,
    width: Glide,
    height: Glide,
    drawn: (f32, f32, f32),
    /// How tall its rows were when last drawn.
    rows_height: f32,
}

pub struct NavigatorTabs {
    app: AppHandle,
    /// Takes the keys, which step along the tabs and down the card's rows.
    focus: FocusHandle,
    /// The tab the keys are on, while they're here. The card shows what it names.
    keyed: Option<TabKey>,
    store: Entity<Store>,
    navigator: Entity<Navigator>,
    scroll: ScrollHandle,
    /// The project opened out, none while the selection's in one not watched, and those on
    /// their way there or back.
    current: Option<ProjectId>,
    /// There's been a project to open out, after which a change of it slides.
    selection_followed: bool,
    opening: HashMap<ProjectId, Slide>,
    /// The tab the selection's in, and the ground behind it, gliding there from the last.
    active: Option<TabKey>,
    active_left: Glide,
    active_width: Glide,
    active_drawn: Option<(f32, f32)>,
    /// Scrolling to what's current, from where it was, and where to, while it is.
    scrolled: Glide,
    scrolling: bool,
    scroll_rest: Option<f32>,
    /// Each label's width, in unscaled pixels, by the interface scale they were measured at.
    labels: HashMap<SharedString, f32>,
    labels_scale: f32,
    card: Option<Card>,
    /// The tab under the pointer, and whether the card is.
    pointed: Option<TabKey>,
    over_card: bool,
    /// The pointer's left the window. Hovers don't follow it out, so the two above still say
    /// where it last was, until it moves in again ([`Self::pointing`]).
    outside: bool,
    /// The card going to a tab once the pointer rests, or back once it's strayed.
    waiting: Option<(Option<TabKey>, Task<()>)>,
    /// Where the pointer was last, and a stride back, which say which way it's going.
    pointer: (Point<Pixels>, Point<Pixels>),
    /// The strip scrolled under the pointer, which hasn't moved since.
    scrolled_away: bool,
    /// There's more in the strip than shows, as last drawn, so it can scroll.
    overflows: bool,
    /// The status each tab shows, sliding in and out.
    marks: HashMap<TabKey, Mark>,
    marks_followed: bool,
    /// The tabs in order as last drawn, which says which way the card's rows slide.
    order: Vec<TabKey>,
    /// Projects that stopped being current with the pointer on the tabs, kept open so the tab it's
    /// on doesn't move, until it goes from them.
    kept_open: Vec<ProjectId>,
    /// What's current isn't scrolled to until the pointer has gone from the strip and the card.
    reveal_later: bool,
    /// Something under way needs the rows in sight: the card stays down from the selection's tab
    /// whatever the pointer does ([`Navigator::holds_card`]).
    held: bool,
    /// The filter opening over the card's rows, or closing.
    filter_open: bool,
    filter_slide: Slide,
    /// The tabs sliding to where a change of their order put them, as one being moved steps,
    /// and where each was laid out last: a project's in the strip, a worktree's among its
    /// project's, which it slides with too.
    reorder: Reorder<TabKey>,
    lefts: Vec<(TabKey, f32)>,
    /// The tab being moved, outlined as the side's card is, until it stops sliding.
    held_tab: Option<TabKey>,
    /// The tab being moved, which goes with no card down from it, and a project's with its
    /// worktrees shut, until it's placed ([`Self::follow_move`]).
    moved_tab: Option<TabKey>,
    /// The tabs as they'll be once they've stopped, as last drawn, which says where one dragged
    /// along them goes.
    rest: Laid,
}

impl NavigatorTabs {
    pub fn new(
        app: AppHandle,
        store: Entity<Store>,
        navigator: Entity<Navigator>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        // A fold in the card's rows opens or shuts there.
        cx.observe(&navigator, |_, _, cx| cx.notify()).detach();
        Self {
            app,
            focus: cx.focus_handle(),
            keyed: None,
            store,
            navigator,
            scroll: ScrollHandle::new(),
            current: None,
            opening: HashMap::new(),
            active: None,
            active_left: Glide::lasting(TAB_MOTION),
            active_width: Glide::lasting(TAB_MOTION),
            active_drawn: None,
            scrolled: Glide::lasting(TAB_MOTION),
            scrolling: false,
            scroll_rest: None,
            labels: HashMap::new(),
            labels_scale: 0.,
            card: None,
            pointed: None,
            outside: false,
            over_card: false,
            waiting: None,
            pointer: Default::default(),
            scrolled_away: false,
            overflows: false,
            selection_followed: false,
            marks: HashMap::new(),
            marks_followed: false,
            order: Vec::new(),
            kept_open: Vec::new(),
            reveal_later: false,
            held: false,
            filter_open: false,
            filter_slide: Slide::lasting(TAB_MOTION),
            reorder: Reorder::lasting(TAB_MOTION),
            lefts: Vec::new(),
            held_tab: None,
            moved_tab: None,
            rest: Laid::default(),
        }
    }

    /// The project opened out: the selection's, none for one not watched, else the first.
    fn current_project(store: &Store, watched: &[ProjectId]) -> Option<ProjectId> {
        match store.selection.as_ref().map(Selection::project) {
            Some(selected) => watched.contains(selected).then(|| selected.clone()),
            None => watched.first().cloned(),
        }
    }

    /// The tab the selection is in.
    pub(super) fn active_tab(store: &Store) -> Option<TabKey> {
        Some(Self::tab_of(store, store.selection.as_ref()?))
    }

    /// The tab `selection` is in.
    fn tab_of(store: &Store, selection: &Selection) -> TabKey {
        let project = selection.project().clone();
        if store.project(&project).is_some_and(|entry| !entry.expanded) {
            return TabKey::More;
        }
        match store.checkout_of(selection) {
            Some(checkout) if Self::has_worktree_tabs(store, &project) => {
                TabKey::Checkout(project, checkout)
            }
            Some(_) | None => TabKey::Project(project),
        }
    }

    /// Whether `project`'s checkouts have tabs of their own: with only the main one, there's
    /// nothing for one to tell from the project's.
    fn has_worktree_tabs(store: &Store, project: &ProjectId) -> bool {
        (store.workspace(project))
            .is_some_and(|workspace| !worktrees_listed(store, project, workspace).is_empty())
    }

    /// The watched projects, in the navigator's order.
    fn watched(&self, cx: &App) -> Vec<ProjectId> {
        let store = self.store.read(cx);
        (self.navigator.read(cx).watched(store).into_iter())
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// Whether `id`'s worktrees show once they've stopped: it's current or kept open, and isn't
    /// being moved.
    fn opens(&self, id: &ProjectId) -> bool {
        (self.current.as_ref() == Some(id) || self.kept_open.contains(id))
            && !matches!(&self.moved_tab, Some(TabKey::Project(moved)) if moved == id)
    }

    /// How much of `id`'s worktrees show at `now`.
    fn share(&self, id: &ProjectId, now: Instant) -> f32 {
        let open = self.opens(id);
        match self.opening.get(id) {
            Some(slide) => slide.share(open, now),
            None if open => 1.,
            None => 0.,
        }
    }

    /// Open out the project the selection moved to, and glide to the tab it's in.
    fn follow_selection(&mut self, projects: &[ProjectId], now: Instant, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let current = Self::current_project(store, projects);
        let active = Self::active_tab(store);
        let moved = current != self.current || active != self.active;
        // With the pointer on the tabs, as a click there just moved the selection, what it's on
        // stays under it: nothing before it shuts or scrolls until it's gone.
        let pointer_on_tabs = self.under_pointer().is_some();
        if current != self.current {
            if self.selection_followed {
                if let Some(was) = self.current.take() {
                    match pointer_on_tabs {
                        true => self.kept_open.push(was),
                        false => self
                            .opening
                            .entry(was)
                            .or_insert_with(opening_slide)
                            .start(false, cx),
                    }
                }
                if let Some(id) = current.clone() {
                    let kept = self.kept_open.iter().position(|kept| *kept == id);
                    match kept {
                        Some(at) => drop(self.kept_open.remove(at)),
                        None => self
                            .opening
                            .entry(id)
                            .or_insert_with(opening_slide)
                            .start(true, cx),
                    }
                }
            }
            self.current = current;
        }
        self.selection_followed |= !projects.is_empty();
        if active != self.active {
            if let Some((left, width)) = self.active_drawn {
                self.active_left.start(left, cx);
                self.active_width.start(width, cx);
            }
            self.active = active;
        }
        match (moved, pointer_on_tabs) {
            (true, true) => self.reveal_later = true,
            (true, false) => self.scroll_to_current(cx),
            (false, _) => {}
        }
        (self.opening).retain(|_, slide| slide.openness(now).is_some());
    }

    /// A project or a worktree being moved goes along the tabs as its tab alone: its card rolls
    /// up, and a project's worktrees shut behind it. Once it's placed or let go of, they open out
    /// again, the card coming back down if the keys are still here.
    fn follow_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let moving = moving_tab(self.store.read(cx));
        if moving == self.moved_tab {
            return;
        }
        for (key, open) in [(self.moved_tab.take(), true), (moving.clone(), false)] {
            if let Some(TabKey::Project(id)) = key
                && self.opens(&id)
            {
                (self.opening.entry(id).or_insert_with(opening_slide)).start(open, cx);
            }
        }
        self.moved_tab = moving;
        match (self.moved_tab.is_some(), self.keyed.clone()) {
            (true, _) => self.hide(window, cx),
            (false, Some(key)) => self.show(key, cx),
            (false, None) => {}
        }
    }

    /// The tab the pointer's on, or the one whose card it's in, unless the keys are what's moving.
    fn under_pointer(&self) -> Option<&TabKey> {
        if self.keyed.is_some() {
            return None;
        }
        let (pointed, over_card) = self.pointing();
        let card = (self.card.as_ref()).filter(|card| card.open && over_card);
        pointed.or(card.map(|card| &card.pane.key))
    }

    /// Shut the projects kept open for the pointer.
    fn shut_kept_open(&mut self, cx: &App) {
        for id in std::mem::take(&mut self.kept_open) {
            self.opening
                .entry(id)
                .or_insert_with(opening_slide)
                .start(false, cx);
        }
    }

    fn scroll_to_current(&mut self, cx: &App) {
        let scale = Theme::get(cx).scale;
        self.scrolled
            .start(-f32::from(self.scroll.offset().x) / scale, cx);
        self.scrolling = true;
    }

    /// Slide the tabs a change of their order moved from where they were laid out, as one being
    /// moved steps, and hold its outline on until they've stopped. A worktree's goes by where it
    /// is among its project's, as it slides with them.
    fn follow_order(&mut self, laid: &Laid, cx: &App) {
        self.reorder.frame();
        let lefts: Vec<(TabKey, f32)> = (laid.tabs.iter())
            .filter_map(|tab| match &tab.key {
                TabKey::Project(_) => Some((tab.key.clone(), tab.left)),
                TabKey::Checkout(id, _) => {
                    let opened = laid.opened.iter().find(|opened| opened.id == *id)?;
                    Some((tab.key.clone(), tab.left - opened.worktrees_left))
                }
                TabKey::More => None,
            })
            .collect();
        let was = std::mem::replace(&mut self.lefts, lefts);
        let left_of = |lefts: &[(TabKey, f32)], key: &TabKey| {
            (lefts.iter()).find_map(|(other, left)| (other == key).then_some(*left))
        };
        // Those there before and now, in their order then and now: only a change of that slides.
        let kept = |of: &[(TabKey, f32)], other: &[(TabKey, f32)]| {
            (of.iter())
                .filter(|(key, _)| left_of(other, key).is_some())
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>()
        };
        if kept(&was, &self.lefts) != kept(&self.lefts, &was) {
            let offsets = (self.lefts.iter())
                .filter_map(|(key, left)| Some((key.clone(), left_of(&was, key)? - left)))
                .collect();
            self.reorder.start(offsets, cx);
        }
        // After that, as letting go slides it back from where it was.
        match moving_tab(self.store.read(cx)) {
            Some(key) => self.held_tab = Some(key),
            None if !self.reorder.any() => self.held_tab = None,
            None => {}
        }
    }

    /// Whether the selection's in the tab being moved, as it always is in one moved with the
    /// keys, so its ground goes with it over the tabs it passes. One dragged by the pointer
    /// needn't have it.
    fn held_active(&self) -> bool {
        match (&self.held_tab, &self.active) {
            (Some(TabKey::Project(id)), Some(active)) => active.project() == Some(id),
            (Some(held), Some(active)) => held == active,
            (Some(_), None) | (None, _) => false,
        }
    }

    /// How far `key`'s tab is drawn from where it's laid out as the tabs slide ([`slid`]).
    fn slid(&self, key: &TabKey) -> f32 {
        slid(&self.reorder, key)
    }

    /// Have each tab show the most urgent of what its terminals are at, as a row does of those
    /// in its tiling, a project's of all its own. A mark comes and goes by the tab widening, so
    /// the tabs after it make room rather than jump.
    fn follow_marks(&mut self, groups: &[Group], now: Instant, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let others = (store.projects.iter()).filter(|entry| !entry.expanded);
        let not_watched = others.flat_map(|entry| store.terminals.statuses_on(&entry.id, None));
        let more = Status::most_urgent(not_watched)
            .filter(|status| **status != Status::Idle)
            .map(|status| (TabKey::More, status.clone()));
        let wanted: HashMap<TabKey, Status> = (groups.iter())
            .flat_map(|group| {
                let terminals = &store.terminals;
                // The current project's worktree tabs show it already, while it has them.
                let on_worktrees =
                    self.current.as_ref() == Some(&group.id) && !group.worktrees.is_empty();
                let project = (!on_worktrees).then(|| (TabKey::Project(group.id.clone()), None));
                let worktrees = (group.worktrees.iter()).map(|(checkout, _)| {
                    let key = TabKey::Checkout(group.id.clone(), checkout.clone());
                    (key, Some(checkout))
                });
                (project.into_iter())
                    .chain(worktrees)
                    .filter_map(|(key, checkout)| {
                        let statuses = terminals.statuses_on(&group.id, checkout);
                        let status = Status::most_urgent(statuses)?;
                        (*status != Status::Idle).then(|| (key, status.clone()))
                    })
            })
            .chain(more)
            .collect();
        for (key, mark) in &mut self.marks {
            match wanted.get(key) {
                Some(status) => {
                    mark.status = status.clone();
                    if !mark.shown {
                        mark.shown = true;
                        mark.slide.start(true, cx);
                    }
                }
                None if mark.shown => {
                    mark.shown = false;
                    mark.slide.start(false, cx);
                }
                None => {}
            }
        }
        // As the window opens, what's going on already is just there.
        let animate = std::mem::replace(&mut self.marks_followed, true);
        for (key, status) in wanted {
            self.marks.entry(key).or_insert_with(|| {
                let mut slide = Slide::default();
                if animate {
                    slide.start(true, cx);
                }
                Mark {
                    status,
                    shown: true,
                    slide,
                }
            });
        }
        (self.marks).retain(|_, mark| mark.shown || mark.slide.openness(now).is_some());
    }

    /// How much of `key`'s mark shows at `now`.
    fn mark_share(&self, key: &TabKey, now: Instant) -> f32 {
        (self.marks.get(key)).map_or(0., |mark| mark.slide.share(mark.shown, now))
    }

    /// How wide `text` draws, in unscaled pixels, cut to what a label shows.
    fn label_width(&mut self, text: &SharedString, window: &Window, cx: &App) -> f32 {
        let theme = Theme::get(cx);
        if self.labels_scale != theme.scale {
            self.labels.clear();
            self.labels_scale = theme.scale;
        }
        if let Some(width) = self.labels.get(text) {
            return *width;
        }
        let run = TextRun {
            len: text.len(),
            font: font(theme.ui_font.clone()),
            color: theme.text_1,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let size = theme.scaled(13.);
        let line = (window.text_system()).shape_line(text.clone(), size, &[run], None);
        // Up, as a label cut by a fraction of a pixel would end in an ellipsis.
        let width = (f32::from(line.width) / theme.scale).ceil().min(LABEL_MOST);
        self.labels.insert(text.clone(), width);
        width
    }

    fn tab_width(&mut self, text: &SharedString, window: &Window, cx: &App) -> f32 {
        2. * TAB_PADDING + TAB_ICON + ICON_GAP + self.label_width(text, window, cx)
    }

    /// `project`'s checkouts that get a tab, each with what it says.
    fn checkouts(&self, project: &ProjectId, cx: &App) -> Vec<(Checkout, SharedString)> {
        let store = self.store.read(cx);
        if !Self::has_worktree_tabs(store, project) {
            return Vec::new();
        }
        let linked = (store.workspace(project).into_iter())
            .flat_map(|workspace| worktrees_listed(store, project, workspace))
            .map(|wt| {
                (
                    Checkout::Linked(wt.name.clone()),
                    wt.name.to_string().into(),
                )
            });
        std::iter::once((Checkout::Main, MAIN_WORKTREE.into()))
            .chain(linked)
            .collect()
    }

    /// The tabs to lay out, and what each worktree's says.
    fn groups(
        &mut self,
        projects: &[ProjectId],
        now: Instant,
        window: &Window,
        cx: &App,
    ) -> (Vec<Group>, HashMap<TabKey, SharedString>) {
        let mut labels = HashMap::new();
        let mut groups = Vec::with_capacity(projects.len());
        for id in projects {
            let name: SharedString = match self.store.read(cx).project(id) {
                Some(entry) => entry.project.name.clone().into(),
                None => continue,
            };
            let project = TabKey::Project(id.clone());
            let width =
                self.tab_width(&name, window, cx) + MARK_ROOM * self.mark_share(&project, now);
            labels.insert(project, name);
            let open = self.share(id, now) > 0. || self.current.as_ref() == Some(id);
            let mut worktrees = Vec::new();
            if open {
                for (checkout, label) in self.checkouts(id, cx) {
                    let key = TabKey::Checkout(id.clone(), checkout.clone());
                    let width =
                        self.tab_width(&label, window, cx) + MARK_ROOM * self.mark_share(&key, now);
                    worktrees.push((checkout, width));
                    labels.insert(key, label);
                }
            }
            groups.push(Group {
                id: id.clone(),
                width,
                worktrees,
            });
        }
        (groups, labels)
    }

    /// Scroll on toward what's current, as it'll be once the tabs have slid there.
    fn glide_scroll(&mut self, rest: &Laid, now: Instant, cx: &App) {
        self.scroll_rest = None;
        if !self.scrolling {
            return;
        }
        let theme = Theme::get(cx);
        let viewport = f32::from(self.scroll.bounds().size.width) / theme.scale;
        if viewport <= 0. {
            self.scrolling = false;
            return;
        }
        let current = self.current.as_ref();
        // Clear of the fades at the strip's ends, which would dim what it scrolled to show.
        let group = (rest.opened.iter())
            .find(|opened| Some(&opened.id) == current)
            .map(|opened| {
                (
                    opened.left - FADE,
                    opened.worktrees_left + opened.whole + FADE,
                )
            });
        let active = (self.keyed.as_ref().or(self.active.as_ref()))
            .and_then(|key| rest.place(key))
            .map(|tab| (tab.left - FADE, tab.left + tab.width + FADE));
        let spans: Vec<(f32, f32)> = group.into_iter().chain(active).collect();
        let offset = -f32::from(self.scroll.offset().x) / theme.scale;
        let to = scrolled_to(&spans, viewport, offset, rest.end);
        self.scroll_rest = Some(to);
        let at = self.scrolled.at(to, now);
        self.scroll
            .set_offset(point(-theme.scaled(at), self.scroll.offset().y));
        self.scrolling = self.scrolled.moving(now);
    }

    /// Where the strip's scrolled once it's stopped, as far as what's left of it allows.
    fn scroll_at_rest(&self, rest: &Laid, cx: &App) -> f32 {
        let theme = Theme::get(cx);
        let viewport = f32::from(self.scroll.bounds().size.width) / theme.scale;
        let offset = -f32::from(self.scroll.offset().x) / theme.scale;
        let most = (rest.end + 2. * OUTLINE - viewport).max(0.);
        self.scroll_rest.unwrap_or(offset).min(most)
    }

    /// What takes the keys here.
    pub(crate) fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The keys came here or went: the card shows the tab they're on, from the selection's, as
    /// the navigator's rows show where they are, and goes with them.
    fn follow_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let here = self.focus.is_focused(window);
        if here == self.keyed.is_some() {
            return;
        }
        (self.navigator).update(cx, |navigator, _| navigator.strip_has_keys = here);
        let key = (self.active.clone()).or_else(|| self.order.first().cloned());
        self.keyed = key.filter(|_| here);
        match self.keyed.clone() {
            Some(key) => {
                self.waiting = None;
                self.show(key, cx);
            }
            None if self.under_pointer().is_none() && !self.held => self.hide(window, cx),
            None => {}
        }
    }

    /// Keep the card down from the selection's tab while the rows are needed, and let it go after.
    fn follow_hold(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let held = self.navigator.read(cx).holds_card(window, cx);
        let was = std::mem::replace(&mut self.held, held);
        // A drag out of the card stays in it, whichever tab it's down from.
        let dragging = self.store.read(cx).dragging.is_some();
        let at_active = (self.card.as_ref()).is_some_and(|card| {
            card.open && (dragging || Some(&card.pane.key) == self.active.as_ref())
        });
        match (held, self.active.clone()) {
            (true, Some(key)) if !at_active => {
                self.waiting = None;
                self.show(key, cx);
            }
            (true, _) => {}
            (false, _) if was => {
                (self.navigator).update(cx, |navigator, cx| navigator.clear_filter(cx));
                if !self.focus.is_focused(window) && self.under_pointer().is_none() {
                    self.hide(window, cx);
                }
            }
            (false, _) => {}
        }
    }

    /// What arriving at `key`'s tab selects: where it was left, as stepping to a project in the
    /// navigator does. A tab the selection's in already goes to its own page.
    fn landing(&self, key: &TabKey, cx: &App) -> Option<Selection> {
        let store = self.store.read(cx);
        let here = self.active.as_ref() == Some(key);
        match key {
            TabKey::Project(id) if !here && self.current.as_ref() != Some(id) => {
                Some(self.navigator.read(cx).left_in_project(id, cx))
            }
            TabKey::Checkout(id, checkout) if !here => (store.left_in)
                .get(&(id.clone(), checkout.clone()))
                .filter(|left| store.still_there(left))
                .cloned()
                .or_else(|| key.selection()),
            TabKey::Project(_) | TabKey::Checkout(..) | TabKey::More => key.selection(),
        }
    }

    /// Step the keys to the next tab along, or back, or with `projects_only` to the next tab
    /// that isn't a worktree's, as ⇧⌘J goes from project to project in the navigator, landing
    /// as [`Self::landing`] says, as a step along the navigator's rows selects. With the keys
    /// elsewhere, from the selection's tab, leaving them there and the card up.
    pub(crate) fn step(&mut self, forward: bool, projects_only: bool, cx: &mut Context<Self>) {
        // A project or a worktree being moved steps along the tabs, as it steps along the rows in
        // the side's. Anything else being moved stays in its card, as what's selected stays on it.
        let store = self.store.read(cx);
        if store.moving.is_some() {
            if moving_tab(store).is_some() {
                let delta = if forward { 1 } else { -1 };
                (self.navigator).update(cx, |navigator, cx| navigator.step_move(delta, cx));
            }
            return;
        }
        let from = self.keyed.as_ref().or(self.active.as_ref());
        let Some(key) = stepped(&self.order, from, forward, projects_only) else {
            return;
        };
        let selection = self.landing(&key, cx);
        // Where that is, which can be a worktree's tab past the project's stepped to.
        let store = self.store.read(cx);
        let key = (selection.as_ref()).map_or(key, |selection| Self::tab_of(store, selection));
        if self.keyed.is_some() {
            self.keyed = Some(key.clone());
            // First, as what the card holds goes with the selection it had.
            self.show(key, cx);
        }
        if let Some(selection) = selection {
            (self.store).update(cx, |store, cx| {
                store.select_by_hand(selection, cx);
            });
        }
        self.scroll_to_current(cx);
    }

    /// Step the keys down the card's rows, or back up them and on to the tab. Not while a
    /// project or a worktree is being moved, which goes along the tabs ([`Self::step`]).
    fn step_row(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if moving_tab(self.store.read(cx)).is_some() {
            return;
        }
        let Some(key) = self.keyed.clone() else {
            return;
        };
        let row = (self.navigator).update(cx, |navigator, cx| {
            navigator.step_in(&key, delta, window, cx)
        });
        if let Some((row, card)) = row.zip(self.card.as_ref()) {
            card.pane.scroll.scroll_to_item(row);
        }
    }

    /// Fold or unfold the selected row of the card, as `fold` does the navigator's.
    fn fold_row(
        &mut self,
        fold: impl FnOnce(&mut Navigator, &mut Window, &mut Context<Navigator>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(key) = self.keyed.clone() {
            (self.navigator).update(cx, |navigator, cx| {
                navigator.in_scope(&key, cx, |navigator, cx| fold(navigator, window, cx))
            });
        }
    }

    /// Hand the keys to the page, its file list first.
    fn leave_to_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.app.update_later(window, cx, |app, window, cx| {
            app.focus_page_files(window, cx);
        });
    }

    /// Follows the pointer over the whole window, as a tab's hover is told only that it came, to
    /// say which way it's going when it does, and out of it. Asks for no frame.
    fn watch_pointer(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity().downgrade();
        gpui::canvas(
            |_, _, _| {},
            move |_, (), window, _| {
                let moved = this.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        moved
                            .update(cx, |this, cx| {
                                this.pointer_moved(event.position, window, cx)
                            })
                            .ok();
                    }
                });
                let left = this;
                window.on_mouse_event(move |_: &MouseExitEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        left.update(cx, |this, cx| this.pointer_left(window, cx))
                            .ok();
                    }
                });
            },
        )
        .absolute()
        .size_0()
    }

    fn pointer_moved(&mut self, to: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        // Back in the window, maybe where it left: after the hovers this move changes, which say
        // what it came onto.
        if std::mem::take(&mut self.outside) {
            cx.defer_in(window, |this, window, cx| this.follow_pointer(window, cx));
        }
        let (at, back) = &mut self.pointer;
        if to == *at {
            return;
        }
        let from = std::mem::replace(at, to);
        let by = to - *back;
        if f32::from(by.x.abs() + by.y.abs()) >= POINTER_STRIDE {
            *back = from;
        }
        // The hand's back on the pointer, so the tab under it means something again.
        if std::mem::take(&mut self.scrolled_away) {
            self.follow_pointer(window, cx);
        }
    }

    /// The pointer left the window, from a tab or the card, so the card goes as it would if the
    /// pointer had strayed from both inside it.
    fn pointer_left(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.outside = true;
        self.follow_pointer(window, cx);
    }

    /// The tab under the pointer, and whether the card is: neither while it's out of the window.
    fn pointing(&self) -> (Option<&TabKey>, bool) {
        match self.outside {
            true => (None, false),
            false => (self.pointed.as_ref(), self.over_card),
        }
    }

    /// The strip scrolled under the pointer, which didn't move: the tabs it passes aren't being
    /// pointed at, so the card goes rather than chasing them, until the pointer moves.
    fn strip_scrolled(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overflows && self.keyed.is_none() && !self.held {
            self.scrolled_away = true;
            self.waiting = None;
            // The strip's where the hand put it now.
            self.reveal_later = false;
            self.shut_kept_open(cx);
            self.hide(window, cx);
        }
    }

    /// The pointer came onto a tab or left one.
    fn tab_hovered(
        &mut self,
        key: &TabKey,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match hovered {
            true => self.pointed = Some(key.clone()),
            false if self.pointed.as_ref() == Some(key) => self.pointed = None,
            false => {}
        }
        self.follow_pointer(window, cx);
    }

    /// Have the card go where the pointer says: to the tab it's on at once, but for one it
    /// crossed on its way down into the card, and back once it's strayed from both.
    fn follow_pointer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scrolled_away || self.keyed.is_some() || self.held {
            return;
        }
        let showing = (self.card.as_ref())
            .filter(|card| card.open)
            .map(|card| &card.pane.key);
        let going_down = heading_down(self.pointer.0 - self.pointer.1);
        let (pointed, over_card) = self.pointing();
        let pointed = pointed.cloned();
        let (wanted, wait) = match &pointed {
            Some(key) if showing == Some(key) => {
                self.waiting = None;
                return;
            }
            Some(key) if showing.is_none() || !going_down => {
                let key = key.clone();
                self.waiting = None;
                return self.show(key, cx);
            }
            Some(key) => (Some(key.clone()), POINTER_REST),
            None if over_card || (showing.is_none() && !self.reveal_later) => {
                self.waiting = None;
                return;
            }
            None => (None, POINTER_GRACE),
        };
        if (self.waiting.as_ref()).is_some_and(|(waiting, _)| *waiting == wanted) {
            return;
        }
        let going = wanted.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update_in(cx, |this, window, cx| {
                this.waiting = None;
                match going {
                    Some(key) => this.show(key, cx),
                    None => this.hide(window, cx),
                }
            })
            .ok();
        });
        self.waiting = Some((wanted, task));
    }

    /// Drop the card from `key`'s tab, or slide it over there. Not while a tab's being moved,
    /// which goes along the tabs with none ([`Self::follow_move`]).
    fn show(&mut self, key: TabKey, cx: &mut Context<Self>) {
        if self.moved_tab.is_some() {
            return;
        }
        // A worktree's tab not out yet goes by its project's.
        let index = |key: &TabKey, order: &[TabKey]| {
            let at = |key: &TabKey| order.iter().position(|tab| tab == key);
            at(key).or_else(|| at(&TabKey::Project(key.project()?.clone())))
        };
        let order = std::mem::take(&mut self.order);
        match &mut self.card {
            Some(card) if card.pane.key == key => {
                if !card.open {
                    card.open = true;
                    card.slide.start(true, cx);
                }
            }
            Some(card) => {
                if !card.open {
                    card.open = true;
                    card.slide.start(true, cx);
                }
                // Rows go the way the pointer came from, and the new ones follow it in.
                let forward = index(&key, &order) >= index(&card.pane.key, &order);
                let away = if forward { -CARD_WIDTH } else { CARD_WIDTH };
                let (left, width, height) = card.drawn;
                card.left.start(left, cx);
                card.width.start(width, cx);
                card.height = Glide::lasting(TAB_MOTION);
                card.height.start(height, cx);
                let selection = self.store.read(cx).selection.clone();
                let back = (card.leaving.iter()).position(|pane| pane.key == key);
                let mut pane = match back {
                    Some(at) => {
                        let mut pane = card.leaving.remove(at);
                        pane.left_with = None;
                        pane.slide(None, 0., cx);
                        pane
                    }
                    None => {
                        let mut pane = Pane::new(key);
                        pane.slide(Some(-away), 0., cx);
                        pane
                    }
                };
                std::mem::swap(&mut card.pane, &mut pane);
                pane.left_with = Some(selection);
                pane.slide(None, away, cx);
                card.leaving.push(pane);
            }
            None => {
                let mut slide = Slide::lasting(TAB_MOTION);
                slide.start(true, cx);
                self.card = Some(Card {
                    open: true,
                    slide,
                    pane: Pane::new(key),
                    leaving: Vec::new(),
                    left: Glide::lasting(TAB_MOTION),
                    width: Glide::lasting(TAB_MOTION),
                    height: Glide::lasting(TAB_MOTION),
                    drawn: (0., 0., 0.),
                    rows_height: 0.,
                });
            }
        }
        self.order = order;
        cx.notify();
    }

    /// The pointer's gone: roll the card back up into its tab, hand on the keys a press in it gave
    /// the navigator, which isn't drawn here, shut what was kept open for it and scroll to what's
    /// current if that waited for it.
    fn hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shut_kept_open(cx);
        if std::mem::take(&mut self.reveal_later) {
            self.scroll_to_current(cx);
        }
        if let Some(card) = self.card.as_mut().filter(|card| card.open) {
            card.open = false;
            card.slide.start(false, cx);
        }
        self.app.update_later(window, cx, |app, window, cx| {
            app.keys_off_hidden_navigator(window, cx)
        });
        cx.notify();
    }

    /// The card as it's drawn at `now`, while there is one.
    fn draw_card(
        &mut self,
        rest: &Laid,
        rest_offset: f32,
        strip_left: f32,
        now: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<DrawnCard> {
        let theme = Theme::get(cx).clone();
        let filtering = self.navigator.read(cx).filter_has_keys(window, cx);
        if std::mem::replace(&mut self.filter_open, filtering) != filtering {
            self.filter_slide.start(filtering, cx);
        }
        let filter_share = self.filter_slide.share(filtering, now);
        let closed = (self.card.as_ref())
            .is_some_and(|card| !card.open && card.slide.openness(now).is_none());
        if closed {
            self.card = None;
        }
        let card = self.card.as_mut()?;
        // Where its tab comes to rest on screen. The tab may be on its way there, and a worktree's
        // starts out from behind its project's, so going by where it is now sends the card there
        // and back.
        let Some(tab) = rest.place(&card.pane.key).cloned() else {
            self.card = None;
            return None;
        };
        let slid = slid(&self.reorder, &card.pane.key);
        let tab_at_rest = strip_left + OUTLINE + tab.left - rest_offset + slid;
        let open = card.slide.share(card.open, now);
        let first_frame = card.drawn == (0., 0., 0.);
        let tab_left = match first_frame {
            true => tab_at_rest,
            false => card.left.at(tab_at_rest, now),
        };
        let width = match first_frame {
            true => tab.width,
            false => card.width.at(tab.width, now),
        };
        // With the edges, which the rows are inside of.
        let edges = 2. / theme.scale;
        let rows_height = (card.pane.height.get()).map_or(card.drawn.2, |rows| rows + edges);
        // Rows that come or go in it while it's held or they slide, as what's being placed shows
        // where it lands or does, and the filter finds what it finds, take it there.
        let was = std::mem::replace(&mut card.rows_height, rows_height);
        let filter_sliding = self.filter_slide.openness(now).is_some();
        let closing_up = self.held || self.navigator.read(cx).reorder.any();
        let rolling = card.slide.openness(now).is_some();
        if closing_up
            && was != rows_height
            && !card.height.moving(now)
            && !filter_sliding
            && !rolling
        {
            // At the rows' pace, so its edge doesn't get there before they do.
            card.height = Glide::lasting(FOLD);
            card.height.start(card.drawn.2, cx);
        }
        let height = card.height.at(rows_height, now);
        card.drawn = (tab_left, width, height);
        (card.leaving).retain(|pane| pane.x.moving(now));
        let scale = theme.scale;
        let offset = -f32::from(self.scroll.offset().x) / scale;
        let window_size = window.viewport_size();
        let room = f32::from(window_size.width) / scale;
        let most = f32::from(window_size.height) / scale - HEADER_HEIGHT - CARD_MARGIN;
        // The card hangs from the tab, its edge running on down the tab's. Only held in from the
        // window's edge does it reach past the tab.
        let left = tab_left - strip_left - OUTLINE + offset;
        let card_left = tab_left
            .min(room - CARD_WIDTH - CARD_MARGIN)
            .max(CARD_MARGIN);
        let corner = |past: f32| past.abs().min(POPOVER_RADIUS);
        let left_corner = corner(tab_left - card_left);
        let right_corner = corner(card_left + CARD_WIDTH - tab_left - width);
        let closing = !card.open;

        // What's sliding out, then what's coming in, which alone follows its folds.
        let shown: Vec<_> = (card.leaving.iter_mut().map(|pane| (pane, false)))
            .chain([(&mut card.pane, true)])
            .map(|(pane, live)| {
                pane.drawn_x = pane.x.at(pane.to, now);
                let (height, scroll) = (pane.height.clone(), pane.scroll.clone());
                let left_with = pane.left_with.clone();
                (
                    pane.key.clone(),
                    pane.drawn_x,
                    live,
                    height,
                    scroll,
                    left_with,
                )
            })
            .collect();

        let navigator = self.navigator.clone();
        let rows_top = navigator.read(cx).card_top();
        let sliding = navigator.read(cx).reorder.any();
        let rows_at = theme.scaled(CARD_PADDING) + card.pane.scroll.offset().y;
        let panes = shown
            .into_iter()
            .map(|(key, x, live, measured, scroll, left_with)| {
                let rows = (navigator).update(cx, |navigator, cx| {
                    navigator.scoped_selection = left_with;
                    let rows = navigator.scoped_rows(&key, window, cx);
                    navigator.scoped_selection = None;
                    rows
                });
                // Closing up, rows are laid out for the list they're going to and drawn where they
                // were, which can be past its end while the card still has room for them.
                let unclipped = live && sliding && scroll.max_offset().y <= gpui::Pixels::ZERO;
                let content = v_flex()
                    .id(key.element_id("tab-rows"))
                    .max_h(dp(most))
                    .when(!unclipped, |el| {
                        el.overflow_y_scroll().track_scroll(&scroll)
                    })
                    .py(dp(CARD_PADDING))
                    .children(rows);
                let (rows_top, rows_scroll) = (rows_top.clone(), scroll);
                let filter = (filter_share > 0. && live).then(|| {
                    let field = (navigator).update(cx, |navigator, cx| {
                        navigator.filter_field(window, &theme, cx)
                    });
                    div()
                        .flex_none()
                        .h(dp(FILTER_ROOM * filter_share))
                        .overflow_hidden()
                        .child(
                            h_flex()
                                .px(dp(CARD_PADDING + 3.))
                                .pt(dp(CARD_PADDING))
                                .child(field),
                        )
                });
                let pane = v_flex()
                    .absolute()
                    .top_0()
                    .left(dp(x))
                    .w(relative(1.))
                    .children(filter)
                    .child(content)
                    .child(
                        gpui::canvas(
                            move |bounds, window, _| {
                                let height = f32::from(bounds.size.height) / scale;
                                if measured.replace(Some(height)) != Some(height) {
                                    window.request_animation_frame();
                                }
                                if live {
                                    let inset = fixed(CARD_PADDING * scale);
                                    let scrolled = rows_scroll.offset().y;
                                    rows_top.set(Some(bounds.top() + inset + scrolled));
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    );
                inert(pane, !live)
            })
            .collect::<Vec<_>>();
        // Where what's dragged would land, over the rows.
        let divider = (navigator.read(cx).drag_divider(&theme, cx)).map(|line| {
            let over = div().absolute().left_0().right_0().bottom_0();
            over.top(rows_at).child(line)
        });
        let drop_height = height * open;
        // Fading in, as a card with no height yet is still its edges: a line out of nowhere.
        let fade = (open * CARD_FADES_IN).min(1.);
        let body = popover(&theme)
            .id("tab-card")
            .role(Role::Group)
            .absolute()
            .top(dp(HEADER_HEIGHT - 1.))
            .left(dp(card_left))
            .w(dp(CARD_WIDTH))
            .h(dp(drop_height))
            .rounded_tl(dp(left_corner))
            .rounded_tr(dp(right_corner))
            .shadow(Vec::new())
            .opacity(fade)
            .overflow_hidden()
            .occlude()
            .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                this.over_card = *hovered;
                this.follow_pointer(window, cx);
            }))
            .map(|el| (self.navigator).update(cx, |navigator, cx| navigator.takes_drags(el, cx)))
            .child(inert(
                div()
                    .relative()
                    .size_full()
                    .children(panes)
                    .children(divider),
                closing,
            ));
        // Cut off at the card's top edge, as the card is drawn after the strip, and a shadow
        // reaching above it would darken the tab coming down into it.
        let shadow = div()
            .absolute()
            .top(dp(HEADER_HEIGHT - 1.))
            .left(dp(card_left - CARD_SHADOW))
            .w(dp(CARD_WIDTH + 2. * CARD_SHADOW))
            .h(dp(drop_height + CARD_SHADOW))
            .overflow_hidden()
            .opacity(fade)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(dp(CARD_SHADOW))
                    .w(dp(CARD_WIDTH))
                    .h(dp(drop_height))
                    .rounded(dp(POPOVER_RADIUS))
                    .shadow_lg(),
            );
        // The card's top edge is open where the tab comes down into it.
        let seam = div()
            .absolute()
            .top(dp(HEADER_HEIGHT - 1.))
            .left(theme.scaled(tab_left) + fixed(1.))
            .w(theme.scaled(width) - fixed(2.))
            .h(fixed(1.))
            .bg(theme.bg_1)
            .opacity(fade);
        Some(DrawnCard {
            elements: [
                deferred(shadow)
                    .with_priority(CARD_LAYER)
                    .into_any_element(),
                deferred(body).with_priority(CARD_LAYER).into_any_element(),
                deferred(seam).with_priority(CARD_LAYER).into_any_element(),
            ],
            left,
            width,
            fade,
        })
    }

    /// What a tab shows, `left` from what holds it: its glyph, its label and the status of its
    /// terminals, or `solid`, as on a selection with the keys, which [`Self::highlight`] draws
    /// over it.
    fn face(
        &self,
        tab: &Placed,
        left: f32,
        label: &SharedString,
        solid: bool,
        now: Instant,
        cx: &Context<Self>,
    ) -> Div {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let key = &tab.key;
        let current = self.active.as_ref() == Some(key);
        let style = RowStyle {
            selected: solid,
            solid,
            ..RowStyle::plain(theme, 0.)
        };
        let glyph_color = match solid {
            true => style.muted(),
            false => theme.text_2,
        };
        let glyph = match key {
            TabKey::Project(id) => project_glyph(store, id, glyph_color, theme),
            TabKey::More => icon(Icon::Kebab)
                .size(dp(TAB_ICON))
                .text_color(glyph_color)
                .into_any_element(),
            TabKey::Checkout(id, checkout) => {
                let changed = (store.workspace(id))
                    .and_then(|ws| ws.uncommitted_in(checkout))
                    .is_some_and(|files| !store.uncommitted_shown(id, checkout, files).is_empty());
                worktree_icon(checkout.is_linked(), changed, style).into_any_element()
            }
        };
        let mark_share = self.mark_share(key, now);
        let mark = (self.marks.get(key))
            .and_then(|mark| status_mark(&mark.status, !store.hidden(), style))
            .map(|mark| {
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(dp(TAB_PADDING))
                    .flex()
                    .items_center()
                    .opacity(mark_share)
                    .child(mark)
            });
        h_flex()
            .absolute()
            .left(dp(left))
            .top_0()
            .h(dp(HEADER_HEIGHT))
            .w(dp(tab.width))
            .px(dp(TAB_PADDING))
            .gap(dp(ICON_GAP))
            .text_color(match (solid, current) {
                (true, _) => theme.text_inverted,
                (false, true) => theme.text_1,
                (false, false) => theme.text_2,
            })
            .child(
                div()
                    .flex_none()
                    .w(dp(TAB_ICON))
                    .flex()
                    .justify_center()
                    .child(glyph),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .mr(dp(MARK_ROOM * mark_share))
                    .child(label.clone()),
            )
            .children(mark)
    }

    /// A tab, `left` from what holds it.
    fn tab(
        &self,
        tab: &Placed,
        left: f32,
        label: &SharedString,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let key = tab.key.clone();
        let current = self.active.as_ref() == Some(&key);
        let (hovered, clicked) = (key.clone(), key.clone());
        let tab_drag = |el, navigator: &mut Navigator, cx: &mut Context<Navigator>| {
            navigator.tab_drag(el, &key, label.clone(), cx)
        };
        // Off the drag area, as `above_drag` is, but the strip behind still scrolls.
        self.face(tab, left, label, false, now, cx)
            .id(key.element_id("tab"))
            .block_mouse_except_scroll()
            .role(Role::Tab)
            .aria_label(label.clone())
            .aria_selected(current)
            .on_hover(cx.listener(move |this, on: &bool, window, cx| {
                this.tab_hovered(&hovered, *on, window, cx)
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                // What's being placed stays in its card.
                if this.store.read(cx).moving.is_some() {
                    return;
                }
                if let Some(selection) = this.landing(&clicked, cx) {
                    this.app.update_later(window, cx, move |app, window, cx| {
                        app.select(selection, window, cx)
                    });
                }
            }))
            .map(|el| (self.navigator).update(cx, |navigator, cx| tab_drag(el, navigator, cx)))
            .map(|el| Self::takes_drops(el, cx))
    }

    /// Let `el` take a project or a linked worktree dragged along the tabs where it's shown, as
    /// the strip does: a tab keeps the pointer from the strip under it.
    fn takes_drops<E: InteractiveElement>(el: E, cx: &mut Context<Self>) -> E {
        el.on_drop(cx.listener(|this, drag: &ProjectDrag, window, cx| {
            (this.navigator).update(cx, |navigator, cx| navigator.drop_project(drag, window, cx))
        }))
        .on_drop(cx.listener(|this, drag: &Dragged, window, cx| {
            (this.navigator).update(cx, |navigator, cx| {
                navigator.drop_worktree(drag, window, cx)
            })
        }))
    }

    /// How far along the tabs the pointer of `event` is, while it's over the strip.
    fn strip_x<T>(&self, event: &DragMoveEvent<T>, cx: &App) -> Option<f32> {
        let at = event.event.position;
        if !event.bounds.contains(&at) {
            return None;
        }
        let scale = Theme::get(cx).scale;
        let scrolled = -f32::from(self.scroll.offset().x) / scale;
        Some(f32::from(at.x - event.bounds.left()) / scale + scrolled - OUTLINE)
    }

    /// Show the project dragged by its tab where letting go puts it, as its card dragged by its
    /// heading shows ([`Navigator::follow_project_drag`]): in the place along the strip where its
    /// tab would be nearest the pointer, the others sliding to make room, as moving it with the
    /// keys does. Off the strip letting go leaves it be, so it shows where it is.
    fn follow_project_drag(
        &mut self,
        event: &DragMoveEvent<ProjectDrag>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let drag = event.drag(cx);
        if !drag.reorders {
            return;
        }
        let dragged = drag.id.clone();
        let projects: Vec<(&ProjectId, f32)> = (self.rest.tabs.iter())
            .filter_map(|tab| match &tab.key {
                TabKey::Project(id) => Some((id, tab.left)),
                TabKey::Checkout(..) | TabKey::More => None,
            })
            .collect();
        let more = (self.rest.place(&TabKey::More)).map_or(self.rest.end, |tab| tab.left);
        // Each with its worktrees, up to the gap before what comes next.
        let ends = (projects.iter().skip(1).map(|(_, left)| *left)).chain([more]);
        let groups: Vec<(&ProjectId, f32, f32)> = (projects.iter().zip(ends))
            .map(|((id, left), end)| (*id, *left, end - left - PROJECT_GAP))
            .collect();
        let width = (self.rest.place(&TabKey::Project(dragged.clone()))).map(|tab| tab.width);
        let slot = (self.strip_x(event, cx).zip(width)).map(|(x, width)| {
            let start = groups.first().map_or(0., |(_, left, _)| *left);
            let others: Vec<(ProjectId, f32)> = (groups.iter())
                .filter(|(id, ..)| **id != dragged)
                .map(|(id, _, width)| ((*id).clone(), *width))
                .collect();
            nearest_slot(&others, start, PROJECT_GAP, width, x)
        });
        (self.navigator).update(cx, |navigator, cx| navigator.hold_card(&dragged, slot, cx));
    }

    /// Show the linked worktree dragged by its tab where letting go puts it, among those listed
    /// with it, as its heading dragged in the rows shows it ([`Navigator::follow_worktree_drag`]).
    fn follow_worktree_drag(
        &mut self,
        event: &DragMoveEvent<Dragged>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let navigator = self.navigator.read(cx);
        let Some((project, name)) = navigator.dragged_worktree.clone() else {
            return;
        };
        let store = self.store.read(cx);
        let order = (moving_worktree(store, &project, name.as_ref()).cloned())
            .or_else(|| listed_with(store, &project, name.as_ref()));
        let tabs: Vec<(&bstr::BString, f32, f32)> = (self.rest.tabs.iter())
            .filter_map(|tab| match &tab.key {
                TabKey::Checkout(id, Checkout::Linked(n))
                    if *id == project && order.as_ref()?.contains(n) =>
                {
                    Some((n, tab.left, tab.width))
                }
                TabKey::Checkout(..) | TabKey::Project(_) | TabKey::More => None,
            })
            .collect();
        let width = (tabs.iter()).find_map(|(n, _, width)| (**n == name).then_some(*width));
        let slot = (self.strip_x(event, cx).zip(width)).map(|(x, width)| {
            let start = tabs.first().map_or(0., |(_, left, _)| *left);
            let others: Vec<(bstr::BString, f32)> = (tabs.iter())
                .filter(|(n, ..)| **n != name)
                .map(|(n, _, width)| ((*n).clone(), *width))
                .collect();
            nearest_slot(&others, start, WORKTREE_GAP, width, x)
        });
        (self.navigator).update(cx, |navigator, cx| {
            navigator.hold_worktree(&project, name.as_ref(), slot, cx)
        });
    }

    /// The project or the worktree itself selected, rather than a row in its card, shows as a
    /// selected row does, inside its tab's ground `at` and going where that goes: solid while the
    /// keys are on the tabs, as the rows' is while they're on the rows, the tabs it's over
    /// showing through as they would on it, so it can glide over them.
    fn highlight(
        &self,
        (left, width): (f32, f32),
        laid: &Laid,
        labels: &HashMap<TabKey, SharedString>,
        now: Instant,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<(Div, bool)> {
        let selection = self.active.as_ref()?.selection()?;
        if self.store.read(cx).selection.as_ref() != Some(&selection) {
            return None;
        }
        let solid = self.keyed.is_some();
        let (top, left) = (TAB_TOP + SELECTION_INSET, left + SELECTION_INSET);
        // As the tabs are drawn: a worktree's inside what's shown of its project's, but for one
        // being moved, which is drawn over them. What's being moved covers the tabs it passes,
        // so only it shows through.
        let held = (self.held_tab.as_ref()).filter(|_| self.held_active());
        let shows = |key: &TabKey| match held {
            Some(TabKey::Project(id)) => key.project() == Some(id),
            Some(held) => key == held,
            None => true,
        };
        let mut faces = Vec::new();
        for tab in laid.tabs.iter().filter(|tab| solid && shows(&tab.key)) {
            let Some(label) = labels.get(&tab.key) else {
                continue;
            };
            let shown_in = match &tab.key {
                TabKey::Checkout(id, _) if self.held_tab.as_ref() != Some(&tab.key) => {
                    laid.opened.iter().find(|opened| opened.id == *id)
                }
                TabKey::Project(_) | TabKey::Checkout(..) | TabKey::More => None,
            };
            faces.push(match shown_in {
                Some(opened) => {
                    let left = tab.left - opened.worktrees_left + self.reorder.offset(&tab.key);
                    let project = TabKey::Project(opened.id.clone());
                    div()
                        .absolute()
                        .top_0()
                        .h(dp(HEADER_HEIGHT))
                        .left(dp(opened.worktrees_left + self.slid(&project)))
                        .w(dp(opened.shown))
                        .overflow_hidden()
                        .child(self.face(tab, left, label, true, now, cx))
                }
                None => self.face(tab, tab.left + self.slid(&tab.key), label, true, now, cx),
            });
        }
        let shown = div()
            .absolute()
            .top(dp(top))
            .h(dp(TAB_HEIGHT - 2. * SELECTION_INSET))
            .left(dp(left))
            .w(dp((width - 2. * SELECTION_INSET).max(0.)))
            .rounded(dp(4.))
            .overflow_hidden()
            .bg(match solid {
                true => theme.fill_gray,
                false => theme.selected_unfocused(),
            })
            .child(
                div()
                    .absolute()
                    .top(dp(-top))
                    .left(dp(-left))
                    .children(faces),
            );
        Some((shown, solid))
    }
}

impl Render for NavigatorTabs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let now = clock::now();
        let projects = self.watched(cx);
        self.follow_selection(&projects, now, cx);
        // First, as the keys going to what's being held leave the card down.
        self.follow_hold(window, cx);
        self.follow_keys(window, cx);
        self.follow_move(window, cx);
        let key = (self.card.as_ref()).filter(|card| card.open);
        let key = key.map(|card| card.pane.key.clone());
        (self.navigator).update(cx, |navigator, _| navigator.set_card_key(key));
        // On until they've stopped, as what lands ends the hold just as the rows start closing up.
        let rows_sliding = self.navigator.read(cx).reorder.any();
        if self.keyed.is_some() || self.held || rows_sliding {
            let (sliding, reveal) =
                (self.navigator).update(cx, |navigator, cx| navigator.frame_for_card(window, cx));
            if sliding {
                window.request_animation_frame();
            }
            if let Some((row, card)) = reveal.zip(self.card.as_ref()) {
                card.pane.scroll.scroll_to_item(row);
            }
        }
        let (groups, mut labels) = self.groups(&projects, now, window, cx);
        self.follow_marks(&groups, now, cx);
        let more_label = SharedString::from(drawer_title(projects.is_empty()));
        let more = self.tab_width(&more_label, window, cx)
            + MARK_ROOM * self.mark_share(&TabKey::More, now);
        labels.insert(TabKey::More, more_label);
        let laid = lay_out(&groups, more, |id| self.share(id, now));
        self.order = laid.tabs.iter().map(|tab| tab.key.clone()).collect();
        self.follow_order(&laid, cx);
        // As the tabs will be once they've stopped, which what goes to a tab aims for.
        let rest = lay_out(&groups, more, |id| if self.opens(id) { 1. } else { 0. });
        self.glide_scroll(&rest, now, cx);
        let rest_offset = self.scroll_at_rest(&rest, cx);

        let placing = self.store.read(cx).moving.as_ref();
        let keys_context = match placing.map(|moving| moving.op) {
            Some(Op::Squash(_)) => "Tabs Navigator Moving Squashing",
            Some(Op::Commit | Op::Move | Op::CherryPick) => "Tabs Navigator Moving",
            None => "Tabs Navigator",
        };

        let top_left = crate::menu_bar::top_left_room(window, &self.store.read(cx).prefs, &theme);
        let strip_left = top_left.map_or(CARD_MARGIN, |room| f32::from(room) / theme.scale);
        let card = self.draw_card(&rest, rest_offset, strip_left, now, window, cx);

        // The ground behind the tab the selection's in.
        // With the keys going along the tabs, the card hanging from that tab is on its way there
        // too, and the tab's reach is that ground: one thing, not two at their own pace. A click
        // on a tab the card's at already has the ground glide over to it.
        let card_key = self.card.as_ref().map(|card| &card.pane.key);
        let under_card =
            (card.as_ref()).filter(|_| self.keyed.is_some() && card_key == self.active.as_ref());
        let offset = -f32::from(self.scroll.offset().x) / theme.scale;
        let ground = (self.active.as_ref())
            .and_then(|key| rest.place(key))
            .map(|tab| {
                let at_rest = tab.left - rest_offset + offset + self.slid(&tab.key);
                let (left, width) = match under_card {
                    Some(card) => (card.left, card.width),
                    None => (
                        self.active_left.at(at_rest, now),
                        self.active_width.at(tab.width, now),
                    ),
                };
                self.active_drawn = Some((left, width));
                (left, width)
            });
        let highlight = ground.and_then(|at| self.highlight(at, &laid, &labels, now, &theme, cx));
        self.rest = rest;
        // A solid one shows the tabs it's over itself, so it goes over them; a faint one goes
        // under them, as it doesn't.
        let (highlight_under, highlight_over) = match highlight {
            Some((shown, true)) => (None, Some(shown)),
            Some((shown, false)) => (Some(shown), None),
            None => (None, None),
        };
        let slid = |id: &ProjectId| self.slid(&TabKey::Project(id.clone()));
        // The tab being moved goes over the tabs it passes, a project's with the worktrees that
        // go with it, on the strip's ground so those don't show through, as the side's card does
        // over the cards it passes, and edged as it is. With the selection in it, its ground goes
        // too ([`Self::held_active`]).
        let held = self.held_tab.as_ref();
        let held_project = match held {
            Some(TabKey::Project(id)) => Some(id),
            Some(TabKey::Checkout(..) | TabKey::More) | None => None,
        };
        let is_held = |id: Option<&ProjectId>| id.is_some() && id == held_project;
        let mut outlines = (Vec::new(), Vec::new());
        for opened in &laid.opened {
            let left = opened.left - OUTLINE + slid(&opened.id);
            let width = opened.worktrees_left - opened.left + opened.shown + 2. * OUTLINE;
            let outline = div()
                .absolute()
                .top(dp(TAB_TOP - OUTLINE))
                .h(dp(TAB_HEIGHT + 2. * OUTLINE))
                .left(dp(left))
                .w(dp(width))
                .rounded(dp(8.))
                .bg(theme.hover())
                .opacity(opened.share);
            match is_held(Some(&opened.id)) {
                true => outlines.1.push(outline),
                false => outlines.0.push(outline),
            }
        }
        let ground = ground.map(|(left, width)| {
            div()
                .absolute()
                .top(dp(TAB_TOP))
                .h(dp(TAB_HEIGHT))
                .left(dp(left))
                .w(dp(width))
                .rounded(dp(6.))
                .bg(theme.bg_1)
                .border_1()
                .border_color(theme.border_2)
        });
        let neck = card.as_ref().map(|card| {
            div()
                .absolute()
                .top(dp(TAB_TOP))
                .h(dp(HEADER_HEIGHT - TAB_TOP))
                .left(dp(card.left))
                .w(dp(card.width))
                .rounded_t(dp(6.))
                .bg(theme.bg_1)
                .border_t_1()
                .border_l_1()
                .border_r_1()
                .border_color(theme.border_2)
                .opacity(card.fade)
        });
        let (ground, ground_over) = match self.held_active() {
            true => ((None, None, None), (ground, neck, highlight_under)),
            false => ((ground, neck, highlight_under), (None, None, None)),
        };
        let held_edges = held
            .and_then(|key| Some((key, laid.place(key).filter(|tab| tab.key == *key)?)))
            .map(|(key, tab)| {
                // A project's reaches past its worktrees.
                let worktrees =
                    (laid.opened.iter()).find(|opened| held_project == Some(&opened.id));
                let right = worktrees.map_or(tab.left + tab.width, |w| w.worktrees_left + w.shown);
                let edges = || {
                    div()
                        .absolute()
                        .top(dp(TAB_TOP - OUTLINE))
                        .h(dp(TAB_HEIGHT + 2. * OUTLINE))
                        .left(dp(tab.left - OUTLINE + self.slid(key)))
                        .w(dp(right - tab.left + 2. * OUTLINE))
                        .rounded(dp(8.))
                };
                (
                    edges().bg(theme.bg_2),
                    edges().border_1().border_color(theme.pop),
                )
            });
        let (backdrop, edge) = held_edges.unzip();
        let mut project_tabs = (Vec::new(), Vec::new());
        for tab in (laid.tabs.iter()).filter(|tab| !matches!(tab.key, TabKey::Checkout(..))) {
            let left = tab.left + self.slid(&tab.key);
            let Some(label) = labels.get(&tab.key) else {
                continue;
            };
            let el = self.tab(tab, left, label, now, cx);
            match is_held(tab.key.project()) {
                true => project_tabs.1.push(el),
                false => project_tabs.0.push(el),
            }
        }
        // Worktrees come out from behind their project's tab, inside what's shown of them, but
        // for one being moved, which goes over them.
        let mut worktree_tabs = (Vec::new(), Vec::new());
        let mut held_worktree = None;
        for opened in &laid.opened {
            let mut tabs = Vec::new();
            for tab in (laid.tabs.iter())
                .filter(|tab| matches!(&tab.key, TabKey::Checkout(id, _) if *id == opened.id))
            {
                let Some(label) = labels.get(&tab.key) else {
                    continue;
                };
                if held == Some(&tab.key) {
                    let left = tab.left + self.slid(&tab.key);
                    held_worktree = Some(self.tab(tab, left, label, now, cx));
                    continue;
                }
                let left = tab.left - opened.worktrees_left + self.reorder.offset(&tab.key);
                tabs.push(self.tab(tab, left, label, now, cx));
            }
            let shown = div()
                .absolute()
                .top_0()
                .h(dp(HEADER_HEIGHT))
                .left(dp(opened.worktrees_left + slid(&opened.id)))
                .w(dp(opened.shown))
                .overflow_hidden()
                .children(tabs);
            match is_held(Some(&opened.id)) {
                true => worktree_tabs.1.push(shown),
                false => worktree_tabs.0.push(shown),
            }
        }
        let strip = div()
            .relative()
            .flex_none()
            .h(dp(HEADER_HEIGHT))
            .w(dp(laid.end + 2. * OUTLINE))
            .left(dp(OUTLINE))
            .children(outlines.0)
            .children(ground.0)
            .children(ground.1)
            .children(ground.2)
            .children(project_tabs.0)
            .children(worktree_tabs.0)
            .children(backdrop)
            .children(outlines.1)
            .children(ground_over.0)
            .children(ground_over.1)
            .children(ground_over.2)
            .children(project_tabs.1)
            .children(worktree_tabs.1)
            .children(held_worktree)
            .children(highlight_over)
            .children(edge);

        let viewport = f32::from(self.scroll.bounds().size.width) / theme.scale;
        let more_before = offset > 0.5;
        let more_after = laid.end + 2. * OUTLINE - offset - viewport > 0.5;
        self.overflows = more_before || more_after;
        let fade = |angle: f32| {
            let (ground, clear) = (theme.bg_2, theme.bg_2.opacity(0.));
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .w(dp(FADE))
                .bg(linear_gradient(
                    angle,
                    linear_color_stop(ground, 0.),
                    linear_color_stop(clear, 1.),
                ))
        };
        let scroller = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                div()
                    .id("navigator-tabs")
                    .role(Role::TabList)
                    .size_full()
                    .flex()
                    .overflow_x_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll_wheel(
                        cx.listener(|this, _, window, cx| this.strip_scrolled(window, cx)),
                    )
                    .on_drag_move(cx.listener(Self::follow_project_drag))
                    .on_drag_move(cx.listener(Self::follow_worktree_drag))
                    .map(|el| Self::takes_drops(el, cx))
                    .child(strip),
            )
            .when(more_before, |el| el.child(fade(90.).left_0()))
            .when(more_after, |el| el.child(fade(270.).right_0()));
        let controls = self
            .navigator
            .update(cx, |navigator, cx| navigator.strip_controls(cx));

        let moving = (self.card.as_ref()).is_some_and(|card| {
            card.slide.openness(now).is_some()
                || card.left.moving(now)
                || card.height.moving(now)
                || !card.leaving.is_empty()
                || card.pane.x.moving(now)
        }) || !self.opening.is_empty()
            || self.filter_slide.openness(now).is_some()
            || self.scrolling
            || self.active_left.moving(now)
            || self.reorder.any()
            || (self.marks.values()).any(|mark| mark.slide.openness(now).is_some())
            || self.navigator.read(cx).folds.any();
        if moving {
            window.request_animation_frame();
        }

        h_flex()
            .size_full()
            .relative()
            .bg(theme.bg_2)
            .map(crate::ui::drags_window)
            .pl(top_left.unwrap_or(theme.scaled(CARD_MARGIN)))
            .pr(dp(8.))
            .when(crate::ui::draws_window_controls(window), |el| {
                el.pr(fixed(crate::ui::WINDOW_CONTROLS_WIDTH + 8.))
            })
            .gap(dp(6.))
            .child(self.watch_pointer(cx))
            // Of no size, so a click on the tabs doesn't give the keys here, and keys here
            // reach what's bound around it. What moves along the tabs is `AppView::move_keys`.
            .child(
                div()
                    .absolute()
                    .size_0()
                    .track_focus(&self.focus)
                    .key_context(keys_context)
                    .map(|el| {
                        (self.navigator).update(cx, |navigator, cx| navigator.key_actions(el, cx))
                    })
                    .on_action(
                        cx.listener(|this, _: &SelectNext, window, cx| {
                            this.step_row(1, window, cx)
                        }),
                    )
                    .on_action(cx.listener(|this, _: &SelectPrevious, window, cx| {
                        this.step_row(-1, window, cx)
                    }))
                    .on_action(cx.listener(|this, action: &Expand, window, cx| {
                        this.fold_row(|nav, window, cx| nav.expand(action, window, cx), window, cx)
                    }))
                    .on_action(cx.listener(|this, action: &Collapse, window, cx| {
                        this.fold_row(
                            |nav, window, cx| nav.collapse(action, window, cx),
                            window,
                            cx,
                        )
                    }))
                    .on_action(cx.listener(|this, _: &ToggleFold, window, cx| {
                        this.leave_to_page(window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                        // A move under way lets go first, as it does in the navigator.
                        match this.store.read(cx).moving.is_some() {
                            true => cx.propagate(),
                            false => this.leave_to_page(window, cx),
                        }
                    })),
            )
            .child(scroller)
            .children(controls)
            .children(card.into_iter().flat_map(|card| card.elements))
    }
}

impl Navigator {
    /// Let tab `el`, naming `key`, drag what its heading in the rows does: a watched project,
    /// its card among the others in the manual order, or a linked worktree among those listed
    /// with it, the tabs sliding to make room ([`NavigatorTabs::follow_project_drag`]).
    fn tab_drag(
        &self,
        el: Stateful<Div>,
        key: &TabKey,
        label: SharedString,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let Some(item) = key.selection() else {
            return el;
        };
        match key {
            TabKey::Project(_) => {
                let manual = self.store.read(cx).prefs.watched_order == WatchedOrder::Manual;
                self.project_drag(el, &item, label, manual, cx)
            }
            TabKey::Checkout(_, Checkout::Linked(name)) => {
                self.worktree_drag(el, &item, name.as_ref(), label, cx)
            }
            TabKey::Checkout(_, Checkout::Main) | TabKey::More => el,
        }
    }

    /// The rows of what `scope` names, as they're built for the cards, whether or not the
    /// navigator has them folded away: only its own folds fold, and a checkout's tab means its
    /// rows.
    pub(super) fn scope_rows(&self, scope: &TabKey, cx: &App) -> Rows {
        let store = self.store.read(cx);
        let mut rows = Rows::default();
        let found = match scope {
            TabKey::Project(id) => (store.project(id)).map(|entry| {
                self.card_rows(store, entry, &mut rows);
            }),
            TabKey::Checkout(id, checkout) => store.workspace(id).map(|workspace| match checkout {
                Checkout::Main => self.main_checkout_rows(store, id, workspace, &mut rows),
                Checkout::Linked(name) => self.linked_checkout_rows(store, id, name, &mut rows),
            }),
            TabKey::More => {
                rows.drawer = self.drawer_listing(store);
                return rows;
            }
        };
        if found.is_none() {
            rows.push(Row::Note("Loading…".into(), false));
        }
        if matches!(rows.rows.last(), Some(Row::WorktreeDivider)) {
            rows.truncate(rows.rows.len() - 1);
        }
        if rows.rows.is_empty() {
            rows.push(Row::Note("Nothing here yet".into(), false));
        }
        rows
    }

    /// Run `act` with the rows of `scope`, as `build_rows` builds them now, standing in for the
    /// navigator's own, which it isn't drawing here, so what works on its rows works on the card's.
    pub(super) fn in_scope<R>(
        &mut self,
        scope: &TabKey,
        cx: &mut Context<Self>,
        act: impl FnOnce(&mut Self, &mut Context<Self>) -> R,
    ) -> R {
        let outer = self.scope.replace(scope.clone());
        self.folds.frame();
        let mut built = self.build_rows(cx);
        if self.follow_folds(&built.folds, cx) {
            built = self.build_rows(cx);
        }
        let Rows {
            rows,
            insets,
            guides,
            moving,
            drawer,
            ..
        } = built;
        // The rows of the tab the selection's in are the navigator's own, keys and all.
        let own = self.top_scope(cx).as_ref() == Some(scope);
        let held_card = self.held_card.take();
        let held_worktree = self.held_worktree.take();
        let focused_card = self.focused_card.take();
        let keys = (!own).then(|| std::mem::take(&mut self.row_keys));
        let rows = std::mem::replace(&mut self.rows, rows);
        let insets = std::mem::replace(&mut self.insets, insets);
        let guides = std::mem::replace(&mut self.guides, guides);
        let moving = std::mem::replace(&mut self.moving, moving);
        let drawer = std::mem::replace(&mut self.drawer, drawer);
        let done = act(self, cx);
        self.rows = rows;
        self.insets = insets;
        self.guides = guides;
        self.moving = moving;
        self.drawer = drawer;
        if let Some(keys) = keys {
            self.row_keys = keys;
        }
        self.focused_card.replace(focused_card);
        self.held_worktree = held_worktree;
        self.held_card = held_card;
        self.scope = outer;
        done
    }

    /// The rows of what `scope` names, drawn as they are in a card but without its edges, for
    /// the card under a tab: the projects not watched under the drawer's bar, with no fold.
    pub(super) fn scoped_rows(
        &mut self,
        scope: &TabKey,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.scoped_keys = window.focused(cx);
        self.in_scope(scope, cx, |this, cx| {
            if *scope != TabKey::More {
                return (0..this.rows.len())
                    .map(|ix| this.row_in_card(ix, window, cx))
                    .collect();
            }
            let theme = Theme::get(cx).clone();
            let alone = this.watched(this.store.read(cx)).is_empty();
            let bar = this
                .drawer_bar(alone, window, &theme, cx)
                .into_any_element();
            let rows = (0..this.drawer.rows.len()).map(|ix| this.render_drawer_row(ix, window, cx));
            std::iter::once(bar).chain(rows).collect()
        })
    }

    /// Step the selection `delta` through what `scope` shows, as a step through the navigator's
    /// rows does, or from its first row back up to what the tab selects, and say which of the
    /// card's rows that is.
    pub(super) fn step_in(
        &mut self,
        scope: &TabKey,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.in_scope(scope, cx, |this, cx| {
            let first = this.items().first().map(|item| (*item).clone());
            let selected = this.store.read(cx).selection.clone();
            let moving = this.store.read(cx).moving.is_some();
            match (
                delta < 0 && !moving && first.is_some() && first == selected,
                scope.selection(),
            ) {
                (true, Some(tab)) => this.select(tab, window, cx),
                (true, None) => {}
                (false, _) => this.step(delta, window, cx),
            }
            let selected = this.store.read(cx).selection.clone()?;
            let row = |rows: &[Row]| {
                let at = |row: &Row| matches!(row, Row::Item(item) if *item == selected);
                rows.iter().position(at)
            };
            // The drawer's bar comes before its rows.
            row(&this.rows).or_else(|| row(&this.drawer.rows).map(|row| row + 1))
        })
    }

    /// What the navigator's header has but its filter, at the strip's right end.
    pub(super) fn strip_controls(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = Theme::get(cx).clone();
        let update = self.update_button(cx);
        update
            .into_iter()
            .chain([self.view_button(cx), self.settings_button(&theme, cx)])
            .map(|el| above_drag(div().flex_none().child(el)).into_any_element())
            .collect()
    }

    /// What a press on a row gives the keys: the navigator, but while the rows are in a card,
    /// what has them already.
    pub(super) fn pressed_keys(&self) -> &FocusHandle {
        self.scoped_keys.as_ref().unwrap_or(&self.focus)
    }

    /// Whether the navigator can take the keys: where it's drawn, which across the top it isn't,
    /// its tabs and the card under them taking them.
    pub(super) fn takes_keys(&self, cx: &App) -> bool {
        self.floating || self.store.read(cx).prefs.navigator_place != NavigatorPlace::Top
    }
}

#[cfg(test)]
mod tests {
    use gpui::point;

    use super::{
        Group, Laid, PROJECT_GAP, TabKey, WORKTREE_GAP, heading_down, lay_out, nearest_slot,
        scrolled_to, stepped,
    };
    use crate::backend::Checkout;
    use crate::state::ProjectId;
    use crate::ui::fixed;

    fn group(id: &str, worktrees: &[f32]) -> Group {
        let checkouts = worktrees.iter().map(|w| (Checkout::Main, *w)).collect();
        Group {
            id: id.into(),
            width: 100.,
            worktrees: checkouts,
        }
    }

    fn left_of(laid: &Laid, id: &str) -> f32 {
        let key = TabKey::Project(ProjectId::from(id));
        laid.place(&key).map_or(f32::NAN, |tab| tab.left)
    }

    #[test]
    fn a_project_opening_out_moves_what_follows_it_over_by_as_much_as_shows() {
        let groups = [group("a", &[50., 60.]), group("b", &[])];
        let whole = 50. + 60. + 2. * WORKTREE_GAP;
        let shut = lay_out(&groups, 90., |_| 0.);
        assert_eq!(
            left_of(&shut, "b"),
            100. + PROJECT_GAP,
            "shut, its tab is all it takes"
        );
        let half = lay_out(&groups, 90., |id| if id.as_ref() == "a" { 0.5 } else { 0. });
        assert_eq!(
            left_of(&half, "b"),
            100. + whole / 2. + PROJECT_GAP,
            "halfway, the next tab has moved over halfway"
        );
        let open = lay_out(&groups, 90., |id| if id.as_ref() == "a" { 1. } else { 0. });
        assert_eq!(
            open.end,
            100. + whole + PROJECT_GAP + 100. + PROJECT_GAP + 90.,
            "open, the strip ends past all of it"
        );
    }

    #[test]
    fn worktrees_slide_out_from_behind_their_project() {
        let groups = [group("a", &[50.])];
        let half = lay_out(&groups, 90., |_| 0.5);
        let whole = 50. + WORKTREE_GAP;
        let tab = half.tabs.get(1).map_or(f32::NAN, |tab| tab.left);
        assert_eq!(
            tab,
            100. + whole / 2. - whole + WORKTREE_GAP,
            "what isn't out yet is still behind the project's tab"
        );
    }

    #[test]
    fn a_pointer_sweeping_along_the_tabs_is_not_heading_for_the_card() {
        assert!(
            !heading_down(point(fixed(30.), fixed(4.))),
            "the card follows it from tab to tab with no wait"
        );
        assert!(
            !heading_down(point(fixed(-30.), fixed(0.))),
            "going back along them too"
        );
    }

    #[test]
    fn a_pointer_going_down_more_than_across_is_heading_for_the_card() {
        assert!(
            heading_down(point(fixed(8.), fixed(20.))),
            "the tab it cuts the corner of doesn't take the card from under it"
        );
        assert!(
            heading_down(point(fixed(24.), fixed(10.))),
            "the way to a row further along the card counts too"
        );
        assert!(
            !heading_down(point(fixed(0.), fixed(-20.))),
            "coming up onto the tabs, it's after the one it reaches"
        );
    }

    fn strip() -> [TabKey; 5] {
        [
            TabKey::Project("a".into()),
            TabKey::Project("b".into()),
            TabKey::Checkout("b".into(), Checkout::Main),
            TabKey::Project("c".into()),
            TabKey::More,
        ]
    }

    #[test]
    fn a_step_goes_to_the_tab_beside_it_worktrees_included() {
        let [a, b, main, ..] = strip();
        assert_eq!(
            stepped(&strip(), Some(&b), true, false),
            Some(main),
            "a project's worktrees come after it"
        );
        assert_eq!(
            stepped(&strip(), Some(&b), false, false),
            Some(a),
            "and back along the tabs"
        );
    }

    #[test]
    fn a_step_between_projects_passes_their_worktrees() {
        let [a, b, main, c, more] = strip();
        assert_eq!(
            stepped(&strip(), Some(&b), true, true),
            Some(c.clone()),
            "the worktree after it is passed"
        );
        assert_eq!(
            stepped(&strip(), Some(&main), true, true),
            Some(c.clone()),
            "from a worktree, on to the next project"
        );
        assert_eq!(
            stepped(&strip(), Some(&main), false, true),
            Some(a),
            "and back past its own project's tab to the one before, so a step there and a step \
             back is a round trip"
        );
        assert_eq!(
            stepped(&strip(), Some(&c), true, true),
            Some(more),
            "the projects not watched are a stop too"
        );
    }

    #[test]
    fn a_step_from_a_tab_gone_goes_on_from_its_projects() {
        let [a, b, _, c, _] = strip();
        let gone = TabKey::Checkout("b".into(), Checkout::Linked("gone".into()));
        assert_eq!(
            stepped(&strip(), Some(&gone), false, false),
            Some(a),
            "the keys aren't left with nowhere to step from"
        );
        assert_eq!(
            stepped(&strip(), Some(&gone), true, true),
            Some(c),
            "by project too"
        );
        assert_eq!(
            b,
            TabKey::Project("b".into()),
            "which is the tab it stands in for"
        );
    }

    #[test]
    fn a_step_stops_at_the_ends() {
        let [a, .., more] = strip();
        assert_eq!(stepped(&strip(), Some(&a), false, false), None, "the first");
        assert_eq!(
            stepped(&strip(), Some(&more), true, false),
            None,
            "the last"
        );
    }

    #[test]
    fn a_dragged_tab_goes_where_its_middle_would_be_nearest_the_pointer() {
        use crate::transfer::Slot;
        // A 40 wide tab among two others, 100 and 200 wide, 10 apart: its middle would be at 20
        // before the first, 130 before the second and 340 after them both.
        let others = [("a", 100.), ("b", 200.)];
        let slot = |x| nearest_slot(&others, 0., 10., 40., x);
        assert_eq!(slot(0.), Slot::Before("a"), "off the start it goes first");
        assert_eq!(slot(74.), Slot::Before("a"), "nearer 20 than 130");
        assert_eq!(slot(76.), Slot::Before("b"), "nearer 130 than 20");
        assert_eq!(slot(234.), Slot::Before("b"), "nearer 130 than 340");
        assert_eq!(slot(236.), Slot::End, "nearer 340 than 130");
        assert_eq!(slot(1000.), Slot::End, "past the end it goes last");
        // Only the others set the places, wherever `start` puts them.
        let shifted = nearest_slot(&others, 50., 10., 40., 126.);
        assert_eq!(
            shifted,
            Slot::Before("b"),
            "130 + 50 is nearer than 20 + 50"
        );
    }

    #[test]
    fn the_projects_not_watched_come_after_the_rest() {
        let groups = [group("a", &[50.]), group("b", &[])];
        let laid = lay_out(&groups, 90., |_| 1.);
        let more = laid.place(&TabKey::More).map_or(f32::NAN, |tab| tab.left);
        assert_eq!(
            more,
            left_of(&laid, "b") + 100. + PROJECT_GAP,
            "their tab follows the last project's"
        );
        assert_eq!(laid.end, more + 90., "and the strip ends with it");
    }

    #[test]
    fn a_project_kept_open_leaves_the_tab_clicked_after_it_where_it_was() {
        let groups = [group("a", &[50., 60.]), group("b", &[70.])];
        let before = lay_out(&groups, 90., |id| if id.as_ref() == "a" { 1. } else { 0. });
        for b in [0.25, 0.5, 1.] {
            let opening = lay_out(&groups, 90., |id| if id.as_ref() == "a" { 1. } else { b });
            assert_eq!(
                left_of(&opening, "b"),
                left_of(&before, "b"),
                "its worktrees open past it, and nothing before it changes"
            );
        }
    }

    #[test]
    fn scrolling_to_a_span_moves_only_as_far_as_it_takes() {
        assert_eq!(
            scrolled_to(&[(10., 90.)], 100., 0., 500.),
            0.,
            "what shows already stays"
        );
        assert_eq!(
            scrolled_to(&[(200., 260.)], 100., 0., 500.),
            160.,
            "what's past the right end comes in to it"
        );
        assert_eq!(
            scrolled_to(&[(20., 60.)], 100., 150., 500.),
            20.,
            "what's past the left end comes in to it"
        );
    }

    #[test]
    fn a_span_too_wide_to_show_starts_at_its_left() {
        assert_eq!(
            scrolled_to(&[(50., 400.)], 100., 0., 500.),
            50.,
            "the group's own tab is what's kept in view"
        );
    }

    #[test]
    fn the_last_span_wins_when_they_cannot_both_show() {
        assert_eq!(
            scrolled_to(&[(50., 400.), (300., 360.)], 100., 0., 500.),
            260.,
            "the active tab shows even where its group can't"
        );
    }
}
