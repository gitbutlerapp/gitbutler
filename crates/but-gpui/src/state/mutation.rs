//! Operations that change a project. A project runs one at a time, its overlay hiding what it
//! takes away until the reload after it, and refuses another meanwhile; fetches, pushes and
//! deleting a linked worktree run beside it.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{Context, SharedString, Task};

use super::{Notice, ProjectId, Reload, Selection, Store, StoreEvent};
use crate::backend::history::Undone;
use crate::backend::upstream::{UpdateConflicts, UpdateOutcome, UpdateScope};
use crate::backend::{
    self, Branch, Checkout, Commit, CommitId, Destination, FileChange, FullName, HunkKey, OpKind,
    Review, ReviewState, Selected, SnapshotId, Workspace, short_ref,
};

/// How long a mutation runs before the status bar says how long it's taking.
pub const SLOW: Duration = Duration::from_millis(800);

/// How long a commit a mutation makes shows as made before it dims, to say the mutation is
/// still running: one done sooner never looks pending.
const PENDING_AFTER: Duration = Duration::from_millis(400);

/// What a running operation takes away, hidden until the reload after it shows
/// the result, and the commit it makes, shown as a stand-in meanwhile. Nothing here predicts
/// what git will do; the reload always has the last word.
#[derive(Default)]
pub struct Overlay {
    /// Commits leaving, by key.
    pub commits: HashSet<String>,
    /// Uncommitted files leaving whole, by path.
    pub files: HashSet<String>,
    /// Hunks leaving uncommitted files that otherwise stay, by path.
    pub hunks: HashMap<String, Vec<HunkKey>>,
    /// The checkout whose files `files` and `hunks` are.
    pub files_in: Checkout,
    /// Branches leaving, by full ref, as a stack's do when it's unapplied.
    pub branches: HashSet<FullName>,
    /// Branches deleted from their stack, by full ref: the ref goes, its commits stay.
    pub deleted_branches: HashSet<FullName>,
    /// Linked worktrees leaving, by name.
    pub worktrees: HashSet<bstr::BString>,
    /// Files leaving a commit whole, by path, under the commit's key.
    pub commit_files: HashMap<String, HashSet<String>>,
    /// Hunks, or lines of them, leaving a commit's files that otherwise stay: by path, under
    /// the commit's key.
    pub commit_hunks: HashMap<String, HashMap<String, Vec<HunkKey>>>,
    /// Where a new commit goes, and its title.
    pub new_commit: Option<(NewAt, SharedString)>,
    /// The commit (by key) it gives a new message, and the message.
    pub reworded: Option<(String, String)>,
    /// The branch (full ref) it renames, and its new name.
    pub renamed: Option<(FullName, String)>,
    /// What it changes, which the reload after it reads again.
    pub rereads: Rereads,
    /// The pull request, by number, it merges.
    pub merges: Option<u64>,
    /// The pull request, by number, whose auto-merge it turns on or off, and which.
    pub auto_merges: Option<(u64, bool)>,
    /// It opens a pull request for the branch it pushes.
    pub publishes: bool,
}

/// What an operation is called: `text` is what the user reads, which may name a branch or a
/// commit, and `name` the same without them, which the metrics know it by. A label that names
/// nothing is both.
pub struct Label {
    name: &'static str,
    text: SharedString,
}

impl Label {
    pub fn new(name: &'static str, text: impl Into<SharedString>) -> Self {
        Self {
            name,
            text: text.into(),
        }
    }
}

impl From<&'static str> for Label {
    fn from(text: &'static str) -> Self {
        Self::new(text, text)
    }
}

/// What an operation changes, for the reload after it to read again.
#[derive(Default, Clone, Copy)]
pub enum Rereads {
    /// Commits and branches, and the files of each checkout whose `HEAD` has another tree
    /// by now, which is all an operation on commits writes: see [`backend::Kept`]. When it
    /// takes uncommitted files, the checkout `files_in` names too.
    #[default]
    Commits,
    /// Only the uncommitted files of the checkout `files_in` names, or of the main one.
    Files,
    /// As [`Self::Commits`], and the main checkout's files whatever its `HEAD`, which
    /// restoring from the oplog and leaving an edit can write under an unchanged tree.
    CommitsAndMain,
}

/// Where a commit an operation makes shows until it's loaded.
pub enum NewAt {
    /// On top of this branch.
    Top(FullName),
    /// Right above the commit with this key.
    Above(String),
    /// Right below the commit with this key.
    Below(String),
}

pub(super) type Checkouts = HashSet<Checkout>;

impl Overlay {
    /// The checkouts it makes commits in: the one whose files it takes, the one its new commit
    /// goes in, those of the commits it changes, and the linked worktrees resting on them,
    /// which their rewrite moves. An agent's commits in another, made meanwhile, aren't its
    /// own.
    pub(super) fn checkouts(&self, ws: &Workspace) -> Checkouts {
        let moved_by =
            |commit: Option<&Commit>| commit.map_or_else(HashSet::new, |c| ws.moved_by(c.id));
        let keyed = |key: &str| ws.commits().find(|commit| commit.key == key);
        let landing = (self.new_commit.iter()).map(|(at, _)| match at {
            NewAt::Top(ref_name) => {
                let Some((lane, at)) = ws.lane_of(Some(ref_name)) else {
                    return HashSet::new();
                };
                let tip = (lane.branches.iter().skip(at)).find_map(|b| b.commits.first());
                let mut moved = moved_by(tip);
                moved.insert(lane.checkout.clone());
                moved
            }
            NewAt::Above(key) | NewAt::Below(key) => moved_by(keyed(key)),
        });
        let changed = (self.commits.iter())
            .chain(self.commit_files.keys())
            .chain(self.commit_hunks.keys())
            .chain(self.reworded.iter().map(|(key, _)| key))
            .map(|key| moved_by(keyed(key)));
        (landing.chain(changed))
            .flatten()
            .chain([self.files_in.clone()])
            .collect()
    }

    /// For an operation that can rewrite the main checkout's files under its `HEAD`: see
    /// [`Rereads::CommitsAndMain`].
    pub fn rewriting_main() -> Self {
        Self {
            rereads: Rereads::CommitsAndMain,
            ..Self::default()
        }
    }

    pub fn commits(keys: impl IntoIterator<Item = String>) -> Self {
        Self {
            commits: keys.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Whether `selection` is on a branch or in a linked worktree this takes away.
    fn takes_away(&self, selection: &Selection, store: &Store) -> bool {
        let branch = match selection {
            Selection::Stack(_, bottom) => bottom.as_ref(),
            _ => store
                .branch_for(selection)
                .and_then(|branch| branch.ref_name.as_ref()),
        };
        let worktree = match selection {
            Selection::Worktree(_, name) => Some(name),
            _ => store.worktree_for(selection).map(|wt| &wt.name),
        };
        let deleted = match selection {
            Selection::Branch(_, ref_name) | Selection::Stack(_, ref_name) => {
                (ref_name.as_ref()).is_some_and(|ref_name| self.deleted_branches.contains(ref_name))
            }
            _ => false,
        };
        deleted
            || branch.is_some_and(|ref_name| self.branches.contains(ref_name))
            || worktree.is_some_and(|name| self.worktrees.contains(name))
    }

    /// Hides the files taken whole, and of the others the hunks taken.
    pub fn files<'a>(selected: impl IntoIterator<Item = &'a backend::Selected>) -> Self {
        let mut overlay = Self::default();
        for s in selected {
            let path = s.file.path.clone();
            match &s.hunks {
                None => {
                    overlay.files.insert(path);
                }
                Some(hunks) => overlay.hunks.entry(path).or_default().extend(hunks),
            }
        }
        overlay
    }

    /// Hides what's taken out of the commit with key `commit`: files whole, or hunks or lines
    /// of them.
    pub fn commit_files<'a>(
        commit: Option<String>,
        selected: impl IntoIterator<Item = &'a backend::Selected>,
    ) -> Self {
        let Some(commit) = commit else {
            return Self::default();
        };
        let Self { files, hunks, .. } = Self::files(selected);
        Self {
            commit_files: HashMap::from([(commit.clone(), files)]),
            commit_hunks: HashMap::from([(commit, hunks)]),
            ..Self::default()
        }
    }
}

/// A project's operation running now, from when it starts until the reload after it lands.
pub(super) struct Running {
    project: ProjectId,
    /// Tells it apart from the project's others running beside it.
    id: usize,
    label: SharedString,
    pub(super) overlay: Overlay,
    kind: OpKind,
    started: Instant,
    pub(super) phase: Phase,
    /// Reloads asked for while it ran, which the reload after it reads too.
    pub(super) deferred: Reload,
}

/// How far a running operation has got: the reload after it, not its own end, ends it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Phase {
    Working,
    /// Finished, waiting for a load to start after it.
    Finished,
    /// Finished, with the load started since in flight, which ends it when it lands.
    Reloading,
}

/// An operation of the user's that ⌘Z takes back: the oplog entry it recorded in its project,
/// when it started, which orders it among the others, when it finished, in milliseconds since
/// the Unix epoch as file times are, and what its snackbar said.
pub(super) struct UndoStep {
    pub(super) project: ProjectId,
    pub(super) snapshot: SnapshotId,
    started: Instant,
    done: i64,
    pub(super) said: Option<SharedString>,
}

/// A file by its path, or several by how many, as an undo or redo card names what changed.
fn files_named(files: &[String]) -> String {
    match files {
        [file] => file.clone(),
        files => crate::ui::plural(files.len(), "file"),
    }
}

const WAIT_TO_UNDO: &str = "Wait for the running operation to finish, then undo it";

impl Store {
    /// Confirm an operation, or warn about the changes it had to leave out.
    pub fn report_left_out(&mut self, done: &str, left_out: &[String], cx: &mut Context<Self>) {
        if left_out.is_empty() {
            self.snack_undoable(done.to_owned(), cx);
        } else {
            self.error(
                format!("{done}, leaving some changes out"),
                left_out.join("\n"),
                cx,
            );
        }
    }

    /// Undo what the user last did, in whichever project they did it.
    pub fn undo_last(&mut self, cx: &mut Context<Self>) {
        // The newest step not being undone already, as by a ⌘Z just before.
        let last = (self.undo_steps.iter().rev())
            .find(|step| !self.pending(&step.project, Some(&OpKind::Undo(step.snapshot)), false));
        // Started after it in any project, as ⌘Z follows the user there, and still running.
        if self.running_after(last.map(|step| step.started), None) {
            return self.snack(WAIT_TO_UNDO, cx);
        }
        match last.map(|step| (step.project.clone(), step.snapshot)) {
            Some((project, snapshot)) => self.undo_snapshot(&project, snapshot, None, None, cx),
            None => self.snack("Nothing to undo", cx),
        }
    }

    /// Whether an operation the user would mean to undo first is still working: one started
    /// after `after` (any, if `None`) in `project` (any, if `None`). Fetches and what only asks
    /// the forge don't count but pushes do, and undos never count.
    fn running_after(&self, after: Option<Instant>, project: Option<&ProjectId>) -> bool {
        self.working().any(|r| {
            !matches!(r.kind, OpKind::Fetch | OpKind::Forge | OpKind::Undo(_))
                && project.is_none_or(|p| r.project == *p)
                && after.is_none_or(|after| r.started > after)
        })
    }

    /// Every operation running, but for one waiting only for its reload.
    fn working(&self) -> impl Iterator<Item = &Running> {
        self.running.iter().filter(|r| r.phase == Phase::Working)
    }

    /// Make the operation that recorded `snapshot` in `project` one ⌘Z takes back, among the
    /// others by when it started.
    fn push_undo_step(&mut self, project: ProjectId, snapshot: SnapshotId, started: Instant) {
        let done = backend::now_millis();
        (self.undo_steps).push(UndoStep {
            project,
            snapshot,
            started,
            done,
            said: None,
        });
        self.undo_steps.sort_by_key(|step| step.started);
    }

    /// What ⌘Z takes back next, as its snackbar said it, with its project and when it
    /// finished, in milliseconds since the Unix epoch. None while that snackbar still shows,
    /// or when it said nothing.
    pub fn last_done(&self) -> Option<(&ProjectId, &SharedString, i64)> {
        let step = self.undo_steps.last()?;
        let said = step.said.as_ref()?;
        let showing = self.toasts.iter().any(|toast| {
            matches!(&toast.notice, Notice::Snackbar { undo: Some((project, snapshot)), .. }
                if *project == step.project && *snapshot == step.snapshot)
        });
        (!showing).then_some((&step.project, said, step.done))
    }

    /// Redo what ⌘Z last took back, unless the user has done something since.
    pub fn redo_last(&mut self, cx: &mut Context<Self>) {
        if (self.working()).any(|r| matches!(r.kind, OpKind::Undo(_))) {
            return self.snack("Wait for the undo to finish, then redo it", cx);
        }
        // The newest not being redone already.
        let Some((project, undo)) = (self.redo_steps.iter().rev())
            .find(|(project, undo)| !self.pending(project, Some(&OpKind::Redo(*undo)), false))
            .cloned()
        else {
            return self.snack("Nothing to redo", cx);
        };
        self.redo_snapshot(project, undo, None, cx)
    }

    /// Redo what the undo that recorded `undo` in `project` took back, unless something
    /// changed the project or a file was written since, or since `accepted` once the user said
    /// to redo anyway.
    fn redo_snapshot(
        &mut self,
        project: ProjectId,
        undo: SnapshotId,
        accepted: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        let (kept, ran, again, started) = (
            self.kept_snapshots
                .get(&project)
                .cloned()
                .unwrap_or_default(),
            self.ran.get(&project).cloned().unwrap_or_default(),
            project.clone(),
            Instant::now(),
        );
        self.apply(
            &project,
            "Redo",
            Overlay::rewriting_main(),
            backend::history::Redo {
                undo,
                kept,
                spans: ran,
                accepted,
            },
            move |store, redone, cx| {
                let operation = match redone {
                    Some(Undone::Done(operation)) => operation,
                    Some(Undone::Newer(files)) => {
                        let (what, asked) = (files_named(&files), Some(backend::now_millis()));
                        return store.error_with_retry(
                            store.card_title(&again, "Redo takes back newer changes too".into()),
                            format!(
                                "{what} changed after the undo. Redo puts files back as they \
                                 were before it, and Undo brings the changes back."
                            ),
                            "Redo anyway",
                            move |store, cx| store.redo_snapshot(again.clone(), undo, asked, cx),
                            cx,
                        );
                    }
                    // Something changed the project since, so none of its undos are to redo.
                    None | Some(Undone::Behind(_) | Undone::Already(_)) => {
                        (store.redo_steps).retain(|(project, _)| *project != again);
                        let why = format!(
                            "Something changed the project{} since the undo, so it can't be redone",
                            store.in_project(&again)
                        );
                        return store.snack(why, cx);
                    }
                };
                (store.redo_steps).retain(|step| *step != (again.clone(), undo));
                // Back on top of what ⌘Z takes back.
                if let Some((project, snapshot)) = store.last_snapshot.clone() {
                    store.push_undo_step(project, snapshot, started);
                }
                store.snack_undoable(format!("Redone: {operation}"), cx)
            },
            cx,
        );
    }

    /// Put `project` back to before the operation `snapshot` was taken for, unless that takes
    /// back one kept from undo.
    pub fn restore(&mut self, project: &ProjectId, snapshot: SnapshotId, cx: &mut Context<Self>) {
        let kept = self
            .kept_snapshots
            .get(project)
            .cloned()
            .unwrap_or_default();
        self.apply(
            project,
            "Restore",
            Overlay::rewriting_main(),
            backend::history::Restore { snapshot, kept },
            |store, (), cx| store.snack_undoable("Restored", cx),
            cx,
        );
    }

    /// Undo the operation that recorded `snapshot` in `project`, as long as nothing came
    /// after it there, in Git's logs or the files, and no push did, or none since `accepted`
    /// and `pushed_ok` once the user said to undo anyway about the files or a push; for ⌘⇧Z to
    /// redo. Its step stays among those ⌘Z takes back until it's undone, or can't ever be on
    /// its own.
    pub fn undo_snapshot(
        &mut self,
        project: &ProjectId,
        snapshot: SnapshotId,
        accepted: Option<i64>,
        pushed_ok: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        if (self.workspace(project)).is_some_and(|ws| ws.editing.is_some()) {
            let why = format!(
                "Finish or cancel editing the commit{} first",
                self.in_project(project)
            );
            return self.snack(why, cx);
        }
        let step = (self.undo_steps.iter())
            .find(|step| step.project == *project && step.snapshot == snapshot);
        let started = step.map(|step| step.started);
        if self.running_after(started, Some(project)) {
            return self.snack(WAIT_TO_UNDO, cx);
        }
        // What came after it, or after the user said to undo anyway, which still stops for
        // anything newer than that.
        let newer_than = accepted.or(step.map(|step| step.done));
        let again = project.clone();
        if (newer_than.max(pushed_ok).zip(self.pushed_at.get(project)))
            .is_some_and(|(since, pushed)| *pushed > since)
        {
            // The card comes before anything else is checked, so going ahead still checks
            // from where it would have, but for pushes up to now.
            let asked_now = Some(backend::now_millis());
            return self.error_with_retry(
                self.card_title(project, "Undo may take back what was pushed".into()),
                "A push came after the operation it undoes. If the push took what the \
                 operation did, the remote keeps that until the branch is force pushed.",
                "Undo anyway",
                move |store, cx| store.undo_snapshot(&again, snapshot, accepted, asked_now, cx),
                cx,
            );
        }
        let ran = self.ran.get(project).cloned().unwrap_or_default();
        // The snackbar offering to undo it goes with it.
        let offered = (project.clone(), snapshot);
        self.toasts.retain(|toast| {
            !matches!(&toast.notice, Notice::Snackbar { undo: Some(undo), .. } if *undo == offered)
        });
        self.apply(
            project,
            "Undo",
            Overlay::rewriting_main(),
            backend::history::Undo {
                snapshot,
                newer_than: newer_than.map(|at| (at, ran)),
            },
            move |store, undone, cx| {
                let title = |store: &Store| store.card_title(&again, "Undo failed".into());
                match undone {
                    Ok(Undone::Done(operation)) => {
                        (store.undo_steps)
                            .retain(|step| step.project != again || step.snapshot != snapshot);
                        let done = store.card_title(&again, format!("Undone: {operation}"));
                        if let Some((_, undo)) = store.last_snapshot {
                            store.redo_steps.push((again, undo));
                        }
                        store.snack(done, cx)
                    }
                    // Behind a newer step of the user's own: that one goes first, and this one
                    // stays.
                    Ok(Undone::Behind(_))
                        if (store.undo_steps.iter())
                            .skip_while(|step| step.project != again || step.snapshot != snapshot)
                            .skip(1)
                            .any(|step| step.project == again) =>
                    {
                        let why = "Undo the newer operation first".into();
                        store.snack(store.card_title(&again, why), cx)
                    }
                    // It, and the project's steps before it, are behind what came after.
                    Ok(Undone::Behind(why)) => {
                        store.undo_steps.retain(|step| {
                            step.project != again
                                || started.is_none_or(|started| step.started > started)
                        });
                        store.error(title(store), why, cx)
                    }
                    Ok(Undone::Already(operation)) => {
                        (store.undo_steps)
                            .retain(|step| step.project != again || step.snapshot != snapshot);
                        let why = format!("{operation} was undone already");
                        store.snack(store.card_title(&again, why), cx)
                    }
                    Ok(Undone::Newer(files)) => {
                        let (what, asked) = (files_named(&files), Some(backend::now_millis()));
                        store.error_with_retry(
                            store.card_title(&again, "Undo takes back newer changes too".into()),
                            format!(
                                "{what} changed after the operation it undoes. Undo puts files \
                                 back as they were before it, and Redo brings the changes back."
                            ),
                            "Undo anyway",
                            move |store, cx| {
                                store.undo_snapshot(&again, snapshot, asked, pushed_ok, cx)
                            },
                            cx,
                        )
                    }
                    Err(err) => store.failed(title(store), &err, cx),
                }
            },
            cx,
        );
    }

    /// Commit the edited commit as its files are now, and go back to the workspace.
    pub fn finish_editing(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        self.apply(
            project,
            "Finish editing",
            Overlay::rewriting_main(),
            backend::edit::Finish,
            |store, (), cx| store.snack("Finished editing", cx),
            cx,
        );
    }

    /// Go back to the workspace leaving the edited commit as it was; unless `force`, asking
    /// first when that drops what changed.
    pub fn cancel_editing(&mut self, project: &ProjectId, force: bool, cx: &mut Context<Self>) {
        let again = project.clone();
        self.apply(
            project,
            "Cancel editing",
            Overlay::rewriting_main(),
            backend::edit::Cancel { force },
            move |store, left, cx| match left {
                true => store.snack("Stopped editing, the commit is as it was", cx),
                // A conflicted commit's files always differ, markers and all, from the commit
                // as git can store it, so this can't tell whether anything was done to them.
                false => store.error_with_retry(
                    format!(
                        "Cancelling{} drops the files as they are",
                        store.in_project(&again)
                    ),
                    "The commit stays as it was, conflicts and all. What was done to its files \
                     since it was checked out, resolving included, is lost.",
                    "Cancel anyway",
                    move |store, cx| store.cancel_editing(&again, true, cx),
                    cx,
                ),
            },
            cx,
        );
    }

    /// Fetch `project`'s remotes, then, as `but pull` does, rebase what `update` picks onto
    /// the target, if it picks anything.
    pub fn fetch(
        &mut self,
        project: &ProjectId,
        update: Option<UpdateScope>,
        cx: &mut Context<Self>,
    ) {
        let fetched = project.clone();
        self.apply(
            project,
            "Fetch",
            Overlay::default(),
            backend::upstream::Fetch,
            move |store, fetched_or_not, cx| match fetched_or_not {
                Ok(any_remote) => {
                    store.fetched_now(&fetched, any_remote);
                    // The update's card says what came of both.
                    if let Some(scope) = update {
                        return store.update_from_target(&fetched, scope, None, cx);
                    }
                    let what = match any_remote {
                        true => "Fetched",
                        false => "No remote to fetch from",
                    };
                    store.snack(store.card_title(&fetched, what.into()), cx)
                }
                Err(err) => {
                    store.fetch_failed_now(&fetched);
                    let title = store.card_title(&fetched, "Fetch failed".into());
                    store.failed(title, &err, cx)
                }
            },
            cx,
        );
    }

    /// Rebase what `scope` picks of the project onto its target's latest commits. It stops
    /// short of leaving anything conflicted that isn't now, with a card that offers to update
    /// anyway, unless that's what was `accepted` there.
    pub fn update_from_target(
        &mut self,
        project: &ProjectId,
        scope: UpdateScope,
        accepted: Option<backend::upstream::UpdateConflicts>,
        cx: &mut Context<Self>,
    ) {
        let updated = project.clone();
        self.apply(
            project,
            "Update",
            Overlay::default(),
            backend::upstream::Update { scope, accepted },
            move |store, outcome, cx| {
                let target = (store.workspace(&updated).and_then(|ws| ws.target.as_ref()))
                    .map_or_else(|| "the target".to_owned(), |target| target.name.clone());
                match outcome {
                    UpdateOutcome::UpToDate => store.snack(up_to_date(scope, &target), cx),
                    UpdateOutcome::WouldConflict(found) => {
                        let again = updated.clone();
                        let title = store.card_title(&updated, "Updating leaves conflicts".into());
                        let workspace = store.workspace(&updated);
                        let detail = update_conflicts_detail(&target, &found, workspace, scope);
                        store.error_with_retry(
                            title,
                            detail,
                            "Update anyway",
                            move |store, cx| {
                                store.update_from_target(&again, scope, Some(found.clone()), cx)
                            },
                            cx,
                        )
                    }
                    UpdateOutcome::Updated { conflicted, landed } => {
                        // Its "Resolve the oldest…" bar is the next step, which a linked
                        // worktree's branch doesn't have, as its conflicts resolve in it.
                        let resolvable = (conflicted.iter())
                            .filter(|(branch, _)| branch.is_some())
                            .map(|(branch, _)| Selection::Branch(updated.clone(), branch.clone()))
                            .find(|branch| !store.in_linked(branch));
                        if let Some(branch) = resolvable {
                            store.select(Some(branch), cx);
                        }
                        store.snack_undoable(updated_message(&target, &conflicted, &landed), cx)
                    }
                }
            },
            cx,
        );
    }

    /// Bring the commits only `ref_name`'s remote has into it, its own rebased on top.
    /// It stops short of leaving commits conflicted that aren't now, unless that's what was
    /// `accepted` on the card saying so, which offers to pull anyway: a fetch since can
    /// bring more.
    pub fn pull_remote(
        &mut self,
        project: &ProjectId,
        ref_name: FullName,
        accepted: Option<Vec<(Option<FullName>, usize)>>,
        cx: &mut Context<Self>,
    ) {
        let name = short_ref(&ref_name).into_owned();
        let pulled = project.clone();
        let pull = backend::upstream::Pull {
            branch: ref_name.clone(),
            accepted,
        };
        self.apply(
            project,
            Label::new("Pull", format!("Pull {name}")),
            Overlay::default(),
            pull,
            move |store, found, cx| {
                let Some(found) = found else {
                    return store.snack_undoable(format!("Pulled into {name}"), cx);
                };
                let detail = format!(
                    "Rebasing onto the remote's commits leaves conflicted commits in {}. \
                     You can resolve them afterwards, or undo the pull.",
                    commit_counts(&found)
                );
                store.error_with_retry(
                    store.card_title(&pulled, "Pulling leaves conflicts".into()),
                    detail,
                    "Pull anyway",
                    move |store, cx| {
                        store.pull_remote(&pulled, ref_name.clone(), Some(found.clone()), cx)
                    },
                    cx,
                )
            },
            cx,
        );
    }

    /// Note that `project` just pushed, so ⌘Z asks before taking back what came before.
    pub fn pushed(&mut self, project: &ProjectId) {
        self.pushed_at
            .insert(project.clone(), backend::now_millis());
    }

    /// Push `ref_name`, and with `protect` refuse to drop commits only the remote has: the
    /// card that says so offers to push anyway. Without it, a forced push goes past the
    /// lease; with `drops`, the commits only the remote has that were shown, it stops at any
    /// others there.
    pub fn push(
        &mut self,
        project: &ProjectId,
        ref_name: FullName,
        protect: bool,
        drops: Option<Vec<CommitId>>,
        cx: &mut Context<Self>,
    ) {
        let name = short_ref(&ref_name).into_owned();
        // The branches below it with commits to send go along, down the lanes it rests on,
        // which the snackbar says.
        let below = (self.workspace(project)).map_or(0, |ws| ws.parents_to_push(&ref_name).count());
        let all = match below {
            0 => name.clone(),
            1 => format!("{name} and the branch below it"),
            n => format!("{name} and the {n} branches below it"),
        };
        // The branches below go up first, so one refused or failing partway may have sent
        // them: undo asks first all the same.
        if below > 0 {
            self.pushed(project);
        }
        let pushed = project.clone();
        let run_hooks = self.runs_hooks(project);
        self.apply(
            project,
            Label::new("Push", format!("Push {name}")),
            Overlay::default(),
            backend::upstream::Push {
                ref_name: ref_name.clone(),
                protect,
                drops,
                run_hooks,
            },
            move |store, outcome, cx| {
                // A push can open or move a pull request's checks, and the branches below
                // may have gone up before the one refused.
                store.refresh_forge(&pushed);
                let reason = match outcome {
                    Err(reason) => reason,
                    Ok(sync_failed) => {
                        store.pushed(&pushed);
                        store.snack(store.card_title(&pushed, format!("Pushed {all}")), cx);
                        if let Some(message) = sync_failed {
                            let title = format!(
                                "Pushed {name}{}, but the stack's pull requests weren't updated",
                                store.in_project(&pushed)
                            );
                            store.error(title, message, cx);
                        }
                        return;
                    }
                };
                let title = store.card_title(&pushed, format!("{name} wasn't pushed"));
                if !reason.is_empty() {
                    // Forcing past a protected branch or a push rule won't help.
                    return store.error(title, format!("The remote refused it: {reason}"), cx);
                }
                store.error_with_retry(
                    title,
                    "Someone pushed since the last fetch, and pushing would drop their commits. \
                     Fetch to see them, or push anyway.",
                    "Push anyway",
                    move |store, cx| store.push(&pushed, ref_name.clone(), false, None, cx),
                    cx,
                );
            },
            cx,
        );
    }

    /// Open `ref_name`'s pull request, pushing it and the branches below it first where they
    /// have anything to push, and with `auto_merge` turn its auto-merge on. A failure's card
    /// tries again with the same title and description.
    #[expect(clippy::too_many_arguments)]
    pub fn publish(
        &mut self,
        project: &ProjectId,
        ref_name: FullName,
        title: String,
        body: String,
        draft: bool,
        auto_merge: bool,
        cx: &mut Context<Self>,
    ) {
        let again = {
            let (project, ref_name) = (project.clone(), ref_name.clone());
            let (title, body) = (title.clone(), body.clone());
            move |store: &mut Store, cx: &mut Context<Store>| {
                let (title, body) = (title.clone(), body.clone());
                store.publish(
                    &project,
                    ref_name.clone(),
                    title,
                    body,
                    draft,
                    auto_merge,
                    cx,
                )
            }
        };
        // An operation is turned down with only a snackbar while a commit is being
        // edited, as in `but resolve`, which would lose what was typed: the card keeps it.
        if self
            .workspace(project)
            .is_some_and(|ws| ws.editing.is_some())
        {
            let said = self.card_title(project, "Couldn't create the pull request".to_owned());
            let why = "Finish or cancel editing the commit first.";
            return self.error_with_retry(said, why, "Try again", again, cx);
        }
        let published = project.clone();
        let run_hooks = self.runs_hooks(project);
        self.apply(
            project,
            "Create pull request",
            Overlay {
                publishes: true,
                ..Overlay::default()
            },
            backend::reviews::Publish {
                ref_name,
                title,
                body,
                draft,
                auto_merge,
                run_hooks,
            },
            move |store, made, cx| {
                // The push can move the stack's other pull requests too.
                store.refresh_forge(&published);
                match made {
                    Ok(made) => {
                        store.pushed(&published);
                        let said = match auto_merge && made.auto_merge_failed.is_none() {
                            true => {
                                format!("Created pull request #{} with auto-merge on", made.number)
                            }
                            false => format!("Created pull request #{}", made.number),
                        };
                        store.snack(store.card_title(&published, said), cx);
                        if let Some(message) = made.sync_failed {
                            let said = "The stack's other pull requests weren't updated";
                            store.error(said, message, cx);
                        }
                        if let Some(err) = made.auto_merge_failed {
                            let said = format!("Auto-merge isn't on for #{}", made.number);
                            store.failed(store.card_title(&published, said), &err, cx);
                        }
                    }
                    Err(err) => {
                        // The branches may have gone up before the forge said no.
                        store.pushed(&published);
                        let failed = match backend::upstream::push_refusal(&err).as_deref() {
                            None => backend::failure_detail(&err),
                            Some("") => "Someone pushed to the branch since the last fetch, and \
                                         pushing would drop their commits. Fetch to see them."
                                .to_owned(),
                            Some(said) => format!("The remote refused the push: {said}"),
                        };
                        let said = "Couldn't create the pull request".to_owned();
                        let said = store.card_title(&published, said);
                        store.error_with_retry(said, failed, "Try again", again, cx);
                    }
                }
            },
            cx,
        );
    }

    /// Hide linked worktree `name` from the workspace, or with `archived` false show it
    /// again. Like GitButler's other apps, this isn't something undo takes back.
    pub fn set_worktree_archived(
        &mut self,
        project: &ProjectId,
        name: bstr::BString,
        archived: bool,
        cx: &mut Context<Self>,
    ) {
        let (label, done) = if archived {
            ("Archive worktree", format!("Archived {name}"))
        } else {
            ("Show worktree", format!("{name} is back"))
        };
        let overlay = Overlay {
            worktrees: archived.then(|| name.clone()).into_iter().collect(),
            ..Overlay::default()
        };
        self.apply(
            project,
            label,
            overlay,
            backend::worktrees::SetArchived { name, archived },
            move |store, (), cx| store.snack(done, cx),
            cx,
        );
    }

    /// Make the linked worktree `create` says, and select it.
    pub fn create_worktree(
        &mut self,
        project: &ProjectId,
        create: backend::worktrees::CreateWorktree,
        cx: &mut Context<Self>,
    ) {
        let selected = project.clone();
        self.apply(
            project,
            "Create worktree",
            Overlay::default(),
            create,
            move |store, name, cx| {
                store.snack(format!("Created worktree {name}"), cx);
                store.select(Some(Selection::Worktree(selected, name)), cx)
            },
            cx,
        );
    }

    /// Delete linked worktree `name` from disk, with its uncommitted files `lost`, by their
    /// write times and sizes, which the user agreed to lose. Its branch stays. Undo can't
    /// bring the folder back.
    pub fn delete_worktree(
        &mut self,
        project: &ProjectId,
        name: bstr::BString,
        lost: Vec<(String, Option<i64>, Option<u64>)>,
        cx: &mut Context<Self>,
    ) {
        let done = format!("Deleted {name}");
        let overlay = Overlay {
            worktrees: HashSet::from([name.clone()]),
            ..Overlay::default()
        };
        self.apply(
            project,
            "Delete worktree",
            overlay,
            backend::worktrees::DeleteWorktree { name, lost },
            move |store, (), cx| store.snack(done, cx),
            cx,
        );
    }

    /// Move commits `ids` into a new branch of their own, and select that branch.
    pub fn move_to_new_branch(
        &mut self,
        project: &ProjectId,
        ids: Arc<[CommitId]>,
        cx: &mut Context<Self>,
    ) {
        let shown = project.clone();
        let keys = ids.iter().filter_map(|id| self.key_of(project, *id));
        self.apply(
            project,
            "Move to a new branch",
            Overlay::commits(keys.collect::<Vec<_>>()),
            backend::MoveToNewBranch { commits: ids },
            move |store, (_, branch), cx| {
                store.select(Some(Selection::Branch(shown, Some(branch))), cx);
                store.snack_undoable("Moved to a new branch", cx)
            },
            cx,
        );
    }

    /// Take `branch` out of the stack it shares into a stack of its own.
    pub fn unstack(&mut self, project: &ProjectId, branch: FullName, cx: &mut Context<Self>) {
        self.apply(
            project,
            "Unstack",
            Overlay::default(),
            backend::TearOffBranch { branch },
            |store, conflicted, cx| {
                store.snack_undoable(branch_moved("Moved to its own stack", &conflicted), cx)
            },
            cx,
        );
    }

    /// Amend commit `into` with uncommitted `files` of `worktree`, then call `then` once that
    /// worked.
    pub fn amend(
        &mut self,
        project: &ProjectId,
        into: CommitId,
        files: Arc<[Selected]>,
        worktree: Checkout,
        then: impl FnOnce(&mut Store, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let undoable = self.undoable(project, [worktree.clone()]);
        let overlay = Overlay {
            files_in: worktree.clone(),
            ..Overlay::files(files.iter())
        };
        // Which commit, as it's often not on screen.
        let said = (self.find_commit(project, into)).map_or("Amended".into(), |(_, commit)| {
            format!("Amended \"{}\"", crate::ui::commit_title(&commit.title))
        });
        let amend = backend::Amend {
            into,
            changes: files,
            checkout: worktree,
            run_hooks: self.runs_hooks(project),
        };
        self.apply(
            project,
            Label::new("Amend", format!("Amend {}", into.short())),
            overlay,
            amend,
            move |store, amended, cx| {
                store.follow(&amended.rewritten, cx);
                if !undoable {
                    store.keep_last_snapshot(cx);
                }
                store.report_left_out(&said, &amended.left_out, cx);
                if let Some(output) = amended.hook_failed {
                    store.error("Amended, but the post-commit hook failed", output, cx);
                }
                then(store, cx)
            },
            cx,
        );
    }

    /// Squash commits `ids`, listed newest first, into commit `into`, its message first and
    /// theirs after it in the order they were written.
    pub fn squash(
        &mut self,
        project: &ProjectId,
        ids: Arc<[CommitId]>,
        into: CommitId,
        cx: &mut Context<Self>,
    ) {
        let overlay = Overlay::commits(ids.iter().filter_map(|id| self.key_of(project, *id)));
        let label = Label::new("Squash", format!("Squash {} commits", ids.len() + 1));
        let squash = backend::Squash { commits: ids, into };
        self.apply(
            project,
            label,
            overlay,
            squash,
            |store, rewritten, cx| {
                store.follow(&rewritten, cx);
                store.snack_undoable("Squashed into one commit", cx)
            },
            cx,
        );
    }

    pub fn uncommit(&mut self, project: &ProjectId, ids: Arc<[CommitId]>, cx: &mut Context<Self>) {
        // A conflicted commit's files are one side's, so uncommitting it would bring back
        // nothing and lose the change it carries.
        let conflicted = (ids.iter())
            .filter(|id| (self.find_commit(project, **id)).is_some_and(|(_, c)| c.conflicted))
            .count();
        match conflicted {
            0 => {}
            1 => return self.snack("Resolve the conflicted commit before uncommitting it", cx),
            _ => {
                return self.snack(
                    "Resolve the conflicted commits before uncommitting them",
                    cx,
                );
            }
        }
        if ids.len() > 1 {
            self.picked.clear();
        }
        let keys: Vec<String> = ids
            .iter()
            .filter_map(|id| self.key_of(project, *id))
            .collect();
        let label = match ids.len() {
            1 => Label::from("Uncommit"),
            n => Label::new("Uncommit", format!("Uncommit {n} commits")),
        };
        let home = (ids.first())
            .and_then(|id| self.checkout_of(&Selection::Commit(project.clone(), *id)))
            .unwrap_or_default();
        let project = project.clone();
        self.apply(
            &project.clone(),
            label,
            Overlay::commits(keys.clone()),
            backend::Uncommit { commits: ids },
            move |store, (), cx| {
                // The selected commit's changes are uncommitted now; show them there.
                if let Some(Selection::Commit(selected, id)) = &store.selection
                    && *selected == project
                    && store
                        .key_of(selected, *id)
                        .is_some_and(|key| keys.contains(&key))
                {
                    store.select(Some(Selection::uncommitted_in(project, home)), cx);
                }
                store.snack_undoable("Uncommitted", cx)
            },
            cx,
        );
    }

    /// Apply `intent` to `project`, then reload it, busy until the reload lands. Until then,
    /// `overlay` hides what it takes away, and another operation on `project` is refused, so
    /// each one acts on what the one before it did, but for those that run beside it
    /// ([`OpKind::beside`]), each refusing only one of its own kind. `done` gets its result.
    pub fn apply<I: backend::Intent>(
        &mut self,
        project: &ProjectId,
        label: impl Into<Label>,
        overlay: Overlay,
        intent: I,
        done: impl FnOnce(&mut Store, I::Done, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let kind = intent.kind();
        if let Some(why) = self.refusal(project, &kind) {
            return self.snack(why, cx);
        }
        let beside = kind.beside();
        let remote_only = kind.remote_only();
        let Label { name, text: label } = label.into();
        let editing = (self.workspace(project)).is_some_and(|ws| ws.editing.is_some());
        let Some(path) = self.path(project) else {
            return;
        };
        // What's selected leaving with its branch or worktree, the selection goes now where
        // the reload after it would put it.
        let selection = self.selection.as_ref();
        let moved = selection
            .filter(|s| s.project() == project && overlay.takes_away(s, self))
            .cloned();
        if moved.is_some() {
            self.select(Some(Selection::Uncommitted(project.clone())), cx);
        }
        // Drawn again when the commit it makes, if any, is due to dim.
        if overlay.new_commit.is_some() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(PENDING_AFTER).await;
                this.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        }
        // Commits shown before it, so that afterwards only its own lose their unread marks; none
        // while editing, nor for one that only talks to the remote or runs beside the others,
        // which make none of their own.
        let before = (!editing && !remote_only && !beside).then(|| {
            let ws = self.workspace(project);
            let keys = ws.map_or_else(HashSet::new, |ws| {
                super::seen::shown(ws)
                    .map(|(_, [key, _])| key.to_owned())
                    .collect()
            });
            (keys, ws.map(|ws| overlay.checkouts(ws)).unwrap_or_default())
        });
        let undo = matches!(kind, OpKind::Undo(_) | OpKind::Redo(_));
        // The reload after it reads just the uncommitted files, or just the commits and
        // branches, if that's all it changed.
        let mut reload = match overlay.rereads {
            // Files taken out of a checkout leave it even when its `HEAD` stays put, as a
            // linked worktree's committed to a branch here do.
            Rereads::Commits if !overlay.files.is_empty() || !overlay.hunks.is_empty() => {
                let mut reload = Reload::commits();
                reload.merge(Reload::files(overlay.files_in.clone()));
                reload
            }
            Rereads::Commits => Reload::commits(),
            Rereads::Files => Reload::files(overlay.files_in.clone()),
            Rereads::CommitsAndMain => {
                let mut reload = Reload::commits();
                reload.merge(Reload::files(Checkout::Main));
                reload
            }
        };
        let (now, id) = (Instant::now(), self.ops_started);
        self.ops_started += 1;
        self.running.push(Running {
            project: project.clone(),
            id,
            label: label.clone(),
            overlay,
            kind,
            started: now,
            phase: Phase::Working,
            deferred: Reload::default(),
        });
        cx.emit(StoreEvent::Pending(project.clone()));
        cx.notify();
        // Replacing the last one's ticker stops it.
        self.busy_ticker = Self::tick_while_busy(cx);
        let project = project.clone();
        let reason = label.to_string();
        cx.spawn(async move |this, cx| {
            // Asked for by the user, so it runs at full speed even if they switch away
            // meanwhile, rather than as macOS runs a hidden app's work.
            let _awake = cx.background_executor().prevent_app_nap(&reason);
            let result = backend::run(move || backend::apply(&path, intent)).await;
            this.update(cx, |store, cx| {
                // What was asked for while it ran too.
                if let Some(running) = store.running.iter_mut().find(|r| r.id == id) {
                    running.phase = Phase::Finished;
                    reload.merge(std::mem::take(&mut running.deferred));
                }
                backend::metrics::operation(
                    name,
                    now.elapsed(),
                    result.as_ref().err(),
                    store.workspace(&project),
                );
                let source = (project.clone(), label.clone());
                let first_card = store.next_toast;
                match result {
                    Ok((value, ran)) => {
                        // A push or fetch writes only what the remote has, which undo leaves,
                        // so what an agent did while it ran still counts.
                        if !remote_only {
                            store.ran.entry(project.clone()).or_default().push(ran.span);
                        }
                        // Set here rather than at the start, so a load the watcher started
                        // mid-operation can't use it up; not after a failure, which made
                        // nothing.
                        if let Some(before) = before {
                            store.rebaseline.insert(project.clone(), before);
                        }
                        // A retry that worked takes back the cards saying it failed or what an
                        // earlier run left undone.
                        store.toasts.retain(|toast| match &toast.notice {
                            Notice::Error {
                                source: Some(s), ..
                            } => *s != source,
                            _ => true,
                        });
                        // One beside the others records nothing, and what changes meanwhile
                        // is theirs.
                        store.last_snapshot = (ran.recorded.filter(|_| !beside))
                            .map(|snapshot| (project.clone(), snapshot));
                        if ran.moved_worktree && !beside {
                            store.keep_last_snapshot(cx);
                        }
                        // Something new done, ⌘Z takes it back first, and what it took back
                        // before can't be redone over it.
                        if let Some((project, snapshot)) =
                            store.last_snapshot.clone().filter(|_| !undo)
                        {
                            store.push_undo_step(project, snapshot, now);
                            store.redo_steps.clear();
                        }
                        // Gone on to another project meanwhile, the user stays there, picks and
                        // all: what this finished selects, as the commit it made, doesn't take
                        // them back, and what it says names its project.
                        store.staying = (store.selection.as_ref())
                            .is_some_and(|selection| *selection.project() != project);
                        done(store, value, cx);
                        store.staying = false;
                    }
                    Err(err) => {
                        // What it would have taken away stays, so the selection goes back.
                        let uncommitted = Selection::Uncommitted(project.clone());
                        if let Some(moved) = moved
                            && store.selection.as_ref() == Some(&uncommitted)
                        {
                            store.select(Some(moved), cx);
                        }
                        let title = store.card_title(&project, format!("{label} failed"));
                        // Paths in the project as it names them, so a long one doesn't push
                        // the reason off the card.
                        let mut detail = backend::failure_detail(&err);
                        if let Some(path) = store.path(&project) {
                            detail = detail.replace(&format!("{}/", path.display()), "");
                        }
                        // Set here with its button, rather than in a terminal.
                        if backend::is_author_missing(&err) {
                            let detail = "Git doesn't know your name and email yet, which \
                                          every commit records.";
                            let asked = project.clone();
                            let ask = move |_: &mut Store, cx: &mut Context<Store>| {
                                cx.emit(StoreEvent::AuthorMissing(asked.clone()))
                            };
                            store.error_with_retry(title, detail, "Set name and email…", ask, cx)
                        } else {
                            store.error(title, detail, cx)
                        }
                    }
                }
                // The cards it wrote are its own, for the next run of it that works to take back.
                for toast in (store.toasts.iter_mut()).filter(|toast| toast.id >= first_card) {
                    if let Notice::Error {
                        source: card @ None,
                        ..
                    } = &mut toast.notice
                    {
                        *card = Some(source.clone());
                    }
                }
                store.load(&project, reload, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Why an operation of `kind` can't start in `project` now, if it can't.
    fn refusal(&self, project: &ProjectId, kind: &OpKind) -> Option<String> {
        let in_project = self.in_project(project);
        let taken = self.running_in(project).any(|r| match kind.beside() {
            true => r.kind == *kind,
            false => !r.kind.beside(),
        });
        if taken {
            return Some(format!(
                "Wait for the running operation{in_project} to finish"
            ));
        }
        let editing = (self.workspace(project)).is_some_and(|ws| ws.editing.is_some());
        if editing && !kind.while_editing() {
            return Some(format!(
                "Finish or cancel editing the commit{in_project} first"
            ));
        }
        // What's shown is what it last read, which an agent's `git merge` onto the workspace
        // can leave it unable to read again: the operation would act on what's gone, and the
        // engine can make a commit it then fails to report.
        let unreadable = (self.project(project)).is_some_and(|p| p.error.is_some())
            && !(kind.remote_only()
                || matches!(kind, OpKind::Undo(_) | OpKind::Redo(_) | OpKind::Restore));
        unreadable
            .then(|| format!("Can't read the project{in_project} now; fix what it says first"))
    }

    /// Redraw while a mutation runs, so the status bar can say how long it's taking.
    fn tick_while_busy(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                // It counts whole seconds.
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let Ok(true) = this.update(cx, |store, cx| {
                    if store.busy().is_some_and(|(_, _, t)| t >= SLOW) {
                        cx.notify();
                    }
                    !store.running.is_empty()
                }) else {
                    break;
                };
            }
        })
    }

    /// The key of `project`'s commit `id`, which survives rewrites.
    pub fn key_of(&self, project: &ProjectId, id: CommitId) -> Option<String> {
        let (_, commit) = self.find_commit(project, id)?;
        Some(commit.key.clone())
    }

    /// The longest-running mutation in flight, its project, and for how long, if any.
    pub fn busy(&self) -> Option<(&ProjectId, &SharedString, Duration)> {
        (self.running.iter())
            .max_by_key(|running| running.started.elapsed())
            .map(|r| (&r.project, &r.label, r.started.elapsed()))
    }

    fn running_in<'a>(&'a self, project: &'a ProjectId) -> impl Iterator<Item = &'a Running> {
        self.running.iter().filter(move |r| r.project == *project)
    }

    /// Whether `project` has an operation running.
    pub(super) fn busy_in(&self, project: &ProjectId) -> bool {
        self.running_in(project).next().is_some()
    }

    /// `project`'s operation under way that reads with its own reload what the watcher sees
    /// meanwhile: not one beside the others, nor one that only talks to the remote, which
    /// write nothing a load would race.
    pub(super) fn loads_wait_on(&mut self, project: &ProjectId) -> Option<&mut Running> {
        self.running.iter_mut().find(|r| {
            r.project == *project
                && r.phase == Phase::Working
                && !r.kind.beside()
                && !r.kind.remote_only()
        })
    }

    /// Hand `project`'s finished operations to the load starting now, which ends them when it
    /// lands; whether there were any.
    pub(super) fn reloads_finished(&mut self, project: &ProjectId) -> bool {
        let mut any = false;
        for r in (self.running.iter_mut()).filter(|r| r.project == *project) {
            if r.phase == Phase::Finished {
                r.phase = Phase::Reloading;
                any = true;
            }
        }
        any
    }

    /// Whether one of `project`'s operations has finished and waits for a load to start.
    pub(super) fn awaits_reload(&self, project: &ProjectId) -> bool {
        self.running_in(project).any(|r| r.phase == Phase::Finished)
    }

    /// End `project`'s operations whose reload has loaded what they did, or can't load at all.
    pub(super) fn settled(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let (ended, running): (Vec<_>, Vec<_>) = std::mem::take(&mut self.running)
            .into_iter()
            .partition(|r| r.project == *project && r.phase == Phase::Reloading);
        self.running = running;
        if ended.is_empty() {
            return;
        }
        // One that only talks to a remote, as a push, writes no files.
        for ended in ended.iter().filter(|r| !r.kind.remote_only()) {
            let now = crate::ui::now();
            // A second early, as whole seconds can put its first writes before its start.
            let started = now - ended.started.elapsed().as_secs() as i64 - 1;
            let spans = self.own_writes.entry(project.clone()).or_default();
            spans.retain(|span| crate::ui::just_now(*span.end()));
            spans.push(started..=now + 2);
        }
        cx.emit(StoreEvent::Pending(project.clone()));
    }

    /// Operations quitting now would cut off.
    pub fn unfinished(&self) -> usize {
        self.working().count()
    }

    /// Operations running besides the longest-running one.
    pub fn others_busy(&self) -> usize {
        self.running.len().saturating_sub(1)
    }

    /// Whether an operation of `kind`, or any with `None`, runs for `project`. One
    /// that has finished and waits for the reload after it counts only with `settling`: a
    /// repeat then would do nothing more, but a retry after it failed may be asked for then.
    pub(super) fn pending(
        &self,
        project: &ProjectId,
        kind: Option<&OpKind>,
        settling: bool,
    ) -> bool {
        (self.running_in(project))
            .filter(|r| settling || r.phase == Phase::Working)
            .any(|r| kind.is_none_or(|kind| r.kind == *kind))
    }

    /// Whether conflicts are being resolved in `project`, whose picks are numbered anew once
    /// that's done.
    pub fn resolving(&self, project: &ProjectId) -> bool {
        self.pending(project, Some(&OpKind::Resolve), false)
    }

    /// Whether an update from the target runs on `project`.
    pub fn updating(&self, project: &ProjectId) -> bool {
        self.pending(project, Some(&OpKind::Update), true)
    }

    pub fn fetching(&self, project: &ProjectId) -> bool {
        self.pending(project, Some(&OpKind::Fetch), true)
    }

    fn overlays<'a>(&'a self, project: &'a ProjectId) -> impl Iterator<Item = &'a Overlay> {
        self.running_in(project).map(|r| &r.overlay)
    }

    pub fn hides_commit(&self, project: &ProjectId, key: &str) -> bool {
        self.overlays(project).any(|o| o.commits.contains(key))
    }

    /// Whether the commit with `key` leaves for where a stand-in shows it, as a moved one does.
    pub fn moves_commit(&self, project: &ProjectId, key: &str) -> bool {
        (self.overlays(project)).any(|o| o.new_commit.is_some() && o.commits.contains(key))
    }

    /// Whether the file at `path` is leaving the uncommitted files of `checkout`.
    pub fn hides_file(&self, project: &ProjectId, checkout: &Checkout, path: &str) -> bool {
        (self.overlays(project)).any(|o| o.files_in == *checkout && o.files.contains(path))
    }

    pub fn hides_branch(&self, project: &ProjectId, ref_name: &FullName) -> bool {
        self.overlays(project)
            .any(|o| o.branches.contains(ref_name))
    }

    pub fn deletes_branch(&self, project: &ProjectId, ref_name: &FullName) -> bool {
        self.overlays(project)
            .any(|o| o.deleted_branches.contains(ref_name))
    }

    /// A checkout's uncommitted `files` without those a pending operation is taking.
    pub fn uncommitted_shown<'a>(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        files: &'a [FileChange],
    ) -> Cow<'a, [FileChange]> {
        // Looked up once rather than for every file: this runs on every frame the row is drawn.
        let taking: Vec<&Overlay> = (self.overlays(project))
            .filter(|o| o.files_in == *checkout && !o.files.is_empty())
            .collect();
        let hidden = |file: &FileChange| taking.iter().any(|o| o.files.contains(&file.path));
        match !taking.is_empty() && files.iter().any(hidden) {
            false => Cow::Borrowed(files),
            true => Cow::Owned(files.iter().filter(|f| !hidden(f)).cloned().collect()),
        }
    }

    /// A stack's `branches` as they'll be once those being deleted have gone: each one's
    /// commits join the named branch above, or the one below when there's none above.
    pub fn after_deletes<'a>(
        &self,
        project: &ProjectId,
        branches: &'a [Branch],
    ) -> Cow<'a, [Branch]> {
        let deleted =
            |b: &Branch| (b.ref_name.as_ref()).is_some_and(|r| self.deletes_branch(project, r));
        if !branches.iter().any(deleted) {
            return Cow::Borrowed(branches);
        }
        let (mut kept, mut carried) = (Vec::<Branch>::new(), Vec::new());
        for branch in branches {
            if !deleted(branch) {
                let mut branch = branch.clone();
                branch.commits.splice(0..0, carried.drain(..));
                kept.push(branch);
            } else if let Some(above) = kept.last_mut().filter(|b| b.ref_name.is_some()) {
                above.commits.extend(branch.commits.iter().cloned());
            } else {
                carried.extend(branch.commits.iter().cloned());
            }
        }
        Cow::Owned(kept)
    }

    /// `review`'s state, merged already while its merge is pending.
    pub fn review_state(&self, project: &ProjectId, review: &Review) -> ReviewState {
        match (self.overlays(project)).any(|o| o.merges == Some(review.number)) {
            true => ReviewState::Merged,
            false => review.state,
        }
    }

    /// Whether `review` merges by itself once it can, as pending mutations leave it.
    pub fn auto_merge_on(&self, project: &ProjectId, review: &Review) -> bool {
        (self.overlays(project))
            .find_map(|o| o.auto_merges.filter(|(number, _)| *number == review.number))
            .map_or(review.auto_merge, |(_, on)| on)
    }

    pub fn hides_worktree(&self, project: &ProjectId, name: &bstr::BStr) -> bool {
        self.overlays(project).any(|o| o.worktrees.contains(name))
    }

    /// The hunks, or lines of them, pending mutations take out of files that otherwise stay,
    /// by path: of the uncommitted files of `checkout`, or with a `commit` key, of that
    /// commit's.
    pub fn parts_leaving<'a>(
        &'a self,
        project: &'a ProjectId,
        checkout: &Checkout,
        commit: Option<&str>,
    ) -> HashMap<&'a str, Vec<HunkKey>> {
        let mut parts: HashMap<&str, Vec<HunkKey>> = HashMap::new();
        for overlay in self.overlays(project) {
            let found = match commit {
                Some(commit) => overlay.commit_hunks.get(commit),
                None => (overlay.files_in == *checkout).then_some(&overlay.hunks),
            };
            for (path, keys) in found.into_iter().flatten() {
                parts.entry(path.as_str()).or_default().extend(keys);
            }
        }
        parts
    }

    /// Whether the file at `path` is leaving the commit with key `commit`.
    pub fn hides_commit_file(&self, project: &ProjectId, commit: &str, path: &str) -> bool {
        self.overlays(project)
            .any(|o| (o.commit_files.get(commit)).is_some_and(|files| files.contains(path)))
    }

    /// The title of the commit a pending operation will make where `at` says, with whether
    /// it's been on its way long enough to show it's still pending, and the key of the commit
    /// it stands in for, when it moves one.
    pub fn new_commit(
        &self,
        project: &ProjectId,
        at: impl Fn(&NewAt) -> bool,
    ) -> Option<(SharedString, bool, Option<String>)> {
        self.running_in(project).find_map(|r| {
            let (place, title) = r.overlay.new_commit.as_ref()?;
            let moved = match r.overlay.commits.iter().collect::<Vec<_>>()[..] {
                [key] => Some(key.clone()),
                _ => None,
            };
            at(place).then(|| (title.clone(), r.started.elapsed() >= PENDING_AFTER, moved))
        })
    }

    /// Where a commit going to `to` shows until it's there.
    pub fn new_at(&self, project: &ProjectId, to: &Destination) -> Option<NewAt> {
        Some(match to {
            Destination::Branch(ref_name) => NewAt::Top(ref_name.clone()),
            Destination::AboveCommit(id) => NewAt::Above(self.key_of(project, *id)?),
            Destination::BelowCommit(id) => NewAt::Below(self.key_of(project, *id)?),
        })
    }

    /// Commits `ids` as they show on their way to `to`: where, and one's title, or its short
    /// id when it's from outside the workspace, or how many there are.
    pub fn arriving(
        &self,
        project: &ProjectId,
        ids: &[CommitId],
        to: &Destination,
    ) -> Option<(NewAt, SharedString)> {
        let title = match (ids, self.find_commit(project, *ids.first()?)) {
            ([_], Some((_, commit))) => self.message_of(project, commit).0.to_owned(),
            ([id], None) => id.short(),
            _ => format!("{} commits", ids.len()),
        };
        Some((self.new_at(project, to)?, title.into()))
    }

    /// `commit`'s title and message, as a pending reword leaves them.
    pub fn message_of<'a>(
        &'a self,
        project: &'a ProjectId,
        commit: &'a Commit,
    ) -> (&'a str, &'a str) {
        let reworded = (self.overlays(project))
            .find_map(|o| o.reworded.as_ref().filter(|(key, _)| *key == commit.key));
        match reworded {
            Some((_, message)) => (message.lines().next().unwrap_or_default().trim(), message),
            None => (&commit.title, &commit.message),
        }
    }

    /// `branch`'s name, as a pending rename leaves it.
    pub fn name_of<'a>(&'a self, project: &'a ProjectId, branch: &'a Branch) -> &'a str {
        let renamed = self.overlays(project).find_map(|o| {
            (o.renamed.as_ref()).filter(|(ref_name, _)| branch.ref_name.as_ref() == Some(ref_name))
        });
        renamed.map_or(&branch.name, |(_, name)| name)
    }

    /// What the navigator calls `branch`: its pull request's title once the forge cache has
    /// one, else its name.
    pub fn label_of<'a>(&'a self, project: &'a ProjectId, branch: &'a Branch) -> &'a str {
        (branch.review.as_ref())
            .map(|review| review.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| self.name_of(project, branch))
    }

    /// Whether a pending operation pushes `ref_name`, on its own or to open its pull request.
    pub fn pushing(&self, project: &ProjectId, ref_name: &FullName) -> bool {
        (self.running_in(project)).any(|r| matches!(&r.kind, OpKind::Push(to) if to == ref_name))
    }

    /// Whether a pending operation opens `ref_name`'s pull request.
    pub fn publishing(&self, project: &ProjectId, ref_name: &FullName) -> bool {
        (self.running_in(project))
            .any(|r| r.overlay.publishes && matches!(&r.kind, OpKind::Push(to) if to == ref_name))
    }

    /// Keep the selection on commits an operation rewrote.
    pub fn follow(&mut self, rewritten: &backend::Rewritten, cx: &mut Context<Self>) {
        if let Some(Selection::Commit(project, id)) = &self.selection
            && let Some(new) = rewritten.get(id)
        {
            let (project, from, to) = (project.clone(), *id, *new);
            self.select_followed(project, from, to, cx);
        }
    }

    /// Select commit `to`, which commit `from`, selected until now, was rewritten into.
    pub(super) fn select_followed(
        &mut self,
        project: ProjectId,
        from: CommitId,
        to: CommitId,
        cx: &mut Context<Self>,
    ) {
        self.select(Some(Selection::Commit(project, to)), cx);
        self.followed = Some((from, to));
    }

    /// Select commit `made`, which an operation just made in `project`.
    pub fn select_made(
        &mut self,
        project: &ProjectId,
        made: backend::Made,
        cx: &mut Context<Self>,
    ) {
        self.select(Some(Selection::Commit(project.clone(), made.id)), cx);
        self.made = Some(made);
    }

    /// Whether the selection moved from commit `from` to `to` as an operation rewrote it,
    /// rather than to another commit: it's the same one to look at.
    pub fn followed(&self, from: CommitId, to: CommitId) -> bool {
        self.followed == Some((from, to))
    }
}

/// What an update of what `scope` picks onto `target` would leave conflicted that isn't now
/// in `workspace`.
fn update_conflicts_detail(
    target: &str,
    found: &UpdateConflicts,
    workspace: Option<&Workspace>,
    scope: UpdateScope,
) -> String {
    let linked = |ref_name: Option<&FullName>| {
        (workspace.and_then(|ws| ws.lane_of(ref_name)))
            .is_some_and(|(lane, _)| lane.checkout.is_linked())
    };
    let mut detail = Vec::new();
    if !found.branches.is_empty() {
        detail.push(format!(
            "Rebasing onto {target} leaves conflicted commits in {}.",
            commit_counts(&found.branches)
        ));
    }
    if !found.files.is_empty() {
        detail.push(format!(
            "{target} changed the same lines as your uncommitted changes to {}, which get \
             conflict markers.",
            found.files.join(", ")
        ));
    }
    let resolve = if found
        .branches
        .iter()
        .any(|(branch, _)| linked(branch.as_ref()))
    {
        "You can resolve them afterwards, a linked worktree's in that worktree"
    } else {
        "You can resolve them afterwards"
    };
    // A linked worktree resting on a stack moves with it, and one on the target's history
    // onto its new commits.
    let moves_worktrees = workspace.is_some_and(|ws| {
        (ws.lanes.iter()).any(|lane| {
            let on_target = lane.checkout.is_linked() && lane.rests_on.is_none();
            let on_stack = || ws.beneath(lane).any(|link| !link.lane.checkout.is_linked());
            (scope.worktrees() && on_target) || (scope.workspace() && on_stack())
        })
    });
    detail.push(if moves_worktrees {
        format!("{resolve}. The update moves linked worktrees too, so it can't be undone.")
    } else {
        format!("{resolve}, or undo the update.")
    });
    detail.join(" ")
}

/// That nothing `scope` picks is behind `target`.
pub fn up_to_date(scope: UpdateScope, target: &str) -> String {
    let what = match scope {
        UpdateScope::All => "Already",
        UpdateScope::Workspace => "The workspace is already",
        UpdateScope::Worktrees => "The worktrees are already",
    };
    format!("{what} up to date with {target}")
}

/// What an update from `target` did: the branches (full refs) it left `conflicted`, and
/// the ones that `landed` and went. Several landed branches are counted, not named, to
/// keep the snackbar to one line.
fn updated_message(
    target: &str,
    conflicted: &[(Option<FullName>, usize)],
    landed: &[FullName],
) -> String {
    let mut said = vec![format!("Updated from {target}")];
    said.extend(conflicted_parts(conflicted));
    match landed {
        [] => {}
        [only] => said.push(format!("{} landed and was removed", short_ref(only))),
        several => said.push(format!(
            "{} branches landed and were removed",
            several.len()
        )),
    }
    // A snackbar is one fragment, so its parts are listed rather than sentences.
    said.join(", ")
}

/// What a branch move did, `done`, and the branches it left `conflicted`: "Branch moved,
/// feature/a has 2 conflicted commits".
pub fn branch_moved(done: &str, conflicted: &[(Option<FullName>, usize)]) -> String {
    let mut said = vec![done.to_owned()];
    said.extend(conflicted_parts(conflicted));
    said.join(", ")
}

/// "feature/a has 2 conflicted commits" for each of `conflicted`, full refs (`None` for an
/// unnamed branch) with how many.
pub fn conflicted_parts(conflicted: &[(Option<FullName>, usize)]) -> impl Iterator<Item = String> {
    conflicted.iter().map(|(branch, count)| {
        format!(
            "{} has {}",
            branch_name(branch.as_ref()),
            crate::ui::plural(*count, "conflicted commit")
        )
    })
}

/// Branches (full refs, `None` for an unnamed one) with how many of their commits:
/// "feature/a (2 commits), feature/b (1 commit)".
fn commit_counts(branches: &[(Option<FullName>, usize)]) -> String {
    let counts: Vec<String> = (branches.iter())
        .map(|(ref_name, count)| {
            let name = branch_name(ref_name.as_ref());
            format!("{name} ({})", crate::ui::plural(*count, "commit"))
        })
        .collect();
    counts.join(", ")
}

/// `ref_name`'s short name, or "an unnamed branch" when it's empty.
fn branch_name(ref_name: Option<&FullName>) -> Cow<'_, str> {
    ref_name.map_or("an unnamed branch".into(), short_ref)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::model::build::{branch, full, workspace, worktree};

    #[test]
    fn an_operation_counts_the_worktrees_its_rewrite_moves_as_its_own() {
        let ws = workspace(
            vec![vec![
                branch("top", &[1]),
                branch("middle", &[2, 3]),
                branch("bottom", &[4]),
            ]],
            vec![
                worktree("wt", Some(3), vec![branch("wt", &[5])]),
                worktree("wt-stacked", Some(5), vec![branch("wt-stacked", &[6])]),
            ],
        );
        let named = |checkouts: Checkouts| {
            let mut names: Vec<_> = (checkouts.into_iter())
                .map(|checkout| {
                    checkout
                        .linked()
                        .map_or("Main".to_owned(), |name| name.to_string())
                })
                .collect();
            names.sort();
            names.join(", ")
        };
        let top_of = |ref_name: &str| Overlay {
            new_commit: Some((NewAt::Top(full(ref_name)), SharedString::default())),
            ..Overlay::default()
        };
        let amending_in = |worktree: &str, key: &str| Overlay {
            files_in: Checkout::Linked(worktree.into()),
            ..Overlay::commits([key.to_owned()])
        };
        let checkouts = [
            ("rewriting top", Overlay::commits(["key1".to_owned()])),
            ("rewriting middle~1", Overlay::commits(["key3".to_owned()])),
            ("rewriting wt", Overlay::commits(["key5".to_owned()])),
            ("amending wt in it", amending_in("wt", "key5")),
            ("committing on middle", top_of("refs/heads/middle")),
            ("committing on bottom", top_of("refs/heads/bottom")),
        ]
        .map(|(what, overlay)| format!("{what}: {}\n", named(overlay.checkouts(&ws))))
        .concat();
        assert_eq!(
            checkouts,
            "rewriting top: Main\n\
             rewriting middle~1: Main, wt, wt-stacked\n\
             rewriting wt: Main, wt, wt-stacked\n\
             amending wt in it: wt, wt-stacked\n\
             committing on middle: Main\n\
             committing on bottom: Main, wt, wt-stacked\n",
            "wt rests on middle~1, below middle's tip and above bottom, and wt-stacked on wt"
        );
    }

    #[test]
    fn an_update_says_what_conflicted_and_what_landed() {
        assert_eq!(
            updated_message(
                "origin/main",
                &[(Some(full("refs/heads/feature-a")), 2)],
                &[full("refs/heads/feature-b")]
            ),
            "Updated from origin/main, feature-a has 2 conflicted commits, \
             feature-b landed and was removed",
            "each branch by its short name"
        );
        assert_eq!(
            updated_message("origin/main", &[], &[]),
            "Updated from origin/main",
            "nothing else happened"
        );
        assert_eq!(
            updated_message(
                "origin/main",
                &[],
                &[
                    full("refs/heads/a"),
                    full("refs/heads/b"),
                    full("refs/heads/c")
                ]
            ),
            "Updated from origin/main, 3 branches landed and were removed",
            "several are counted so the snackbar stays one line"
        );
    }

    #[test]
    fn an_update_card_says_what_a_linked_worktree_keeps_from_it() {
        let found = |names: &[&str]| UpdateConflicts {
            branches: (names.iter())
                .map(|name| (Some(full(&format!("refs/heads/{name}"))), 1))
                .collect(),
            files: Vec::new(),
        };
        let alone = workspace(vec![vec![branch("middle", &[2])]], vec![]);
        assert_eq!(
            update_conflicts_detail("main", &found(&["middle"]), Some(&alone), UpdateScope::All),
            "Rebasing onto main leaves conflicted commits in middle (1 commit). \
             You can resolve them afterwards, or undo the update.",
            "nothing rests on the stack"
        );
        let rested_on = workspace(
            vec![vec![branch("middle", &[2])]],
            vec![worktree("wt", Some(2), vec![branch("wt", &[5])])],
        );
        assert_eq!(
            update_conflicts_detail(
                "main",
                &found(&["middle", "wt"]),
                Some(&rested_on),
                UpdateScope::All
            ),
            "Rebasing onto main leaves conflicted commits in middle (1 commit), wt (1 commit). \
             You can resolve them afterwards, a linked worktree's in that worktree. The update \
             moves linked worktrees too, so it can't be undone.",
            "wt moves with middle, and its conflicts resolve in it"
        );
        let on_a_worktree = workspace(
            vec![vec![branch("middle", &[2])]],
            vec![
                worktree("a", None, vec![branch("a", &[5])]),
                worktree("b", Some(5), vec![branch("b", &[6])]),
            ],
        );
        assert_eq!(
            update_conflicts_detail(
                "main",
                &found(&["middle"]),
                Some(&on_a_worktree),
                UpdateScope::All
            ),
            "Rebasing onto main leaves conflicted commits in middle (1 commit). \
             You can resolve them afterwards. The update moves linked worktrees too, so it \
             can't be undone.",
            "a rests on the target, which the update moves it onto, b with it"
        );
        assert_eq!(
            update_conflicts_detail(
                "main",
                &found(&["middle"]),
                Some(&on_a_worktree),
                UpdateScope::Workspace
            ),
            "Rebasing onto main leaves conflicted commits in middle (1 commit). \
             You can resolve them afterwards, or undo the update.",
            "updating only the workspace leaves a, on the target, where it is"
        );
        assert_eq!(
            update_conflicts_detail(
                "main",
                &found(&["a"]),
                Some(&rested_on),
                UpdateScope::Worktrees
            ),
            "Rebasing onto main leaves conflicted commits in a (1 commit). \
             You can resolve them afterwards, or undo the update.",
            "updating only the worktrees leaves wt, on middle, where it is"
        );
    }
}
