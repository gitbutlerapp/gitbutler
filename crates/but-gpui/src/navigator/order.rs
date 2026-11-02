//! The order of the watched projects' cards: as the user drags them, by name, or by last
//! activity, from the header's view menu, which also folds them all or opens them all. Also
//! dragging a card or a linked worktree to another place, the others sliding to make room, and
//! how the projects not watched are listed.

use std::cmp::Reverse;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, DragMoveEvent, Pixels, Point, SharedString,
    Stateful, Window, actions, prelude::*,
};

use bstr::{BStr, BString};

use super::Navigator;
use super::rows::{HEADING_MIDDLE, Row, in_checkout, last_active, listed_with, span_height};
use crate::app::{AppView, MoveHere};
use crate::backend::Checkout;
use crate::overlays::MenuItem;
use crate::state::{Cut, Prefs, ProjectEntry, ProjectId, Selection, Store, WatchedOrder};
use crate::theme::Theme;
use crate::transfer::{
    CardSlot, Move, Moving, Op, ProjectDrag, Slot, Stop, drag_preview, project_preview,
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
        set::<ShowActiveProjects>(el, |p| p.active_projects_only = true, cx).on_action(cx.listener(
            |this, _: &crate::detail::ShowOnlyLocalBranches, _, cx| {
                (this.store).update(cx, |store, cx| {
                    store.set_prefs(|p| p.branches_local_only = !p.branches_local_only, cx)
                });
                // The branch opened may have just been filtered out.
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

    /// How the projects not expanded are listed, with headed choices as ui-react's view
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
        id: &ProjectId,
        label: SharedString,
        reorders: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let navigator = cx.entity().downgrade();
        let preview = project_preview(label);
        let drag = ProjectDrag {
            id: id.clone(),
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
    fn hold_card(&self, id: &ProjectId, slot: Option<CardSlot>, cx: &mut Context<Self>) {
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
    fn hold_worktree(
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
                .map(|(id, span)| (id, span_height(&self.rows, span)))
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
        project: &ProjectId,
        name: &BStr,
        payload: Cut,
        chip: SharedString,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let navigator = cx.entity().downgrade();
        let preview = drag_preview(self.store.clone(), chip);
        let dragged = (project.clone(), name.to_owned());
        el.on_drag(payload, move |payload, offset, window, cx| {
            let pill = preview(payload, offset, window, cx);
            let (navigator, dragged) = (navigator.clone(), dragged.clone());
            let started = dragged.clone();
            (navigator.update(cx, |this, _| this.dragged_worktree = Some(started))).ok();
            // A drop anywhere else, or none, puts it back where it was.
            cx.observe_release(&pill, move |_, cx| {
                (navigator.update(cx, |this, cx| {
                    this.dragged_worktree = None;
                    let (project, name) = &dragged;
                    this.hold_worktree(project, name.as_ref(), None, cx)
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
        event: &DragMoveEvent<Cut>,
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
        let slot = (depth.zip(order))
            .and_then(|(depth, order)| worktree_slot(&self.rows, project, name, &order, depth));
        self.hold_worktree(project, name.as_ref(), slot, cx);
    }

    /// Whether a drop on row `item` lands there: always, but while a linked worktree is dragged
    /// by its heading, only on its own rows, and only while it's where it was.
    pub(super) fn takes_drop(&self, item: &Selection, store: &Store) -> bool {
        let Some((project, name)) = &self.dragged_worktree else {
            return true;
        };
        let own = match item {
            Selection::Worktree(_, heading) => heading == name,
            _ => (store.workspace(project))
                .is_some_and(|ws| in_checkout(store, item, ws, &Checkout::Linked(name.clone()))),
        };
        item.project() == project && own && !worktree_held(self.dragged_worktree.as_ref(), store)
    }

    /// Let go of the linked worktree dragged by its heading where it's shown, as Return lands
    /// it, if it's moving.
    pub(super) fn drop_worktree(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if worktree_held(self.dragged_worktree.as_ref(), self.store.read(cx)) {
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
            |row| matches!(row, Row::Item(Selection::Worktree(p, n)) if p == project && n == name),
        )?;
        Some(heading..worktree_end(&self.rows, heading))
    }

    /// How far down the cards' rows `at` is, from the top of the first, as the rows count
    /// their heights ([`span_height`]): from the drawn row it's over, or else the nearest.
    fn depth_at(&self, at: Point<Pixels>, theme: &Theme) -> Option<f32> {
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
        let above = span_height(&self.rows, 0..ix);
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
    let gap = Row::Gap.height();
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
            Row::Item(Selection::Worktree(..)) | Row::Gap => Some(ix),
            _ => None,
        })
        .unwrap_or(rows.len())
}

/// Those listed with linked worktree `name` of `project` as the move of it under way started,
/// if it's moving.
fn moving_worktree<'a>(
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
) -> Option<Slot<BString>> {
    let (_, card) = cards(rows).into_iter().find(|(id, _)| *id == project)?;
    let top = span_height(rows, 0..card.start);
    if depth < top || depth >= top + span_height(rows, card.clone()) {
        return None;
    }
    // Each one listed with it as shown, by its name, how far down its heading is, and how tall
    // it is, down through the divider below it.
    let shown: Vec<(&BString, f32, f32)> = card
        .filter_map(|ix| match rows.get(ix)? {
            Row::Item(Selection::Worktree(p, n)) if p == project && order.contains(n) => {
                let end = worktree_end(rows, ix);
                Some((n, span_height(rows, 0..ix), span_height(rows, ix..end)))
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
    let off = |top: f32| (top + ROW_HEIGHT / 2. - depth).abs();
    let (nearest, _) =
        (tops.iter().enumerate()).min_by(|(_, a), (_, b)| off(**a).total_cmp(&off(**b)))?;
    Some(match others.get(nearest) {
        Some((before, _)) => Slot::Before((*before).clone()),
        None => Slot::End,
    })
}

#[cfg(test)]
mod tests {
    use bstr::BString;

    use super::{CardSlot, Row, cards, nearest_slot, worktree_slot};
    use crate::state::{ProjectId, Selection};
    use crate::transfer::Slot;

    /// Project `p`'s card with linked worktrees `a`, `b` and `c` listed in `order`, each its
    /// heading, `b` open on two more rows, and a divider below; and its rows, with how far down
    /// each worktree's heading is.
    fn worktrees(order: [&str; 3]) -> (ProjectId, Vec<Row>, Vec<(BString, f32)>) {
        let p = ProjectId::from("p");
        let mut rows = vec![
            Row::Item(Selection::Project(p.clone())),
            Row::Item(Selection::Uncommitted(p.clone())),
            Row::WorktreeDivider,
        ];
        let mut tops = Vec::new();
        for name in order {
            tops.push((
                BString::from(name),
                super::span_height(&rows, 0..rows.len()),
            ));
            rows.push(Row::Item(Selection::Worktree(p.clone(), name.into())));
            if name == "b" {
                let branch = Selection::Branch(p.clone(), None);
                rows.extend([Row::Item(branch.clone()), Row::Item(branch)]);
            }
            rows.push(Row::WorktreeDivider);
        }
        rows.push(Row::Item(Selection::Branches(p.clone())));
        (p, rows, tops)
    }

    #[test]
    fn a_dragged_worktree_stays_over_its_own_rows_and_goes_where_its_heading_is_nearest() {
        let order: Vec<BString> = ["a", "b", "c"].map(BString::from).into();
        let (p, rows, tops) = worktrees(["a", "b", "c"]);
        let top = |name: &str| (tops.iter().find(|(n, _)| n == name)).map_or(0., |(_, top)| *top);
        let b = BString::from("b");
        let goes = |depth| worktree_slot(&rows, &p, &b, &order, depth);
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
        let worktree = |name: &str| Row::Item(Selection::Worktree(p.clone(), name.into()));
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
        let top = |ix: usize| super::span_height(&rows, 0..ix);
        let b = BString::from("b");
        let goes = |depth| worktree_slot(&rows, &p, &b, &order, depth);
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
            worktree_slot(&rows, &p, &a, &order, top(4) + 1.) == Some(Slot::End),
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
                .map(|(p, rows, _)| worktree_slot(rows, p, &b, &order, depth))
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
        // Without b, a's card starts at 0, c's at 120 and the end at 180, the gap after each 20
        // with the cards' edges, so the middles of their headings are 20, 140 and 200 down.
        let listed = [(&a, 100.), (&b, 400.), (&c, 40.)];
        let goes = |depth| nearest_slot(&listed, &b, depth);
        assert!(
            goes(-50.) == CardSlot::Before(a.clone()),
            "above the cards, it goes first"
        );
        assert!(
            goes(79.) == CardSlot::Before(a.clone()),
            "nearer a's heading than c's"
        );
        assert!(
            goes(81.) == CardSlot::Before(c.clone()),
            "past halfway to c's"
        );
        assert!(
            goes(169.) == CardSlot::Before(c.clone()),
            "nearer c's heading than the end"
        );
        assert!(
            goes(171.) == CardSlot::End,
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

    #[test]
    fn cards_run_from_their_heading_to_the_gap_before_the_next() {
        let (a, b) = (ProjectId::from("a"), ProjectId::from("b"));
        let rows = [
            Row::Item(Selection::Project(a.clone())),
            Row::Item(Selection::Uncommitted(a.clone())),
            Row::Item(Selection::History(a.clone())),
            Row::Gap,
            Row::Item(Selection::Project(b.clone())),
        ];
        assert!(
            cards(&rows) == vec![(&a, 0..3), (&b, 4..5)],
            "the gap belongs to neither card"
        );
    }
}
