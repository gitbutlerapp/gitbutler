//! Operations that change a project. A project's operations run one at a time, and the ones
//! asked for meanwhile wait in a queue, their overlays hiding what they take away.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail};
use gpui::{Context, SharedString, Task};

use super::{Notice, ProjectId, Reload, Selection, Store, StoreEvent};
use crate::backend::history::Undone;
use crate::backend::upstream::{UpdateConflicts, UpdateOutcome};
use crate::backend::{
    self, Branch, Checkout, Commit, CommitId, Destination, FileChange, FullName, HunkKey, Review,
    ReviewState, Selected, SnapshotId, Workspace, short_ref,
};

/// How long a mutation runs before the status bar says how long it's taking.
pub const SLOW: Duration = Duration::from_millis(800);

/// How long a commit a mutation makes shows as made before it dims, to say the mutation is
/// still running: one done sooner never looks pending.
const PENDING_AFTER: Duration = Duration::from_millis(400);

/// What a running or queued operation takes away, hidden until the reload after it shows
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
    /// The branch (full ref) it pushes, with the ones below it.
    pub pushes: Option<FullName>,
    /// It opens a pull request for the branch it pushes.
    pub publishes: bool,
    /// It only talks to the remote or the forge and moves refs, leaving the worktree and
    /// the commits alone, so reloads the watcher starts meanwhile go ahead.
    pub remote_only: bool,
    /// Which operation it is, where the queue treats it apart from the rest.
    pub kind: OpKind,
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

/// The operations the queue treats apart from the rest. Labels are only what the user reads.
#[derive(Default, Clone, PartialEq)]
pub enum OpKind {
    #[default]
    Other,
    Fetch,
    /// An update from the target.
    Update,
    /// A pull into the branch with this full ref.
    Pull(FullName),
    /// Resolving conflicts, which are numbered anew once it's done.
    Resolve,
    MarkResolved,
    FinishEditing,
    CancelEditing,
    /// ⌘Z and ⌘⇧Z, of the step with this oplog entry, which keep the undo history themselves.
    Undo(SnapshotId),
    Redo(SnapshotId),
    /// Operations history's restore, the way back when a project can't be read.
    Restore,
}

impl OpKind {
    /// Whether it runs while a commit is checked out for editing: fetching, which leaves the
    /// worktree alone, and what resolves the commit and ends the edit.
    fn while_editing(&self) -> bool {
        matches!(
            self,
            Self::Fetch
                | Self::Resolve
                | Self::MarkResolved
                | Self::FinishEditing
                | Self::CancelEditing
        )
    }

    /// Whether a second, asked for while the first waits or runs, as by a double-click, would
    /// only fail as there's no edit left.
    fn once(&self) -> bool {
        matches!(self, Self::FinishEditing | Self::CancelEditing)
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

    pub fn push(ref_name: &FullName, publishes: bool) -> Self {
        Self {
            pushes: Some(ref_name.to_owned()),
            publishes,
            remote_only: true,
            ..Self::default()
        }
    }

    /// For an operation that only talks to the remote or the forge.
    pub fn remote() -> Self {
        Self {
            remote_only: true,
            ..Self::default()
        }
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

/// The commits an operation captured, found again by key when it runs, as operations queued
/// before it may have rewritten them, and the branches renamed meanwhile, by full name.
pub struct Remap {
    commits: HashMap<CommitId, CommitId>,
    branches: HashMap<FullName, FullName>,
}

impl Remap {
    fn new<'a>(
        captured: &HashMap<CommitId, String>,
        now: impl IntoIterator<Item = (&'a str, CommitId)>,
    ) -> Self {
        // A key two commits share, as a linked worktree's copy of a commit can, doesn't say
        // which one a rewritten commit became.
        let mut by_key: HashMap<&str, Option<CommitId>> = HashMap::new();
        let mut ids = HashSet::new();
        for (key, id) in now {
            ids.insert(id);
            (by_key.entry(key))
                .and_modify(|found| *found = None)
                .or_insert(Some(id));
        }
        Self {
            commits: (captured.iter())
                .filter_map(|(id, key)| match ids.contains(id) {
                    // Still there: nothing rewrote it.
                    true => Some((*id, *id)),
                    false => Some((*id, (*by_key.get(key.as_str())?)?)),
                })
                .collect(),
            branches: HashMap::new(),
        }
    }

    /// With the branches of `captured`, each with the key of its top commit (empty for one
    /// with none), that are gone from `now` followed to the one whose top that commit is now,
    /// as a rename queued before leaves them. Not to a branch `captured` has, as the one below
    /// that took a deleted branch's commits, nor to no name: those aren't renames.
    fn following(mut self, captured: &[(FullName, String)], now: &Workspace) -> Self {
        for (name, top) in captured {
            if top.is_empty() || now.find_branch(Some(name)).is_some() {
                continue;
            }
            let top_now = now.all_branches().find_map(|b| {
                let ref_name = b.ref_name.as_ref()?;
                (b.commits.first().is_some_and(|c| c.key == *top)
                    && !captured.iter().any(|(was, _)| was == ref_name))
                .then_some(ref_name)
            });
            if let Some(ref_name) = top_now {
                self.branches.insert(name.clone(), ref_name.clone());
            }
        }
        self
    }

    pub fn get(&self, id: CommitId) -> Result<CommitId> {
        self.commits
            .get(&id)
            .copied()
            .context("A commit changed before this could run. Try again.")
    }

    /// `id`, as [`Self::get`] has it, for taking `files` out of: some of a file goes by line
    /// numbers that rewriting the commit may have moved, so a rewritten one is refused.
    pub fn taken_from(&self, id: CommitId, files: &[Selected]) -> Result<CommitId> {
        let now = self.get(id)?;
        if now != id && files.iter().any(|f| f.hunks.is_some()) {
            bail!("A commit changed before this could run. Try again.");
        }
        Ok(now)
    }

    /// `id` itself, refusing once it was rewritten: for what undo can't take back, which
    /// mustn't reach what an agent made of the commit since it was asked about.
    pub fn exactly(&self, id: CommitId) -> Result<CommitId> {
        if self.get(id)? != id {
            bail!("The commit changed since you asked, so it was left as it is.");
        }
        Ok(id)
    }

    /// Each of `ids`, as [`Self::get`] has it.
    pub fn all(&self, ids: &[CommitId]) -> Result<Vec<CommitId>> {
        ids.iter().map(|id| self.get(*id)).collect()
    }

    /// `to`, with the commit it's relative to as it is after the operations queued before.
    pub fn destination(&self, to: Destination) -> Result<Destination> {
        Ok(match to {
            Destination::AboveCommit(id) => Destination::AboveCommit(self.get(id)?),
            Destination::BelowCommit(id) => Destination::BelowCommit(self.get(id)?),
            Destination::Branch(name) => {
                Destination::Branch(self.branches.get(&name).cloned().unwrap_or(name))
            }
        })
    }
}

/// An operation waiting for the ones before it.
pub(super) struct Queued {
    project: ProjectId,
    label: SharedString,
    overlay: Overlay,
    /// The project's commits when it was asked for, and its branches with their top commits'
    /// keys (empty for one with none), for its [`Remap`].
    captured: HashMap<CommitId, String>,
    tops: Vec<(FullName, String)>,
    /// The selection it moved off what it takes away, back if it fails.
    moved: Option<Selection>,
    /// When it was asked for, which its stand-ins count from.
    asked: Instant,
    start: Start,
}

/// Starts a queued operation, with its commits as they are by then.
type Start = Box<dyn FnOnce(&mut Store, Remap, &mut Context<Store>)>;

/// A project's operation running now, from when it starts until the reload after it lands.
pub(super) struct Running {
    label: SharedString,
    pub(super) overlay: Overlay,
    started: Instant,
    asked: Instant,
    /// It finished and waits for its reload. The next load to start ends it when it lands.
    pub(super) settling: bool,
    /// Reloads asked for while it ran, which the reload after it reads too.
    pub(super) deferred: Reload,
    moved: Option<Selection>,
}

/// An operation of the user's that ⌘Z takes back: the oplog entry it recorded in its project,
/// when it was asked for, which orders it among the others, when it finished, in
/// milliseconds since the Unix epoch as file times are, and what its snackbar said.
pub(super) struct UndoStep {
    pub(super) project: ProjectId,
    pub(super) snapshot: SnapshotId,
    asked: Instant,
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

/// Now, in milliseconds since the Unix epoch, as file times are.
fn now_millis() -> i64 {
    (SystemTime::now().duration_since(UNIX_EPOCH)).map_or(0, |t| t.as_millis() as i64)
}

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
        // Asked for after it in any project, as ⌘Z follows the user there, and still running.
        if self.running_after(last.map(|step| step.asked), None) {
            return self.snack(WAIT_TO_UNDO, cx);
        }
        match last.map(|step| (step.project.clone(), step.snapshot)) {
            Some((project, snapshot)) => self.undo_snapshot(&project, snapshot, None, None, cx),
            None => self.snack("Nothing to undo", cx),
        }
    }

    /// Whether an operation asked for after `asked`, or any at all without it, is waiting or
    /// running in `project`, or in any without it, a push or a redo too: it's what the user
    /// means to undo, and an undo asked for now would run after it.
    fn running_after(&self, asked: Option<Instant>, project: Option<&ProjectId>) -> bool {
        self.unfinished_asked().any(|(p, o, at)| {
            project.is_none_or(|project| p == project)
                && (!o.remote_only || o.pushes.is_some())
                && !matches!(o.kind, OpKind::Undo(_))
                && asked.is_none_or(|asked| at > asked)
        })
    }

    /// Every operation waiting or running, but for one waiting only for its reload, with its
    /// project and when it was asked for.
    fn unfinished_asked(&self) -> impl Iterator<Item = (&ProjectId, &Overlay, Instant)> {
        let running = self.running.iter().filter(|(_, r)| !r.settling);
        (running.map(|(p, r)| (p, &r.overlay, r.asked)))
            .chain(self.queue.iter().map(|q| (&q.project, &q.overlay, q.asked)))
    }

    /// Make the operation that recorded `snapshot` in `project` one ⌘Z takes back, among the
    /// others by when it was asked for.
    fn push_undo_step(&mut self, project: ProjectId, snapshot: SnapshotId, asked: Instant) {
        let done = now_millis();
        (self.undo_steps).push(UndoStep {
            project,
            snapshot,
            asked,
            done,
            said: None,
        });
        self.undo_steps.sort_by_key(|step| step.asked);
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

    /// Make `snapshot`, which the operation that just ran recorded on its way, a ⌘Z step of its
    /// own taken back after that operation's, as the branch a commit was made on.
    pub fn push_earlier_step(&mut self, snapshot: SnapshotId) {
        let Some((project, last)) = self.last_snapshot.clone() else {
            return;
        };
        let Some((at, &UndoStep { asked, done, .. })) =
            (self.undo_steps.iter().enumerate()).find(|(_, step)| step.snapshot == last)
        else {
            return;
        };
        let step = UndoStep {
            project,
            snapshot,
            asked,
            done,
            said: None,
        };
        self.undo_steps.insert(at, step);
    }

    /// Redo what ⌘Z last took back, unless the user has done something since.
    pub fn redo_last(&mut self, cx: &mut Context<Self>) {
        if (self.unfinished_asked()).any(|(_, o, _)| matches!(o.kind, OpKind::Undo(_))) {
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
        let (kept, ran, of, again, asked) = (
            self.kept_snapshots.clone(),
            self.ran.clone(),
            project.clone(),
            project.clone(),
            Instant::now(),
        );
        self.run(
            &project,
            "Redo",
            Overlay {
                kind: OpKind::Redo(undo),
                ..Overlay::rewriting_main()
            },
            move |path, _| {
                let kept = kept.lock().get(&of).cloned();
                let ran = ran.lock().get(&of).cloned().unwrap_or_default();
                backend::history::redo_snapshot(
                    path,
                    undo,
                    &kept.unwrap_or_default(),
                    &ran,
                    accepted,
                )
            },
            move |store, redone, cx| {
                let operation = match redone {
                    Some(Undone::Done(operation)) => operation,
                    Some(Undone::Newer(files)) => {
                        let (what, asked) = (files_named(&files), Some(now_millis()));
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
                    store.push_undo_step(project, snapshot, asked);
                }
                store.snack_undoable(format!("Redone: {operation}"), cx)
            },
            cx,
        );
    }

    /// Put `project` back to before the operation `snapshot` was taken for, unless that takes
    /// back one kept from undo.
    pub fn restore(&mut self, project: &ProjectId, snapshot: SnapshotId, cx: &mut Context<Self>) {
        let (kept, of) = (self.kept_snapshots.clone(), project.clone());
        self.run(
            project,
            "Restore",
            Overlay {
                kind: OpKind::Restore,
                ..Overlay::rewriting_main()
            },
            move |path, _| {
                let kept = kept.lock().get(&of).cloned();
                backend::history::restore(path, snapshot, &kept.unwrap_or_default())
            },
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
        let asked = step.map(|step| step.asked);
        if self.running_after(asked, Some(project)) {
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
            let asked_now = Some(now_millis());
            return self.error_with_retry(
                self.card_title(project, "Undo may take back what was pushed".into()),
                "A push came after the operation it undoes. If the push took what the \
                 operation did, the remote keeps that until the branch is force pushed.",
                "Undo anyway",
                move |store, cx| store.undo_snapshot(&again, snapshot, accepted, asked_now, cx),
                cx,
            );
        }
        let (ran, of) = (self.ran.clone(), project.clone());
        // The snackbar offering to undo it goes with it.
        let offered = (project.clone(), snapshot);
        self.toasts.retain(|toast| {
            !matches!(&toast.notice, Notice::Snackbar { undo: Some(undo), .. } if *undo == offered)
        });
        self.run(
            project,
            "Undo",
            Overlay {
                kind: OpKind::Undo(snapshot),
                ..Overlay::rewriting_main()
            },
            move |path, _| {
                let ran = ran.lock().get(&of).cloned().unwrap_or_default();
                let newer_than = newer_than.map(|at| (at, ran.as_slice()));
                Ok(backend::history::undo_snapshot(path, snapshot, newer_than))
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
                    // Behind a newer step of the user's own, as a quick second ⌘Z asks for
                    // this one while the first still waits to run: that one goes first, and
                    // this one stays.
                    // The list is oldest first, a new branch's step just before its commit's.
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
                            step.project != again || asked.is_none_or(|asked| step.asked > asked)
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
                        let (what, asked) = (files_named(&files), Some(now_millis()));
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
        self.run(
            project,
            "Finish editing",
            Overlay {
                kind: OpKind::FinishEditing,
                ..Overlay::rewriting_main()
            },
            |path, _| backend::edit::finish(path),
            |store, (), cx| store.snack("Finished editing", cx),
            cx,
        );
    }

    /// Go back to the workspace leaving the edited commit as it was; unless `force`, asking
    /// first when that drops what changed.
    pub fn cancel_editing(&mut self, project: &ProjectId, force: bool, cx: &mut Context<Self>) {
        let again = project.clone();
        self.run(
            project,
            "Cancel editing",
            Overlay {
                kind: OpKind::CancelEditing,
                ..Overlay::rewriting_main()
            },
            move |path, _| backend::edit::cancel(path, force),
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

    /// Fetch `project`'s remotes, unless a fetch already waits or runs.
    pub fn fetch(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        if self.fetching(project) {
            return;
        }
        let fetched = project.clone();
        self.run(
            project,
            "Fetch",
            Overlay {
                kind: OpKind::Fetch,
                ..Overlay::remote()
            },
            // Failing, it's noted as a background fetch's failure is, for the project page.
            |path, _| Ok(backend::fetch(path)),
            move |store, fetched_or_not, cx| match fetched_or_not {
                Ok(any_remote) => {
                    store.fetched_now(&fetched, any_remote);
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

    /// Rebase the project's branches onto its target's latest commits. It stops short of
    /// leaving anything conflicted that isn't now, with a card that offers to update anyway,
    /// unless that's what was `accepted` there. Unless `accepted`, it does nothing while
    /// another update waits or runs; the card shows before its update has finished, so an
    /// accepted one queues regardless.
    pub fn update_from_target(
        &mut self,
        project: &ProjectId,
        accepted: Option<backend::upstream::UpdateConflicts>,
        cx: &mut Context<Self>,
    ) {
        if accepted.is_none() && self.updating(project) {
            return;
        }
        let updated = project.clone();
        self.run(
            project,
            "Update",
            Overlay {
                kind: OpKind::Update,
                ..Overlay::default()
            },
            move |path, _| backend::update_from_target(path, accepted.as_ref()),
            move |store, outcome, cx| {
                let target = (store.workspace(&updated).and_then(|ws| ws.target.as_ref()))
                    .map_or_else(|| "the target".to_owned(), |target| target.name.clone());
                match outcome {
                    UpdateOutcome::WouldConflict(found) => {
                        let again = updated.clone();
                        let title = store.card_title(&updated, "Updating leaves conflicts".into());
                        let detail =
                            update_conflicts_detail(&target, &found, store.workspace(&updated));
                        store.error_with_retry(
                            title,
                            detail,
                            "Update anyway",
                            move |store, cx| {
                                store.update_from_target(&again, Some(found.clone()), cx)
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
        // A second pull of the branch while one waits would find nothing left to bring in.
        // An accepted one comes from the card that shows once its pull has finished.
        let kind = OpKind::Pull(ref_name.clone());
        if accepted.is_none() && self.pending(project, Some(&kind), true) {
            return;
        }
        let pulled = project.clone();
        self.run(
            project,
            Label::new("Pull", format!("Pull {name}")),
            Overlay {
                kind,
                ..Overlay::default()
            },
            {
                let ref_name = ref_name.clone();
                move |path, _| {
                    let found = backend::pull_conflicts(path, &ref_name)?;
                    if !found.is_empty() && accepted.as_ref() != Some(&found) {
                        return Ok(Some(found));
                    }
                    backend::pull_remote(path, &ref_name, false).map(|()| None)
                }
            },
            move |store, found: Option<Vec<(Option<FullName>, usize)>>, cx| {
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
        self.pushed_at.insert(project.clone(), now_millis());
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
        let below = self.workspace(project).map_or(0, |ws| {
            (ws.pushed_with(&ref_name).skip(1))
                .filter(|b| b.ref_name.is_some() && b.can_push())
                .count()
        });
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
        self.run(
            project,
            Label::new("Push", format!("Push {name}")),
            Overlay::push(&ref_name, false),
            {
                let ref_name = ref_name.clone();
                move |path, _| match backend::push(
                    path,
                    &ref_name,
                    protect,
                    drops.as_deref(),
                    run_hooks,
                ) {
                    Ok((_, sync_failed)) => Ok(Ok(sync_failed)),
                    Err(err) => backend::push_refusal(&err).map(Err).ok_or(err),
                }
            },
            move |store, outcome: Result<Option<String>, String>, cx| {
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
    /// have anything to push. A failure's card tries again with the same title and description.
    pub fn publish(
        &mut self,
        project: &ProjectId,
        ref_name: FullName,
        title: String,
        body: String,
        draft: bool,
        cx: &mut Context<Self>,
    ) {
        let again = {
            let (project, ref_name) = (project.clone(), ref_name.clone());
            let (title, body) = (title.clone(), body.clone());
            move |store: &mut Store, cx: &mut Context<Store>| {
                let (title, body) = (title.clone(), body.clone());
                store.publish(&project, ref_name.clone(), title, body, draft, cx)
            }
        };
        // The queue turns an operation down with only a snackbar while a commit is being
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
        self.run(
            project,
            "Create pull request",
            Overlay::push(&ref_name, true),
            {
                let ref_name = ref_name.clone();
                move |path, _| {
                    let made =
                        backend::reviews::publish(path, &ref_name, title, body, draft, run_hooks);
                    Ok(
                        made.map_err(|err| match backend::push_refusal(&err).as_deref() {
                            None => backend::failure_detail(&err),
                            Some("") => "Someone pushed to the branch since the last fetch, and \
                                     pushing would drop their commits. Fetch to see them."
                                .to_owned(),
                            Some(said) => format!("The remote refused the push: {said}"),
                        }),
                    )
                }
            },
            move |store, made, cx| {
                // The push can move the stack's other pull requests too.
                store.refresh_forge(&published);
                match made {
                    Ok(made) => {
                        store.pushed(&published);
                        let said = format!("Created pull request #{}", made.number);
                        store.snack(store.card_title(&published, said), cx);
                        if let Some(message) = made.sync_failed {
                            let said = "The stack's other pull requests weren't updated";
                            store.error(said, message, cx);
                        }
                    }
                    Err(failed) => {
                        // The branches may have gone up before the forge said no.
                        store.pushed(&published);
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
        self.run(
            project,
            label,
            overlay,
            move |path, _| backend::worktrees::set_archived(path, name.as_ref(), archived),
            move |store, (), cx| store.snack(done, cx),
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
        self.run(
            project,
            "Delete worktree",
            overlay,
            move |path, _| backend::worktrees::remove(path, name.as_ref(), &lost),
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
        self.run(
            project,
            "Move to a new branch",
            Overlay::commits(keys.collect::<Vec<_>>()),
            move |path, remap| backend::move_to_new_branch(path, &remap.all(&ids)?),
            move |store, (_, branch), cx| {
                store.select(Some(Selection::Branch(shown, Some(branch))), cx);
                store.snack_undoable("Moved to a new branch", cx)
            },
            cx,
        );
    }

    /// Take `branch` out of the stack it shares into a stack of its own.
    pub fn unstack(&mut self, project: &ProjectId, branch: FullName, cx: &mut Context<Self>) {
        self.run(
            project,
            "Unstack",
            Overlay::default(),
            move |path, _| backend::tear_off_branch(path, &branch),
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
        let run_hooks = self.runs_hooks(project);
        let undoable = self.undoable(project, [worktree.clone()]);
        let overlay = Overlay {
            files_in: worktree.clone(),
            ..Overlay::files(files.iter())
        };
        // Which commit, as it's often not on screen.
        let amended = (self.find_commit(project, into)).map_or("Amended".into(), |(_, commit)| {
            format!("Amended \"{}\"", crate::ui::commit_title(&commit.title))
        });
        self.run(
            project,
            Label::new("Amend", format!("Amend {}", into.short())),
            overlay,
            move |path, remap| {
                let into = remap.get(into)?;
                backend::amend(path, into, &files, worktree.linked(), run_hooks)
            },
            move |store, (rewritten, left_out, hook_failed), cx| {
                store.follow(&rewritten, cx);
                if !undoable {
                    store.keep_last_snapshot(cx);
                }
                store.report_left_out(&amended, &left_out, cx);
                if let Some(output) = hook_failed {
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
        ids: Vec<CommitId>,
        into: CommitId,
        cx: &mut Context<Self>,
    ) {
        let overlay = Overlay::commits(ids.iter().filter_map(|id| self.key_of(project, *id)));
        self.run(
            project,
            Label::new("Squash", format!("Squash {} commits", ids.len() + 1)),
            overlay,
            move |path, remap| {
                let oldest_first: Vec<CommitId> = ids.iter().rev().copied().collect();
                backend::squash(path, &remap.all(&oldest_first)?, remap.get(into)?)
            },
            move |store, rewritten, cx| {
                store.follow(&rewritten, cx);
                store.snack_undoable("Squashed into one commit", cx)
            },
            cx,
        );
    }

    pub fn uncommit(&mut self, project: &ProjectId, ids: Vec<CommitId>, cx: &mut Context<Self>) {
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
        self.run(
            &project.clone(),
            label,
            Overlay::commits(keys.clone()),
            move |path, remap| backend::uncommit(path, &remap.all(&ids)?),
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

    /// Run `op` on `project` once the mutations before it have finished, then reload it,
    /// busy until the reload lands. Until then, `overlay` hides what it takes away. `op`
    /// gets the commits it captured as they are by the time it runs; `done` gets its result.
    pub fn run<T: Send + 'static>(
        &mut self,
        project: &ProjectId,
        label: impl Into<Label>,
        overlay: Overlay,
        op: impl FnOnce(&Path, &Remap) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        if overlay.kind.once() && self.pending(project, Some(&overlay.kind), false) {
            return;
        }
        let Label { name, text: label } = label.into();
        let mut captured = self.workspace(project).map_or_else(HashMap::new, |ws| {
            ws.commits()
                .map(|commit| (commit.id, commit.key.clone()))
                .collect()
        });
        // The selected commit as it was before an agent rewrote it, which a prompt opened on
        // it still names.
        if let Some((from, to)) = self.followed
            && let Some(key) = captured.get(&to).cloned()
        {
            captured.insert(from, key);
        }
        let tops = (self
            .workspace(project)
            .into_iter()
            .flat_map(Workspace::all_branches))
        .filter_map(|b| {
            let top = b
                .commits
                .first()
                .map_or_else(String::new, |c| c.key.clone());
            Some((b.ref_name.clone()?, top))
        })
        .collect();
        // What's selected leaving with its branch or worktree, the selection goes now where
        // the reload after it would put it.
        let selection = self.selection.as_ref();
        let moved = selection
            .filter(|s| s.project() == project && overlay.takes_away(s, self))
            .cloned();
        if moved.is_some() {
            self.select(Some(Selection::Uncommitted(project.clone())), cx);
        }
        let id = project.clone();
        let start_label = Label::new(name, label.clone());
        // Drawn again when the commit it makes, if any, is due to dim.
        if overlay.new_commit.is_some() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(PENDING_AFTER).await;
                this.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        }
        self.queue.push_back(Queued {
            project: project.clone(),
            label,
            overlay,
            captured,
            tops,
            moved,
            asked: Instant::now(),
            start: Box::new(move |store, remap, cx| {
                store.start(&id, start_label, op, done, remap, cx)
            }),
        });
        cx.emit(StoreEvent::Pending(project.clone()));
        cx.notify();
        self.start_next(cx);
    }

    /// Start the first waiting mutation of every project that has none running.
    fn start_next(&mut self, cx: &mut Context<Self>) {
        let mut started = false;
        while let Some(ix) =
            (self.queue.iter()).position(|q| !self.running.contains_key(&q.project))
        {
            let Some(next) = self.queue.remove(ix) else {
                break;
            };
            let ws = self.workspace(&next.project);
            let now = ws.into_iter().flat_map(|ws| ws.commits());
            let remap = Remap::new(
                &next.captured,
                now.map(|commit| (commit.key.as_str(), commit.id)),
            );
            let remap = match ws {
                Some(ws) => remap.following(&next.tops, ws),
                None => remap,
            };
            self.running.insert(
                next.project,
                Running {
                    label: next.label,
                    overlay: next.overlay,
                    started: Instant::now(),
                    asked: next.asked,
                    settling: false,
                    deferred: Reload::default(),
                    moved: next.moved,
                },
            );
            (next.start)(self, remap, cx);
            started = true;
        }
        if started {
            // Replacing the last one's ticker stops it.
            self.busy_ticker = Self::tick_while_busy(cx);
        }
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

    fn start<T: Send + 'static>(
        &mut self,
        project: &ProjectId,
        label: Label,
        op: impl FnOnce(&Path, &Remap) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
        remap: Remap,
        cx: &mut Context<Self>,
    ) {
        let Label { name, text: label } = label;
        let editing = self
            .workspace(project)
            .is_some_and(|ws| ws.editing.is_some());
        let refused =
            editing && !(self.running.get(project)).is_some_and(|r| r.overlay.kind.while_editing());
        // What's shown is what it last read, which an agent's `git merge` onto the workspace
        // can leave it unable to read again: the operation would act on what's gone, and the
        // engine can make a commit it then fails to report.
        let unreadable = (self.project(project)).is_some_and(|p| p.error.is_some())
            && !(self.running.get(project)).is_some_and(|r| {
                r.overlay.remote_only
                    || matches!(
                        r.overlay.kind,
                        OpKind::Undo(_) | OpKind::Redo(_) | OpKind::Restore
                    )
            });
        let path = self.path(project).filter(|_| !refused && !unreadable);
        let Some(path) = path else {
            if refused {
                let why = format!(
                    "Finish or cancel editing the commit{} first",
                    self.in_project(project)
                );
                self.snack(why, cx);
            } else if unreadable {
                let why = format!(
                    "Can't read the project{} now; fix what it says first",
                    self.in_project(project)
                );
                self.snack(why, cx);
            }
            // `start_next` goes on to this project's next operation.
            self.running.remove(project);
            cx.emit(StoreEvent::Pending(project.clone()));
            return;
        };
        // While editing there are no commits to compare with afterwards; every commit would
        // count as the operation's own and lose its unread mark, as an agent's commit that
        // lands during a push would: one that only talks to the remote makes none.
        let running = self.running.get(project);
        let remote_only = running.is_some_and(|r| r.overlay.remote_only);
        // Those only a remote has too, so pulling them keeps their read marks.
        let before = (!editing && !remote_only).then(|| {
            let ws = self.workspace(project);
            let keys = ws.map_or_else(HashSet::new, |ws| {
                super::seen::shown(ws)
                    .map(|(_, [key, _])| key.to_owned())
                    .collect()
            });
            let checkouts = (running.zip(ws)).map(|(r, ws)| r.overlay.checkouts(ws));
            (keys, checkouts.unwrap_or_default())
        });
        cx.notify();
        let project = project.clone();
        let reason = label.to_string();
        let began = Instant::now();
        cx.spawn(async move |this, cx| {
            // Asked for by the user, so it runs at full speed even if they switch away
            // meanwhile, rather than as macOS runs a hidden app's work.
            let _awake = cx.background_executor().prevent_app_nap(&reason);
            // Which oplog entry the operation recorded, if it did, for its Undo button.
            #[expect(
                clippy::disallowed_methods,
                reason = "in `backend::run`, off the UI thread"
            )]
            let result = backend::run(move || {
                // Left by a Git that crashed or was killed, as an agent's can be, the lock
                // stops an operation partway, after it has written files.
                anyhow::ensure!(
                    remote_only || !path.join(".git/index.lock").exists(),
                    "Another Git process is using the repository, or one that stopped left \
                     .git/index.lock behind. Delete it if no Git command is running."
                );
                let before = backend::history::oplog_head(&path);
                let heads = backend::worktrees::heads(&path);
                // Just the work itself: what Git logs right after it, as an agent's commit
                // waiting on the lock does, isn't the app's.
                let started = now_millis();
                let value = op(&path, &remap)?;
                let ran = (started, now_millis());
                let recorded =
                    backend::history::oplog_head(&path).filter(|after| Some(*after) != before);
                // The oplog records only the main checkout: what moved a linked worktree,
                // even one stacked on a branch here, can't be undone.
                let moved_worktree = backend::worktrees::heads(&path) != heads;
                Ok((value, recorded, moved_worktree, ran))
            })
            .await;
            this.update(cx, |store, cx| {
                // The reload after it reads just the uncommitted files, or just the commits
                // and branches, if that's all it changed, and what was asked for while it ran.
                let mut reload = Reload::full();
                let mut moved = None;
                let (mut undo, mut asked) = (false, Instant::now());
                if let Some(running) = store.running.get_mut(&project) {
                    let kind = &running.overlay.kind;
                    (undo, asked) = (
                        matches!(kind, OpKind::Undo(_) | OpKind::Redo(_)),
                        running.asked,
                    );
                    running.settling = true;
                    moved = running.moved.take();
                    let overlay = &running.overlay;
                    reload = match overlay.rereads {
                        // Files taken out of a checkout leave it even when its `HEAD` stays
                        // put, as a linked worktree's committed to a branch here do.
                        Rereads::Commits
                            if !overlay.files.is_empty() || !overlay.hunks.is_empty() =>
                        {
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
                    reload.merge(std::mem::take(&mut running.deferred));
                }
                backend::metrics::operation(
                    name,
                    began.elapsed(),
                    result.as_ref().err(),
                    store.workspace(&project),
                );
                let source = (project.clone(), label.clone());
                let first_card = store.next_toast;
                match result {
                    Ok((value, recorded, moved_worktree, ran)) => {
                        // A push or fetch writes only what the remote has, which undo leaves,
                        // so what an agent did while it ran still counts.
                        if !remote_only {
                            store
                                .ran
                                .lock()
                                .entry(project.clone())
                                .or_default()
                                .push(ran);
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
                        store.last_snapshot = recorded.map(|snapshot| (project.clone(), snapshot));
                        if moved_worktree {
                            store.keep_last_snapshot(cx);
                        }
                        // Something new done, ⌘Z takes it back first, and what it took back
                        // before can't be redone over it.
                        if let Some((project, snapshot)) =
                            store.last_snapshot.clone().filter(|_| !undo)
                        {
                            store.push_undo_step(project, snapshot, asked);
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

    /// The key of `project`'s commit `id`, which survives rewrites.
    pub fn key_of(&self, project: &ProjectId, id: CommitId) -> Option<String> {
        let (_, commit) = self.find_commit(project, id)?;
        Some(commit.key.clone())
    }

    /// The longest-running mutation in flight, its project, and for how long, if any.
    pub fn busy(&self) -> Option<(&ProjectId, &SharedString, Duration)> {
        (self.running.iter())
            .max_by_key(|(_, running)| running.started.elapsed())
            .map(|(project, running)| (project, &running.label, running.started.elapsed()))
    }

    /// End `id`'s running mutation once it has loaded what it did, or can't load at all,
    /// and start what waits on it.
    pub(super) fn settled(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        if self.settles(id) {
            // One that only talks to a remote, as a push, writes no files.
            if let Some(running) = self.running.remove(id).filter(|r| !r.overlay.remote_only) {
                let now = crate::ui::now();
                // A second early, as whole seconds can put its first writes before its start.
                let started = now - running.started.elapsed().as_secs() as i64 - 1;
                let spans = self.own_writes.entry(id.clone()).or_default();
                spans.retain(|span| crate::ui::just_now(*span.end()));
                spans.push(started..=now + 2);
            }
            cx.emit(StoreEvent::Pending(id.clone()));
            self.start_next(cx);
        }
    }

    /// Whether the next load of `id` to start ends its running mutation.
    pub(super) fn settles(&self, id: &ProjectId) -> bool {
        self.running.get(id).is_some_and(|r| r.settling)
    }

    /// Operations quitting now would cut off: the ones running, and the ones waiting.
    pub fn unfinished(&self) -> usize {
        self.unfinished_asked().count()
    }

    /// Operations besides the longest-running one: running elsewhere, or waiting.
    pub fn others_busy(&self) -> usize {
        self.running.len().saturating_sub(1) + self.queue.len()
    }

    /// Whether an operation of `kind`, or any with `None`, waits or runs for `project`. One
    /// that has finished and waits for the reload after it counts only with `settling`: a
    /// repeat then would do nothing more, but a retry after it failed may be asked for then.
    pub(super) fn pending(
        &self,
        project: &ProjectId,
        kind: Option<&OpKind>,
        settling: bool,
    ) -> bool {
        let running = (self.running.get(project)).filter(|r| settling || !r.settling);
        let queued = (self.queue.iter()).filter(|q| q.project == *project);
        (running.map(|r| &r.overlay.kind).into_iter())
            .chain(queued.map(|q| &q.overlay.kind))
            .any(|pending| kind.is_none_or(|kind| pending == kind))
    }

    /// Whether conflicts are being resolved in `project`, whose picks are numbered anew once
    /// that's done.
    pub fn resolving(&self, project: &ProjectId) -> bool {
        self.pending(project, Some(&OpKind::Resolve), false)
    }

    /// Whether an update from the target waits or runs on `project`.
    pub fn updating(&self, project: &ProjectId) -> bool {
        self.pending(project, Some(&OpKind::Update), true)
    }

    pub fn fetching(&self, project: &ProjectId) -> bool {
        self.pending(project, Some(&OpKind::Fetch), true)
    }

    fn overlays<'a>(&'a self, project: &'a ProjectId) -> impl Iterator<Item = &'a Overlay> {
        self.overlays_asked(project).map(|(overlay, _)| overlay)
    }

    /// [`Self::overlays`], each with when its mutation was asked for.
    fn overlays_asked<'a>(
        &'a self,
        project: &'a ProjectId,
    ) -> impl Iterator<Item = (&'a Overlay, Instant)> {
        let queued = self.queue.iter().filter(move |q| q.project == *project);
        (self.running.get(project).map(|r| (&r.overlay, r.asked)))
            .into_iter()
            .chain(queued.map(|q| (&q.overlay, q.asked)))
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

    pub fn hides_hunk(
        &self,
        project: &ProjectId,
        checkout: &Checkout,
        path: &str,
        hunk: HunkKey,
    ) -> bool {
        self.overlays(project).any(|o| {
            o.files_in == *checkout && o.hunks.get(path).is_some_and(|hunks| hunks.contains(&hunk))
        })
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
        // The last asked for wins.
        (self.overlays(project).filter_map(|o| o.auto_merges))
            .filter(|(number, _)| *number == review.number)
            .last()
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

    /// The titles of commits pending mutations will make where `at` says, each with whether
    /// it's been on its way long enough to show it's still pending.
    pub fn new_commits(
        &self,
        project: &ProjectId,
        at: impl Fn(&NewAt) -> bool,
    ) -> Vec<(SharedString, bool)> {
        self.overlays_asked(project)
            .filter_map(|(o, asked)| Some((o.new_commit.as_ref()?, asked)))
            .filter(|((place, _), _)| at(place))
            .map(|((_, title), asked)| (title.clone(), asked.elapsed() >= PENDING_AFTER))
            .collect()
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

    /// `commit`'s title and message, as the last pending mutation to reword it leaves them.
    pub fn message_of<'a>(
        &'a self,
        project: &'a ProjectId,
        commit: &'a Commit,
    ) -> (&'a str, &'a str) {
        let reworded = (self.overlays(project).filter_map(|o| o.reworded.as_ref()))
            .filter(|(key, _)| *key == commit.key)
            .last();
        match reworded {
            Some((_, message)) => (message.lines().next().unwrap_or_default().trim(), message),
            None => (&commit.title, &commit.message),
        }
    }

    /// `branch`'s name, as the last pending mutation to rename it leaves it.
    pub fn name_of<'a>(&'a self, project: &'a ProjectId, branch: &'a Branch) -> &'a str {
        let renamed = (self.overlays(project).filter_map(|o| o.renamed.as_ref()))
            .filter(|(ref_name, _)| branch.ref_name.as_ref() == Some(ref_name))
            .last();
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
        self.overlays(project)
            .any(|o| o.pushes.as_ref() == Some(ref_name))
    }

    /// Whether a pending operation opens `ref_name`'s pull request.
    pub fn publishing(&self, project: &ProjectId, ref_name: &FullName) -> bool {
        self.overlays(project)
            .any(|o| o.publishes && o.pushes.as_ref() == Some(ref_name))
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

    /// Whether the selection moved from commit `from` to `to` as an operation rewrote it,
    /// rather than to another commit: it's the same one to look at.
    pub fn followed(&self, from: CommitId, to: CommitId) -> bool {
        self.followed == Some((from, to))
    }
}

/// What an update onto `target` would leave conflicted that isn't now in `workspace`.
fn update_conflicts_detail(
    target: &str,
    found: &UpdateConflicts,
    workspace: Option<&Workspace>,
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
    let moves_worktrees = workspace.is_some_and(|ws| {
        (ws.lanes.iter()).any(|lane| ws.beneath(lane).any(|link| !link.lane.checkout.is_linked()))
    });
    detail.push(if moves_worktrees {
        format!(
            "{resolve}. The update moves the linked worktrees resting on your branches too, \
             so it can't be undone."
        )
    } else {
        format!("{resolve}, or undo the update.")
    });
    detail.join(" ")
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
            update_conflicts_detail("main", &found(&["middle"]), Some(&alone)),
            "Rebasing onto main leaves conflicted commits in middle (1 commit). \
             You can resolve them afterwards, or undo the update.",
            "nothing rests on the stack"
        );
        let rested_on = workspace(
            vec![vec![branch("middle", &[2])]],
            vec![worktree("wt", Some(2), vec![branch("wt", &[5])])],
        );
        assert_eq!(
            update_conflicts_detail("main", &found(&["middle", "wt"]), Some(&rested_on)),
            "Rebasing onto main leaves conflicted commits in middle (1 commit), wt (1 commit). \
             You can resolve them afterwards, a linked worktree's in that worktree. The update \
             moves the linked worktrees resting on your branches too, so it can't be undone.",
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
            update_conflicts_detail("main", &found(&["middle"]), Some(&on_a_worktree)),
            "Rebasing onto main leaves conflicted commits in middle (1 commit). \
             You can resolve them afterwards, or undo the update.",
            "b rests on a, which the update leaves where it is"
        );
    }

    fn id(n: u8) -> CommitId {
        CommitId(gix::ObjectId::from([n; 20]))
    }

    #[test]
    fn remap_follows_a_renamed_branch_by_its_top_commit() {
        let now = workspace(
            vec![vec![branch("fix-v2", &[1]), branch("other", &[2])]],
            vec![worktree("wt", None, vec![branch("wt-v2", &[3])])],
        );
        let captured = [
            (full("refs/heads/fix"), "key1".to_owned()),
            (full("refs/heads/gone"), "missing".to_owned()),
            (full("refs/heads/wt"), "key3".to_owned()),
        ];
        let remap = Remap::new(&HashMap::new(), []).following(&captured, &now);
        let to = |name: &str| match remap.destination(Destination::Branch(full(name))) {
            Ok(Destination::Branch(name)) => name.to_string(),
            _ => String::new(),
        };
        assert_eq!(
            to("refs/heads/fix"),
            "refs/heads/fix-v2",
            "renamed, its top went with it"
        );
        assert_eq!(
            to("refs/heads/gone"),
            "refs/heads/gone",
            "nothing to follow, as it was"
        );
        assert_eq!(to("refs/heads/other"), "refs/heads/other", "still there");
        assert_eq!(
            to("refs/heads/wt"),
            "refs/heads/wt-v2",
            "renamed in a linked worktree"
        );
    }

    #[test]
    fn remap_doesnt_take_the_branch_below_for_a_deleted_one() {
        // Deleted from the top of their stacks, their commits joined the branch below, which
        // had commits of its own in one and none in the other.
        let now = workspace(
            vec![vec![branch("below", &[1])], vec![branch("empty", &[2])]],
            Vec::new(),
        );
        let captured = [
            (full("refs/heads/top"), "key1".to_owned()),
            (full("refs/heads/below"), "own".to_owned()),
            (full("refs/heads/top2"), "key2".to_owned()),
            (full("refs/heads/empty"), String::new()),
        ];
        let remap = Remap::new(&HashMap::new(), []).following(&captured, &now);
        for gone in ["refs/heads/top", "refs/heads/top2"] {
            let to = remap.destination(Destination::Branch(full(gone)));
            assert!(
                matches!(to, Ok(Destination::Branch(name)) if name == full(gone)),
                "deleted, not renamed, so what waited for it refuses rather than go below"
            );
        }
    }

    #[test]
    fn remap_follows_rewritten_commits_by_key() {
        let captured = HashMap::from([
            (id(1), "a".to_owned()),
            (id(2), "b".to_owned()),
            (id(3), "c".to_owned()),
        ]);
        let remap = Remap::new(&captured, [("a", id(11)), ("b", id(12)), ("c", id(3))]);
        assert_eq!(
            remap.get(id(2)).unwrap(),
            id(12),
            "b was rebased to a new id"
        );
        assert_eq!(
            remap.get(id(1)).unwrap(),
            id(11),
            "a was rebased to a new id"
        );
        assert!(
            remap.exactly(id(2)).is_err(),
            "what undo can't take back doesn't follow b to its new id"
        );
        assert_eq!(remap.exactly(id(3)).unwrap(), id(3), "c wasn't rewritten");
    }

    #[test]
    fn remap_refuses_commits_that_are_gone() {
        let captured = HashMap::from([(id(1), "a".to_owned())]);
        let remap = Remap::new(&captured, [("b", id(12))]);
        assert!(
            remap.get(id(1)).is_err(),
            "a was squashed away while it waited"
        );
        assert!(
            remap.get(id(9)).is_err(),
            "an id never captured has nothing to follow"
        );
    }

    #[test]
    fn remap_follows_no_key_two_commits_share() {
        let captured = HashMap::from([(id(1), "a".to_owned()), (id(2), "a".to_owned())]);
        let remap = Remap::new(&captured, [("a", id(2)), ("a", id(13))]);
        assert!(
            remap.get(id(1)).is_err(),
            "1 was rewritten, and either commit with its key could be what it became"
        );
        assert_eq!(
            remap.get(id(2)).unwrap(),
            id(2),
            "2 is still there as it was"
        );
    }
}
