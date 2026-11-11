//! The Graph page around the graph: the changes of the commit pinned on it, beside it, and what
//! a right-click on a commit offers.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, Context, Pixels, Point, SharedString, Window};

use super::GraphEvent;
use crate::app::{CopyCommitId, FocusNavigator};
use crate::backend::CommitId;
use crate::commands::Command;
use crate::detail::Detail;
use crate::detail::page::PageState;
use crate::edits::{CherryPick, CommitEdits, CopyChangeId, CopyMessage, NewBranchHere, Uncommit};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, ProjectTab, Selection};
use crate::transfer::Moving;
use crate::{clipboard, clock};

impl Detail {
    /// The commit pinned on the Graph page shown, whose changes show beside the graph.
    pub(in crate::detail) fn graph_commit(&self) -> Option<CommitId> {
        match &self.page {
            PageState::Graph { commit, .. } => *commit,
            _ => None,
        }
    }

    pub(in crate::detail) fn graph_event(
        &mut self,
        event: &GraphEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            GraphEvent::Pinned { id, by_hand } => self.graph_pinned(*id, *by_hand, window, cx),
            GraphEvent::Menu {
                at,
                id,
                title,
                outside,
            } => self.graph_menu(*at, *id, title.clone(), *outside, cx),
        }
    }

    /// Whether the graph is in the list's column beside a commit's changes: one is pinned, or
    /// its changes are still sliding away.
    pub(in crate::detail) fn graph_beside(&self) -> bool {
        self.showing_graph()
            && (self.graph_commit().is_some() || self.graph_slide.closing(clock::now()))
    }

    /// Show the changes of commit `id`, pinned now, beside the graph, sliding them in, or with
    /// none, slide them away and give the graph the page again.
    fn graph_pinned(
        &mut self,
        id: Option<CommitId>,
        by_hand: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let PageState::Graph { commit, .. } = &mut self.page else {
            return;
        };
        if *commit == id {
            return;
        }
        let was = std::mem::replace(commit, id);
        if was.is_some() != id.is_some() {
            // The page's width, which the column narrows from or widens to, at rest.
            if self.graph_slide.openness(clock::now()).is_none() {
                self.graph_page = match was {
                    Some(_) => self.files_width(window, cx) + self.beside_width.get(),
                    None => self.graph.read(cx).width(),
                };
            }
            self.graph_slide.start(id.is_some(), cx);
            // The keys go with the graph into the list's column. Out of it, the render hands
            // them back once it's gone.
            if id.is_some() && self.focus.is_focused(window) {
                window.focus(&self.side_focus, cx);
            }
            self.draw_through_slide(cx);
        }
        // Going, the changes stay as they were while they slide away. Rewritten, as by an
        // agent's amend, they open where they're looked at, as a page shown again does.
        match (id, was) {
            (None, _) => {}
            (Some(_), Some(_)) if !by_hand => {
                let left_at = self.place_left();
                if let Some(files) = self.page.files_mut() {
                    files.place = left_at.or(files.place.take());
                }
                self.refresh_diff(false, cx)
            }
            (Some(_), _) => self.show_picked(cx),
        }
    }

    /// Show `row`, a branch or a commit, pinned on its project's Graph tab, unless an agent took
    /// it away since its menu opened.
    pub(crate) fn show_in_graph(&self, row: Selection, cx: &mut App) {
        let project = row.project().clone();
        let shown = self.store.read(cx).branch_for(&row).is_some()
            && (self.store).update(cx, |store, cx| {
                store.show_tab(project, ProjectTab::Graph, cx)
            });
        if shown {
            self.graph.update(cx, |graph, cx| graph.show_row(row, cx));
        }
    }

    /// Unpin the graph's commit, sliding its changes away, as Esc does. Whether one was.
    pub(crate) fn unpin_graph(&mut self, cx: &mut Context<Self>) -> bool {
        let pinned = self.graph_commit().is_some();
        if pinned {
            self.graph.update(cx, |graph, cx| graph.pin(None, cx));
        }
        pinned
    }

    fn graph_menu(
        &self,
        at: Point<Pixels>,
        id: CommitId,
        title: SharedString,
        outside: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.on_tab(ProjectTab::Graph) else {
            return;
        };
        self.open_menu(
            at,
            move |this, cx| this.graph_menu_items(&project, id, title, outside, cx),
            cx,
        );
    }

    /// What a right-click on commit `id` offers: for one of the workspace's, what the
    /// navigator's row offers that needs nothing more than the commit, and the way to its row for
    /// the rest; for one outside it, copying it in where a remote or the target has it.
    fn graph_menu_items(
        &self,
        project: &ProjectId,
        id: CommitId,
        title: SharedString,
        outside: bool,
        cx: &App,
    ) -> Vec<MenuItem> {
        let copy_id = MenuItem::new(CopyCommitId.title(), move |app, _, cx| {
            clipboard::copy(&app.store, id.0.to_string(), cx)
        });
        let store = self.store.read(cx);
        let Some((branch, commit)) = store.find_commit(project, id) else {
            // A fetch can take it away while the menu is open: then it isn't copied in.
            let (project, graph) = (project.clone(), self.graph.downgrade());
            let pick = MenuItem::new(CherryPick.title(), move |app, window, cx| {
                if graph.upgrade().is_some_and(|graph| graph.read(cx).has(id)) {
                    let copies = Moving::Copies(Arc::from([(id, title.clone())]));
                    app.cherry_pick(project.clone(), copies, window, cx)
                }
            });
            return match outside {
                true => vec![pick, copy_id.separated()],
                false => vec![copy_id],
            };
        };
        let edits = CommitEdits::of(branch, commit);
        let row = Selection::Commit(project.clone(), id);
        let (change_id, message) = (commit.key.clone(), commit.message.clone());
        let (shown, branched) = (row.clone(), project.clone());
        let uncommits = (!commit.conflicted).then(|| {
            let (project, ids) = (project.clone(), store.with_picks(project, id));
            let label = match ids.len() {
                0 | 1 => Uncommit.title().to_owned(),
                n => format!("Uncommit {n} commits"),
            };
            MenuItem::new(label, move |app, _, cx| {
                let ids = ids.clone();
                (app.store).update(cx, |store, cx| store.uncommit(&project, ids, None, cx))
            })
        });
        let groups = [
            vec![Some(MenuItem::new(
                "Show in navigator",
                move |app, window, cx| {
                    let row = shown.clone();
                    (app.store).update(cx, |store, cx| store.select(Some(row), cx));
                    app.focus_navigator(&FocusNavigator, window, cx)
                },
            ))],
            vec![
                edits.new_branch_here.then(|| {
                    MenuItem::new(NewBranchHere.title(), move |app, window, cx| {
                        app.new_branch_ending_at(branched.clone(), id, window, cx)
                    })
                }),
                uncommits,
            ],
            vec![
                Some(copy_id),
                edits.change_id.then(|| {
                    MenuItem::new(CopyChangeId.title(), move |app, _, cx| {
                        clipboard::copy(&app.store, change_id.clone(), cx)
                    })
                }),
                (!message.trim().is_empty()).then(|| {
                    MenuItem::new(CopyMessage.title(), move |app, _, cx| {
                        clipboard::copy(&app.store, message.clone(), cx)
                    })
                }),
            ],
            vec![Some(MenuItem::discard_row(&row, cx))],
        ];
        // An agent can rewrite the commit while the menu is open: then they do nothing.
        let project = project.clone();
        let guarded = move |mut item: MenuItem| {
            let (run, project) = (item.handler.clone(), project.clone());
            item.handler = Rc::new(move |app, window, cx| {
                if app.store.read(cx).find_commit(&project, id).is_some() {
                    run(app, window, cx)
                }
            });
            item
        };
        (groups.into_iter())
            .flat_map(|group| {
                (group.into_iter().flatten().enumerate())
                    .map(|(ix, item)| if ix == 0 { item.separated() } else { item })
            })
            .map(guarded)
            .collect()
    }
}
