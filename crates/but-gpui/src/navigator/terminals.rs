//! The terminal overview under the navigator: every terminal open on every project, tiled or
//! not, under a heading for each status the terminal picker groups them by, what wants the user
//! first and what's under way last. The headings fold as the navigator's sections do, and
//! stepping through the terminals shows each where it is, as the navigator's rows do.

use std::collections::HashSet;
use std::time::Instant;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable, ListAlignment, ListState,
    MouseButton, MouseDownEvent, Role, Subscription, Window, div, list, prelude::*, relative,
};

use super::rows::ROW_LEFT;
use super::{CARD_BAR_HEIGHT, CARD_BAR_INSET, RowStyle, STEP, card_bar, status_mark};
use crate::app::{
    AppHandle, AppView, Cancel, Confirm, FocusNavigator, SelectNext, SelectPrevious, SplitBelow,
    SplitRight,
};
use crate::backend::Checkout;
use crate::edits::{Discard, RenameTerminal};
use crate::overlays::{MenuItem, has_keys};
use crate::palette::StatusGroup;
use crate::state::{Axis, NavigatorPlace, Store};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::ui::fold::{Folds, Place, clipped};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    Icon, dp, faint, field, fixed, fold_button, guide, icon_button, section_label, small_icon,
    v_flex,
};

/// The room under the last row.
const ROWS_PADDING: f32 = 6.;
/// How far a status's heading sits in, as a section's heading in the navigator does.
const HEADING_INDENT: f32 = 6.;

/// Where the guide down beside a status's terminals runs: under the middle of its heading's
/// 14-point chevron.
const GUIDE_AT: f32 = ROW_LEFT + HEADING_INDENT + 7.;

/// The statuses in the order they're listed: what wants the user first, and what's under way
/// last, as it needs nothing until it's done.
const ORDER: [StatusGroup; 5] = [
    StatusGroup::Blocked,
    StatusGroup::Failed,
    StatusGroup::Done,
    StatusGroup::Idle,
    StatusGroup::Working,
];

/// A row of the overview.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    /// A status's heading, with how many terminals are at it.
    Heading(StatusGroup, usize),
    Terminal(TerminalId),
}

/// A terminal as the overview sorts it: what it's at, and since when.
type Sorted = (TerminalId, StatusGroup, Instant);

/// The overview's rows for `terminals`, in the order they opened: each status with any, in
/// [`ORDER`], under its heading, but for the terminals of those `shut`. Idle ones go by when
/// they went idle, the latest first.
fn items_of(terminals: impl IntoIterator<Item = Sorted>, shut: &HashSet<StatusGroup>) -> Vec<Item> {
    let terminals: Vec<Sorted> = terminals.into_iter().collect();
    let mut items = Vec::new();
    for group in ORDER {
        let mut at: Vec<&Sorted> = (terminals.iter())
            .filter(|(_, of, _)| *of == group)
            .collect();
        if at.is_empty() {
            continue;
        }
        // Stable, so those idle since the same moment keep the order they opened in.
        if group == StatusGroup::Idle {
            at.sort_by_key(|(_, _, since)| std::cmp::Reverse(*since));
        }
        items.push(Item::Heading(group, at.len()));
        if !shut.contains(&group) {
            items.extend(at.into_iter().map(|(id, ..)| Item::Terminal(*id)));
        }
    }
    items
}

pub struct TerminalOverview {
    app: AppHandle,
    store: Entity<Store>,
    items: Vec<Item>,
    /// Each row's place in the fold moving it, while one does.
    places: Vec<Option<Place<StatusGroup>>>,
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
    /// The statuses folded to their headings.
    folded: HashSet<StatusGroup>,
    /// The headings opening or closing now.
    folds: Folds<StatusGroup>,
    /// The keys stepped onto a status's heading, which folds but shows nothing, so the terminal
    /// shown stays as it was: see [`TerminalOverview::step`].
    on_heading: Option<StatusGroup>,
    /// The terminal shown when the store last changed, to leave the heading once another is.
    shown: Option<TerminalId>,
    /// The terminal whose name is being edited in its row, and the field it's edited in.
    renaming: Option<Renaming>,
    focus: FocusHandle,
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
            }
            cx.notify()
        });
        let empty = store.read(cx).terminals.count() == 0;
        Self {
            app,
            store,
            items: Vec::new(),
            places: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, fixed(56.)),
            hovered: false,
            shut: empty,
            was_empty: empty,
            folded: HashSet::new(),
            folds: Folds::default(),
            on_heading: None,
            shown: None,
            renaming: None,
            focus: cx.focus_handle(),
            _store,
        }
    }

    /// The terminals open, each with what it's at and since when.
    fn sorted(&self, cx: &App) -> Vec<Sorted> {
        (self.store.read(cx).terminals.all())
            .map(|t| (t.id, StatusGroup::of(t.status), t.since))
            .collect()
    }

    /// The statuses whose terminals aren't listed: folded, and not still closing.
    fn shut(&self) -> HashSet<StatusGroup> {
        (self.folded.iter())
            .filter(|group| !self.folds.closing(group))
            .copied()
            .collect()
    }

    /// Whether the card is folded to its bar.
    pub fn is_shut(&self) -> bool {
        self.shut
    }

    /// Whether it's in the card under its tab, with the navigator across the top, which has
    /// edges of its own and drops it open.
    fn in_tab(&self, cx: &App) -> bool {
        self.store.read(cx).prefs.navigator_place == NavigatorPlace::Top
    }

    /// Whether it shows only its bar: folded to it, and not in the card under its tab.
    fn folded_away(&self, cx: &App) -> bool {
        self.shut && !self.in_tab(cx)
    }

    /// How tall it is with every row showing, as far as a fold moving shows them, in design
    /// pixels, or folded, its bar's.
    pub fn height(&self, cx: &App) -> f32 {
        let edges = if self.in_tab(cx) { 0. } else { 2. };
        let bar = edges + CARD_BAR_HEIGHT + 2. * CARD_BAR_INSET;
        if self.folded_away(cx) {
            return bar;
        }
        let row = Theme::get(cx).sizes.control;
        let rows = match self.folds.any() {
            true => (0..self.items.len())
                .map(|ix| {
                    let moved = self
                        .folds
                        .moved(self.places.get(ix).and_then(Option::as_ref));
                    moved.map_or(row, |moved| moved.shown(row))
                })
                .sum(),
            false => items_of(self.sorted(cx), &self.shut()).len() as f32 * row,
        };
        // Or the line saying there are none.
        bar + rows.max(row) + ROWS_PADDING
    }

    /// Draw its rows at their heights again, as the interface's scale changed.
    pub fn remeasure(&self) {
        self.list.remeasure();
    }

    /// Sort the rows out again from the terminals open, but not while the pointer holds them,
    /// unless `now`.
    fn sort_out(&mut self, now: bool, cx: &App) {
        if self.hovered && !now {
            let store = self.store.read(cx);
            // Those closed meanwhile go, as nothing is there to point at, and the headings
            // count what's at each status now, which moves nothing.
            (self.items).retain(|item| match item {
                Item::Terminal(id) => store.terminals.get(*id).is_some(),
                Item::Heading(..) => true,
            });
            let sorted = self.sorted(cx);
            for item in &mut self.items {
                if let Item::Heading(group, count) = item {
                    *count = sorted.iter().filter(|(_, of, _)| of == group).count();
                }
            }
        } else {
            let items = items_of(self.sorted(cx), &self.shut());
            let (items, places) = self.folds.place(
                items,
                |item| match item {
                    Item::Heading(group, _) => Some(*group),
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
    /// do: onto a terminal, showing it, or onto a status's heading, which shows nothing. From
    /// none selected, forward starts at the top and back at the bottom.
    fn step(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.folded_away(cx) {
            return;
        }
        let shown = self.store.read(cx).terminal_shown();
        let at = (self.items.iter()).position(|item| match (item, self.on_heading) {
            (Item::Heading(group, _), Some(on)) => *group == on,
            (Item::Terminal(id), None) => Some(*id) == shown,
            (Item::Heading(..), None) | (Item::Terminal(_), Some(_)) => false,
        });
        let last = self.items.len().saturating_sub(1);
        let next = match at {
            Some(at) => (at as isize + delta).clamp(0, last as isize) as usize,
            None if delta > 0 => 0,
            None => last,
        };
        if Some(next) == at {
            return;
        }
        match self.items.get(next) {
            Some(Item::Heading(group, _)) => {
                self.on_heading = Some(*group);
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

    /// Fold `group` to its heading, or unfold it, its terminals sliding shut or open under it.
    fn toggle_fold(&mut self, group: StatusGroup, cx: &mut Context<Self>) {
        let open = self.folded.remove(&group);
        if !open {
            self.folded.insert(group);
        }
        self.folds.start(group, open, cx);
        // Under the pointer too, as it's what was clicked.
        self.sort_out(true, cx);
        cx.notify();
    }

    /// Fold the card to its bar, or open it.
    fn toggle_card(&mut self, cx: &mut Context<Self>) {
        self.shut ^= true;
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
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Terminal name", false, cx);
            input.load(current.to_string(), cx);
            input.select_all_text(cx);
            input
        });
        let typed = cx.subscribe_in(&input, window, move |this, input, event, window, cx| {
            let name = match event {
                InputEvent::Submit => Some(input.read(cx).text().to_owned()),
                InputEvent::Cancel => None,
                InputEvent::Changed => return,
            };
            this.renaming = None;
            window.focus(&this.focus, cx);
            if let Some(name) = name {
                (this.store).update(cx, |store, cx| store.name_terminal(id, &name, cx))
            }
        });
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
        (self.app).update_later(window, cx, move |app, window, cx| {
            app.show_terminal(id, enter, window, cx)
        });
    }

    /// Row `ix`, with the guide down from its heading's chevron beside a terminal's, drawn as
    /// far as a fold moving it is open.
    fn render_item(&self, ix: usize, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let moved = self
            .folds
            .moved(self.places.get(ix).and_then(Option::as_ref));
        let shown = moved.map(|moved| moved.shown(theme.sizes.control));
        // Under the curtain, a row isn't drawn at all.
        if shown.is_some_and(|height| height <= 0.) {
            return div().into_any_element();
        }
        let (row, guided) = match self.items.get(ix) {
            Some(Item::Heading(group, count)) => (
                Some(
                    self.heading_row(*group, *count, window, cx)
                        .into_any_element(),
                ),
                false,
            ),
            Some(Item::Terminal(id)) => (self.terminal_row(*id, window, cx), true),
            None => (None, false),
        };
        // A list item is as wide as its content unless something stretches it.
        let slot = div()
            .relative()
            .w_full()
            .h(dp(theme.sizes.control))
            .children(row)
            .when(guided, |el| el.child(guide(GUIDE_AT, theme)));
        match moved.zip(shown) {
            Some((moved, height)) => clipped(slot, dp(height), moved.closing)
                .w_full()
                .into_any_element(),
            None => slot.into_any_element(),
        }
    }

    /// A status's heading, as a section's in the navigator: its chevron, which folds it, its
    /// name and how many are at it. A double-click folds it too.
    fn heading_row(
        &self,
        group: StatusGroup,
        count: usize,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = Theme::get(cx);
        let selected = self.on_heading == Some(group);
        let style = RowStyle {
            selected,
            solid: selected && has_keys(&self.focus, window, cx),
            ..RowStyle::plain(theme, 0.)
        };
        let open = !self.folded.contains(&group);
        let quiet = style.quiet();
        let toggle = move |this: &mut Self, _: &mut Window, cx: &mut Context<Self>| {
            this.on_heading = Some(group);
            this.toggle_fold(group, cx)
        };
        let id = format!("overview-fold-{}", group.label());
        (style.row(format!("overview-group-{}", group.label()), HEADING_INDENT))
            .gap(dp(6.))
            .pr(dp(12.))
            .aria_label(group.label())
            .aria_expanded(open)
            .child(fold_button(id, Some(open), quiet, &self.focus, cx, toggle))
            .child(
                (section_label(group.label(), theme).text_color(quiet))
                    .min_w_0()
                    .truncate(),
            )
            .child(faint(count.to_string(), theme).text_color(quiet))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus, cx);
                this.on_heading = Some(group);
                if event.click_count() == 2 {
                    this.toggle_fold(group, cx);
                }
                cx.notify();
            }))
    }

    /// A terminal's row, set in under its status's heading: what it's at, its title, and the
    /// worktree it's open on, with its project while terminals are open on several.
    fn terminal_row(
        &self,
        id: TerminalId,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let terminal = store.terminals.all().find(|t| t.id == id)?;
        let selected = store.terminal_shown() == Some(id) && self.on_heading.is_none();
        let style = RowStyle {
            selected,
            solid: selected && has_keys(&self.focus, window, cx),
            ..RowStyle::plain(theme, 0.)
        };
        let mark = match status_mark(terminal.status, !store.hidden(), style) {
            Some(mark) => mark.into_any_element(),
            None => small_icon(Icon::BulletDisc)
                .text_color(style.quiet())
                .opacity(0.5)
                .into_any_element(),
        };
        let worktree = match terminal.checkout {
            Checkout::Main => "Main worktree".to_owned(),
            Checkout::Linked(name) => name.to_string(),
        };
        let mut projects = store.terminals.all().map(|t| t.project);
        let first = projects.next();
        let place = match projects.any(|project| Some(project) != first) {
            true => {
                let name = (store.project(terminal.project))
                    .map_or_else(|| terminal.project.to_string(), |p| p.project.name.clone());
                format!("{name} · {worktree}")
            }
            false => worktree,
        };
        let row = style
            .row(format!("overview-terminal-{}", id.0), STEP)
            .child(
                div()
                    .flex_none()
                    .w(dp(14.))
                    .flex()
                    .justify_center()
                    .child(mark),
            )
            .child(match self.rename_field(id, theme) {
                Some(field) => field,
                None => div()
                    .flex_grow(1.)
                    .min_w_0()
                    .truncate()
                    .child(terminal.title.clone())
                    .into_any_element(),
            })
            // Always there, the title giving way to it, up to a share of the row.
            .child(
                div()
                    .flex_none()
                    .max_w(relative(0.4))
                    .truncate()
                    .text_size(dp(11.))
                    .text_color(style.quiet())
                    .child(place),
            )
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
                        .text_color(theme.text_1),
                )
                .into_any_element(),
        )
    }

    /// The card's bar, as the projects' drawer's: its chevron, which folds it as a click on it
    /// does, its name, how many are open, and a way to open another. In the card under its tab,
    /// it doesn't fold.
    fn render_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let count = self.store.read(cx).terminals.count();
        let app = self.app.clone();
        let in_tab = self.in_tab(cx);
        let open = (!in_tab).then_some(!self.shut);
        card_bar(
            "overview-bar",
            "Terminals",
            Some(count.to_string()),
            open,
            false,
            false,
            theme,
        )
        .child(
            icon_button("overview-new-terminal", Icon::Plus)
                .small()
                .tooltip("New terminal…")
                .on_click(move |_, window, cx| {
                    app.update_later(window, cx, |app, window, cx| {
                        app.new_terminal_in(window, cx)
                    })
                }),
        )
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
        let focused = has_keys(&self.focus, window, cx);
        let in_tab = self.in_tab(cx);
        let rows = (!folded_away).then(|| {
            let rows = match self.items.is_empty() {
                true => div()
                    .h(dp(theme.sizes.control))
                    .flex()
                    .items_center()
                    // Under the bar's name, past its chevron and the gap after it, if it has one.
                    .pl(dp(CARD_BAR_INSET
                        + 10.
                        + if in_tab { 0. } else { 12. + 6. }))
                    .child(faint("No terminals open", theme))
                    .into_any_element(),
                false => list(
                    self.list.clone(),
                    cx.processor(|this, ix: usize, window, cx| this.render_item(ix, window, cx)),
                )
                .size_full()
                .into_any_element(),
            };
            div()
                .id("terminal-overview-rows")
                .flex_1()
                .min_h_0()
                .pb(dp(ROWS_PADDING))
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
                if let Some(group) = this.on_heading {
                    return this.toggle_fold(group, cx);
                }
                if let Some(id) = this.store.read(cx).terminal_shown() {
                    this.show(id, true, window, cx);
                }
            }))
            // The terminal selected's, as the navigator's keys act on its row's, but not on a
            // heading, which neither renames nor closes.
            .on_action(cx.listener(|this, _: &RenameTerminal, window, cx| {
                if this.on_heading.is_none()
                    && let Some(id) = this.store.read(cx).terminal_shown()
                {
                    this.rename(id, window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &Discard, window, cx| {
                if this.on_heading.is_none()
                    && let Some(id) = this.store.read(cx).terminal_shown()
                {
                    this.close(id, window, cx)
                }
            }))
            .size_full()
            // The card under its tab is its edges.
            .when(!in_tab, |el| {
                el.bg(theme.bg_1)
                    .border_1()
                    .border_color(match focused {
                        true => theme.focus_edge(),
                        false => theme.border_3,
                    })
                    .rounded(dp(theme.shape.card))
            })
            .overflow_hidden()
            .child(self.render_bar(cx))
            .children(rows)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::{Duration, Instant};

    use super::{Item, items_of};
    use crate::palette::StatusGroup;
    use crate::terminal::TerminalId;

    #[test]
    fn lists_what_wants_the_user_first_and_what_works_last() {
        let at = Instant::now();
        let terminals = [
            (TerminalId(1), StatusGroup::Idle, at),
            (TerminalId(2), StatusGroup::Working, at),
            (TerminalId(3), StatusGroup::Done, at),
            (TerminalId(4), StatusGroup::Failed, at),
            (TerminalId(5), StatusGroup::Blocked, at),
            (TerminalId(6), StatusGroup::Working, at),
        ];
        assert_eq!(
            items_of(terminals, &HashSet::new()),
            [
                Item::Heading(StatusGroup::Blocked, 1),
                Item::Terminal(TerminalId(5)),
                Item::Heading(StatusGroup::Failed, 1),
                Item::Terminal(TerminalId(4)),
                Item::Heading(StatusGroup::Done, 1),
                Item::Terminal(TerminalId(3)),
                Item::Heading(StatusGroup::Idle, 1),
                Item::Terminal(TerminalId(1)),
                Item::Heading(StatusGroup::Working, 2),
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
            (TerminalId(1), StatusGroup::Idle, later(30)),
            (TerminalId(2), StatusGroup::Idle, later(10)),
            (TerminalId(3), StatusGroup::Idle, later(20)),
        ];
        assert_eq!(
            items_of(terminals, &HashSet::new()),
            [
                Item::Heading(StatusGroup::Idle, 3),
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
            (TerminalId(1), StatusGroup::Idle, at),
            (TerminalId(2), StatusGroup::Working, at),
            (TerminalId(3), StatusGroup::Idle, at),
        ];
        let shut = HashSet::from([StatusGroup::Idle]);
        assert_eq!(
            items_of(terminals, &shut),
            [
                Item::Heading(StatusGroup::Idle, 2),
                Item::Heading(StatusGroup::Working, 1),
                Item::Terminal(TerminalId(2)),
            ],
            "a folded status lists only its heading, which still counts its terminals"
        );
    }
}
