//! The order of the watched projects' cards: as the user drags them, by name, or by last
//! activity, from the header's view menu, which also folds them all or opens them all. Also
//! dragging a card, a linked worktree or a terminal to another place, the others sliding to
//! make room, and
//! how the projects not watched are listed.

use std::cmp::Reverse;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, DragMoveEvent, Pixels, Point, SharedString,
    Stateful, Window, actions, prelude::*,
};

use bstr::{BStr, BString};

use super::Navigator;
use super::rows::{HEADING_MIDDLE, Row, heads, last_active, listed_with, span_height};
use crate::app::{AppView, MoveHere};
use crate::backend::Checkout;
use crate::overlays::MenuItem;
use crate::state::{
    Prefs, ProjectCheckout, ProjectEntry, ProjectId, Selection, Store, WatchedOrder,
};
use crate::terminal::TerminalId;
use crate::theme::Theme;
use crate::transfer::{
    CardSlot, Dragged, Move, Moving, Op, ProjectDrag, Slot, Stop, TerminalDrag, drag_preview,
    project_preview, row_preview, terminal_preview,
};
use crate::ui::{Icon, ROW_HEIGHT, icon_button};

actions!(
    navigator,
    [
        SortProjectsByActivity,
        SortProjectsByName,
        ShowAllProjects,
        ShowActiveProjects,
    ]
);

impl AppView {
    /// Register how the projects not watched are listed, their order and which show, and the
    /// branches the Branches page lists: from anywhere, as the menus with them are buttons'.
    pub(crate) fn project_list_actions(
        &self,
        el: Stateful<Div>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        fn set<A: gpui::Action>(
            el: Stateful<Div>,
            update: fn(&mut Prefs),
            cx: &Context<AppView>,
        ) -> Stateful<Div> {
            el.on_action(cx.listener(move |this, _: &A, _, cx| {
                (this.store).update(cx, |store, cx| store.set_prefs(update, cx))
            }))
        }
        let el = set::<SortProjectsByActivity>(el, |p| p.projects_by_activity = true, cx);
        let el = set::<SortProjectsByName>(el, |p| p.projects_by_activity = false, cx);
        let el = set::<ShowAllProjects>(el, |p| p.active_projects_only = false, cx);
        set::<ShowActiveProjects>(el, |p| p.active_projects_only = true, cx)
            .on_action(
                cx.listener(|this, _: &crate::detail::ShowOnlyLocalBranches, _, cx| {
                    (this.store).update(cx, |store, cx| {
                        store.set_prefs(|p| p.branches_local_only = true, cx)
                    });
                    // The branch opened may have just been filtered out.
                    (this.detail).update(cx, |detail, cx| detail.select_shown_branch(cx));
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::detail::ShowRemoteBranches, _, cx| {
                    (this.store).update(cx, |store, cx| {
                        store.set_prefs(|p| p.branches_local_only = false, cx)
                    });
                    (this.detail).update(cx, |detail, cx| detail.select_shown_branch(cx));
                }),
            )
            .on_action(cx.listener(
                |this, _: &crate::detail::ShowBranchesWithoutCommits, _, cx| {
                    (this.store).update(cx, |store, cx| {
                        store.set_prefs(|p| p.branches_hide_empty = !p.branches_hide_empty, cx)
                    });
                    (this.detail).update(cx, |detail, cx| detail.select_shown_branch(cx));
                },
            ))
    }
}

impl Navigator {
    /// The watched projects, in the order picked.
    pub(super) fn watched<'a>(&self, store: &'a Store) -> Vec<&'a ProjectEntry> {
        // Listed in the manual order already.
        let mut watched: Vec<&ProjectEntry> =
            store.projects.iter().filter(|e| e.expanded).collect();
        // A card being moved, dragged by its heading or with the keys, shows where letting go
        // puts it, among the others as they were listed when the move started, whatever the
        // order picked. One added meanwhile goes last.
        if let Some(order) = store
            .moving
            .as_ref()
            .and_then(|moving| moving.project_order())
        {
            let at = |e: &&ProjectEntry| order.iter().position(|id| *id == e.id);
            watched.sort_by_key(|e| at(e).unwrap_or(usize::MAX));
            return watched;
        }
        match store.prefs.watched_order {
            WatchedOrder::Manual => {}
            WatchedOrder::Name => watched.sort_by_key(|e| e.project.name.to_lowercase()),
            WatchedOrder::Activity => match &self.held_order {
                // A project added meanwhile goes last.
                Some(held) => watched
                    .sort_by_key(|e| held.iter().position(|id| *id == e.id).unwrap_or(usize::MAX)),
                None => watched.sort_by_key(|e| Reverse(last_active(e))),
            },
        }
        watched
    }

    /// The button beside the account that opens the view menu.
    pub(super) fn view_button(&self, cx: &Context<Self>) -> AnyElement {
        icon_button("watched-view-options", Icon::Mixer)
            .small()
            .tooltip("Order and fold watched projects")
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                this.open_menu(event.position(), |this, cx| this.view_menu(cx), cx)
            }))
            .into_any_element()
    }

    /// How the watched projects go: their order, then folding or opening them all.
    fn view_menu(&self, cx: &App) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let order = store.prefs.watched_order;
        let shown: Vec<ProjectId> = (self.watched(store).into_iter())
            .map(|e| e.id.clone())
            .collect();
        let set = |label: &'static str, to: WatchedOrder| {
            let shown = shown.clone();
            MenuItem::new(label, move |app: &mut AppView, _, cx| {
                let shown = shown.clone();
                (app.store).update(cx, |store, cx| store.set_watched_order(to, shown, cx))
            })
            .ticked(order == to)
        };
        let fold = |label: &'static str, folded: bool| {
            MenuItem::new(label, move |app: &mut AppView, _, cx| {
                (app.store).update(cx, |store, cx| store.fold_all(folded, cx))
            })
        };
        vec![
            set("Manual, by dragging", WatchedOrder::Manual).under("Order watched projects"),
            set("Name", WatchedOrder::Name),
            set("Last activity", WatchedOrder::Activity),
            fold("Fold all", true).separated(),
            fold("Unfold all", false),
        ]
    }

    /// How the projects not expanded are listed, with headed choices as but-ui-react's view
    /// options menu has them: their order, and which to show.
    pub(super) fn view_options(&self, cx: &App) -> Vec<MenuItem> {
        let prefs = &self.store.read(cx).prefs;
        let (by_activity, active_only) = (prefs.projects_by_activity, prefs.active_projects_only);
        vec![
            MenuItem::command(&SortProjectsByActivity, cx)
                .under("Sort by")
                .ticked(by_activity),
            MenuItem::command(&SortProjectsByName, cx).ticked(!by_activity),
            MenuItem::command(&ShowAllProjects, cx)
                .under("Show")
                .ticked(!active_only),
            MenuItem::command(&ShowActiveProjects, cx).ticked(active_only),
        ]
    }

    /// Let `el` drag project `id` under a pill with its folder's `label`, as other rows' drags
    /// have: into a terminal, which pastes the folder, and while it `reorders`, its card among
    /// the others, which make room for it where it'd go (see [`Self::follow_project_drag`]), as
    /// moving it with the keys does.
    pub(super) fn project_drag(
        &self,
        el: Stateful<Div>,
        item: &Selection,
        label: SharedString,
        reorders: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        if !self.drags(cx) {
            return el;
        }
        let navigator = cx.entity().downgrade();
        let preview = project_preview(self.store.clone(), label);
        let preview = row_preview(self.store.clone(), item.clone(), preview);
        let drag = ProjectDrag {
            id: item.project().clone(),
            reorders,
        };
        el.on_drag(drag, move |drag, offset, window, cx| {
            let pill = preview(drag, offset, window, cx);
            // A drop anywhere else, or none, puts the cards back as they were.
            if reorders {
                let (navigator, dragged) = (navigator.clone(), drag.id.clone());
                cx.observe_release(&pill, move |_, cx| {
                    (navigator.update(cx, |this, cx| this.hold_card(&dragged, None, cx))).ok();
                })
                .detach();
            }
            pill
        })
    }

    /// Show project `id`'s card at `slot`, as the move of it under way does, starting one among
    /// the cards as they're listed now if there's none; or with `None` end that move, its card
    /// going back.
    pub(super) fn hold_card(&self, id: &ProjectId, slot: Option<CardSlot>, cx: &mut Context<Self>) {
        let held = |what: &Moving| matches!(what, Moving::Project(_));
        let listed = |store: &Store| {
            let order = self.watched(store).iter().map(|e| e.id.clone()).collect();
            Some(Moving::Project(order))
        };
        self.hold(id, held, listed, slot.map(Stop::Card), cx);
    }

    /// Show linked worktree `name` of `project` at `slot`, as the move of it under way does,
    /// starting one among those listed with it now if there's none; or with `None` end that
    /// move, it going back.
    pub(super) fn hold_worktree(
        &self,
        project: &ProjectId,
        name: &BStr,
        slot: Option<Slot<BString>>,
        cx: &mut Context<Self>,
    ) {
        let held =
            |what: &Moving| matches!(what, Moving::Worktree { name: held, .. } if held == name);
        let listed = |store: &Store| {
            let order = listed_with(store, project, name)?;
            let name = name.to_owned();
            Some(Moving::Worktree { name, order })
        };
        self.hold(project, held, listed, slot.map(Stop::Worktree), cx);
    }

    /// Show terminal `id` of `project` at `stop`, among the terminals of the watched projects'
    /// worktrees, as the move of it under way does, starting one among those listed now if
    /// there's none; or with `None` end that move, it going back.
    fn hold_terminal(
        &self,
        project: &ProjectId,
        id: TerminalId,
        stop: Option<(ProjectCheckout, Slot<TerminalId>)>,
        cx: &mut Context<Self>,
    ) {
        let held = |what: &Moving| matches!(what, Moving::Terminal { id: held, .. } if *held == id);
        let listed = |store: &Store| {
            let lists = self.listed_terminals(store);
            Some(Moving::Terminal { id, lists })
        };
        let stop = stop.map(|(checkout, slot)| Stop::Terminal { checkout, slot });
        self.hold(project, held, listed, stop, cx);
    }

    /// Have the move under way in `project` of what `held` says it moves be at `at`, starting
    /// one moving what `listed` says if there's none; or with `None` end that move.
    fn hold(
        &self,
        project: &ProjectId,
        held: impl Fn(&Moving) -> bool,
        listed: impl FnOnce(&Store) -> Option<Moving>,
        at: Option<Stop>,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let held = (store.moving.as_ref())
            .filter(|moving| moving.project == *project && held(&moving.what));
        let next = at.and_then(|at| {
            let what = match held {
                Some(moving) => moving.what.clone(),
                None => listed(store)?,
            };
            let (project, op) = (project.clone(), Op::Move);
            Some(Move {
                project,
                what,
                at,
                op,
            })
        });
        let unchanged = match (held, &next) {
            (Some(held), Some(next)) => held.at == next.at,
            (held, next) => held.is_none() && next.is_none(),
        };
        if !unchanged {
            (self.store).update(cx, |store, cx| store.set_moving(next, cx));
        }
    }

    /// Show the card dragged by its heading where letting go puts it, while the pointer is
    /// over the navigator: in the place where its heading would be nearest the pointer, as
    /// [`nearest_slot`] says, the cards around it sliding to make room. Elsewhere letting go
    /// leaves it be, so it shows where it is.
    pub(super) fn follow_project_drag(
        &mut self,
        event: &DragMoveEvent<ProjectDrag>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = event.event.position;
        let drag = event.drag(cx);
        if !drag.reorders {
            return;
        }
        let dragged = drag.id.clone();
        let depth = (event.bounds.contains(&at))
            .then(|| self.depth_at(at, Theme::get(cx)))
            .flatten();
        let slot = depth.map(|depth| {
            let cards: Vec<(&ProjectId, f32)> = (cards(&self.rows).into_iter())
                .map(|(id, span)| (id, span_height(&self.rows, span, self.row)))
                .collect();
            nearest_slot(&cards, &dragged, depth)
        });
        self.hold_card(&dragged, slot, cx);
    }

    /// Let go of a card dragged by its heading where it's shown, as Return lands it.
    pub(super) fn drop_project(
        &mut self,
        drag: &ProjectDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Not a move the keys started, which a folder dragged meanwhile isn't part of.
        if !drag.reorders {
            return;
        }
        let held = (self.store.read(cx).moving.as_ref())
            .is_some_and(|moving| moving.project == drag.id && moving.project_order().is_some());
        if held {
            self.move_here(&crate::app::MoveHere, window, cx);
        }
    }

    /// Let `el`, linked worktree `name`'s heading in `project`, drag `payload`, the worktree or
    /// its files, under a pill with `chip`, as other rows' drags do, the worktree going among
    /// those listed with it as it goes (see [`Self::follow_worktree_drag`]), as moving it with
    /// the keys does.
    pub(super) fn worktree_drag(
        &self,
        el: Stateful<Div>,
        item: &Selection,
        name: &BStr,
        chip: SharedString,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        if !self.drags(cx) {
            return el;
        }
        let Some(order) = listed_with(self.store.read(cx), item.project(), name) else {
            return el;
        };
        let payload = Dragged {
            project: item.project().clone(),
            what: Moving::Worktree {
                name: name.to_owned(),
                order,
            },
        };
        let navigator = cx.entity().downgrade();
        let preview = drag_preview(self.store.clone(), chip);
        let preview = row_preview(self.store.clone(), item.clone(), preview);
        let dragged = (item.project().clone(), name.to_owned());
        el.on_drag(payload, move |payload, offset, window, cx| {
            let pill = preview(payload, offset, window, cx);
            let (navigator, dragged) = (navigator.clone(), dragged.clone());
            let started = dragged.clone();
            (navigator.update(cx, |this, cx| {
                this.dragged_worktree = Some(started);
                cx.notify()
            }))
            .ok();
            // A drop anywhere else, or none, puts it back where it was.
            cx.observe_release(&pill, move |_, cx| {
                (navigator.update(cx, |this, cx| {
                    this.dragged_worktree = None;
                    let (project, name) = &dragged;
                    this.hold_worktree(project, name.as_ref(), None, cx);
                    // Let go where it was, nothing moved, but its heading no longer shows held.
                    cx.notify()
                }))
                .ok();
            })
            .detach();
            pill
        })
    }

    /// Show the linked worktree dragged by its heading where letting go puts it, as
    /// [`worktree_slot`] says, those listed with it sliding to make room; over its own rows
    /// where it is, and away from its project's card, it stays where it is.
    pub(super) fn follow_worktree_drag(
        &mut self,
        event: &DragMoveEvent<Dragged>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, name)) = &self.dragged_worktree else {
            return;
        };
        let at = event.event.position;
        let store = self.store.read(cx);
        let order = (moving_worktree(store, project, name.as_ref()).cloned())
            .or_else(|| listed_with(store, project, name.as_ref()));
        let depth = (event.bounds.contains(&at))
            .then(|| self.depth_at(at, Theme::get(cx)))
            .flatten();
        let slot = (depth.zip(order)).and_then(|(depth, order)| {
            worktree_slot(&self.rows, project, name, &order, depth, self.row)
        });
        self.hold_worktree(project, name.as_ref(), slot, cx);
    }

    /// Let go of the linked worktree dragged by its heading where it's shown, as Return lands
    /// it, if it's moving.
    pub(super) fn drop_worktree(
        &mut self,
        _: &Dragged,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if worktree_held(self.dragged_worktree.as_ref(), self.store.read(cx)) {
            self.move_here(&MoveHere, window, cx);
        }
    }

    /// Let `el`, terminal `id`'s row in `project`, drag it under a pill with its title, the
    /// terminal going among those of the watched projects' worktrees as it goes (see
    /// [`Self::follow_terminal_drag`]), as moving it with the keys does.
    pub(super) fn terminal_drag(
        &self,
        el: Stateful<Div>,
        item: &Selection,
        id: TerminalId,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        if !self.drags(cx) {
            return el;
        }
        let navigator = cx.entity().downgrade();
        let preview = terminal_preview(self.store.clone(), false);
        let preview = row_preview(self.store.clone(), item.clone(), preview);
        let drag = TerminalDrag {
            project: item.project().clone(),
            id,
            left_zone_end: None,
        };
        el.on_drag(drag, move |drag, offset, window, cx| {
            let pill = preview(drag, offset, window, cx);
            let (navigator, project, id) = (navigator.clone(), drag.project.clone(), drag.id);
            // A drop anywhere else, or none, puts it back where it was.
            cx.observe_release(&pill, move |_, cx| {
                (navigator.update(cx, |this, cx| this.hold_terminal(&project, id, None, cx))).ok();
            })
            .detach();
            pill
        })
    }

    /// Show the terminal dragged by its row where letting go puts it, as [`terminal_slot`]
    /// says, the terminals around it sliding to make room. Where it can't go, between cards or
    /// over a folded one, it stays where it's shown, so passing there doesn't send it back and
    /// forth; off the navigator, or still in the zone along the left of the tiles, it goes back.
    pub(super) fn follow_terminal_drag(
        &mut self,
        event: &DragMoveEvent<TerminalDrag>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = event.event.position;
        let drag = event.drag(cx);
        let (project, id) = (drag.project.clone(), drag.id);
        if !takes_terminal(event.bounds, at, drag.left_zone_end) {
            return self.hold_terminal(&project, id, None, cx);
        }
        let stop = self.depth_at(at, Theme::get(cx)).and_then(|depth| {
            let lists = self.listed_terminals(self.store.read(cx));
            terminal_slot(&self.rows, id, &lists, depth, self.row)
        });
        if stop.is_some() {
            self.hold_terminal(&project, id, stop, cx);
        }
    }

    /// Let go of the terminal dragged by its row where it's shown, as Return lands it.
    pub(super) fn drop_terminal(
        &mut self,
        drag: &TerminalDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let moving = moving_terminal(self.store.read(cx));
        if moving.is_some_and(|(id, _)| id == drag.id) {
            self.move_here(&MoveHere, window, cx);
        }
    }

    /// The rows of the card being moved, dragged by its heading or with the keys, while it
    /// shows where it'd go, and once it's let go of until the cards stop sliding.
    pub(super) fn held_card(&mut self, store: &Store) -> Option<Range<usize>> {
        let moved = (store.moving.as_ref())
            .filter(|moving| moving.project_order().is_some())
            .map(|moving| &moving.project);
        match moved {
            Some(held) => self.held_project = Some(held.clone()),
            None if !self.reorder.any() => self.held_project = None,
            None => {}
        }
        let held = self.held_project.as_ref()?;
        (cards(&self.rows).into_iter())
            .find(|(id, _)| *id == held)
            .map(|(_, span)| span)
    }

    /// The rows of the linked worktree being moved, dragged by its heading or with the keys,
    /// while it shows where it'd go, and once it's let go of until the rows stop sliding.
    pub(super) fn held_worktree(&mut self, store: &Store) -> Option<Range<usize>> {
        let moved = (store.moving.as_ref()).and_then(|moving| match &moving.what {
            Moving::Worktree { name, .. } => Some((moving.project.clone(), name.clone())),
            _ => None,
        });
        match moved {
            Some(held) => self.held_worktree_name = Some(held),
            None if !self.reorder.any() => self.held_worktree_name = None,
            None => {}
        }
        let (project, name) = self.held_worktree_name.as_ref()?;
        let heading = (self.rows.iter()).position(
            |row| matches!(row, Row::Item(Selection::Checkout(p, Checkout::Linked(n))) if p == project && n == name),
        )?;
        Some(heading..worktree_end(&self.rows, heading))
    }

    /// How far down the cards' rows `at` is, from the top of the first, as the rows count
    /// their heights ([`span_height`]): from the drawn row it's over, or else the nearest.
    pub(super) fn depth_at(&self, at: Point<Pixels>, theme: &Theme) -> Option<f32> {
        // In the card under a tab, which is as tall as they are, they're all there is to go by.
        if let Some(top) = self.card_top.get() {
            return Some(f32::from(at.y - top) / theme.scale);
        }
        let list = &self.list;
        let mut drawn = (list.logical_scroll_top().item_ix..self.rows.len())
            .map_while(|ix| Some((ix, list.bounds_for_item(ix)?)));
        let (mut ix, mut bounds) = drawn.next()?;
        for (next, next_bounds) in drawn {
            if next_bounds.top() > at.y {
                break;
            }
            (ix, bounds) = (next, next_bounds);
        }
        let above = span_height(&self.rows, 0..ix, self.row);
        Some(above + f32::from(at.y - bounds.top()) / theme.scale)
    }
}

/// The cards `rows` list, each by its project and the rows it takes, from its heading up to
/// the gap before the next.
pub(super) fn cards(rows: &[Row]) -> Vec<(&ProjectId, Range<usize>)> {
    let mut cards: Vec<(&ProjectId, Range<usize>)> = Vec::new();
    for (ix, row) in rows.iter().enumerate() {
        match row {
            Row::Item(Selection::Project(id)) => cards.push((id, ix..ix + 1)),
            Row::Gap => {}
            _ => {
                if let Some((_, span)) = cards.last_mut() {
                    span.end = ix + 1;
                }
            }
        }
    }
    cards
}

/// Where card `dragged`, held by its heading, goes among `cards`, each by its project and how
/// tall it is, a gap apart: the place where the middle of its heading would be nearest
/// `depth`, which is as far down from the top of the first card. Only the other cards set the
/// places, so where the dragged one is shown now doesn't change where it goes, and showing it
/// there never sends it back.
fn nearest_slot(cards: &[(&ProjectId, f32)], dragged: &ProjectId, depth: f32) -> CardSlot {
    // A gap is as tall at every density.
    let gap = Row::Gap.height(ROW_HEIGHT);
    let off = |top: f32| (top + HEADING_MIDDLE - depth).abs();
    let (mut top, mut nearest) = (0., (f32::INFINITY, CardSlot::End));
    for (id, height) in cards.iter().filter(|(id, _)| *id != dragged) {
        if off(top) < nearest.0 {
            nearest = (off(top), CardSlot::Before((*id).clone()));
        }
        top += height + gap;
    }
    match off(top) < nearest.0 {
        true => CardSlot::End,
        false => nearest.1,
    }
}

/// Where the rows of the linked worktree headed at row `heading` of `rows` end: past the
/// divider below them, or under the filter, which lists none between worktrees, before the next
/// worktree or card.
fn worktree_end(rows: &[Row], heading: usize) -> usize {
    (rows.iter().enumerate().skip(heading + 1))
        .find_map(|(ix, row)| match row {
            Row::WorktreeDivider => Some(ix + 1),
            Row::Item(Selection::Checkout(_, Checkout::Linked(_))) | Row::Gap => Some(ix),
            _ => None,
        })
        .unwrap_or(rows.len())
}

/// Those listed with linked worktree `name` of `project` as the move of it under way started,
/// if it's moving.
pub(super) fn moving_worktree<'a>(
    store: &'a Store,
    project: &ProjectId,
    name: &BStr,
) -> Option<&'a Arc<[BString]>> {
    let moving = store
        .moving
        .as_ref()
        .filter(|moving| moving.project == *project)?;
    match &moving.what {
        Moving::Worktree { name: held, order } if held == name => Some(order),
        _ => None,
    }
}

/// The terminals of the watched projects' worktrees, each with those open on it in order.
type TerminalLists = Arc<[(ProjectCheckout, Vec<TerminalId>)]>;

/// The terminal moving, if one is, and the terminals of the worktrees as the move of it under
/// way started.
pub(super) fn moving_terminal(store: &Store) -> Option<(TerminalId, &TerminalLists)> {
    match &store.moving.as_ref()?.what {
        Moving::Terminal { id, lists } => Some((*id, lists)),
        _ => None,
    }
}

/// The rows of `project`'s card in `rows`, and how far down its top is ([`span_height`]), if a
/// pointer `depth` down the rows is on it.
fn card_at(rows: &[Row], project: &ProjectId, depth: f32, row: f32) -> Option<(Range<usize>, f32)> {
    let (on, card, top) = card_under(rows, depth, row)?;
    (on == project).then_some((card, top))
}

/// The card in `rows` a pointer `depth` down the rows is on, by its project, with its rows and
/// how far down its top is ([`span_height`]).
fn card_under(rows: &[Row], depth: f32, row: f32) -> Option<(&ProjectId, Range<usize>, f32)> {
    cards(rows).into_iter().find_map(|(project, card)| {
        let top = span_height(rows, 0..card.start, row);
        let on = depth >= top && depth < top + span_height(rows, card.clone(), row);
        on.then_some((project, card, top))
    })
}

/// Whether linked worktree `dragged`, by its project and name, dragged by its heading, is
/// moving, shown where it'd go.
pub(super) fn worktree_held(dragged: Option<&(ProjectId, BString)>, store: &Store) -> bool {
    dragged.is_some_and(|(project, name)| moving_worktree(store, project, name.as_ref()).is_some())
}

/// Where linked worktree `name` of `project`, dragged by its heading, goes among those listed
/// with it in `order`, as `rows` show them, from a pointer `depth` down the rows ([`span_height`]):
/// the place where its heading would be nearest the pointer, as a card's goes
/// ([`nearest_slot`]). Off its project's card, or over where its own rows are while it's where
/// it was, it goes nowhere, so what it carries lands on those rows. Only the others set the
/// places, and where its own rows are is counted from them, so where it's shown now never
/// changes where it goes.
fn worktree_slot(
    rows: &[Row],
    project: &ProjectId,
    name: &BString,
    order: &[BString],
    depth: f32,
    row: f32,
) -> Option<Slot<BString>> {
    let (card, _) = card_at(rows, project, depth, row)?;
    // The last one in the card has no divider below it, as the card's edge rules it off, so it
    // counts one, as below another, and where it's shown changes no place. Under the filter
    // none has one.
    let ruled = card
        .clone()
        .any(|ix| matches!(rows.get(ix), Some(Row::WorktreeDivider)));
    // Each one listed with it as shown, by its name, how far down its heading is, and how tall
    // it is, down through the divider below it.
    let shown: Vec<(&BString, f32, f32)> = card
        .filter_map(|ix| match rows.get(ix)? {
            Row::Item(Selection::Checkout(p, Checkout::Linked(n)))
                if p == project && order.contains(n) =>
            {
                let end = worktree_end(rows, ix);
                let divider = match end.checked_sub(1).and_then(|last| rows.get(last)) {
                    Some(Row::WorktreeDivider) => 0.,
                    _ if ruled => Row::WorktreeDivider.height(row),
                    _ => 0.,
                };
                Some((
                    n,
                    span_height(rows, 0..ix, row),
                    span_height(rows, ix..end, row) + divider,
                ))
            }
            _ => None,
        })
        .collect();
    let (_, first, _) = shown.first()?;
    let (_, _, own) = shown.iter().find(|(n, ..)| *n == name)?;
    let others: Vec<(&BString, f32)> = (shown.iter())
        .filter(|(n, ..)| *n != name)
        .map(|(n, _, height)| (*n, *height))
        .collect();
    // How far down its heading would be in each place, before each of the others or after
    // them all, and which of those is where it was: after the others it was listed after.
    let tops: Vec<f32> = (0..=others.len())
        .map(|k| first + others.iter().take(k).map(|(_, height)| height).sum::<f32>())
        .collect();
    let home = (order.iter())
        .take_while(|n| *n != name)
        .filter(|n| others.iter().any(|(other, _)| other == n))
        .count();
    let home_top = *tops.get(home)?;
    if depth >= home_top && depth < home_top + own {
        return None;
    }
    let off = |top: f32| (top + row / 2. - depth).abs();
    let (nearest, _) =
        (tops.iter().enumerate()).min_by(|(_, a), (_, b)| off(**a).total_cmp(&off(**b)))?;
    Some(match others.get(nearest) {
        Some((before, _)) => Slot::Before((*before).clone()),
        None => Slot::End,
    })
}

/// Whether the navigator, in `bounds`, takes a terminal dragged with the pointer `at`: only
/// left of `left_zone_end`, as up to there the zone along the left of the tiles has it.
fn takes_terminal(
    bounds: Bounds<Pixels>,
    at: Point<Pixels>,
    left_zone_end: Option<Pixels>,
) -> bool {
    bounds.contains(&at) && left_zone_end.is_none_or(|end| at.x < end)
}

/// Where terminal `id`, dragged by its row, goes among the terminals of the worktrees `lists`
/// has, as `rows` show them, from a pointer `depth` down the rows ([`span_height`]): the place
/// on the card under the pointer, its own project's or another's, before one of the others or
/// after them all under a worktree's heading, where its row would be nearest the pointer. Off
/// the cards it goes nowhere. Its own row is left out of how far down each place is, so where
/// it's shown now never changes where it goes.
fn terminal_slot(
    rows: &[Row],
    id: TerminalId,
    lists: &[(ProjectCheckout, Vec<TerminalId>)],
    depth: f32,
    row: f32,
) -> Option<(ProjectCheckout, Slot<TerminalId>)> {
    let (project, card, top) = card_under(rows, depth, row)?;
    let listed = |checkout: Checkout| {
        (lists.iter()).find_map(|(on, list)| {
            (on.project == *project && on.checkout == checkout).then_some((on, list))
        })
    };
    // How far down each place is, the rows above it but its own, and which it is.
    let mut places: Vec<(f32, (ProjectCheckout, Slot<TerminalId>))> = Vec::new();
    // The worktree whose terminals are listed from here, while it's one of those `lists` has.
    let mut under: Option<(&ProjectCheckout, &Vec<TerminalId>)> = None;
    let own = |row: &&Row| matches!(row, Row::Item(Selection::Terminal(_, other)) if *other == id);
    let above = rows.get(..card.start).unwrap_or_default().iter();
    let mut at = top - above.filter(own).map(|r| r.height(row)).sum::<f32>();
    let mut card_rows = rows.get(card)?.iter().peekable();
    while let Some(shown) = card_rows.next() {
        match shown {
            Row::Item(Selection::Terminal(p, terminal)) if p == project => {
                if *terminal == id {
                    continue;
                }
                if let Some((on, _)) = under.filter(|(_, list)| list.contains(terminal)) {
                    places.push((at, (on.clone(), Slot::Before(*terminal))));
                }
            }
            // A worktree's uncommitted changes sit between its heading and its terminals.
            Row::Item(Selection::Uncommitted(p, _)) if p == project => {}
            other => {
                if let Some((on, _)) = under.take() {
                    places.push((at, (on.clone(), Slot::End)));
                }
                under = match other {
                    Row::Item(heading) if heading.project() == project => {
                        heads(other, card_rows.peek().copied()).and_then(listed)
                    }
                    _ => None,
                };
            }
        }
        at += shown.height(row);
    }
    if let Some((on, _)) = under {
        places.push((at, (on.clone(), Slot::End)));
    }
    let off = |top: f32| (top + row / 2. - depth).abs();
    let (_, nearest) = (places.into_iter()).min_by(|(a, _), (b, _)| off(*a).total_cmp(&off(*b)))?;
    Some(nearest)
}

#[cfg(test)]
mod tests {
    use bstr::BString;
    use gpui::{Bounds, point, size};

    use super::{CardSlot, Row, cards, nearest_slot, takes_terminal, terminal_slot, worktree_slot};
    use crate::backend::Checkout;
    use crate::state::{ProjectCheckout, ProjectId, Selection};
    use crate::terminal::TerminalId;
    use crate::transfer::Slot;
    use crate::ui::{ROW_HEIGHT, fixed};

    /// Project `p`'s card with linked worktrees `a`, `b` and `c` listed in `order`, each its
    /// heading, `b` open on two more rows, and a divider below but for the last, which the
    /// card's edge rules off; and its rows, with how far down each worktree's heading is.
    fn worktrees(order: [&str; 3]) -> (ProjectId, Vec<Row>, Vec<(BString, f32)>) {
        let p = ProjectId::from("p");
        let mut rows = vec![
            Row::Item(Selection::Project(p.clone())),
            Row::Item(Selection::Uncommitted(p.clone(), Checkout::Main)),
            Row::WorktreeDivider,
        ];
        let mut tops = Vec::new();
        for name in order {
            tops.push((
                BString::from(name),
                super::span_height(&rows, 0..rows.len(), ROW_HEIGHT),
            ));
            rows.push(Row::Item(Selection::Checkout(
                p.clone(),
                Checkout::Linked(name.into()),
            )));
            if name == "b" {
                let branch = Selection::Branch(p.clone(), None);
                rows.extend([Row::Item(branch.clone()), Row::Item(branch)]);
            }
            rows.push(Row::WorktreeDivider);
        }
        rows.pop();
        (p, rows, tops)
    }

    #[test]
    fn a_dragged_worktree_stays_over_its_own_rows_and_goes_where_its_heading_is_nearest() {
        let order: Vec<BString> = ["a", "b", "c"].map(BString::from).into();
        let (p, rows, tops) = worktrees(["a", "b", "c"]);
        let top = |name: &str| (tops.iter().find(|(n, _)| n == name)).map_or(0., |(_, top)| *top);
        let b = BString::from("b");
        let goes = |depth| worktree_slot(&rows, &p, &b, &order, depth, ROW_HEIGHT);
        assert!(
            goes(top("b") + 1.).is_none() && goes(top("c") - 1.).is_none(),
            "over its own rows, down through its divider, what it carries lands there"
        );
        assert!(
            goes(top("a")) == Some(Slot::Before("a".into())),
            "over a's heading it goes before a"
        );
        assert!(
            goes(top("b") - 1.) == Some(Slot::Before("c".into())),
            "just above its own rows, its heading is nearest where it was, so it moves nowhere"
        );
        assert!(
            goes(top("c") + 2.) == Some(Slot::End),
            "just below them, its heading is nearest the place after c, as c is short"
        );
        assert!(
            goes(-10.).is_none(),
            "off its project's card it goes nowhere"
        );
    }

    #[test]
    fn under_the_filter_a_dragged_worktree_still_ends_at_the_next_one() {
        let order: Vec<BString> = ["a", "b"].map(BString::from).into();
        let p = ProjectId::from("p");
        let worktree = |name: &str| {
            Row::Item(Selection::Checkout(
                p.clone(),
                Checkout::Linked(name.into()),
            ))
        };
        let branch = || Row::Item(Selection::Branch(p.clone(), None));
        // The filter lists no dividers between worktrees, and another card follows.
        let rows = [
            Row::Item(Selection::Project(p.clone())),
            worktree("a"),
            branch(),
            worktree("b"),
            branch(),
            Row::Gap,
            Row::Item(Selection::Project(ProjectId::from("q"))),
            branch(),
            branch(),
        ];
        let top = |ix: usize| super::span_height(&rows, 0..ix, ROW_HEIGHT);
        let b = BString::from("b");
        let goes = |depth| worktree_slot(&rows, &p, &b, &order, depth, ROW_HEIGHT);
        assert!(
            goes(top(4) + 1.).is_none(),
            "over its own branch, what it carries lands there"
        );
        assert!(
            goes(top(1)) == Some(Slot::Before("a".into())),
            "over a's heading it goes before a"
        );
        let a = BString::from("a");
        assert!(
            worktree_slot(&rows, &p, &a, &order, top(4) + 1., ROW_HEIGHT) == Some(Slot::End),
            "and a dragged over b's branch goes below b"
        );
    }

    #[test]
    fn where_a_dragged_worktree_shows_never_changes_where_it_goes() {
        let order: Vec<BString> = ["a", "b", "c"].map(BString::from).into();
        let b = BString::from("b");
        let shown = [["b", "a", "c"], ["a", "b", "c"], ["a", "c", "b"]].map(worktrees);
        for depth in (0..300).map(|d| d as f32) {
            let went: Vec<Option<Slot<BString>>> = (shown.iter())
                .map(|(p, rows, _)| worktree_slot(rows, p, &b, &order, depth, ROW_HEIGHT))
                .collect();
            assert!(
                went.windows(2).all(|pair| pair.first() == pair.last()),
                "only the others, and where it was among them, set where it goes, at {depth}"
            );
        }
    }

    #[test]
    fn a_dragged_card_goes_where_its_heading_would_be_nearest_the_pointer() {
        let (a, b) = (ProjectId::from("a"), ProjectId::from("b"));
        let c = ProjectId::from("c");
        // Without b, a's card starts at 0, c's at 116 and the end at 172, the gap after each 16
        // with the cards' edges, so the middles of their headings are 20, 136 and 192 down.
        let listed = [(&a, 100.), (&b, 400.), (&c, 40.)];
        let goes = |depth| nearest_slot(&listed, &b, depth);
        assert!(
            goes(-50.) == CardSlot::Before(a.clone()),
            "above the cards, it goes first"
        );
        assert!(
            goes(77.) == CardSlot::Before(a.clone()),
            "nearer a's heading than c's"
        );
        assert!(
            goes(79.) == CardSlot::Before(c.clone()),
            "past halfway to c's"
        );
        assert!(
            goes(163.) == CardSlot::Before(c.clone()),
            "nearer c's heading than the end"
        );
        assert!(
            goes(165.) == CardSlot::End,
            "past halfway to the end, it goes last"
        );
    }

    #[test]
    fn where_a_dragged_card_shows_never_changes_where_it_goes() {
        let (a, b) = (ProjectId::from("a"), ProjectId::from("b"));
        let c = ProjectId::from("c");
        let orders = [
            [(&b, 400.), (&a, 100.), (&c, 40.)],
            [(&a, 100.), (&b, 400.), (&c, 40.)],
            [(&a, 100.), (&c, 40.), (&b, 400.)],
        ];
        for depth in (-40..400).map(|d| d as f32) {
            let went: Vec<CardSlot> = (orders.iter())
                .map(|listed| nearest_slot(listed, &b, depth))
                .collect();
            assert!(
                went.windows(2).all(|pair| pair.first() == pair.last()),
                "only the other cards set the places, so showing b in one never moves it on"
            );
        }
    }

    /// Project `p`'s card, as [`card`] has it.
    fn terminals(main: &[u64], linked: &[u64]) -> (ProjectId, Vec<Row>) {
        card("p", main, linked)
    }

    /// Project `name`'s card with terminals `main` open under its main worktree, above two
    /// branches, and `linked` under linked worktree `w`, above one; then linked worktree `f`,
    /// folded.
    fn card(name: &str, main: &[u64], linked: &[u64]) -> (ProjectId, Vec<Row>) {
        let p = ProjectId::from(name.to_owned());
        let terminal = |n: &u64| Row::Item(Selection::Terminal(p.clone(), TerminalId(*n)));
        let branch = || Row::Item(Selection::Branch(p.clone(), None));
        let mut rows = vec![
            Row::Item(Selection::Project(p.clone())),
            Row::Item(Selection::Checkout(p.clone(), Checkout::Main)),
            Row::Item(Selection::Uncommitted(p.clone(), Checkout::Main)),
        ];
        rows.extend(main.iter().map(terminal));
        rows.extend([branch(), branch(), Row::WorktreeDivider]);
        rows.push(Row::Item(Selection::Checkout(
            p.clone(),
            Checkout::Linked("w".into()),
        )));
        rows.extend(linked.iter().map(terminal));
        rows.extend([branch(), Row::WorktreeDivider]);
        rows.push(Row::Item(Selection::Checkout(
            p.clone(),
            Checkout::Linked("f".into()),
        )));
        rows.push(Row::WorktreeDivider);
        (p, rows)
    }

    /// The terminals of `p`'s worktrees open as the move started: 1 and 2 on the main one, 3
    /// on `w`, and none of folded `f`'s.
    fn lists() -> Vec<(ProjectCheckout, Vec<TerminalId>)> {
        vec![
            (on("p", Checkout::Main), vec![TerminalId(1), TerminalId(2)]),
            (on("p", Checkout::Linked("w".into())), vec![TerminalId(3)]),
        ]
    }

    fn on(project: &str, checkout: Checkout) -> ProjectCheckout {
        let project = ProjectId::from(project.to_owned());
        ProjectCheckout { project, checkout }
    }

    #[test]
    fn the_navigator_takes_a_terminal_only_past_the_zone_along_the_left_of_the_tiles() {
        let bounds = Bounds::new(point(fixed(0.), fixed(0.)), size(fixed(340.), fixed(800.)));
        let at = |x| point(fixed(x), fixed(100.));
        let end = Some(fixed(288.));
        assert!(
            !takes_terminal(bounds, at(300.), end),
            "over the navigator but in the zone, the zone has it"
        );
        assert!(
            takes_terminal(bounds, at(287.), end),
            "past the zone, the navigator takes it"
        );
        assert!(
            takes_terminal(bounds, at(300.), None),
            "with no zone, as for a row, all of the navigator takes it"
        );
        assert!(
            !takes_terminal(bounds, at(400.), None),
            "outside the navigator it stays where it is"
        );
    }

    #[test]
    fn a_terminal_from_a_tile_goes_where_its_row_would_be_nearest() {
        let (_, rows) = terminals(&[1, 2], &[3]);
        let lists = lists();
        let middle = |ix: usize| super::span_height(&rows, 0..ix, ROW_HEIGHT) + ROW_HEIGHT / 2.;
        let goes = |depth| terminal_slot(&rows, TerminalId(4), &lists, depth, ROW_HEIGHT);
        assert!(
            goes(middle(4)) == Some((on("p", Checkout::Main), Slot::Before(TerminalId(2)))),
            "unlisted, it has no row of its own to leave out, so it goes before the row it's over"
        );
        assert!(
            goes(-10.).is_none(),
            "off the cards it goes nowhere, and stays a tile"
        );
    }

    #[test]
    fn a_dragged_terminal_goes_where_its_row_would_be_nearest_among_every_open_worktree() {
        let (_, rows) = terminals(&[1, 2], &[3]);
        let lists = lists();
        let middle = |ix: usize| super::span_height(&rows, 0..ix, ROW_HEIGHT) + ROW_HEIGHT / 2.;
        let goes = |depth| terminal_slot(&rows, TerminalId(2), &lists, depth, ROW_HEIGHT);
        let (main, w) = (
            on("p", Checkout::Main),
            on("p", Checkout::Linked("w".into())),
        );
        assert!(
            goes(middle(3)) == Some((main.clone(), Slot::Before(TerminalId(1)))),
            "over terminal 1 it goes before it"
        );
        assert!(
            goes(middle(4)) == Some((main, Slot::End)),
            "over its own row it stays where it is, after 1"
        );
        assert!(
            goes(middle(8)) == Some((w.clone(), Slot::Before(TerminalId(3)))),
            "over w's heading it goes first under it, as its row would be there once it's gone \
             from the main worktree"
        );
        assert!(
            goes(middle(9)) == Some((w.clone(), Slot::End)),
            "over terminal 3 it goes after it, as 3 moves up a row once it's gone from the main \
             worktree"
        );
        assert!(
            goes(middle(12)) == Some((w, Slot::End)),
            "folded f lists no terminals to go among, so the nearest place is under w"
        );
        assert!(
            goes(-10.).is_none()
                && goes(super::span_height(&rows, 0..rows.len(), ROW_HEIGHT)).is_none(),
            "off the cards it goes nowhere"
        );
    }

    #[test]
    fn a_dragged_terminal_goes_to_the_same_place_on_a_card_from_wherever_it_shows() {
        let two_cards = |p: &[u64], q: &[u64]| {
            let (_, mut rows) = card("p", p, &[]);
            rows.push(Row::Gap);
            rows.extend(card("q", q, &[]).1);
            rows
        };
        let lists = [
            (on("p", Checkout::Main), vec![TerminalId(1), TerminalId(2)]),
            (on("q", Checkout::Main), vec![TerminalId(3)]),
        ];
        let (still_above, there) = (two_cards(&[1, 2], &[3]), two_cards(&[1], &[3, 2]));
        let own_row = (there.iter())
            .position(|row| matches!(row, Row::Item(Selection::Terminal(_, id)) if id.0 == 2))
            .expect("it's listed on q");
        let depth = super::span_height(&there, 0..own_row, ROW_HEIGHT) + ROW_HEIGHT / 2.;
        let goes = |rows| terminal_slot(rows, TerminalId(2), &lists, depth, ROW_HEIGHT);
        assert!(
            goes(&there) == Some((on("q", Checkout::Main), Slot::End)),
            "over its own row, after 3, it stays there"
        );
        assert!(
            goes(&still_above) == goes(&there),
            "its row leaves the card above as it lands, so that row counts for nothing"
        );
    }

    #[test]
    fn a_dragged_terminal_goes_among_another_projects_terminals_on_its_card() {
        let (_, mut rows) = terminals(&[1, 2], &[3]);
        rows.push(Row::Gap);
        let gap = rows.len() - 1;
        let (_, q) = card("q", &[4], &[]);
        let q_start = rows.len();
        rows.extend(q);
        let mut lists = lists();
        lists.extend([
            (on("q", Checkout::Main), vec![TerminalId(4)]),
            (on("q", Checkout::Linked("w".into())), vec![]),
        ]);
        let middle = |ix: usize| super::span_height(&rows, 0..ix, ROW_HEIGHT) + ROW_HEIGHT / 2.;
        let goes = |depth| terminal_slot(&rows, TerminalId(2), &lists, depth, ROW_HEIGHT);
        let q_main = on("q", Checkout::Main);
        assert!(
            goes(middle(q_start + 2)) == Some((q_main.clone(), Slot::Before(TerminalId(4)))),
            "over the row above q's terminal 4 it goes before it, in q's main worktree, as 4 \
             moves up a row once it's gone from p"
        );
        assert!(
            goes(middle(q_start + 3)) == Some((q_main.clone(), Slot::End)),
            "over terminal 4 it goes after it, for the same reason"
        );
        assert!(
            goes(middle(q_start + 4)) == Some((q_main, Slot::End)),
            "over q's first branch it goes after q's terminals"
        );
        assert!(
            goes(middle(gap)).is_none(),
            "between the cards it goes nowhere"
        );
    }

    #[test]
    fn where_a_dragged_terminal_shows_never_changes_where_it_goes() {
        let lists = lists();
        let shown = [
            terminals(&[1, 2], &[3]),
            terminals(&[2, 1], &[3]),
            terminals(&[2], &[1, 3]),
            terminals(&[2], &[3, 1]),
        ];
        for depth in (0..400).map(|d| d as f32) {
            let went: Vec<_> = (shown.iter())
                .map(|(_, rows)| terminal_slot(rows, TerminalId(1), &lists, depth, ROW_HEIGHT))
                .collect();
            assert!(
                went.windows(2).all(|pair| pair.first() == pair.last()),
                "only the others set where it goes, at {depth}"
            );
        }
    }

    #[test]
    fn cards_run_from_their_heading_to_the_gap_before_the_next() {
        let (a, b) = (ProjectId::from("a"), ProjectId::from("b"));
        let rows = [
            Row::Item(Selection::Project(a.clone())),
            Row::Item(Selection::Uncommitted(a.clone(), Checkout::Main)),
            Row::Item(Selection::Terminal(a.clone(), TerminalId(1))),
            Row::Gap,
            Row::Item(Selection::Project(b.clone())),
        ];
        assert!(
            cards(&rows) == vec![(&a, 0..3), (&b, 4..5)],
            "the gap belongs to neither card"
        );
    }
}
