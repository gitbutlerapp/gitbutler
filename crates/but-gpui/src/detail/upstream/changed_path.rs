//! Filtering the project's history to the commits that changed a path: one typed into the
//! page's filter, or a file or folder's from its menu.

use bstr::ByteSlice;
use gpui::{AnyElement, Context, div, prelude::*};

use super::{Picked, Upstream, View, clear_button};
use crate::backend::upstream::PathQuery;
use crate::detail::Detail;
use crate::detail::file_list::At;
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::{dp, faint, h_flex};

impl Detail {
    /// Filter the history to the commits that changed the paths of `query`, and nothing else,
    /// or let go of them. What's read so far may hold none of them, so the list starts again
    /// from the target's tip.
    fn filter_path(
        &mut self,
        project: &ProjectId,
        query: Option<PathQuery>,
        cx: &mut Context<Self>,
    ) {
        let upstream = (self.upstream.entry(project.clone()))
            .or_insert_with(|| Upstream::unread(View::default()));
        // What merged lately stays beside the list as it comes back unfiltered.
        upstream.selected = query.is_none().then_some(Picked::Shipped);
        if query.is_some() {
            // A day or words picked before would hide what's asked for now.
            let open_merges = std::mem::take(&mut upstream.view.open_merges);
            upstream.view = View {
                open_merges,
                ..View::default()
            };
            (self.target_filter).update(cx, |input, cx| input.set_text("", cx));
        }
        upstream.view.path = query;
        upstream.commits.clear();
        upstream.more = true;
        upstream.merged_pick = None;
        upstream.pending = None;
        upstream.failed = None;
        upstream.paging.set(super::Paging::Loading);
        // What merged lately doesn't change with the filter, so only a page without it reads it.
        let summed = upstream.summary.is_some();
        self.show_newest_kept(cx);
        self.load_upstream(project, summed, true, cx);
        cx.notify();
    }

    /// Filter the history to the commits that changed a path with the filter's text in it.
    pub(super) fn search_paths(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let letters = self.target_filter.read(cx).text().trim().to_owned();
        self.filter_path(project, Some(PathQuery::Letters(letters)), cx);
    }

    /// What the menu of a file or folder `at` offers while `project` has a history to list: the
    /// target's commits that changed it, on the project's page.
    pub(in crate::detail) fn path_history_item(
        &self,
        project: &ProjectId,
        at: &At,
        cx: &mut Context<Self>,
    ) -> Option<MenuItem> {
        (self.store.read(cx).workspace(project))
            .filter(|workspace| workspace.target.is_some() && workspace.editing.is_none())?;
        let (label, path) = match at {
            At::File(path, _) => ("Show file history", path),
            At::Folder(path) => ("Show folder history", path),
        };
        // As Git has it: what the menu holds is decoded to show.
        let whole = (self.diff.iter().flat_map(|diff| diff.files.iter()))
            .find(|file| file.file.path == *path)
            .map_or_else(
                || path.as_str().into(),
                |file| file.file.change.path.clone(),
            );
        let (project, entity) = (project.clone(), cx.entity().downgrade());
        Some(MenuItem::new(label, move |_, window, cx| {
            let (project, query) = (project.clone(), PathQuery::Whole(whole.clone()));
            (entity.update(cx, |detail, cx| {
                let selection = Selection::Project(project.clone());
                if !(detail.store).update(cx, |store, cx| store.select_by_hand(selection, cx)) {
                    return;
                }
                // Once the page has opened, on what merged lately.
                cx.spawn_in(window, async move |this, cx| {
                    this.update_in(cx, |this, window, cx| {
                        this.filter_path(&project, Some(query), cx);
                        // The arrow keys step through the commits listed.
                        window.focus(&this.side_focus, cx);
                    })
                    .ok();
                })
                .detach();
            }))
            .ok();
        }))
    }

    /// Have the next diff open at the file the history is filtered to, as a commit can change
    /// many.
    pub(super) fn open_at_path_filtered(&mut self, project: &ProjectId) {
        let file = self.upstream_for(project).and_then(|u| match &u.view.path {
            Some(PathQuery::Whole(file)) => Some(file.to_str_lossy().into_owned()),
            _ => None,
        });
        if file.is_some() {
            self.pending_file = file;
        }
    }

    /// What the history says while it's filtered to the commits that changed a path, and how to
    /// let go of that.
    pub(super) fn render_path_filter(
        &self,
        project: &ProjectId,
        upstream: &Upstream,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let path = match upstream.view.path.as_ref()? {
            PathQuery::Whole(path) => path.to_str_lossy().into_owned(),
            PathQuery::Letters(letters) => letters.clone(),
        };
        let project = project.clone();
        Some(
            h_flex()
                .flex_none()
                .mx(dp(14.))
                .mb(dp(6.))
                .gap(dp(6.))
                .child(faint("Changed", theme).flex_none())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(dp(12.))
                        .text_ellipsis_middle()
                        .child(path),
                )
                .child(clear_button("path-filter-clear").on_click(
                    cx.listener(move |this, _, _, cx| this.filter_path(&project, None, cx)),
                ))
                .into_any_element(),
        )
    }
}
