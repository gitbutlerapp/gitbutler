//! The terminal overview under the navigator: every terminal open on every project, tiled or
//! not, what wants the user first and what's under way last. They go under a heading for each
//! status the terminal picker groups them by, for each project, or for each project after one
//! for those wanting the user, or under none, as its bar's menu picks. The headings fold as the
//! navigator's sections do, and stepping through the terminals shows each where it is, as the
//! navigator's rows do.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::time::Instant;

use gpui::{
    AnyElement, App, ClickEvent, Context, ElementId, Entity, FocusHandle, Focusable, ListAlignment,
    ListState, MouseButton, MouseDownEvent, Role, Subscription, Window, div, list, prelude::*,
};

use super::cursor::{Step, Stops, step_to};
use super::keys::{List, NavigatorKeys};
use super::rename::{on_terminal_name, terminal_name_input};
use super::rows::CARD_PADDING;
use super::{
    CARD_BAR_HEIGHT, CARD_BAR_INSET, DRAWER_BLOCK, DRAWER_GAP, DRAWER_ROW, RowStyle, card_bar,
    drawer_bar, on_hover, status_mark,
};
use crate::app::{
    AppHandle, AppView, Cancel, Confirm, FocusNavigator, SelectNext, SelectPrevious, SplitBelow,
    SplitRight,
};
use crate::backend::{self, Checkout};
use crate::edits::{Discard, RenameTerminal};
use crate::overlays::MenuItem;
use crate::palette::StatusGroup;
use crate::state::{Axis, NavigatorPlace, Part, ProjectId, Store, TerminalGrouping};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::ui::fold::{Folds, Place, clipped};
use crate::ui::text_input::TextInput;
use crate::ui::{
    Button, ButtonStyle, EMPTY_TIDY_HEIGHT, Guide, Icon, button, dp, empty_state_tidy, field,
    fixed, group_label, icon_button, terminal_item, v_flex,
};

/// The room around a drawer's empty state in its card, above and below it and at its sides, as
/// but-ui-react's Drawer stories give it.
pub(super) const EMPTY_CARD_PADDING: (f32, f32) = (28., 16.);

/// The room between two rows of the card, as but-ui-react's `TerminalGroup` sets them apart.
const ROW_GAP: f32 = 1.;

/// The room above row `ix` of `items`, which its slot in the list holds: none over the first,
/// a group's padding below, its hairline and the next group's padding above over a heading,
/// as but-ui-react's `TerminalGroup`s are ruled off, and the gap between rows over the rest.
fn room_above(items: &[Item], ix: usize) -> f32 {
    match (ix, items.get(ix)) {
        (0, _) | (_, None) => 0.,
        (_, Some(Item::Heading(_))) => CARD_PADDING + 1. + CARD_PADDING,
        (_, Some(Item::Terminal(_))) => ROW_GAP,
    }
}

/// The statuses in the order they're listed: what wants the user first, and what's under way
/// last, as it needs nothing until it's done.
const ORDER: [StatusGroup; 5] = [
    StatusGroup::Blocked,
    StatusGroup::Failed,
    StatusGroup::Done,
    StatusGroup::Idle,
    StatusGroup::Working,
];

/// What a heading of the overview lists terminals under.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Group {
    Status(StatusGroup),
    Project(ProjectId),
    /// Those that want the user, whatever their project.
    NeedsYou,
}

#[derive(Debug, Clone, PartialEq)]
enum Item {
    /// A group's label, as but-ui-react's `GroupLabel`.
    Heading(Group),
    Terminal(TerminalId),
}

/// A terminal as the overview sorts it: its project, what it's at, and since when.
type Sorted = (TerminalId, ProjectId, StatusGroup, Instant);

/// Whether a terminal at `status` wants the user: it's stopped for them, or stopped.
fn needs_you(status: StatusGroup) -> bool {
    match status {
        StatusGroup::Blocked | StatusGroup::Failed | StatusGroup::Done => true,
        StatusGroup::Idle | StatusGroup::Working => false,
    }
}

/// The heading a terminal of `project` at `status` goes under `by` what, if any does.
fn group_of(by: TerminalGrouping, project: &ProjectId, status: StatusGroup) -> Option<Group> {
    match by {
        TerminalGrouping::Status => Some(Group::Status(status)),
        TerminalGrouping::Project => Some(Group::Project(project.clone())),
        TerminalGrouping::NeedsYouThenProject if needs_you(status) => Some(Group::NeedsYou),
        TerminalGrouping::NeedsYouThenProject => Some(Group::Project(project.clone())),
        TerminalGrouping::Ungrouped => None,
    }
}

/// The overview's rows for `terminals`, in the order they opened, under headings `by` what:
/// statuses in [`ORDER`], projects in the order of `projects`, and those wanting the user
/// first, each with any, but for the terminals of those `shut`. Under each, or with no
/// headings, terminals go by their status in [`ORDER`], and idle ones by when they went idle,
/// the latest first.
fn items_of(
    terminals: impl IntoIterator<Item = Sorted>,
    by: TerminalGrouping,
    projects: &[ProjectId],
    shut: &HashSet<Group>,
) -> Vec<Item> {
    let mut terminals: Vec<Sorted> = terminals.into_iter().collect();
    // Stable, so those alike keep the order they opened in.
    terminals.sort_by_key(|(_, _, status, since)| {
        let idle_since = (*status == StatusGroup::Idle).then_some(Reverse(*since));
        (ORDER.iter().position(|of| of == status), idle_since)
    });
    let projects = projects.iter().cloned().map(Group::Project);
    let groups: Vec<Group> = match by {
        TerminalGrouping::Status => ORDER.into_iter().map(Group::Status).collect(),
        TerminalGrouping::Project => projects.collect(),
        TerminalGrouping::NeedsYouThenProject => {
            std::iter::once(Group::NeedsYou).chain(projects).collect()
        }
        TerminalGrouping::Ungrouped => {
            return (terminals.into_iter())
                .map(|(id, ..)| Item::Terminal(id))
                .collect();
        }
    };
    let mut items = Vec::new();
    for group in groups {
        let at: Vec<TerminalId> = (terminals.iter())
            .filter(|(_, project, status, _)| {
                group_of(by, project, *status).as_ref() == Some(&group)
            })
            .map(|(id, ..)| *id)
            .collect();
        if at.is_empty() {
            continue;
        }
        items.push(Item::Heading(group.clone()));
        if !shut.contains(&group) {
            items.extend(at.into_iter().map(Item::Terminal));
        }
    }
    items
}

/// The menu item grouping the rows `by` what, ticked if it's what's `picked`.
fn group_by(label: &'static str, by: TerminalGrouping, picked: TerminalGrouping) -> MenuItem {
    MenuItem::new(label, move |app: &mut AppView, _, cx| {
        if by != picked {
            backend::metrics::terminal_grouping_changed(by);
        }
        (app.store).update(cx, |store, cx| {
            store.set_prefs(|prefs| prefs.terminal_grouping = by, cx)
        })
    })
    .ticked(picked == by)
}

/// The name project `id` goes by, or its id while it isn't loaded.
fn project_name(store: &Store, id: &ProjectId) -> String {
    store
        .project(id)
        .map_or_else(|| id.to_string(), |p| p.project.name.clone())
}

pub struct TerminalOverview {
    app: AppHandle,
    store: Entity<Store>,
    items: Vec<Item>,
    /// Each row's place in the fold moving it, while one does.
    places: Vec<Option<Place<Group>>>,
    list: ListState,
    /// The pointer is over the rows, which hold their order meanwhile, so what an agent does
    /// moves no row from under it. Each row's mark still follows its terminal.
    hovered: bool,
    /// The card folded to its bar: as it starts with no terminal open, and as the last one
    /// closes, till its chevron opens it or one opens.
    shut: bool,
    /// Whether no terminal was open when the store last changed, to fold or open the card as
    /// that changes.
    was_empty: bool,
    /// What the rows were last sorted out under, to sort them out again at once as it changes.
    grouped_by: TerminalGrouping,
    folded: HashSet<Group>,
    folds: Folds<Group>,
    /// The keys stepped onto a heading, which folds but shows nothing, so the terminal shown
    /// stays as it was: see [`TerminalOverview::step`].
    on_heading: Option<Group>,
    /// The terminal shown when the store last changed, to leave the heading once another is.
    shown: Option<TerminalId>,
    renaming: Option<Renaming>,
    focus: FocusHandle,
    /// Where the navigator's keys are, as found for this frame: see [`super::keys::find_keys`].
    keys: NavigatorKeys,
    /// Its card is sliding out from under its tab, which had the keys, so no row shows as
    /// selected, as it was left, as found for this frame.
    left_unselected: bool,
    _store: Subscription,
}

/// A terminal's name edited in its row, as the navigator's rows edit theirs.
struct Renaming {
    id: TerminalId,
    input: Entity<TextInput>,
    _subscriptions: [Subscription; 2],
}

impl Focusable for TerminalOverview {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TerminalOverview {
    pub fn new(app: AppHandle, store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let _store = cx.observe(&store, |this, store, cx| {
            let store = store.read(cx);
            let shown = store.terminal_shown();
            if shown != this.shown {
                this.shown = shown;
                this.on_heading = None;
            }
            let empty = store.terminals.count() == 0;
            if empty != this.was_empty {
                this.was_empty = empty;
                this.shut = empty;
                backend::metrics::terminal_card_folded(empty);
            }
            cx.notify()
        });
        let empty = store.read(cx).terminals.count() == 0;
        backend::metrics::terminal_card_folded(empty);
        let grouped_by = store.read(cx).prefs.terminal_grouping;
        Self {
            app,
            store,
            items: Vec::new(),
            places: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, fixed(56.)),
            hovered: false,
            shut: empty,
            was_empty: empty,
            grouped_by,
            folded: HashSet::new(),
            folds: Folds::default(),
            on_heading: None,
            shown: None,
            renaming: None,
            focus: cx.focus_handle(),
            keys: NavigatorKeys::Away,
            left_unselected: false,
            _store,
        }
    }

    /// The terminals open, each with its project, what it's at and since when.
    fn sorted(&self, cx: &App) -> Vec<Sorted> {
        (self.store.read(cx).terminals.all())
            .map(|t| (t.id, t.project.clone(), StatusGroup::of(t.status), t.since))
            .collect()
    }

    /// The rows for the terminals open, under the headings the user picked.
    fn rows(&self, cx: &App) -> Vec<Item> {
        let store = self.store.read(cx);
        // The projects terminals are open on, by name.
        let mut projects: Vec<ProjectId> = Vec::new();
        for terminal in store.terminals.all() {
            if !projects.contains(terminal.project) {
                projects.push(terminal.project.clone());
            }
        }
        projects.sort_by_cached_key(|id| project_name(store, id).to_lowercase());
        let by = store.prefs.terminal_grouping;
        items_of(self.sorted(cx), by, &projects, &self.shut())
    }

    /// The headings whose terminals aren't listed: folded, and not still closing.
    fn shut(&self) -> HashSet<Group> {
        (self.folded.iter())
            .filter(|group| !self.folds.closing(group))
            .cloned()
            .collect()
    }

    pub fn is_shut(&self) -> bool {
        self.shut
    }

    /// Whether it's in the card under its tab, with the navigator across the top, which has
    /// edges of its own and drops it open.
    fn in_tab(&self, cx: &App) -> bool {
        self.store.read(cx).prefs.navigator_place == NavigatorPlace::Top
    }

    /// Note where the keys are for this frame, and whether its card slides out with no row
    /// shown as selected, drawing again after it if either changed.
    pub(super) fn set_keys(
        &mut self,
        keys: NavigatorKeys,
        left_unselected: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.keys != keys || self.left_unselected != left_unselected {
            self.keys = keys;
            self.left_unselected = left_unselected;
            // After this draw, as a notify during it draws nothing.
            cx.defer_in(window, |_, _, cx| cx.notify());
        }
    }

    /// Whether a row shows as selected: not with the keys on its tab, which heads it, or while
    /// its card slides out as it was left so.
    fn shows_selection(&self) -> bool {
        !self.keys.on_head_of(&List::Terminals) && !self.left_unselected
    }

    /// Whether it shows only its bar: folded to it, and not in the card under its tab.
    /// Where a new terminal opens, as its empty card says: the project and worktree of the
    /// selection, which the picker New terminal opens starts on, a project's being its main one.
    fn opens_in(&self, cx: &App) -> String {
        let store = self.store.read(cx);
        let place = store.selection.as_ref().and_then(|selection| {
            let checkout = crate::palette::terminal_checkout(store, selection)?;
            let project = store.project(selection.project())?;
            let worktree = match checkout {
                Checkout::Main => "Main worktree".to_owned(),
                Checkout::Linked(name) => name.to_string(),
            };
            Some(format!("{} / {worktree}", project.project.name))
        });
        match place {
            Some(place) => format!("New ones open in {place}"),
            None => "Pick a worktree for a new one".to_owned(),
        }
    }

    fn folded_away(&self, cx: &App) -> bool {
        self.shut && !self.in_tab(cx)
    }

    /// How tall it is with every row showing, as far as a fold moving shows them, in design
    /// pixels, or folded, its bar's.
    pub fn height(&self, cx: &App) -> f32 {
        let in_tab = self.in_tab(cx);
        // At the navigator's foot, a drawer: its hairline and the room over its row, the row,
        // and the room under it, then its card's edges, the room under the card, and the
        // hairline under the last drawer. Under its tab, the bar and its card's.
        let bar = match in_tab {
            true => CARD_BAR_HEIGHT + CARD_BAR_INSET,
            false => 1. + DRAWER_BLOCK + DRAWER_ROW,
        };
        if self.folded_away(cx) {
            return bar + DRAWER_BLOCK + 1.;
        }
        let bar = match in_tab {
            true => bar,
            false => bar + DRAWER_GAP + 2. + DRAWER_BLOCK + 1.,
        };
        // None open, a card saying so, as tall as its padding and what it says.
        if self.items.is_empty() {
            return bar + EMPTY_CARD_PADDING.0 * 2. + EMPTY_TIDY_HEIGHT;
        }
        let row = Theme::get(cx).sizes.control;
        let slot = |items: &[Item], ix| row + room_above(items, ix);
        let rows: f32 = match self.folds.any() {
            true => (0..self.items.len())
                .map(|ix| {
                    let moved = self
                        .folds
                        .moved(self.places.get(ix).and_then(Option::as_ref));
                    let slot = slot(&self.items, ix);
                    moved.map_or(slot, |moved| moved.shown(slot))
                })
                .sum(),
            false => {
                let items = self.rows(cx);
                (0..items.len()).map(|ix| slot(&items, ix)).sum()
            }
        };
        // Its card's padding above and below, as a drawer's; under its tab, below only. Or the
        // line saying there are none.
        let padding = match in_tab {
            true => CARD_PADDING,
            false => CARD_PADDING * 2.,
        };
        bar + rows.max(row) + padding
    }

    /// Draw its rows at their heights again, as the interface's scale changed.
    pub fn remeasure(&self) {
        self.list.remeasure();
    }

    /// Sort the rows out again from the terminals open, but not while the pointer holds them,
    /// unless `now`.
    pub(super) fn sort_out(&mut self, now: bool, cx: &App) {
        let by = self.store.read(cx).prefs.terminal_grouping;
        // Picked from a menu, so the rows go under what was picked at once.
        let regrouped = by != self.grouped_by;
        self.grouped_by = by;
        if regrouped {
            self.on_heading = None;
        }
        if self.hovered && !now && !regrouped {
            let store = self.store.read(cx);
            // Those closed meanwhile go, as nothing is there to point at.
            (self.items).retain(|item| match item {
                Item::Terminal(id) => store.terminals.get(*id).is_some(),
                Item::Heading(_) => true,
            });
        } else {
            let items = self.rows(cx);
            let (items, places) = self.folds.place(
                items,
                |item| match item {
                    Item::Heading(group) => Some(group.clone()),
                    Item::Terminal(_) => None,
                },
                |_, item| matches!(item, Item::Terminal(_)),
                |_| usize::MAX,
            );
            self.items = items;
            self.places = places;
        }
        if self.list.item_count() != self.items.len() {
            self.list.reset(self.items.len());
        }
    }

    /// Step `delta` rows from the one selected, the keys staying here, as the navigator's rows
    /// do ([`step_to`]): onto a terminal, showing it, or onto a heading, which shows nothing. In
    /// the card under its tab, up off the first row goes back onto the tab.
    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.folded_away(cx) {
            return;
        }
        let shown = self.store.read(cx).terminal_shown();
        let at = (self.items.iter()).position(|item| self.selects(item, shown));
        let next = match step_to(
            at,
            self.items.len(),
            Stops {
                tab: self.in_tab(cx),
                ..Stops::default()
            },
            delta,
        ) {
            Some(Step::Item(next)) => next,
            Some(Step::Tab) => {
                return (self.app).update_later(window, cx, |app, window, cx| {
                    app.focus_navigator(&FocusNavigator, window, cx)
                });
            }
            Some(Step::Bar) | None => return,
        };
        match self.items.get(next) {
            Some(Item::Heading(group)) => {
                self.on_heading = Some(group.clone());
                cx.notify();
            }
            Some(Item::Terminal(id)) => {
                self.on_heading = None;
                self.show(*id, false, window, cx);
            }
            None => return,
        }
        self.list.scroll_to_reveal_item(next);
    }

    /// Whether `item`'s row is the one selected: the heading the keys stepped onto, else the
    /// terminal `shown`.
    fn selects(&self, item: &Item, shown: Option<TerminalId>) -> bool {
        match (item, &self.on_heading) {
            (Item::Heading(group), Some(on)) => group == on,
            (Item::Terminal(id), None) => Some(*id) == shown,
            (Item::Heading(_), None) | (Item::Terminal(_), Some(_)) => false,
        }
    }

    /// Whether `keys` are here, in its rows or a terminal's name being edited in one.
    pub(super) fn holds(&self, keys: &FocusHandle, window: &Window) -> bool {
        self.focus.contains(keys, window)
    }

    /// The terminal selected, while its row is listed, which is what its keys act on.
    fn listed_terminal(&self, cx: &App) -> Option<TerminalId> {
        let listed = self.lists_selection(cx) && self.on_heading.is_none();
        self.store.read(cx).terminal_shown().filter(|_| listed)
    }

    /// Whether the row selected is one it lists: not with it folded to its bar, or with a
    /// terminal folded away under a heading, which leave its keys nothing to act on.
    pub(super) fn lists_selection(&self, cx: &App) -> bool {
        let shown = self.store.read(cx).terminal_shown();
        !self.folded_away(cx) && self.items.iter().any(|item| self.selects(item, shown))
    }

    /// Fold `group` to its heading, or unfold it, its terminals sliding shut or open under it.
    fn toggle_fold(&mut self, group: Group, cx: &mut Context<Self>) {
        let open = self.folded.remove(&group);
        if !open {
            self.folded.insert(group.clone());
        }
        self.folds.start(group, open, cx);
        // Under the pointer too, as it's what was clicked.
        self.sort_out(true, cx);
        cx.notify();
    }

    fn toggle_card(&mut self, cx: &mut Context<Self>) {
        self.shut ^= true;
        backend::metrics::terminal_card_folded(self.shut);
        cx.notify();
    }

    /// What a terminal's row offers when it's right-clicked: what the navigator's offers for
    /// a terminal, but Move, as one living in a tile can't, and with a way to its row there.
    /// Each holds the terminal it opened on.
    fn menu_items(id: TerminalId, cx: &App) -> Vec<MenuItem> {
        let split = move |axis: Axis| {
            move |app: &mut AppView, window: &mut Window, cx: &mut Context<AppView>| {
                let Some((_, tile)) = app.store.read(cx).terminal_tile(id) else {
                    return;
                };
                app.show_terminal(id, false, window, cx);
                app.split_tile(tile, axis, window, cx)
            }
        };
        vec![
            MenuItem::keyed(
                "Rename terminal",
                &RenameTerminal,
                cx,
                move |app, window, cx| {
                    (app.terminals.clone()).update(cx, |this, cx| this.rename(id, window, cx))
                },
            ),
            MenuItem::new("Show in navigator", move |app, window, cx| {
                app.show_terminal(id, false, window, cx);
                app.focus_navigator(&FocusNavigator, window, cx)
            }),
            MenuItem::keyed("Close terminal", &Discard, cx, move |app, window, cx| {
                let project = (app.store.read(cx).terminals.all())
                    .find(|t| t.id == id)
                    .map(|t| t.project.clone());
                if let Some(project) = project {
                    app.close_terminal(project, id, window, cx)
                }
            })
            .danger(),
            MenuItem::keyed("Split right", &SplitRight, cx, split(Axis::Across)).separated(),
            MenuItem::keyed("Split below", &SplitBelow, cx, split(Axis::Down)),
        ]
    }

    /// Edit the name of terminal `id` in its row. Enter names it what's typed; with nothing
    /// typed, its shell's title shows again. Escape, or a click anywhere else, leaves it as it
    /// was.
    fn rename(&mut self, id: TerminalId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.store.read(cx).terminals.title(id).cloned() else {
            return;
        };
        let input = terminal_name_input(&current, cx);
        let typed = on_terminal_name(
            &input,
            self.store.clone(),
            id,
            |this: &mut Self, window, cx| {
                this.renaming = None;
                window.focus(&this.focus, cx)
            },
            window,
            cx,
        );
        // The keys go where the click went. GPUI blurs it as another app comes in front too,
        // which leaves it as it is for when the window's back.
        let left = cx.on_blur(&input.focus_handle(cx), window, |this, window, cx| {
            if window.is_window_active() {
                this.renaming = None;
                cx.notify()
            }
        });
        if let Some(ix) = (self.items.iter()).position(|item| *item == Item::Terminal(id)) {
            self.list.scroll_to_reveal_item(ix);
        }
        window.focus(&input.focus_handle(cx), cx);
        self.renaming = Some(Renaming {
            id,
            input,
            _subscriptions: [typed, left],
        });
        cx.notify();
    }

    /// Close terminal `id`, after asking if Settings says to, as the navigator's rows do.
    fn close(&self, id: TerminalId, window: &mut Window, cx: &mut App) {
        let Some(project) = (self.store.read(cx).terminals.all())
            .find(|t| t.id == id)
            .map(|t| t.project.clone())
        else {
            return;
        };
        (self.app).update_later(window, cx, move |app, window, cx| {
            app.close_terminal(project, id, window, cx)
        });
    }

    /// Show terminal `id` where it is, and with `enter` give it the keys.
    fn show(&self, id: TerminalId, enter: bool, window: &mut Window, cx: &mut App) {
        backend::metrics::terminal_shown(backend::metrics::TerminalFrom::Card);
        (self.app).update_later(window, cx, move |app, window, cx| {
            app.show_terminal(id, enter, window, cx)
        });
    }

    /// Row `ix` in its slot, with the room above it ([`room_above`]), drawn as far as a fold
    /// moving it is open.
    fn render_item(&self, ix: usize, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let above = room_above(&self.items, ix);
        let height = theme.sizes.control + above;
        let moved = self
            .folds
            .moved(self.places.get(ix).and_then(Option::as_ref));
        let shown = moved.map(|moved| moved.shown(height));
        // Under the curtain, a row isn't drawn at all.
        if shown.is_some_and(|height| height <= 0.) {
            return div().into_any_element();
        }
        let (row, ruled) = match self.items.get(ix) {
            Some(Item::Heading(group)) => (
                Some(self.heading_row(group.clone(), cx).into_any_element()),
                ix > 0,
            ),
            Some(Item::Terminal(id)) => (self.terminal_row(ix, *id, cx), false),
            None => (None, false),
        };
        // A list item is as wide as its content unless something stretches it. A heading after
        // another group's rows sits under the hairline that ends them, across the whole card.
        let slot = v_flex()
            .relative()
            .w_full()
            .h(dp(height))
            .justify_end()
            .when(ruled, |el| {
                el.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(dp(CARD_PADDING))
                        .h(fixed(1.))
                        .bg(theme.border_subtle),
                )
            })
            .children(row);
        match moved.zip(shown) {
            Some((moved, height)) => clipped(slot, dp(height), moved.closing)
                .w_full()
                .into_any_element(),
            None => slot.into_any_element(),
        }
    }

    /// A group's heading, as but-ui-react's `GroupLabel`: its chevron and its name. A click folds
    /// it and leaves the selection and the keys where they were; only the keys select it.
    fn heading_row(&self, group: Group, cx: &Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let selected = self.on_heading.as_ref() == Some(&group) && self.shows_selection();
        let style = RowStyle {
            selected,
            solid: selected && self.keys.on_row_of(&List::Terminals),
            ..RowStyle::plain(theme, 0.)
        };
        let open = !self.folded.contains(&group);
        let (key, label) = match &group {
            Group::Status(status) => (status.label().to_owned(), status.label().to_owned()),
            Group::Project(project) => (
                format!("project-{project}"),
                project_name(self.store.read(cx), project),
            ),
            Group::NeedsYou => ("needs-you".to_owned(), "Needs you".to_owned()),
        };
        let row = group_label(
            ElementId::Name(format!("overview-group-{key}").into()),
            label.clone(),
            None,
            Some(!open),
            theme,
        );
        (style.draw(row, 0.))
            .aria_label(label)
            .aria_expanded(open)
            .on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_fold(group.clone(), cx)),
            )
    }

    /// Row `ix`, terminal `id`'s, as but-ui-react's `TerminalItem`: its name, then where it's
    /// open, with its project while terminals are open on several and its heading doesn't name
    /// it, and what it's at. Under a heading it sits a level in, on the guide down from the
    /// heading's chevron.
    fn terminal_row(&self, ix: usize, id: TerminalId, cx: &Context<Self>) -> Option<AnyElement> {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let terminal = store.terminals.all().find(|t| t.id == id)?;
        let selected = store.terminal_shown() == Some(id)
            && self.on_heading.is_none()
            && self.shows_selection();
        let style = RowStyle {
            selected,
            solid: selected && self.keys.on_row_of(&List::Terminals),
            ..RowStyle::plain(theme, 0.)
        };
        let worktree = match terminal.checkout {
            Checkout::Main => "Main worktree".to_owned(),
            Checkout::Linked(name) => name.to_string(),
        };
        let mut projects = store.terminals.all().map(|t| t.project);
        let first = projects.next();
        let several = projects.any(|project| Some(project) != first);
        let group = group_of(
            self.grouped_by,
            terminal.project,
            StatusGroup::of(terminal.status),
        );
        let place = match several && !matches!(group, Some(Group::Project(_))) {
            true => format!("{} / {worktree}", project_name(store, terminal.project)),
            false => worktree,
        };
        let row_id = ElementId::Name(format!("overview-terminal-{}", id.0).into());
        let row = terminal_item(row_id, terminal.title.clone(), Some(place.into()), theme);
        let row = match self.rename_field(id, theme) {
            Some(field) => row.rename(field),
            None => row,
        };
        let row = match status_mark(terminal.status, !store.hidden(), style) {
            Some(mark) => row.meta(mark),
            None => row,
        };
        // With no headings, there's no chevron to hang a guide from.
        let row = match group {
            Some(_) => {
                let last = !matches!(self.items.get(ix + 1), Some(Item::Terminal(_)));
                row.depth(1, vec![if last { Guide::Last } else { Guide::Line }])
            }
            None => row,
        };
        let row = (style.draw(row, 0.))
            .on_click(cx.listener(move |this, _, window, cx| {
                // In the card under its tab, which goes as the pointer does, into the terminal,
                // as a row clicked in another tab's card hands the keys on.
                let enter = this.in_tab(cx);
                if !enter {
                    window.focus(&this.focus, cx);
                }
                this.on_heading = None;
                this.show(id, enter, window, cx);
            }))
            // As a click selects it, then its menu.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.on_heading = None;
                    this.show(id, false, window, cx);
                    let items =
                        move |_: &mut Self, cx: &mut Context<Self>| Self::menu_items(id, cx);
                    (this.app).open_menu(&cx.weak_entity(), event.position, items, cx);
                }),
            );
        Some(row.into_any_element())
    }

    /// The field terminal `id`'s name is being edited in, in place of its name.
    fn rename_field(&self, id: TerminalId, theme: &Theme) -> Option<AnyElement> {
        let renaming = self.renaming.as_ref().filter(|r| r.id == id)?;
        Some(
            div()
                .flex_grow(1.)
                .min_w_0()
                // A press in it edits the name, rather than select the row.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Drawn focused, as it ends once the keys go elsewhere in the window. Its text
                // where the name was, past its border and padding.
                .child(
                    field(&renaming.input, true, theme)
                        .ml(dp(-5.))
                        .px(dp(4.))
                        .py(dp(1.))
                        .text_color(theme.foreground),
                )
                .into_any_element(),
        )
    }

    /// The menu of what the rows go under, the one picked ticked.
    fn grouping_items(&self, cx: &App) -> Vec<MenuItem> {
        let picked = self.store.read(cx).prefs.terminal_grouping;
        Vec::from([
            group_by("Status", TerminalGrouping::Status, picked).under("Group by"),
            group_by("Project", TerminalGrouping::Project, picked),
            group_by(
                "Needs you, then project",
                TerminalGrouping::NeedsYouThenProject,
                picked,
            ),
            group_by("None", TerminalGrouping::Ungrouped, picked),
        ])
    }

    /// The card's bar, as the projects' drawer's: its chevron, which folds it as a click on it
    /// does, its name, how many are open, what they're grouped by, and a way to open another. In
    /// the card under its tab, it doesn't fold. Its rows go right under it, as a project's go
    /// under its heading in the navigator.
    fn render_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let count = self.store.read(cx).terminals.count();
        let app = self.app.clone();
        let in_tab = self.in_tab(cx);
        let open = (!in_tab).then_some(!self.shut);
        let folded_away = self.folded_away(cx);
        let bar = card_bar(
            "overview-bar",
            "Terminals",
            Some(count.to_string()),
            open,
            false,
            false,
            theme,
        );
        // At the navigator's foot a drawer, as but-ui-react's: its buttons only under the
        // pointer, and folded, the room under its row. Under its tab, its rows go right under it.
        let bar = match in_tab {
            true => bar.mb_0(),
            false => drawer_bar(bar).when(folded_away, |el| el.mb(dp(DRAWER_BLOCK))),
        };
        // A drawer's buttons are regular, as but-ui-react's Drawer's; under its tab, small.
        let sized = |button: Button| match in_tab {
            true => button.small(),
            false => button,
        };
        let grouping = sized(icon_button("overview-grouping", Icon::Mixer))
            .tooltip("Group terminals")
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                let items = |this: &mut Self, cx: &mut Context<Self>| this.grouping_items(cx);
                (this.app).open_menu(&cx.weak_entity(), event.position(), items, cx)
            }));
        let new_terminal = sized(icon_button("overview-new-terminal", Icon::Plus))
            .tooltip("New terminal…")
            .on_click(move |_, window, cx| {
                app.update_later(window, cx, |app, window, cx| {
                    app.new_terminal_in(window, cx)
                })
            });
        bar.child(on_hover(grouping, in_tab))
            .child(on_hover(new_terminal, in_tab))
            .when(!in_tab, |el| {
                el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_card(cx)))
            })
    }
}

impl Render for TerminalOverview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A fold that got where it was going lists what it holds, or stops, at once.
        let landed = self.folds.frame();
        // With no rows drawn there's nothing under the pointer to hold, and nothing to say it
        // left, so what opens meanwhile is listed at once.
        let folded_away = self.folded_away(cx);
        if folded_away || self.items.is_empty() {
            self.hovered = false;
        }
        self.sort_out(landed || self.folds.any(), cx);
        if self.folds.any() {
            window.request_animation_frame();
        }
        let theme = Theme::get(cx);
        let outlined = self.store.read(cx).prefs.shows(Part::FocusOutline)
            && self.keys.in_list(&List::Terminals);
        let in_tab = self.in_tab(cx);
        let empty = self.items.is_empty();
        let rows = (!folded_away).then(|| {
            let rows = match empty {
                // As but-ui-react's Drawer with none: a tidy empty state saying where a new one
                // opens, and a button opening one.
                true => {
                    let app = self.app.clone();
                    let new_terminal = button("overview-empty-new-terminal", "New terminal")
                        .small()
                        .style(ButtonStyle::Gray)
                        .icon(Icon::Plus)
                        .on_click(move |_, window, cx| {
                            app.update_later(window, cx, |app, window, cx| {
                                app.new_terminal_in(window, cx)
                            })
                        })
                        .into_any_element();
                    div()
                        .py(dp(EMPTY_CARD_PADDING.0))
                        .px(dp(EMPTY_CARD_PADDING.1))
                        .child(empty_state_tidy(
                            "No terminals open",
                            self.opens_in(cx),
                            [new_terminal],
                            theme,
                        ))
                        .into_any_element()
                }
                false => list(
                    self.list.clone(),
                    cx.processor(|this, ix: usize, _, cx| this.render_item(ix, cx)),
                )
                .size_full()
                .into_any_element(),
            };
            div()
                .id("terminal-overview-rows")
                .flex_1()
                .min_h_0()
                // Its line dragged lower than what it holds, what's past its card is cut.
                .overflow_hidden()
                .when(!empty, |el| el.pb(dp(CARD_PADDING)))
                .when(!empty && !in_tab, |el| el.pt(dp(CARD_PADDING)))
                // Its rows in a card under the bar, as but-ui-react's Drawer holds them; the card
                // under its tab is its edges.
                .when(!in_tab, |el| {
                    el.bg(theme.background)
                        .border_1()
                        .border_color(match outlined {
                            true => theme.focus_edge(),
                            false => theme.border_subtle,
                        })
                        .rounded(dp(theme.shape.card))
                })
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    this.hovered = *hovered;
                    // Let go, they take the order they'd have taken meanwhile.
                    if !*hovered {
                        cx.notify();
                    }
                }))
                .child(rows)
        });
        v_flex()
            .id("terminal-overview")
            .role(Role::List)
            .aria_label("Terminals")
            .aria_expanded(!folded_away)
            .key_context("TerminalOverview")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SelectNext, window, cx| this.step(1, window, cx)))
            .on_action(
                cx.listener(|this, _: &SelectPrevious, window, cx| this.step(-1, window, cx)),
            )
            // In the card under its tab, Escape hands the keys to the page, as on the tabs.
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                if !this.in_tab(cx) || this.store.read(cx).moving.is_some() {
                    return cx.propagate();
                }
                (this.app).update_later(window, cx, |app, window, cx| {
                    app.focus_page_files(window, cx)
                });
            }))
            .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                // Folded to its bar, it opens, as the drawer's bar does.
                if this.folded_away(cx) {
                    return this.toggle_card(cx);
                }
                if let Some(group) = this.on_heading.clone() {
                    return this.toggle_fold(group, cx);
                }
                if let Some(id) = this.listed_terminal(cx) {
                    this.show(id, true, window, cx);
                }
            }))
            // The terminal selected's, as the navigator's keys act on its row's, but not on a
            // heading, which neither renames nor closes.
            .on_action(cx.listener(|this, _: &RenameTerminal, window, cx| {
                if let Some(id) = this.listed_terminal(cx) {
                    this.rename(id, window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &Discard, window, cx| {
                if let Some(id) = this.listed_terminal(cx) {
                    this.close(id, window, cx)
                }
            }))
            .size_full()
            .relative()
            .overflow_hidden()
            .when(!in_tab, |el| el.child(super::hairline(theme)))
            .child(self.render_bar(cx))
            .children(rows)
            // The last drawer at the navigator's foot: the room under its card, and a hairline
            // under it, as but-ui-react's Drawer's `last`.
            .when(!in_tab, |el| {
                el.pb(dp(if folded_away { 0. } else { DRAWER_BLOCK } + 1.))
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .h(fixed(1.))
                            .bg(theme.border_subtle),
                    )
            })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::{Duration, Instant};

    use super::{Group, Item, items_of};
    use crate::palette::StatusGroup;
    use crate::state::{ProjectId, TerminalGrouping};
    use crate::terminal::TerminalId;

    fn alpha() -> ProjectId {
        ProjectId::from("alpha")
    }

    fn beta() -> ProjectId {
        ProjectId::from("beta")
    }

    #[test]
    fn lists_what_wants_the_user_first_and_what_works_last() {
        let at = Instant::now();
        let terminals = [
            (TerminalId(1), alpha(), StatusGroup::Idle, at),
            (TerminalId(2), alpha(), StatusGroup::Working, at),
            (TerminalId(3), alpha(), StatusGroup::Done, at),
            (TerminalId(4), alpha(), StatusGroup::Failed, at),
            (TerminalId(5), alpha(), StatusGroup::Blocked, at),
            (TerminalId(6), alpha(), StatusGroup::Working, at),
        ];
        assert_eq!(
            items_of(
                terminals,
                TerminalGrouping::Status,
                &[alpha()],
                &HashSet::new()
            ),
            [
                Item::Heading(Group::Status(StatusGroup::Blocked)),
                Item::Terminal(TerminalId(5)),
                Item::Heading(Group::Status(StatusGroup::Failed)),
                Item::Terminal(TerminalId(4)),
                Item::Heading(Group::Status(StatusGroup::Done)),
                Item::Terminal(TerminalId(3)),
                Item::Heading(Group::Status(StatusGroup::Idle)),
                Item::Terminal(TerminalId(1)),
                Item::Heading(Group::Status(StatusGroup::Working)),
                Item::Terminal(TerminalId(2)),
                Item::Terminal(TerminalId(6)),
            ],
            "working last, and terminals alike keep the order they opened in"
        );
    }

    #[test]
    fn idle_terminals_go_by_when_they_went_idle() {
        let at = Instant::now();
        let later = |secs| at + Duration::from_secs(secs);
        let terminals = [
            (TerminalId(1), alpha(), StatusGroup::Idle, later(30)),
            (TerminalId(2), alpha(), StatusGroup::Idle, later(10)),
            (TerminalId(3), alpha(), StatusGroup::Idle, later(20)),
        ];
        assert_eq!(
            items_of(
                terminals,
                TerminalGrouping::Status,
                &[alpha()],
                &HashSet::new()
            ),
            [
                Item::Heading(Group::Status(StatusGroup::Idle)),
                Item::Terminal(TerminalId(1)),
                Item::Terminal(TerminalId(3)),
                Item::Terminal(TerminalId(2)),
            ],
            "the one that went idle last leads"
        );
    }

    #[test]
    fn a_shut_status_keeps_its_heading_and_count() {
        let at = Instant::now();
        let terminals = [
            (TerminalId(1), alpha(), StatusGroup::Idle, at),
            (TerminalId(2), alpha(), StatusGroup::Working, at),
            (TerminalId(3), alpha(), StatusGroup::Idle, at),
        ];
        let shut = HashSet::from([Group::Status(StatusGroup::Idle)]);
        assert_eq!(
            items_of(terminals, TerminalGrouping::Status, &[alpha()], &shut),
            [
                Item::Heading(Group::Status(StatusGroup::Idle)),
                Item::Heading(Group::Status(StatusGroup::Working)),
                Item::Terminal(TerminalId(2)),
            ],
            "a folded status lists only its heading"
        );
    }

    #[test]
    fn by_project_lists_each_project_in_order_with_what_wants_the_user_first() {
        let at = Instant::now();
        let terminals = [
            (TerminalId(1), alpha(), StatusGroup::Working, at),
            (TerminalId(2), beta(), StatusGroup::Idle, at),
            (TerminalId(3), alpha(), StatusGroup::Blocked, at),
            (TerminalId(4), beta(), StatusGroup::Done, at),
        ];
        assert_eq!(
            items_of(
                terminals,
                TerminalGrouping::Project,
                &[beta(), alpha()],
                &HashSet::new()
            ),
            [
                Item::Heading(Group::Project(beta())),
                Item::Terminal(TerminalId(4)),
                Item::Terminal(TerminalId(2)),
                Item::Heading(Group::Project(alpha())),
                Item::Terminal(TerminalId(3)),
                Item::Terminal(TerminalId(1)),
            ],
            "projects go in the order given, and within each, blocked and done before the rest"
        );
    }

    #[test]
    fn needs_you_then_project_lists_what_wants_the_user_first_whatever_its_project() {
        let at = Instant::now();
        let terminals = [
            (TerminalId(1), alpha(), StatusGroup::Working, at),
            (TerminalId(2), beta(), StatusGroup::Done, at),
            (TerminalId(3), alpha(), StatusGroup::Blocked, at),
            (TerminalId(4), beta(), StatusGroup::Idle, at),
            (TerminalId(5), alpha(), StatusGroup::Failed, at),
        ];
        assert_eq!(
            items_of(
                terminals,
                TerminalGrouping::NeedsYouThenProject,
                &[alpha(), beta()],
                &HashSet::new()
            ),
            [
                Item::Heading(Group::NeedsYou),
                Item::Terminal(TerminalId(3)),
                Item::Terminal(TerminalId(5)),
                Item::Terminal(TerminalId(2)),
                Item::Heading(Group::Project(alpha())),
                Item::Terminal(TerminalId(1)),
                Item::Heading(Group::Project(beta())),
                Item::Terminal(TerminalId(4)),
            ],
            "blocked, failed and done lead under one heading, the rest under their projects'"
        );
    }

    #[test]
    fn needs_you_then_project_leaves_out_a_project_with_only_what_wants_the_user() {
        let at = Instant::now();
        let terminals = [
            (TerminalId(1), alpha(), StatusGroup::Blocked, at),
            (TerminalId(2), beta(), StatusGroup::Working, at),
        ];
        assert_eq!(
            items_of(
                terminals,
                TerminalGrouping::NeedsYouThenProject,
                &[alpha(), beta()],
                &HashSet::new()
            ),
            [
                Item::Heading(Group::NeedsYou),
                Item::Terminal(TerminalId(1)),
                Item::Heading(Group::Project(beta())),
                Item::Terminal(TerminalId(2)),
            ],
            "alpha's only terminal is under Needs you, so alpha has no heading"
        );
    }

    #[test]
    fn ungrouped_lists_no_headings_with_what_wants_the_user_first() {
        let at = Instant::now();
        let terminals = [
            (TerminalId(1), alpha(), StatusGroup::Working, at),
            (TerminalId(2), beta(), StatusGroup::Idle, at),
            (TerminalId(3), alpha(), StatusGroup::Blocked, at),
        ];
        assert_eq!(
            items_of(
                terminals,
                TerminalGrouping::Ungrouped,
                &[alpha(), beta()],
                &HashSet::new()
            ),
            [
                Item::Terminal(TerminalId(3)),
                Item::Terminal(TerminalId(2)),
                Item::Terminal(TerminalId(1)),
            ],
            "the terminals alone, in the order status grouping lists them"
        );
    }
}
