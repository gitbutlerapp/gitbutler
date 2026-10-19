//! A watched project's checkouts on the user's other machines, after its own worktrees: each
//! a heading that opens its page, with its unfinished branches under it.

use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, MouseButton, MouseDownEvent, Window, div, prelude::*, px,
};

use super::{NEST, Navigator, Row, RowStyle, Rows, meta_count};
use crate::backend::mesh::{self, Checkout};
use crate::detail::branch_glyph;
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::ui::{
    Icon, ROW_HEIGHT, Tooltip, dot, icon, just_now, plural, section_label, small_icon,
};

impl Navigator {
    /// `id`'s checkouts on other machines, under a heading, each with its branches unless
    /// it's folded without the selection on it; then a line ruling them off.
    pub(super) fn mesh_rows(&self, store: &Store, id: &ProjectId, rows: &mut Rows) {
        let checkouts = store.mesh_checkouts(id);
        if checkouts.is_empty() {
            return;
        }
        if !matches!(rows.rows.last(), Some(Row::WorktreeDivider)) {
            rows.push(Row::WorktreeDivider);
        }
        rows.push(Row::MeshHeading);
        for checkout in checkouts {
            let key: Arc<str> = checkout.id.as_str().into();
            let item = Selection::Machine(id.clone(), key.clone());
            let selected = store.selection.as_ref() == Some(&item);
            rows.push(Row::Item(item));
            if checkout.branches.is_empty()
                || (self.folded_machines.contains(&(id.clone(), key.clone())) && !selected)
            {
                continue;
            }
            rows.open();
            rows.inset = NEST;
            for branch in &checkout.branches {
                rows.push(Row::MeshBranch(
                    id.clone(),
                    key.clone(),
                    branch.name.clone(),
                ));
            }
            rows.inset = 0.;
            rows.close();
        }
        rows.push(Row::WorktreeDivider);
    }

    /// [`Self::mesh_rows`] under the filter, with `held` the words found in the project's
    /// name: the checkouts that match, or have branches that do, with those branches.
    pub(super) fn filtered_mesh_rows(
        &self,
        store: &Store,
        id: &ProjectId,
        terms: &[String],
        held: u64,
        rows: &mut Rows,
    ) {
        for checkout in store.mesh_checkouts(id) {
            let own = super::found(terms, &[&checkout.label()]);
            let hit = super::matches(terms, held, own);
            let branches: Vec<_> = (checkout.branches.iter())
                .filter(|b| super::matches(terms, held | own, super::found(terms, &[&b.name])))
                .collect();
            if !hit && branches.is_empty() {
                continue;
            }
            let key: Arc<str> = checkout.id.as_str().into();
            rows.push_found(Selection::Machine(id.clone(), key.clone()), hit);
            rows.open();
            rows.inset = NEST;
            for branch in branches {
                rows.push(Row::MeshBranch(
                    id.clone(),
                    key.clone(),
                    branch.name.clone(),
                ));
            }
            rows.inset = 0.;
            rows.close();
        }
    }

    /// Over the other machines' checkouts, as the worktrees' sections are labelled.
    pub(super) fn mesh_heading(&self, theme: &Theme) -> AnyElement {
        div()
            .id("mesh-heading")
            .h(px(ROW_HEIGHT))
            .px(px(14.))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(small_icon(Icon::Globe).text_color(theme.text_3))
            .child(section_label("Other machines", theme))
            .tooltip(Tooltip::text(
                "Unfinished work on your other machines, from your mesh",
            ))
            .into_any_element()
    }

    /// A checkout on another machine, which opens its page: whether that machine is online,
    /// and what's unfinished there. A chevron folds its branches away.
    pub(super) fn machine_row(
        &self,
        ix: usize,
        item: &Selection,
        style: RowStyle,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let Selection::Machine(project, id) = item else {
            return None;
        };
        let store = self.store.read(cx);
        let checkout = store.mesh_checkout(project, id)?;
        let theme = style.theme;
        let key = (project.clone(), id.clone());
        let open = self.machine_open(&key, store);
        let step_color = match style.solid {
            true => style.muted(),
            false => theme.text_3,
        };
        let step = div()
            .id(("machine-fold", ix))
            .flex_none()
            .w(px(14.))
            .flex()
            .justify_center();
        let step = match checkout.branches.is_empty() {
            true => step.child(small_icon(Icon::BulletDisc).text_color(step_color)),
            false => {
                let key = key.clone();
                step.child(
                    small_icon(match open {
                        true => Icon::ChevronDown,
                        false => Icon::ChevronRight,
                    })
                    .text_color(step_color),
                )
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    this.toggle_machine(key.clone(), open, window, cx);
                }))
            }
        };
        let files = checkout.uncommitted.len();
        let unpushed: usize = (checkout.branches.iter())
            .map(|b| b.commits.iter().filter(|c| !c.pushed).count())
            .sum();
        let active = checkout.online && checkout.last_activity.is_some_and(just_now);
        Some(
            style
                .row(format!("machine-{project}-{id}"), 6.)
                .aria_label(checkout.label())
                .aria_expanded(open)
                .child(step)
                .child(icon(Icon::Globe).text_color(style.muted()))
                .child(
                    div()
                        .id(("machine-name", ix))
                        .flex_grow(1.)
                        .min_w(px(0.))
                        .truncate()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .when(!checkout.online, |el| el.text_color(style.muted()))
                        .tooltip(Tooltip::text(checkout.path.clone()))
                        .child(checkout.label()),
                )
                .children(meta_count(
                    Icon::File,
                    files,
                    style.meta(theme.warn),
                    plural(files, "uncommitted file"),
                ))
                .when(!open, |el| {
                    el.children(meta_count(
                        Icon::ArrowUp,
                        unpushed,
                        style.meta(theme.blue),
                        plural(unpushed, "unpushed commit"),
                    ))
                })
                .child(
                    div()
                        .id(("machine-online", ix))
                        .child(dot(match (active, checkout.online) {
                            (true, _) => style.meta(theme.safe),
                            (false, true) => style.muted(),
                            (false, false) => style.muted().opacity(0.4),
                        }))
                        .tooltip(Tooltip::text(match (active, checkout.online) {
                            (true, _) => "Online, changed in the last 2 minutes",
                            (false, true) => "Online",
                            (false, false) => "Offline",
                        })),
                )
                .on_mouse_down(MouseButton::Left, self.select_on_press(item, cx))
                .when(!checkout.branches.is_empty(), |el| {
                    el.on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        if event.click_count() == 2 {
                            this.toggle_machine(key.clone(), open, window, cx);
                        }
                    }))
                })
                .into_any_element(),
        )
    }

    /// A branch on another machine. A click opens its checkout's page, which lists its
    /// commits; right-clicking offers to bring it here.
    pub(super) fn mesh_branch_row(
        &self,
        ix: usize,
        project: &ProjectId,
        id: &Arc<str>,
        name: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return div().into_any_element();
        };
        let Some(branch) = checkout.branches.iter().find(|b| b.name == name) else {
            return div().into_any_element();
        };
        let (glyph, color) = branch_glyph(branch.status, theme);
        let unpushed = branch.commits.iter().filter(|c| !c.pushed).count();
        let page = Selection::Machine(project.clone(), id.clone());
        let menu = (page.clone(), name.to_owned());
        let pick = (project.clone(), id.clone(), name.to_owned());
        gpui::div()
            .id(("mesh-branch", ix))
            .h(px(ROW_HEIGHT))
            .mx(px(3.))
            .pl(px(14.))
            .pr(px(12.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|el| el.bg(theme.hover()))
            .text_color(match checkout.online {
                true => theme.text_1,
                false => theme.text_2,
            })
            .child(
                div()
                    .flex_none()
                    .w(px(14.))
                    .flex()
                    .justify_center()
                    .child(small_icon(Icon::BulletDisc).text_color(theme.text_3)),
            )
            .child(icon(glyph).text_color(color))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child(name.to_owned()),
            )
            .children(meta_count(
                Icon::ArrowUp,
                unpushed,
                theme.blue,
                plural(unpushed, "unpushed commit"),
            ))
            .children(meta_count(
                Icon::Commit,
                branch.commits.len(),
                theme.text_2,
                plural(branch.commits.len(), "commit"),
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    let (project, id, name) = &pick;
                    (this.detail).update(cx, |detail, cx| {
                        detail.pick_machine_branch(project, id, name, cx)
                    });
                    this.select(page.clone(), window, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let (page, name) = &menu;
                    let items = this.mesh_branch_menu(page, name, cx);
                    this.select(page.clone(), window, cx);
                    if !items.is_empty() {
                        this.app
                            .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                            .ok();
                    }
                }),
            )
            .into_any_element()
    }

    /// What right-clicking another machine's branch `name` offers: bringing it here, and
    /// its page on but.dev.
    fn mesh_branch_menu(&self, page: &Selection, name: &str, cx: &gpui::App) -> Vec<MenuItem> {
        let Selection::Machine(project, id) = page else {
            return Vec::new();
        };
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return Vec::new();
        };
        let Some(branch) = checkout.branches.iter().find(|b| b.name == name) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let managed = store.workspace(project).is_some_and(|ws| ws.managed);
        if let Some(mesh_id) = branch.mesh_id.clone().filter(|_| managed) {
            let (project, name) = (project.clone(), branch.name.clone());
            items.push(MenuItem::new("Apply here", move |app, _, cx| {
                let (mesh_id, name) = (mesh_id.clone(), name.clone());
                app.store.update(cx, |store, cx| {
                    store.mesh_apply(&project, mesh_id, name, cx)
                })
            }));
        }
        items.extend(link_items(store, checkout, Some(&branch.name)));
        items
    }

    /// What right-clicking a checkout on another machine offers.
    pub(super) fn machine_menu(
        &self,
        project: &ProjectId,
        id: &str,
        cx: &gpui::App,
    ) -> Vec<MenuItem> {
        let store = self.store.read(cx);
        let Some(checkout) = store.mesh_checkout(project, id) else {
            return Vec::new();
        };
        let mut items = link_items(store, checkout, None);
        items.push(MenuItem::action("Copy path", &crate::app::CopySelection, cx).separated());
        items
    }

    /// Whether checkout `key` lists its branches: it isn't folded, or it's selected.
    fn machine_open(&self, key: &(ProjectId, Arc<str>), store: &Store) -> bool {
        !self.folded_machines.contains(key)
            || store.selection.as_ref() == Some(&Selection::Machine(key.0.clone(), key.1.clone()))
    }

    pub(super) fn toggle_machine(
        &mut self,
        key: (ProjectId, Arc<str>),
        open: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match open {
            true => self.folded_machines.insert(key),
            false => self.folded_machines.remove(&key),
        };
        cx.notify();
    }
}

/// Opening `checkout`, or its `branch`, on but.dev, where its diffs are.
fn link_items(store: &Store, checkout: &Checkout, branch: Option<&str>) -> Vec<MenuItem> {
    let Some(connection) = store.mesh.connection() else {
        return Vec::new();
    };
    let url = mesh::link(&connection.server, &checkout.id, branch);
    vec![MenuItem::new("Open on but.dev", move |_, _, cx| {
        cx.open_url(&url)
    })]
}
