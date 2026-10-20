//! The Operations history page: what GitButler did to a project, newest first under each day,
//! with what it did to which commit or branch; over the diff beside it, a header saying all of
//! that for the selected one, then the files it changed; and a way back to before any of them.

use gpui::{
    AnyElement, Context, MouseButton, MouseDownEvent, Pixels, Point, Window, div, prelude::*, px,
};

use super::Detail;
use super::panes::{
    list_heading, meta_item, meta_row, page_title, side_list, side_row, title_text,
};
use crate::app::{SelectNext, SelectPrevious};
use crate::backend::history::{Branches, Change, Operation, Recorded};
use crate::backend::{self, CommitId, DiffSource, SnapshotId};
use crate::overlays::MenuItem;
use crate::state::{Overlay, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use but_oplog::legacy::OperationKind;

use crate::ui::{
    Icon, Tooltip, clock_time, commit_title, day_label, faint, h_flex, icon, icon_button, list_row,
    local_day, plural, small_icon, v_flex,
};

/// How many of an operation's changes its header lists; the rest it counts.
const LISTED: usize = 8;

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

/// The glyph for a kind of operation, which tells what it did at a glance: a commit made,
/// changes amended into one, commits squashed, something edited, thrown away, done to a
/// branch, brought in from the target, taken back, or kept as a snapshot.
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
        ) => Icon::Bin,
        Some(K::CreateCommit | K::InsertBlankCommit) => Icon::Commit,
        Some(K::AmendCommit) => Icon::Absorb,
        Some(K::SquashCommit) => Icon::ArrowDoubleDown,
        Some(
            K::UpdateCommitMessage
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
            K::CreateBranch
            | K::CreateDependentBranch
            | K::StashIntoBranch
            | K::ApplyBranch
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
        Some(K::MergeUpstream | K::UpdateWorkspaceBase) => Icon::ArrowLineDown,
        Some(K::SyncWorkspace) => Icon::Refresh,
        Some(K::OnDemandSnapshot) => Icon::Camera,
        _ => Icon::Commit,
    }
}

/// Whether `kind` takes earlier operations back: an undo, a redo or a restore.
fn takes_back(kind: Option<OperationKind>) -> bool {
    use OperationKind as K;
    matches!(
        kind,
        Some(
            K::RestoreFromSnapshot | K::RestoreFromSnapshotViaUndo | K::RestoreFromSnapshotViaRedo
        )
    )
}

/// The color of `kind`'s glyph: green for what it added, blue for what it rewrote, red for
/// what it threw away, amber for what it took back, and quiet for the rest.
fn tint(kind: Option<OperationKind>, theme: &Theme) -> gpui::Hsla {
    use OperationKind as K;
    let Some(kind) = kind else {
        return theme.text_3;
    };
    match kind {
        K::CreateCommit
        | K::AutoCommit
        | K::InsertBlankCommit
        | K::CherryPick
        | K::CreateBranch
        | K::CreateDependentBranch
        | K::StashIntoBranch
        | K::MergeUpstream
        | K::UpdateWorkspaceBase => theme.safe,
        K::AmendCommit
        | K::Absorb
        | K::SquashCommit
        | K::UpdateCommitMessage
        | K::UpdateBranchName
        | K::UpdateDependentBranchName
        | K::MoveCommit
        | K::MoveHunk
        | K::MoveCommitFile
        | K::MoveBranch
        | K::ReorderCommit
        | K::ReorderBranches
        | K::TearOffBranch
        | K::SplitBranch
        | K::ResolveConflicts
        | K::ResolveConflictsAi => theme.blue,
        K::DiscardLines
        | K::DiscardHunk
        | K::DiscardFile
        | K::DiscardChanges
        | K::Discard
        | K::DiscardCommit
        | K::CleanWorkspace
        | K::DeleteBranch
        | K::RemoveDependentBranch
        | K::UndoCommit => theme.danger,
        K::RestoreFromSnapshot | K::RestoreFromSnapshotViaUndo | K::RestoreFromSnapshotViaRedo => {
            theme.warn
        }
        _ => theme.text_2,
    }
}

/// What a row says the operation was done to: the commit or branch its snapshot shows it
/// changed, else what the log says, as the operation an undo took back.
fn summary(operation: &Operation) -> Option<String> {
    subject(operation).or_else(|| operation.detail.clone())
}

/// What hovering a row's glyph says it did: its title, and for an undo what it took back.
fn glyph_tooltip(operation: &Operation) -> String {
    match (takes_back(operation.kind), &operation.detail) {
        (true, Some(undone)) => format!("{}: {undone}", operation.title),
        _ => operation.title.clone(),
    }
}

/// The commit or branch `operation`'s changes are about, or for an update what it brought.
fn subject(operation: &Operation) -> Option<String> {
    use OperationKind as K;
    if operation.changes.is_empty() {
        return None;
    }
    let commits = |pick: fn(&Change) -> Option<&Recorded>| -> Vec<&Recorded> {
        operation.changes.iter().filter_map(pick).collect()
    };
    let edited = commits(|c| match c {
        Change::Edited { after, .. } => Some(after),
        _ => None,
    });
    let added = commits(|c| match c {
        Change::Added { commit, .. } => Some(commit),
        _ => None,
    });
    let moved = commits(|c| match c {
        Change::Moved { commit, .. } => Some(commit),
        _ => None,
    });
    let removed = commits(|c| match c {
        Change::Removed { commit, .. } => Some(commit),
        _ => None,
    });
    let name = |commits: &[&Recorded]| match commits {
        [] => None,
        [one] => Some(commit_title(one.title()).to_string()),
        many => Some(plural(many.len(), "commit")),
    };
    if let (Some(K::SquashCommit), [gone], [into]) = (operation.kind, &removed[..], &edited[..]) {
        let (gone, into) = (commit_title(gone.title()), commit_title(into.title()));
        return Some(format!("{gone} into {into}"));
    }
    if let Some(K::MergeUpstream | K::UpdateWorkspaceBase) = operation.kind {
        let brought = operation.changes.iter().find_map(|change| match change {
            Change::TargetMoved { target, commits } => {
                Some(format!("{} from {target}", plural(*commits, "commit")))
            }
            _ => None,
        });
        let integrated = (!removed.is_empty())
            .then(|| format!("{} integrated", plural(removed.len(), "commit")));
        return match (brought, integrated) {
            (Some(brought), Some(integrated)) => Some(format!("{brought}, {integrated}")),
            (brought, integrated) => brought.or(integrated),
        };
    }
    let branch = operation.changes.iter().find_map(|change| match change {
        Change::Renamed { to: name, .. }
        | Change::BranchAdded(name)
        | Change::BranchRemoved(name) => Some(name.clone()),
        _ => None,
    });
    // An operation on a branch is about it, though its commits came or went with it.
    let on_commits = matches!(
        operation.kind,
        Some(
            K::CreateCommit
                | K::AmendCommit
                | K::UpdateCommitMessage
                | K::SquashCommit
                | K::CherryPick
                | K::MoveCommit
                | K::MoveCommitFile
                | K::UndoCommit
                | K::DiscardCommit
                | K::InsertBlankCommit
        )
    );
    if !on_commits && branch.is_some() {
        return branch;
    }
    (name(&edited).or_else(|| name(&added)))
        .or_else(|| name(&moved))
        .or_else(|| name(&removed))
        .or(branch)
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
        // What the newest operation left, which its snapshot is compared with.
        let now: Branches = (self.store.read(cx).workspace(project).into_iter())
            .flat_map(|ws| &ws.stacks)
            .flat_map(|stack| &stack.branches)
            .map(|branch| {
                let commits = (branch.commits.iter()).map(|commit| Recorded {
                    id: commit.id,
                    key: commit.key.clone(),
                    message: commit.message.clone(),
                    parent: commit.parent,
                    tree: commit.tree,
                });
                (branch.name.clone(), commits.collect())
            })
            .collect();
        let project = project.clone();
        self._history_load = cx.spawn(async move |this, cx| {
            let result =
                backend::run(move || backend::history::list(&path, after, limit, now)).await;
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

    /// Whether [`Self::render_operation_header`] draws one for `project`.
    pub(super) fn has_operation_header(&self, project: &ProjectId) -> bool {
        self.selected_operation(project).is_some()
    }

    /// Over the diff, what the selected operation was: what and when, the branch it was on,
    /// and each commit and branch it changed, a commit still in the workspace a way to it.
    pub(super) fn render_operation_header(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let operation = self.selected_operation(project)?;
        let mut branches: Vec<&str> = Vec::new();
        for change in &operation.changes {
            if let Change::Added { branch, .. }
            | Change::Removed { branch, .. }
            | Change::Edited { branch, .. }
            | Change::Moved { to: branch, .. } = change
                && !branches.contains(&branch.as_str())
            {
                branches.push(branch);
            }
        }
        let undone = takes_back(operation.kind);
        let when = format!(
            "{} {}",
            day_label(operation.time),
            clock_time(operation.time)
        );
        let meta = meta_row(theme)
            .child(meta_item(Icon::Clock, theme).child(when))
            .when_some(operation.detail.clone().filter(|_| undone), |el, what| {
                el.child(meta_item(Icon::Undo, theme).child(what))
            })
            .when_some(Some(branches).filter(|b| b.len() == 1), |el, branches| {
                el.child(meta_item(Icon::Branch, theme).child(branches[0].to_owned()))
            });
        let more = operation.changes.len().saturating_sub(LISTED);
        let lines = (operation.changes.iter().take(LISTED).enumerate())
            .map(|(ix, change)| self.change_line(ix, project, operation.kind, change, theme, cx));
        let main = v_flex()
            .gap(px(10.))
            .child(
                page_title(operation_icon(operation.kind), theme)
                    .child(title_text(operation.title.clone())),
            )
            .child(meta)
            .when(!operation.changes.is_empty(), |el| {
                el.child(v_flex().gap(px(4.)).children(lines).when(more > 0, |el| {
                    el.child(faint(format!("and {more} more"), theme))
                }))
            });
        let (id, menu_project) = (operation.id, project.clone());
        let menu = icon_button("operation-menu", Icon::Kebab).on_click(cx.listener(
            move |this, event: &gpui::ClickEvent, _, cx| {
                this.operation_menu(&menu_project, id, event.position(), cx)
            },
        ));
        Some(
            v_flex()
                .flex_1()
                .h_full()
                .min_w(px(0.))
                .child(self.header_frame(main, menu, None, None, theme))
                .into_any_element(),
        )
    }

    /// One of an operation's changes in its header: a glyph, the commit's title or the
    /// branch's name, then where, faint.
    fn change_line(
        &self,
        ix: usize,
        project: &ProjectId,
        kind: Option<OperationKind>,
        change: &Change,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        use OperationKind as K;
        let integrated = matches!(kind, Some(K::MergeUpstream | K::UpdateWorkspaceBase));
        let title = |commit: &Recorded| commit_title(commit.title()).to_string();
        let (glyph, color, commit, text, note): (_, _, Option<CommitId>, _, _) = match change {
            Change::Added { commit, branch } => (
                Icon::Plus,
                theme.safe,
                Some(commit.id),
                title(commit),
                format!("on {branch}"),
            ),
            Change::Removed { commit, .. } if integrated => (
                Icon::Tick,
                theme.safe,
                None,
                title(commit),
                "now in the target".to_owned(),
            ),
            Change::Removed { commit, branch } => (
                Icon::Cross,
                theme.text_3,
                None,
                title(commit),
                format!("from {branch}"),
            ),
            Change::Edited { before, after, .. } => {
                let reworded = match (
                    before.message == after.message,
                    before.title() == after.title(),
                ) {
                    (true, _) => None,
                    (false, true) => Some("its description changed".to_owned()),
                    (false, false) => Some(format!("was \"{}\"", title(before))),
                };
                let (glyph, note) = match (after.amends(before), reworded) {
                    (true, None) => (Icon::Absorb, "amended".to_owned()),
                    (true, Some(reworded)) => (Icon::Absorb, format!("amended, {reworded}")),
                    (false, reworded) => (Icon::Edit, reworded.unwrap_or_default()),
                };
                (glyph, theme.text_2, Some(after.id), title(after), note)
            }
            Change::Moved { commit, from, to } => (
                Icon::Commit,
                theme.text_2,
                Some(commit.id),
                title(commit),
                format!("from {from} to {to}"),
            ),
            Change::BranchAdded(name) => (
                Icon::Branch,
                theme.text_2,
                None,
                name.clone(),
                "new branch".to_owned(),
            ),
            Change::BranchRemoved(name) => (
                Icon::Branch,
                theme.text_3,
                None,
                name.clone(),
                "removed".to_owned(),
            ),
            Change::Renamed { from, to } => (
                Icon::Branch,
                theme.text_2,
                None,
                to.clone(),
                format!("was {from}"),
            ),
            Change::TargetMoved { target, commits } => (
                Icon::ArrowLineDown,
                theme.text_2,
                None,
                plural(*commits, "commit"),
                format!("new on {target}"),
            ),
        };
        // A commit the workspace still has opens, as a breadcrumb does.
        let here = commit.filter(|id| self.store.read(cx).find_commit(project, *id).is_some());
        let (store, project) = (self.store.clone(), project.clone());
        h_flex()
            .id(("operation-change", ix))
            .min_w(px(0.))
            .gap(px(8.))
            .text_size(px(13.))
            .child(small_icon(glyph).text_color(color))
            .child(div().min_w(px(0.)).truncate().child(text))
            .child(faint(note, theme).flex_none().max_w(px(320.)).truncate())
            .when_some(here, |el, id| {
                el.cursor_pointer()
                    .hover(|el| el.text_color(theme.blue))
                    .tooltip(Tooltip::text("Show this commit"))
                    .on_click(move |_, _, cx| {
                        let selection = Selection::Commit(project.clone(), id);
                        store.update(cx, |store, cx| store.select(Some(selection), cx))
                    })
            })
            .into_any_element()
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
                        // What it did as a glyph, named on hover, then what it was done to, or
                        // its name, faint, when nothing says; then when.
                        let (text, said) = match summary(operation) {
                            Some(text) => (text, true),
                            None => (operation.title.clone(), false),
                        };
                        side_row(("operation", ix), selected, on, theme)
                            .child(
                                div()
                                    .id(("operation-glyph", ix))
                                    .flex_none()
                                    .tooltip(Tooltip::text(glyph_tooltip(operation)))
                                    .child(
                                        icon(operation_icon(operation.kind))
                                            .text_color(quiet(tint(operation.kind, theme))),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .when(!said, |el| el.text_color(quiet(theme.text_3)))
                                    .child(text),
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
