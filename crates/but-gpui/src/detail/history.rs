//! The Operations history page: what GitButler did to a project, newest first under each day,
//! with what it did to which commit or branch; over the diff beside it, a header saying all of
//! that for the selected one, then the files it changed; and a way back to before any of them.

use gpui::{
    AnyElement, App, ClickEvent, Context, MouseButton, MouseDownEvent, Pixels, Point, Window, div,
    prelude::*,
};

use super::Detail;
use super::panes::{
    list_empty, list_filtered_out, list_heading, meta_item, meta_row, page_title, side_list,
    step_from, title_text,
};
use crate::backend::history::{Change, Operation, Recorded};
use crate::backend::{self, CommitId, DiffSource, SnapshotId};
use crate::overlays::MenuItem;
use crate::state::{Overlay, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::illustration::Illustration;
use crate::ui::text_input::TextInput;
use but_oplog::legacy::OperationKind;

use crate::ui::{
    Icon, Tooltip, commit_title, day_label, dp, error_summary, faint, h_flex, icon, icon_button,
    list_row, loading_state, local_clock, local_day, plural, side_row, small_icon, v_flex,
};

/// How many of an operation's changes its header lists; the rest it counts.
const LISTED: usize = 8;

/// How many operations a page of the log holds.
const PAGE: usize = 60;

/// How many operations the filter reads on its own to find one, as a reload reads as many
/// again while the page is open.
const READ_FOR_FILTER: usize = 10 * PAGE;

/// A row of the list: a day's heading over its operations, an operation by index, or the
/// button for older ones.
enum Row {
    Day(i64),
    Operation(usize),
    More,
}

/// The list's rows: each day's `shown` operations under its heading, then Show older while
/// there's more.
fn rows(history: &History, shown: &[usize]) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut day = None;
    for &ix in shown {
        let Some(operation) = history.operations.get(ix) else {
            continue;
        };
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

/// What a kind of operation did: its glyph, which tells at a glance, and its [`Group`]. A
/// commit made, changes amended into one, commits squashed, something edited, thrown away, done
/// to a branch, brought in from the target, taken back, or kept as a snapshot.
fn describe(kind: Option<OperationKind>) -> (Icon, Group) {
    use OperationKind as K;
    match kind.unwrap_or(K::Unknown) {
        K::CreateCommit | K::AutoCommit | K::InsertBlankCommit | K::CherryPick => {
            (Icon::Commit, Group::Commits)
        }
        K::MoveCommit | K::ReorderCommit => (Icon::Commit, Group::Rewrites),
        K::AmendCommit | K::Absorb => (Icon::Absorb, Group::Rewrites),
        K::SquashCommit => (Icon::ArrowDoubleDown, Group::Rewrites),
        K::UpdateCommitMessage | K::EnterEditMode | K::ResolveConflicts | K::ResolveConflictsAi => {
            (Icon::Edit, Group::Rewrites)
        }
        K::MoveHunk | K::MoveCommitFile => (Icon::File, Group::Rewrites),
        K::DiscardLines
        | K::DiscardHunk
        | K::DiscardFile
        | K::DiscardChanges
        | K::Discard
        | K::DiscardCommit
        | K::CleanWorkspace
        | K::DeleteBranch
        | K::RemoveDependentBranch => (Icon::Bin, Group::Discards),
        K::UndoCommit => (Icon::Undo, Group::Discards),
        K::UpdateBranchName
        | K::UpdateBranchNotes
        | K::UpdateBranchRemoteName
        | K::UpdateDependentBranchName
        | K::UpdateDependentBranchDescription
        | K::UpdateDependentBranchPrNumber => (Icon::Edit, Group::Branches),
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
        | K::GenericBranchUpdate => (Icon::Branch, Group::Branches),
        K::SetBaseBranch => (Icon::Branch, Group::Updates),
        K::MergeUpstream | K::UpdateWorkspaceBase => (Icon::ArrowLineDown, Group::Updates),
        // A branch landed on the target, as a land onto a target without a remote records.
        K::MergeBranch => (Icon::BranchMerge, Group::Updates),
        K::SyncWorkspace => (Icon::Refresh, Group::Updates),
        // A restore picked here wears the history's clock, apart from ⌘Z and ⇧⌘Z's steps.
        K::RestoreFromSnapshot => (Icon::Clock, Group::TakeBacks),
        K::RestoreFromSnapshotViaUndo | K::RestoreFromSnapshotViaRedo => {
            (Icon::Undo, Group::TakeBacks)
        }
        K::OnDemandSnapshot => (Icon::Camera, Group::Snapshots),
        K::FileChanges | K::AutoHandleChangesBefore | K::AutoHandleChangesAfter => {
            (Icon::File, Group::Other)
        }
        K::Unknown => (Icon::Commit, Group::Other),
    }
}

/// What an operation did, as the list's filter menu sorts them and their glyphs are tinted.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Group {
    Commits,
    Rewrites,
    Discards,
    Branches,
    Updates,
    TakeBacks,
    Snapshots,
    Other,
}

impl Group {
    const ALL: [Group; 8] = [
        Group::Commits,
        Group::Rewrites,
        Group::Discards,
        Group::Branches,
        Group::Updates,
        Group::TakeBacks,
        Group::Snapshots,
        Group::Other,
    ];

    fn label(self) -> &'static str {
        match self {
            Group::Commits => "Commits made",
            Group::Rewrites => "Commits amended, squashed or moved",
            Group::Discards => "Changes thrown away",
            Group::Branches => "Branches",
            Group::Updates => "Updates from the target",
            Group::TakeBacks => "Undos and restores",
            Group::Snapshots => "Snapshots",
            Group::Other => "Other",
        }
    }

    /// Green for what it added, blue for what it rewrote, red for what it threw away, amber
    /// for what it took back, and quiet for the rest.
    fn tint(self, theme: &Theme) -> gpui::Hsla {
        match self {
            Group::Commits | Group::Updates => theme.safe,
            Group::Rewrites => theme.blue,
            Group::Discards => theme.danger,
            Group::TakeBacks => theme.warn,
            Group::Branches | Group::Snapshots => theme.text_2,
            Group::Other => theme.text_3,
        }
    }
}

/// What a row says the operation was done to: the commit or branch its snapshot shows it
/// changed, else what the log says, as the operation an undo took back.
fn summary(operation: &Operation) -> Option<String> {
    let snapshot = operation.kind == Some(OperationKind::OnDemandSnapshot);
    (subject(operation).or_else(|| operation.detail.clone()))
        // A snapshot taken by hand is what its row is about, with a message or without.
        .or_else(|| snapshot.then(|| operation.title.clone()))
}

/// What hovering a row's glyph says it did: its title, and for an undo what it took back.
fn glyph_tooltip(operation: &Operation) -> String {
    match (
        describe(operation.kind).1 == Group::TakeBacks,
        &operation.detail,
    ) {
        (true, Some(undone)) => format!("{}: {undone}", operation.title),
        _ => operation.title.clone(),
    }
}

/// The commit or branch `operation`'s changes are about, or for an update what it brought.
fn subject(operation: &Operation) -> Option<String> {
    use OperationKind as K;
    let (mut edited, mut added, mut moved, mut removed) = (vec![], vec![], vec![], vec![]);
    for change in &operation.changes {
        match change {
            Change::Edited { after, .. } => edited.push(after),
            Change::Added { commit, .. } => added.push(commit),
            Change::Moved { commit, .. } => moved.push(commit),
            Change::Removed { commit, .. } => removed.push(commit),
            _ => {}
        }
    }
    let name = |commits: &[&Recorded]| match commits {
        [] => None,
        [one] => Some(commit_title(&one.title()).to_string()),
        many => Some(plural(many.len(), "commit")),
    };
    if let (Some(K::SquashCommit), [gone], [into]) = (operation.kind, &removed[..], &edited[..]) {
        let (gone, into) = (commit_title(&gone.title()), commit_title(&into.title()));
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

#[derive(Default)]
pub(super) struct History {
    operations: Vec<Operation>,
    /// Whether older operations remain to load.
    more: bool,
    loading: bool,
    /// A reload was asked for and hasn't landed. One asked for while an older page loads, or
    /// cancelled by one, follows it.
    reload_queued: bool,
    selected: Option<SnapshotId>,
    /// Why reading the newest operations failed, shown in place of the list.
    failed: Option<String>,
}

impl Detail {
    /// Load `project`'s newest operations, as many as are shown so the older pages and the
    /// selection stay, or with `more` the next page after those.
    pub(super) fn load_history(&mut self, project: &ProjectId, more: bool, cx: &mut Context<Self>) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        // A reload waits for an older page still loading, which replacing it would cancel, and
        // Show older does nothing while it loads.
        if let Some(history) = self.history.get_mut(project) {
            history.reload_queued |= !more;
            if history.loading {
                return;
            }
        }
        let shown = self.history.get(project);
        let after = (shown.filter(|_| more))
            .and_then(|history| history.operations.last())
            .map(|operation| operation.id);
        let limit = match more {
            true => PAGE,
            false => shown.map_or(PAGE, |history| history.operations.len().max(PAGE)),
        };
        // One load runs at a time, so another project's older page still loading no longer is.
        for (p, history) in &mut self.history {
            history.loading = more && p == project;
            history.reload_queued &= p == project;
        }
        // What the newest operation left, which its snapshot is compared with: the branches,
        // which snapshots don't record mid-edit.
        let now = (self.store.read(cx).workspace(project))
            .filter(|ws| ws.editing.is_none())
            .map(backend::history::branches);
        let project = project.clone();
        self._history_load = cx.spawn(async move |this, cx| {
            let result =
                backend::run(move || backend::history::list(&path, after, limit, now)).await;
            this.update(cx, |this, cx| {
                let history = this.history.entry(project.clone()).or_default();
                history.loading = false;
                let queued = std::mem::take(&mut history.reload_queued) && after.is_some();
                let selected_before = history.selected;
                let (read, first) = (result.is_ok(), history.operations.is_empty());
                match result {
                    // What's listed stays, Show older too, to try again.
                    Err(err) if after.is_some() => this.store.update(cx, |store, cx| {
                        store.failed("Couldn't read the history", &err, cx)
                    }),
                    Err(err) => history.failed = Some(format!("{err:#}")),
                    Ok((operations, more)) if after.is_some() => {
                        history.operations.extend(operations);
                        history.more = more;
                    }
                    Ok((mut operations, mut more)) => {
                        // New operations push the oldest of the rows asked for out: those
                        // already listed stay, older pages and the selection with them, as the
                        // log only grows at the top.
                        let last = operations.last().map(|op| op.id);
                        let ops = &history.operations;
                        if let Some(at) = ops.iter().position(|op| Some(op.id) == last) {
                            operations.extend(history.operations.split_off(at + 1));
                            more = history.more;
                        }
                        history.operations = operations;
                        history.more = more;
                        history.failed = None;
                    }
                }
                // Keep the selection when it's still listed, else take the newest.
                let listed = |id| history.operations.iter().any(|op| op.id == id);
                if !history.selected.is_some_and(listed) {
                    history.selected = history.operations.first().map(|op| op.id);
                }
                let moved = history.selected != selected_before;
                this.refresh_diff(moved, cx);
                // Not one the filter or the kinds hide. An older page, or the first, read while
                // the filter showed none goes on to the next; a reload doesn't, nor a page that
                // failed, which Show older asks for again.
                let older = read && (after.is_some() || first);
                if read && !(older && this.read_older_while_none_shown(cx)) {
                    this.select_first_shown(cx);
                }
                if queued {
                    this.load_history(&project, false, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// `project`'s operations, once read.
    fn history_for(&self, project: &ProjectId) -> Option<&History> {
        self.history.get(project).filter(|h| h.failed.is_none())
    }

    /// The operations of the History page selected.
    fn page_history(&self, cx: &App) -> Option<&History> {
        match &self.store.read(cx).selection {
            Some(Selection::History(project)) => self.history_for(project),
            _ => None,
        }
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
        let Some(Selection::History(project)) = self.selection(cx) else {
            return;
        };
        let Some(history) = self.history.get_mut(&project) else {
            return;
        };
        // Selected already, as when its menu opens, how its diff is looked at stays.
        if history.selected.replace(id) != Some(id) {
            self.show_picked(cx);
        }
    }

    /// ↓ or ↑ (`delta`) in the list: the next or previous operation, kept in view.
    fn step_history(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(history) = self.page_history(cx) else {
            return;
        };
        let shown = self.shown_operations(history, cx);
        let at = (shown.iter()).position(|&ix| {
            (history.operations.get(ix)).is_some_and(|op| Some(op.id) == history.selected)
        });
        let Some(&next) = step_from(&shown, at, delta) else {
            return;
        };
        let Some(id) = history.operations.get(next).map(|op| op.id) else {
            return;
        };
        // Kept in view by its row, below the day headings.
        let row = rows(history, &shown)
            .iter()
            .position(|row| matches!(row, Row::Operation(ix) if *ix == next));
        self.select_operation(id, cx);
        if let Some(row) = row {
            (self.history_scroll).scroll_to_item(row, gpui::ScrollStrategy::Nearest);
        }
    }

    /// The operations the list shows, by index: of kinds not left out, and matching the
    /// filter by what was done or to what.
    fn shown_operations(&self, history: &History, cx: &App) -> Vec<usize> {
        let query = self.history_filter.read(cx).text().trim().to_lowercase();
        let matches = |text: &str| text.to_lowercase().contains(&query);
        (history.operations.iter().enumerate())
            .filter(|(_, op)| !self.history_hidden.contains(&describe(op.kind).1))
            .filter(|(_, op)| {
                query.is_empty() || matches(&op.title) || summary(op).is_some_and(|s| matches(&s))
            })
            .map(|(ix, _)| ix)
            .collect()
    }

    /// Select the first operation the list shows when it hides the selected one, so the
    /// header and diff are of one in the list, or read older ones while it shows none.
    pub(super) fn select_shown_operation(&mut self, cx: &mut Context<Self>) {
        if !self.read_older_while_none_shown(cx) {
            self.select_first_shown(cx);
        }
    }

    /// Read the next older page while the filter shows none of those read, as it finds an old
    /// operation in a long log agents add to, up to [`READ_FOR_FILTER`] operations, past which
    /// Show older goes on. Whether it did.
    fn read_older_while_none_shown(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(history) = self.page_history(cx) else {
            return false;
        };
        let reads = history.more
            && history.operations.len() < READ_FOR_FILTER
            && self.history_hidden.len() < Group::ALL.len()
            && self.shown_operations(history, cx).is_empty();
        match (reads, self.store.read(cx).selection.clone()) {
            (true, Some(Selection::History(project))) => {
                self.load_history(&project, true, cx);
                true
            }
            _ => false,
        }
    }

    fn select_first_shown(&mut self, cx: &mut Context<Self>) {
        let Some(history) = self.page_history(cx) else {
            return;
        };
        let shown = self.shown_operations(history, cx);
        let id_of = |ix: &usize| history.operations.get(*ix).map(|op| op.id);
        let listed = (shown.iter())
            .filter_map(id_of)
            .any(|id| Some(id) == history.selected);
        if let (false, Some(first)) = (listed, shown.first().and_then(id_of)) {
            self.select_operation(first, cx);
        }
    }

    /// Which kinds of operation the list shows, each ticked or not.
    fn history_filter_items(&self, _: &mut Context<Self>) -> Vec<MenuItem> {
        (Group::ALL.into_iter())
            .map(|group| {
                MenuItem::new(group.label(), move |app, _, cx| {
                    app.detail.update(cx, |detail, cx| {
                        if !detail.history_hidden.remove(&group) {
                            detail.history_hidden.insert(group);
                        }
                        detail.select_shown_operation(cx);
                        cx.notify()
                    })
                })
                .ticked(!self.history_hidden.contains(&group))
            })
            .collect()
    }

    /// Opening `project`'s history, wherever from: at the top, on the newest operation, which
    /// the reload that follows picks, rather than on one picked a visit ago and since pushed
    /// out of view by newer ones.
    pub(super) fn open_on_newest_operation(&mut self, project: &ProjectId) {
        if let Some(history) = self.history.get_mut(project) {
            history.selected = None;
        }
        (self.history_scroll).scroll_to_item(0, gpui::ScrollStrategy::Top);
    }

    /// Back to the newest operation listed, at the top, as the page opens on, when it's picked
    /// again.
    pub(super) fn pick_newest_operation(&mut self, cx: &mut Context<Self>) {
        let Some(history) = self.page_history(cx) else {
            return;
        };
        let newest = self.shown_operations(history, cx).first().copied();
        if let Some(op) = newest.and_then(|ix| history.operations.get(ix)) {
            self.select_operation(op.id, cx);
            (self.history_scroll).scroll_to_item(0, gpui::ScrollStrategy::Top);
        }
    }

    /// The selected operation, which [`Self::render_operation_header`] draws a header for.
    pub(crate) fn selected_operation(&self, project: &ProjectId) -> Option<&Operation> {
        let history = self.history_for(project)?;
        let id = history.selected?;
        history.operations.iter().find(|op| op.id == id)
    }

    /// Return on the list: restore to before the selected operation, after asking, as its
    /// menu's Restore does.
    fn restore_selected_operation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::History(project)) = self.store.read(cx).selection.clone() else {
            return;
        };
        if let Some(id) = self.selected_operation(&project).map(|op| op.id) {
            self.restore_operation(&project, id, window, cx)
        }
    }

    /// Put the project back to how it was before operation `id`, after asking. The restore
    /// goes in the log too, so it can be undone.
    fn restore_operation(
        &self,
        project: &ProjectId,
        id: SnapshotId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let history = self.history_for(project);
        let Some(operation) = history.and_then(|h| h.operations.iter().find(|op| op.id == id))
        else {
            return;
        };
        // Named as its row is, by what it was done to.
        let title = summary(operation).unwrap_or_else(|| operation.title.clone());
        let project = project.clone();
        self.app.update_later(window, cx, move |app, window, cx| {
            app.confirm_then(
                Some(Selection::History(project.clone())),
                format!("Restore to before \"{title}\"?"),
                &crate::ui::keys_text(
                    "Branches, commits and uncommitted changes go back to how they were. \
                     You can undo this with ⌘Z.",
                ),
                "Restore",
                window,
                cx,
                move |app, cx| {
                    app.store
                        .update(cx, |store, cx| store.restore(&project, id, cx))
                },
            )
        });
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
        self.open_menu(
            at,
            move |_, cx| Self::operation_menu_items(&project, id, cx),
            cx,
        );
    }

    /// What Operations history offers for the operation before snapshot `id`, as
    /// right-clicking it does.
    pub(crate) fn operation_menu_items(
        project: &ProjectId,
        id: SnapshotId,
        cx: &App,
    ) -> Vec<MenuItem> {
        let project = project.clone();
        vec![
            MenuItem::keyed(
                "Restore to before this…",
                &crate::app::Confirm,
                cx,
                move |app, window, cx| {
                    (app.detail).update(cx, |detail, cx| {
                        detail.restore_operation(&project, id, window, cx)
                    })
                },
            ),
            MenuItem::new("Copy snapshot ID", move |app, _, cx| {
                app.copy_text(id.0.to_string(), cx)
            })
            .separated(),
        ]
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
        let undone = describe(operation.kind).1 == Group::TakeBacks;
        let when = format!(
            "{} {}",
            day_label(operation.time),
            local_clock(operation.time)
        );
        let meta = meta_row(theme)
            .child(meta_item(Icon::Clock, theme).child(when))
            .when_some(operation.detail.clone().filter(|_| undone), |el, what| {
                el.child(meta_item(Icon::Undo, theme).child(what))
            })
            // What a snapshot taken by hand is for, when it was given a message.
            .when_some(
                (operation.detail.clone())
                    .filter(|_| operation.kind == Some(OperationKind::OnDemandSnapshot)),
                |el, message| el.child(meta_item(Icon::TextBlock, theme).child(message)),
            )
            .when_some(
                branches.first().filter(|_| branches.len() == 1),
                |el, branch| el.child(meta_item(Icon::Branch, theme).child(branch.to_string())),
            );
        let more = operation.changes.len().saturating_sub(LISTED);
        let lines = (operation.changes.iter().take(LISTED).enumerate())
            .map(|(ix, change)| self.change_line(ix, project, operation.kind, change, theme, cx));
        let title = page_title(describe(operation.kind).0, theme)
            .child(title_text(operation.title.clone()));
        let body = v_flex()
            .gap(dp(10.))
            .child(meta)
            .when(!operation.changes.is_empty(), |el| {
                el.child(v_flex().gap(dp(4.)).children(lines).when(more > 0, |el| {
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
                .min_w_0()
                .child(self.header_frame(title, body, menu, None, None, theme))
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
        let title = |commit: &Recorded| commit_title(&commit.title()).to_string();
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
            .min_w_0()
            .gap(dp(8.))
            .text_size(dp(13.))
            .child(small_icon(glyph).text_color(color))
            .child(div().min_w_0().truncate().child(text))
            .child(faint(note, theme).flex_none().max_w(dp(320.)).truncate())
            .when_some(here, |el, id| {
                el.hover(|el| el.text_color(theme.blue))
                    .tooltip(Tooltip::text("Show this commit"))
                    .on_click(move |_, _, cx| {
                        let selection = Selection::Commit(project.clone(), id);
                        store.update(cx, |store, cx| store.select(Some(selection), cx))
                    })
            })
            .into_any_element()
    }

    /// Ask what a snapshot is for, which it may go without, then take it.
    fn take_snapshot(&self, project: &ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        let project = project.clone();
        self.app.update_later(window, cx, move |app, window, cx| {
            let input = cx.new(|cx| TextInput::new("What it's for (optional)", false, cx));
            let place = app
                .store
                .read(cx)
                .place(&Selection::Project(project.clone()));
            app.open_prompt(
                Some(place),
                "Take a snapshot",
                Some("Saves the project as it is now, to restore later from its menu.".into()),
                "Take snapshot",
                input,
                None,
                None,
                window,
                cx,
                move |app, message, _, cx| {
                    let message = Some(message.trim().to_owned()).filter(|m| !m.is_empty());
                    app.store.update(cx, |store, cx| {
                        store.run(
                            &project,
                            "Snapshot",
                            Overlay::default(),
                            move |path, _| backend::history::take_snapshot(path, message),
                            |store, (), cx| store.snack("Snapshot taken", cx),
                            cx,
                        )
                    })
                },
            )
        });
    }

    pub(super) fn render_history_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(history) = self.history.get(project) else {
            return loading_state("Loading history…", theme, cx);
        };
        if let Some(err) = &history.failed {
            let title = "Couldn't read the history".to_owned();
            return list_empty(Illustration::Papers, title, error_summary(err), theme, cx);
        }
        let snapshot = project.clone();
        // The oplog is kept against the target, so without one there's nothing to record.
        let no_target =
            (self.store.read(cx).workspace(project)).is_some_and(|ws| ws.target.is_none());
        // The filter, which kinds of operation it shows, and a snapshot of the project now.
        let narrowed = !self.history_hidden.is_empty();
        let bar = self
            .filter_bar(
                &self.history_filter,
                "Kinds of operation",
                Self::history_filter_items,
                narrowed,
                window,
                cx,
            )
            .child(
                icon_button("take-snapshot", Icon::Plus)
                    .small()
                    .tooltip("Take a snapshot of the project as it is, to restore later")
                    .disabled_because(no_target.then_some("Pick a target branch first"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.take_snapshot(&snapshot, window, cx)
                    })),
            );
        let shown = self.shown_operations(history, cx);
        let list = if history.operations.is_empty() {
            list_empty(
                Illustration::Cactus,
                "Nothing here yet".to_owned(),
                match no_target {
                    true => "GitButler records history once the project has a target branch",
                    false => "Changes you make with GitButler show up here",
                },
                theme,
                cx,
            )
        } else if shown.is_empty() && !history.more {
            let query = self.history_filter.read(cx).text();
            list_filtered_out(query, ("operation", "operations"), theme, cx)
        } else {
            let rows = rows(history, &shown);
            let project = project.clone();
            side_list(
                "history",
                rows.len(),
                &self.history_scroll,
                "history-scrollbar",
                cx,
                move |this, ix, theme, window, cx| {
                    let history = this.history_for(&project)?;
                    let row = match rows.get(ix)? {
                        Row::Day(time) => return Some(list_heading(day_label(*time), theme)),
                        Row::Operation(op) => {
                            let operation = history.operations.get(*op)?;
                            let selected = history.selected == Some(operation.id);
                            let on = selected && this.side_focus.is_focused(window);
                            let quiet = |color: gpui::Hsla| match on {
                                true => theme.muted_on_solid(),
                                false => color,
                            };
                            let id = operation.id;
                            // What it did as a glyph, named on hover, then what it was done
                            // to, or its name, faint, when nothing says; then when.
                            let (text, said) = match summary(operation) {
                                Some(text) => (text, true),
                                None => (operation.title.clone(), false),
                            };
                            let (glyph, group) = describe(operation.kind);
                            side_row(("operation", ix), selected, on, theme)
                                .child(
                                    div()
                                        .id(("operation-glyph", ix))
                                        .flex_none()
                                        .tooltip(Tooltip::text(glyph_tooltip(operation)))
                                        .child(icon(glyph).text_color(quiet(group.tint(theme)))),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .when(!said, |el| el.text_color(quiet(theme.text_3)))
                                        .child(text),
                                )
                                .child(
                                    faint(local_clock(operation.time), theme)
                                        .flex_none()
                                        .text_color(quiet(theme.text_3)),
                                )
                                .on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.select_operation(id, cx)
                                    }),
                                )
                                .on_mouse_down(MouseButton::Right, {
                                    let project = project.clone();
                                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                        this.operation_menu(&project, id, event.position, cx)
                                    })
                                })
                        }
                        Row::More => {
                            let project = project.clone();
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
            .into_any_element()
        };
        self.list_column(
            Self::step_history,
            None,
            Some(Self::restore_selected_operation),
            cx,
        )
        .child(bar)
        .child(list)
        .into_any_element()
    }
}
