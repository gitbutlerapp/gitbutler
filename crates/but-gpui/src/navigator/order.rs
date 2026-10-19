//! The order of the watched projects' cards: as the user drags them, by name, or by last
//! activity, from the header's view menu, which also folds them all or opens them all.

use std::cmp::Reverse;

use gpui::{AnyElement, App, ClickEvent, Context, prelude::*};

use super::{Navigator, last_active};
use crate::app::AppView;
use crate::overlays::MenuItem;
use crate::state::{ProjectEntry, ProjectId, Store, WatchedOrder};
use crate::ui::{Icon, icon_button};

impl Navigator {
    /// The watched projects, in the order picked.
    pub(super) fn watched<'a>(&self, store: &'a Store) -> Vec<&'a ProjectEntry> {
        // Listed in the manual order already.
        let mut watched: Vec<&ProjectEntry> =
            store.projects.iter().filter(|e| e.expanded).collect();
        match store.prefs.watched_order {
            WatchedOrder::Manual => {}
            WatchedOrder::Name => watched.sort_by_key(|e| e.project.name.to_lowercase()),
            WatchedOrder::Activity => watched.sort_by_key(|e| Reverse(last_active(e))),
        }
        watched
    }

    fn watched_ids(&self, cx: &App) -> Vec<ProjectId> {
        let store = self.store.read(cx);
        (self.watched(store).into_iter())
            .map(|e| e.id.clone())
            .collect()
    }

    /// The button beside the account that opens the view menu.
    pub(super) fn view_button(&self, cx: &Context<Self>) -> AnyElement {
        icon_button("watched-view-options", Icon::Mixer)
            .small()
            .tooltip("Order and fold watched projects")
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                let items = this.view_menu(cx);
                let position = event.position();
                (this.app)
                    .update(cx, |app, cx| app.open_menu(position, items, cx))
                    .ok();
            }))
            .into_any_element()
    }

    /// How the watched projects go: their order, then folding or opening them all, then the
    /// app's settings.
    fn view_menu(&self, cx: &App) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let order = store.prefs.watched_order;
        let shown = self.watched_ids(cx);
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
}
