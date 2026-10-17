//! The History page: what GitButler did to a project, newest first, the files the selected
//! operation changed in the diff beside it, and a way back to before any of them.

use std::ops::Range;

use gpui::{AnyElement, Context, SharedString, Window, div, prelude::*, px, uniform_list};

use super::Detail;
use crate::backend::history::Operation;
use crate::backend::{self, DiffSource, SnapshotId};
use crate::state::{Overlay, ProjectId};
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{list_row, relative_time};

/// How many operations a page of the log holds.
const PAGE: usize = 60;

pub(super) struct History {
    pub project: ProjectId,
    pub operations: Vec<Operation>,
    /// Whether older operations remain to load.
    pub more: bool,
    pub loading: bool,
    pub selected: Option<SnapshotId>,
}

impl Detail {
    /// Load `project`'s newest operations, or with `more` the next page after those shown.
    pub(super) fn load_history(&mut self, project: &ProjectId, more: bool, cx: &mut Context<Self>) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        let after = self
            .history
            .as_ref()
            .filter(|_| more)
            .and_then(|history| history.operations.last())
            .map(|operation| operation.id);
        if let Some(history) = &mut self.history
            && history.project == *project
        {
            history.loading = true;
        }
        let project = project.clone();
        self._history_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || backend::history::list(&path, after, PAGE)).await;
            this.update(cx, |this, cx| {
                let (operations, more) = match result {
                    Ok(page) => page,
                    Err(err) => {
                        this.store.update(cx, |store, cx| {
                            store.error("Couldn't read the history", format!("{err:#}"), cx)
                        });
                        (Vec::new(), false)
                    }
                };
                let selected_before = this.history.as_ref().and_then(|h| h.selected);
                let history = match &mut this.history {
                    Some(history) if history.project == project && after.is_some() => {
                        history.operations.extend(operations);
                        history
                    }
                    _ => {
                        let previous = this
                            .history
                            .take()
                            .filter(|history| history.project == project)
                            .and_then(|history| history.selected);
                        this.history.insert(History {
                            project: project.clone(),
                            operations,
                            more: false,
                            loading: false,
                            selected: previous,
                        })
                    }
                };
                history.more = more;
                history.loading = false;
                // Keep the selection when it's still listed, else take the newest.
                let listed = |id| history.operations.iter().any(|op| op.id == id);
                if !history.selected.is_some_and(listed) {
                    history.selected = history.operations.first().map(|op| op.id);
                }
                let moved = history.selected != selected_before;
                this.refresh_diff(moved, cx);
                cx.notify();
            })
            .ok();
        });
    }

    /// The diff of the selected operation: what changed between its snapshot and the next.
    pub(super) fn history_source(&self, project: &ProjectId) -> Option<DiffSource> {
        let history = self.history.as_ref().filter(|h| h.project == *project)?;
        let id = history.selected?;
        let ix = history.operations.iter().position(|op| op.id == id)?;
        let newer = ix
            .checked_sub(1)
            .and_then(|ix| history.operations.get(ix))
            .map(|op| op.id);
        Some(DiffSource::Snapshot { id, newer })
    }

    fn select_operation(&mut self, id: SnapshotId, cx: &mut Context<Self>) {
        if let Some(history) = &mut self.history {
            history.selected = Some(id);
        }
        self.reset_view();
        self.refresh_diff(true, cx);
        cx.notify();
    }

    /// The selected operation, for the page's header.
    pub(super) fn selected_operation(&self, project: &ProjectId) -> Option<&Operation> {
        let history = self.history.as_ref().filter(|h| h.project == *project)?;
        let id = history.selected?;
        history.operations.iter().find(|op| op.id == id)
    }

    /// Put the project back to how it was before the selected operation, after asking.
    /// The restore goes in the log too, so it can be undone.
    pub(super) fn restore_selected(
        &self,
        project: &ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(operation) = self.selected_operation(project) else {
            return;
        };
        let (id, title) = (operation.id, operation.title.clone());
        let project = project.clone();
        self.app
            .update(cx, |app, cx| {
                app.confirm_then(
                    format!("Restore to before \"{title}\"?"),
                    "Branches, commits and uncommitted changes go back to how they were. \
                     You can undo this with ⌘Z.",
                    "Restore",
                    window,
                    cx,
                    move |app, cx| {
                        app.store.update(cx, |store, cx| {
                            store.run(
                                &project,
                                "Restore",
                                Overlay::default(),
                                move |path, _| backend::history::restore(path, id),
                                |store, (), cx| store.snack_undoable("Restored", cx),
                                cx,
                            )
                        })
                    },
                )
            })
            .ok();
    }

    /// Record how the project is now, to come back to later.
    pub(super) fn take_snapshot(&self, project: &ProjectId, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.run(
                project,
                "Snapshot",
                Overlay::default(),
                move |path, _| backend::history::take_snapshot(path),
                |store, (), cx| store.snack("Snapshot taken", cx),
                cx,
            )
        });
    }

    pub(super) fn render_history_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(history) = self.history.as_ref().filter(|h| h.project == *project) else {
            return crate::ui::empty_state("Loading history…", "", theme);
        };
        if history.operations.is_empty() {
            return crate::ui::empty_state(
                "Nothing here yet",
                "Changes you make with GitButler show up here",
                theme,
            );
        }
        let count = history.operations.len() + usize::from(history.more);
        let list =
            uniform_list(
                "history",
                count,
                cx.processor(move |this, range: Range<usize>, _, cx| {
                    let theme = Theme::get(cx).clone();
                    let Some(history) = this.history.as_ref() else {
                        return Vec::new();
                    };
                    range
                        .map(|ix| match history.operations.get(ix) {
                            Some(operation) => {
                                let selected = history.selected == Some(operation.id);
                                let id = operation.id;
                                list_row(("operation", ix), selected, &theme)
                                    .child(
                                        div()
                                            .flex_none()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child(SharedString::from(operation.title.clone())),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_color(theme.text_2)
                                            .children(operation.detail.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_size(px(11.))
                                            .text_color(theme.text_3)
                                            .child(relative_time(operation.time)),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_operation(id, cx)
                                    }))
                                    .into_any_element()
                            }
                            None => {
                                let project = history.project.clone();
                                list_row("history-more", false, &theme)
                                    .text_color(theme.text_2)
                                    .child(if history.loading {
                                        "Loading…"
                                    } else {
                                        "Show older"
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_history(&project, true, cx)
                                    }))
                                    .into_any_element()
                            }
                        })
                        .map(|row| div().w_full().child(row).into_any_element())
                        .collect()
                }),
            )
            .track_scroll(&self.history_scroll)
            .size_full();
        let scroll = Scroll::Uniform(self.history_scroll.clone());
        scrolled(list, "history-scrollbar", scroll, theme)
            .pt(px(6.))
            .into_any_element()
    }
}
