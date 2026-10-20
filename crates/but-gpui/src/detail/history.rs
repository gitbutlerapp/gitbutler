//! The Operations history page: what GitButler did to a project, newest first under each day,
//! the files the selected operation changed in the diff beside it, and a way back to before
//! any of them.

use gpui::{
    AnyElement, Context, MouseButton, MouseDownEvent, Pixels, Point, SharedString, Window, div,
    prelude::*, px,
};

use super::Detail;
use super::panes::{list_heading, side_list, side_row};
use crate::app::{SelectNext, SelectPrevious};
use crate::backend::history::Operation;
use crate::backend::{self, DiffSource, SnapshotId};
use crate::overlays::MenuItem;
use crate::state::{Overlay, ProjectId};
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use but_oplog::legacy::OperationKind;

use crate::ui::{Icon, clock_time, day_label, faint, list_row, local_day, small_icon};

/// How many operations a page of the log holds.
const PAGE: usize = 60;

/// A row of the list: a day's heading over its operations, an operation by index, or the
/// button for older ones.
enum Row {
    Day(i64),
    Operation(usize),
    More,
}

/// The list's rows: each day's operations under its heading, then Show older while there's
/// more.
fn rows(history: &History) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut day = None;
    for (ix, operation) in history.operations.iter().enumerate() {
        let this = local_day(operation.time);
        if day.as_ref() != Some(&this) {
            rows.push(Row::Day(operation.time));
            day = Some(this);
        }
        rows.push(Row::Operation(ix));
    }
    if history.more {
        rows.push(Row::More);
    }
    rows
}

/// The glyph for a kind of operation, as Tauri's history has one: what it took away, added,
/// edited, did to a branch or a commit, took back, or kept as a snapshot.
fn operation_icon(kind: Option<OperationKind>) -> Icon {
    use OperationKind as K;
    match kind {
        Some(
            K::DiscardLines
            | K::DiscardHunk
            | K::DiscardFile
            | K::DiscardChanges
            | K::Discard
            | K::DiscardCommit
            | K::DeleteBranch
            | K::RemoveDependentBranch,
        ) => Icon::Cross,
        Some(
            K::CreateCommit
            | K::CreateBranch
            | K::CreateDependentBranch
            | K::InsertBlankCommit
            | K::StashIntoBranch,
        ) => Icon::Plus,
        Some(
            K::AmendCommit
            | K::UpdateCommitMessage
            | K::UpdateBranchName
            | K::UpdateBranchNotes
            | K::UpdateBranchRemoteName
            | K::UpdateDependentBranchName
            | K::UpdateDependentBranchDescription
            | K::UpdateDependentBranchPrNumber
            | K::EnterEditMode
            | K::ResolveConflicts
            | K::ResolveConflictsAi,
        ) => Icon::Edit,
        Some(
            K::ApplyBranch
            | K::UnapplyBranch
            | K::SwitchBranch
            | K::SwitchToWorkspace
            | K::ReorderBranches
            | K::MoveBranch
            | K::TearOffBranch
            | K::SplitBranch
            | K::SetBaseBranch
            | K::GenericBranchUpdate,
        ) => Icon::Branch,
        Some(
            K::UndoCommit
            | K::RestoreFromSnapshot
            | K::RestoreFromSnapshotViaUndo
            | K::RestoreFromSnapshotViaRedo,
        ) => Icon::Undo,
        Some(K::MoveHunk | K::MoveCommitFile | K::FileChanges) => Icon::File,
        Some(K::MergeUpstream | K::UpdateWorkspaceBase | K::SyncWorkspace) => Icon::Refresh,
        Some(K::OnDemandSnapshot) => Icon::Camera,
        _ => Icon::Commit,
    }
}

pub(super) struct History {
    project: ProjectId,
    operations: Vec<Operation>,
    /// Whether older operations remain to load.
    more: bool,
    loading: bool,
    selected: Option<SnapshotId>,
}

impl Detail {
    /// Load `project`'s newest operations, as many as are shown so the older pages and the
    /// selection stay, or with `more` the next page after those.
    pub(super) fn load_history(&mut self, project: &ProjectId, more: bool, cx: &mut Context<Self>) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        let shown = self.history_for(project);
        let after = (shown.filter(|_| more))
            .and_then(|history| history.operations.last())
            .map(|operation| operation.id);
        let limit = match more {
            true => PAGE,
            false => shown.map_or(PAGE, |history| history.operations.len().max(PAGE)),
        };
        if let Some(history) = &mut self.history
            && history.project == *project
        {
            history.loading = true;
        }
        let project = project.clone();
        self._history_load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || backend::history::list(&path, after, limit)).await;
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

    fn history_for(&self, project: &ProjectId) -> Option<&History> {
        self.history.as_ref().filter(|h| h.project == *project)
    }

    /// The diff of the selected operation: what changed between its snapshot and the next.
    pub(super) fn history_source(&self, project: &ProjectId) -> Option<DiffSource> {
        let history = self.history_for(project)?;
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
        self.show_picked(cx);
    }

    /// ↓ or ↑ (`delta`) in the list: the next or previous operation, kept in view.
    fn step_history(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(history) = &self.history else {
            return;
        };
        let at = (history.operations.iter()).position(|op| Some(op.id) == history.selected);
        let next = at.map_or(0, |at| at as isize + delta);
        let Some(op) = usize::try_from(next)
            .ok()
            .and_then(|ix| history.operations.get(ix))
        else {
            return;
        };
        let id = op.id;
        // Kept in view by its row, below the day headings.
        let row = rows(history)
            .iter()
            .position(|row| matches!(row, Row::Operation(ix) if *ix as isize == next));
        self.select_operation(id, cx);
        if let Some(row) = row {
            (self.history_scroll).scroll_to_item(row, gpui::ScrollStrategy::Nearest);
        }
    }

    fn selected_operation(&self, project: &ProjectId) -> Option<&Operation> {
        let history = self.history_for(project)?;
        let id = history.selected?;
        history.operations.iter().find(|op| op.id == id)
    }

    /// Put the project back to how it was before the selected operation, after asking.
    /// The restore goes in the log too, so it can be undone.
    fn restore_selected(&self, project: &ProjectId, window: &mut Window, cx: &mut Context<Self>) {
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
                        app.store
                            .update(cx, |store, cx| store.restore(&project, id, cx))
                    },
                )
            })
            .ok();
    }

    /// Select operation `id` and offer what its page's header does for it, and its ID.
    fn operation_menu(
        &mut self,
        project: &ProjectId,
        id: SnapshotId,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.select_operation(id, cx);
        let project = project.clone();
        let items = vec![
            // Once the menu's done with the app, which asking to confirm updates too.
            MenuItem::new("Restore to before this…", move |app, window, cx| {
                let (detail, project) = (app.detail.clone(), project.clone());
                window.defer(cx, move |window, cx| {
                    detail.update(cx, |detail, cx| {
                        detail.restore_selected(&project, window, cx)
                    })
                })
            }),
            MenuItem::new("Copy snapshot ID", move |app, _, cx| {
                app.copy_text(id.0.to_string(), cx)
            })
            .separated(),
        ];
        self.app
            .update(cx, |app, cx| app.open_menu(at, items, cx))
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
        let Some(history) = self.history_for(project) else {
            return crate::ui::empty_state(None, "Loading history…", "", theme, cx);
        };
        if history.operations.is_empty() {
            return crate::ui::empty_state(
                Some(Illustration::Cactus),
                "Nothing here yet",
                "Changes you make with GitButler show up here",
                theme,
                cx,
            );
        }
        let rows = rows(history);
        side_list(
            "history",
            rows.len(),
            &self.history_scroll,
            "history-scrollbar",
            cx,
            move |this, ix, theme, window, cx| {
                let history = this.history.as_ref()?;
                let row = match rows.get(ix)? {
                    Row::Day(time) => return Some(list_heading(day_label(*time), theme)),
                    Row::Operation(op) => {
                        let operation = history.operations.get(*op)?;
                        let selected = history.selected == Some(operation.id);
                        let on = selected && this.side_focus.is_focused(window);
                        let quiet = |color: gpui::Hsla| match on {
                            true => theme.text_inverted.opacity(0.7),
                            false => color,
                        };
                        let id = operation.id;
                        // As Tauri's: its kind's glyph, what was done and to what, then its
                        // snapshot's short ID and when, faint.
                        side_row(("operation", ix), selected, on, theme)
                            .child(
                                small_icon(operation_icon(operation.kind))
                                    .text_color(quiet(theme.text_3)),
                            )
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
                                    .text_color(quiet(theme.text_2))
                                    .children(operation.detail.clone()),
                            )
                            .child(
                                faint(id.0.to_hex_with_len(7).to_string(), theme)
                                    .flex_none()
                                    .font_family(theme.mono_font.clone())
                                    .text_color(quiet(theme.text_3)),
                            )
                            .child(
                                faint(clock_time(operation.time), theme)
                                    .flex_none()
                                    .text_color(quiet(theme.text_3)),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                window.focus(&this.side_focus, cx);
                                this.select_operation(id, cx)
                            }))
                            .on_mouse_down(MouseButton::Right, {
                                let project = history.project.clone();
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    window.focus(&this.side_focus, cx);
                                    this.operation_menu(&project, id, event.position, cx)
                                })
                            })
                    }
                    Row::More => {
                        let project = history.project.clone();
                        list_row("history-more", false, theme)
                            .text_color(theme.text_2)
                            .child(if history.loading {
                                "Loading…"
                            } else {
                                "Show older"
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.load_history(&project, true, cx)
                            }))
                    }
                };
                Some(row.into_any_element())
            },
        )
        .pt(px(6.))
        .track_focus(&self.side_focus)
        .key_context("SideList")
        .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_history(1, cx)))
        .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.step_history(-1, cx)))
        .into_any_element()
    }
}
