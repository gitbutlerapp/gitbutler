//! The order of the watched projects' cards: as the user drags them, by name, or by last
//! activity, from the header's view menu, which also folds them all or opens them all. Also
//! dragging a card to another place, the others sliding to make room, and how the projects
//! not watched are listed.

use std::cmp::Reverse;
use std::ops::Range;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, DragMoveEvent, EmptyView, Pixels, Point, Stateful,
    Window, actions, prelude::*,
};

use super::Navigator;
use super::rows::{HEADING_MIDDLE, Row, last_active, span_height};
use crate::app::AppView;
use crate::overlays::MenuItem;
use crate::state::{Prefs, ProjectEntry, ProjectId, Selection, Store, WatchedOrder};
use crate::theme::Theme;
use crate::ui::{Icon, icon_button};

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
        match store.prefs.watched_order {
            // A card dragged by its heading shows where letting go puts it.
            WatchedOrder::Manual => {
                if let Some((dragged, slot)) = &self.project_drag
                    && let Some(from) = watched.iter().position(|e| e.id == *dragged)
                {
                    let entry = watched.remove(from);
                    let to = match slot {
                        CardSlot::Before(before) => watched.iter().position(|e| e.id == *before),
                        CardSlot::End => None,
                    };
                    watched.insert(to.unwrap_or(watched.len()), entry);
                }
            }
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

    /// Let a watched project's heading drag its card among the others, which make room for it
    /// where it'd go (see [`Self::follow_project_drag`]). The card shows where it goes, so
    /// nothing follows the pointer.
    pub(super) fn reorderable(
        &self,
        el: Stateful<Div>,
        id: &ProjectId,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let navigator = cx.entity().downgrade();
        el.on_drag(ProjectDrag(id.clone()), move |_, _, _, cx| {
            let nothing = cx.new(|_| EmptyView);
            // A drop anywhere else, or none, puts the cards back as they were.
            let navigator = navigator.clone();
            cx.observe_release(&nothing, move |_, cx| {
                (navigator.update(cx, |this, cx| {
                    this.project_drag = None;
                    cx.notify()
                }))
                .ok();
            })
            .detach();
            nothing
        })
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
        let dragged = event.drag(cx).0.clone();
        let depth = (event.bounds.contains(&at))
            .then(|| self.depth_at(at, Theme::get(cx)))
            .flatten();
        let next = depth.map(|depth| {
            let cards: Vec<(&ProjectId, f32)> = (cards(&self.rows).into_iter())
                .map(|(id, span)| (id, span_height(&self.rows, span)))
                .collect();
            let slot = nearest_slot(&cards, &dragged, depth);
            (dragged, slot)
        });
        if self.project_drag != next {
            self.project_drag = next;
            cx.notify();
        }
    }

    /// Let go of a card dragged by its heading where it's shown.
    pub(super) fn drop_project(&mut self, _: &ProjectDrag, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((id, slot)) = self.project_drag.take()
            && slot_moves(self.store.read(cx), &id, &slot)
        {
            let before = match &slot {
                CardSlot::Before(before) => Some(before),
                CardSlot::End => None,
            };
            (self.store).update(cx, |store, cx| store.move_watched(&id, before, cx));
        }
        cx.notify();
    }

    /// The rows of the card dragged by its heading, while it shows where it'd go.
    pub(super) fn held_card(&self) -> Option<Range<usize>> {
        let (dragged, _) = self.project_drag.as_ref()?;
        (cards(&self.rows).into_iter())
            .find(|(id, _)| *id == dragged)
            .map(|(_, span)| span)
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

/// A watched project's card being dragged, by its heading.
pub(super) struct ProjectDrag(ProjectId);

/// Where a dragged project card goes: before a watched project's card, or after them all.
#[derive(Clone, PartialEq)]
pub(super) enum CardSlot {
    Before(ProjectId),
    End,
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

/// Whether dropping watched project `id` at `slot` moves its card: not right before or after
/// itself, where it already is.
fn slot_moves(store: &Store, id: &ProjectId, slot: &CardSlot) -> bool {
    let watched = store.expanded_ids();
    let Some(from) = watched.iter().position(|other| other == id) else {
        return false;
    };
    let to = match slot {
        CardSlot::Before(before) => watched.iter().position(|other| other == before),
        CardSlot::End => Some(watched.len()),
    };
    to.is_some_and(|to| to != from && to != from + 1)
}

#[cfg(test)]
mod tests {
    use super::{CardSlot, Row, cards, nearest_slot};
    use crate::state::{ProjectId, Selection};

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
