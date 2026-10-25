//! The order of the watched projects' cards: as the user drags them, by name, or by last
//! activity, from the header's view menu, which also folds them all or opens them all. Also
//! dragging a card to another place, and how the projects not watched are listed.

use std::cmp::Reverse;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, DragMoveEvent, Pixels, Point, SharedString,
    Stateful, Window, div, prelude::*,
};

use super::Navigator;
use super::rows::last_active;
use crate::app::AppView;
use crate::overlays::MenuItem;
use crate::state::{Prefs, ProjectEntry, ProjectId, Store, WatchedOrder};
use crate::theme::Theme;
use crate::transfer::drag_pill;
use crate::ui::{Icon, dp, icon_button};

impl Navigator {
    /// The watched projects, in the order picked.
    pub(super) fn watched<'a>(&self, store: &'a Store) -> Vec<&'a ProjectEntry> {
        // Listed in the manual order already.
        let mut watched: Vec<&ProjectEntry> =
            store.projects.iter().filter(|e| e.expanded).collect();
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

    /// How the watched projects go: their order, then folding or opening them all, then the
    /// app's settings.
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
            MenuItem::action("Settings…", &crate::settings::OpenSettings, cx).separated(),
        ]
    }

    /// How the projects not expanded are listed, with headed choices as ui-react's view
    /// options menu has them: their order, and which to show.
    pub(super) fn view_options(&self, cx: &App) -> Vec<MenuItem> {
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

    /// Let a watched project's heading drag its card among the others (see [`Self::card_drop`]).
    pub(super) fn reorderable(
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
    pub(super) fn card_drop(&self, slot: CardSlot, cx: &Context<Self>) -> Div {
        let (over, onto) = (slot.clone(), slot);
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(dp(-10.))
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
}

/// A watched project's card being dragged, by its heading.
pub(super) struct ProjectDrag(ProjectId);

/// Where a dragged project card goes: before a watched project's card, or after them all.
#[derive(Clone, PartialEq)]
pub(super) enum CardSlot {
    Before(ProjectId),
    End,
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
