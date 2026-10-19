//! Operations that change a project. A project's operations run one at a time, and the ones
//! asked for meanwhile wait in a queue, their overlays hiding what they take away.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use gpui::{Context, SharedString, Task};

use super::{ProjectId, Reload, Selection, Store, StoreEvent};
use crate::backend::upstream::{UpdateConflicts, UpdateOutcome};
use crate::backend::{
    self, Branch, Commit, CommitId, Destination, HunkKey, Selected, SnapshotId, short_ref,
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
    /// The linked worktree, by name, whose files `files` and `hunks` are; the main
    /// checkout's without one.
    pub files_in: Option<Arc<str>>,
    /// Branches leaving, by full ref, as a stack's do when it's unapplied.
    pub branches: HashSet<String>,
    /// Linked worktrees leaving, by name.
    pub worktrees: HashSet<Arc<str>>,
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
    pub renamed: Option<(String, String)>,
    /// What it changes, which the reload after it reads again.
    pub rereads: Rereads,
    /// The branch (full ref) it pushes, with the ones below it.
    pub pushes: Option<String>,
    /// It opens a pull request for the branch it pushes.
    pub publishes: bool,
    /// It only talks to the remote or the forge and moves refs, leaving the worktree and
    /// the commits alone, so reloads the watcher starts meanwhile go ahead.
    pub remote_only: bool,
}

/// What an operation changes, for the reload after it to read again.
#[derive(Default, Clone, Copy)]
pub enum Rereads {
    /// Commits and branches, and the files of each checkout whose `HEAD` has another tree
    /// by now, which is all an operation on commits writes: see [`backend::Kept`].
    #[default]
    Commits,
    /// Only the uncommitted files of the checkout `files_in` names, or of the main one.
    Files,
}

/// Where a commit an operation makes shows until it's loaded.
pub enum NewAt {
    /// On top of this branch (a full ref).
    Top(String),
    /// Right above the commit with this key.
    Above(String),
    /// Right below the commit with this key.
    Below(String),
}

impl Overlay {
    pub fn push(ref_name: &str, publishes: bool) -> Self {
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

    pub fn commits(keys: impl IntoIterator<Item = String>) -> Self {
        Self {
            commits: keys.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Whether `selection` is on a branch or in a linked worktree this takes away.
    fn takes_away(&self, selection: &Selection, store: &Store) -> bool {
        let branch = match selection {
            Selection::Stack(_, bottom) => Some(bottom.as_str()),
            _ => store
                .branch_for(selection)
                .map(|branch| branch.ref_name.as_str()),
        };
        let worktree = match selection {
            Selection::Worktree(_, name) => Some(name),
            _ => store.worktree_of(selection).map(|wt| &wt.name),
        };
        branch.is_some_and(|ref_name| self.branches.contains(ref_name))
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
        let (mut files, mut hunks) = (HashSet::new(), HashMap::new());
        for s in selected {
            match &s.hunks {
                None => {
                    files.insert(s.file.path.clone());
                }
                Some(keys) => {
                    hunks.insert(s.file.path.clone(), keys.clone());
                }
            }
        }
        Self {
            commit_files: HashMap::from([(commit.clone(), files)]),
            commit_hunks: HashMap::from([(commit, hunks)]),
            ..Self::default()
        }
    }
}

const FETCH: &str = "Fetch";
const UPDATE: &str = "Update";
/// What runs while a commit is checked out for editing: fetching, which leaves the worktree
/// alone, and what resolves the commit and ends the edit.
const WHILE_EDITING: [&str; 5] = [
    FETCH,
    "Resolve",
    "Mark resolved",
    "Finish editing",
    "Cancel editing",
];

/// The commits an operation captured, found again by key when it runs, as operations queued
/// before it may have rewritten them.
pub struct Remap(HashMap<CommitId, CommitId>);

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
        Self(
            captured
                .iter()
                .filter_map(|(id, key)| match ids.contains(id) {
                    // Still there: nothing rewrote it.
                    true => Some((*id, *id)),
                    false => Some((*id, (*by_key.get(key.as_str())?)?)),
                })
                .collect(),
        )
    }

    pub fn get(&self, id: CommitId) -> Result<CommitId> {
        self.0
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

    /// Each of `ids`, as [`Self::get`] has it.
    pub fn all(&self, ids: &[CommitId]) -> Result<Vec<CommitId>> {
        ids.iter().map(|id| self.get(*id)).collect()
    }

    /// `to`, with the commit it's relative to as it is after the operations queued before.
    pub fn destination(&self, to: Destination) -> Result<Destination> {
        Ok(match to {
            Destination::AboveCommit(id) => Destination::AboveCommit(self.get(id)?),
            Destination::BelowCommit(id) => Destination::BelowCommit(self.get(id)?),
            branch => branch,
        })
    }
}

/// An operation waiting for the ones before it.
pub(super) struct Queued {
    project: ProjectId,
    label: SharedString,
    overlay: Overlay,
    /// The project's commits when it was asked for, for its [`Remap`].
    captured: HashMap<CommitId, String>,
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
    overlay: Overlay,
    started: Instant,
    asked: Instant,
    /// It finished and waits for its reload. The next load to start ends it when it lands.
    settling: bool,
    /// Reloads asked for while it ran, which the reload after it reads too.
    deferred: Reload,
    moved: Option<Selection>,
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

    /// Restore `project` to before its last operation, or with `redo` take back the last undo.
    pub fn undo(&mut self, project: &ProjectId, redo: bool, cx: &mut Context<Self>) {
        let (label, done, nothing) = if redo {
            ("Redo", "Redone", "Nothing to redo")
        } else {
            ("Undo", "Undone", "Nothing to undo")
        };
        let kept = self.kept_snapshots.clone();
        self.run(
            project,
            label,
            Overlay::default(),
            move |path, _| {
                let kept = kept.lock().expect("not poisoned").clone();
                backend::history::undo(path, redo, &kept)
            },
            move |store, undone, cx| match undone {
                Some(operation) => store.snack(format!("{done}: {operation}"), cx),
                None => store.snack(nothing, cx),
            },
            cx,
        );
    }

    /// Put `project` back to before the operation `snapshot` was taken for, unless that takes
    /// back one kept from undo.
    pub fn restore(&mut self, project: &ProjectId, snapshot: SnapshotId, cx: &mut Context<Self>) {
        let kept = self.kept_snapshots.clone();
        self.run(
            project,
            "Restore",
            Overlay::default(),
            move |path, _| {
                let kept = kept.lock().expect("not poisoned").clone();
                backend::history::restore(path, snapshot, &kept)
            },
            |store, (), cx| store.snack_undoable("Restored", cx),
            cx,
        );
    }

    /// Undo the operation that recorded `snapshot`, as long as nothing came after it.
    pub fn undo_snapshot(
        &mut self,
        project: &ProjectId,
        snapshot: SnapshotId,
        cx: &mut Context<Self>,
    ) {
        self.run(
            project,
            "Undo",
            Overlay::default(),
            move |path, _| backend::history::undo_snapshot(path, snapshot),
            |store, operation, cx| store.snack(format!("Undone: {operation}"), cx),
            cx,
        );
    }

    /// Commit the edited commit as its files are now, and go back to the workspace.
    pub fn finish_editing(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        self.run(
            project,
            "Finish editing",
            Overlay::default(),
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
            Overlay::default(),
            move |path, _| backend::edit::cancel(path, force),
            move |store, left, cx| match left {
                true => store.snack("Stopped editing, the commit is as it was", cx),
                // A conflicted commit's files always differ, markers and all, from the commit
                // as git can store it, so this can't tell whether anything was done to them.
                false => store.error_with_retry(
                    "Cancelling drops the files as they are",
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
            FETCH,
            Overlay::remote(),
            |path, _| backend::fetch(path),
            move |store, any_remote, cx| {
                store.fetch_failed.remove(&fetched);
                store.fetched.insert(fetched, Instant::now());
                store.snack(
                    match any_remote {
                        true => "Fetched",
                        false => "No remote to fetch from",
                    },
                    cx,
                )
            },
            cx,
        );
    }

    /// Rebase the project's branches onto its target's latest commits. Unless `confirmed`,
    /// it does nothing while another update waits or runs, and stops short of leaving
    /// anything conflicted that isn't now, with a card that offers to update anyway. The
    /// card shows before its update has finished, so a confirmed one queues regardless.
    pub fn update_from_target(
        &mut self,
        project: &ProjectId,
        confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        if !confirmed && self.updating(project) {
            return;
        }
        let updated = project.clone();
        self.run(
            project,
            UPDATE,
            Overlay::default(),
            move |path, _| backend::update_from_target(path, confirmed),
            move |store, outcome, cx| {
                let target = (store.workspace(&updated).and_then(|ws| ws.target.as_ref()))
                    .map_or_else(|| "the target".to_owned(), |target| target.name.clone());
                match outcome {
                    UpdateOutcome::WouldConflict(found) => {
                        let again = updated.clone();
                        store.error_with_retry(
                            "Updating leaves conflicts",
                            update_conflicts_detail(&target, &found),
                            "Update anyway",
                            move |store, cx| store.update_from_target(&again, true, cx),
                            cx,
                        )
                    }
                    UpdateOutcome::Updated { conflicted, landed } => {
                        // Its "Resolve the oldest…" bar is the next step.
                        if let Some((branch, _)) = conflicted.iter().find(|(b, _)| !b.is_empty()) {
                            let branch = Selection::Branch(updated, branch.clone());
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
    /// Unless `confirmed`, it stops short of leaving commits conflicted that aren't now, and
    /// the card saying so offers to pull anyway.
    pub fn pull_remote(
        &mut self,
        project: &ProjectId,
        ref_name: String,
        confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        let name = short_ref(&ref_name).to_owned();
        // A second pull of the branch while one waits would find nothing left to bring in.
        // A confirmed one comes from the card that shows once its pull has finished.
        let label = format!("Pull {name}");
        if !confirmed && self.pending(project, &label) {
            return;
        }
        let pulled = project.clone();
        self.run(
            project,
            label,
            Overlay::default(),
            {
                let ref_name = ref_name.clone();
                move |path, _| {
                    if !confirmed {
                        let found = backend::pull_conflicts(path, &ref_name)?;
                        if !found.is_empty() {
                            return Ok(Some(found));
                        }
                    }
                    backend::pull_remote(path, &ref_name, false).map(|()| None)
                }
            },
            move |store, found: Option<Vec<(String, usize)>>, cx| {
                let Some(found) = found else {
                    return store.snack_undoable(format!("Pulled into {name}"), cx);
                };
                let detail = format!(
                    "Rebasing onto the remote's commits leaves conflicted commits in {}. \
                     You can resolve them afterwards, or undo the pull.",
                    commit_counts(&found)
                );
                store.error_with_retry(
                    "Pulling leaves conflicts",
                    detail,
                    "Pull anyway",
                    move |store, cx| store.pull_remote(&pulled, ref_name.clone(), true, cx),
                    cx,
                )
            },
            cx,
        );
    }

    /// Push `ref_name`, and with `protect` refuse to drop commits only the remote has: the
    /// card that says so offers to push anyway. Without it, a forced push goes past the
    /// lease; with `drops`, the commits only the remote has that were shown, it stops at any
    /// others there.
    pub fn push(
        &mut self,
        project: &ProjectId,
        ref_name: String,
        protect: bool,
        drops: Option<Vec<CommitId>>,
        cx: &mut Context<Self>,
    ) {
        let name = short_ref(&ref_name).to_owned();
        let pushed = project.clone();
        let run_hooks = self.runs_hooks(project);
        self.run(
            project,
            format!("Push {name}"),
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
                    Ok(_) => Ok(None),
                    Err(err) => backend::push_refusal(&err).map(Some).ok_or(err),
                }
            },
            move |store, refused: Option<String>, cx| {
                // A push can open or move a pull request's checks, and the branches below
                // may have gone up before the one refused.
                store.refresh_forge(&pushed);
                let Some(reason) = refused else {
                    return store.snack(format!("Pushed {name}"), cx);
                };
                let title = format!("{name} wasn't pushed");
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

    /// Hide linked worktree `name` from the workspace, or with `archived` false show it
    /// again. Like GitButler's other apps, this isn't something undo takes back.
    pub fn set_worktree_archived(
        &mut self,
        project: &ProjectId,
        name: Arc<str>,
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
            move |path, _| backend::worktrees::set_archived(path, &name, archived),
            move |store, (), cx| store.snack(done, cx),
            cx,
        );
    }

    /// Delete linked worktree `name` from disk, with `force` even with uncommitted changes,
    /// which are lost. Its branch stays. Undo can't bring the folder back.
    pub fn delete_worktree(
        &mut self,
        project: &ProjectId,
        name: Arc<str>,
        force: bool,
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
            move |path, _| backend::worktrees::remove(path, &name, force),
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
        // Out of a linked worktree, which the oplog can't put them back into.
        let undoable = !ids.iter().any(|id| self.in_worktree(project, *id));
        let keys = ids.iter().filter_map(|id| self.key_of(project, *id));
        self.run(
            project,
            "Move to a new branch",
            Overlay::commits(keys.collect::<Vec<_>>()),
            move |path, remap| backend::move_to_new_branch(path, &remap.all(&ids)?),
            move |store, (rewritten, branch), cx| {
                store.follow(&rewritten, cx);
                store.select(Some(Selection::Branch(shown, branch)), cx);
                store.snack_after(undoable, "Moved to a new branch", cx)
            },
            cx,
        );
    }

    /// Amend commit `into` with uncommitted `files` of the main checkout, or of the linked
    /// `worktree`, then call `then` once that worked.
    pub fn amend(
        &mut self,
        project: &ProjectId,
        into: CommitId,
        files: Arc<[Selected]>,
        worktree: Option<Arc<str>>,
        then: impl FnOnce(&mut Store, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let run_hooks = self.runs_hooks(project);
        let undoable = worktree.is_none() && !self.in_worktree(project, into);
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
            format!("Amend {}", into.short()),
            overlay,
            move |path, remap| {
                let into = remap.get(into)?;
                backend::amend(path, into, &files, worktree.as_deref(), run_hooks)
            },
            move |store, (rewritten, left_out, hook_failed), cx| {
                store.follow(&rewritten, cx);
                if !undoable {
                    store.keep_last_snapshot();
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

    pub fn uncommit(&mut self, project: &ProjectId, ids: Vec<CommitId>, cx: &mut Context<Self>) {
        let keys: Vec<String> = ids
            .iter()
            .filter_map(|id| self.key_of(project, *id))
            .collect();
        let label = match ids.len() {
            1 => "Uncommit".to_owned(),
            n => format!("Uncommit {n} commits"),
        };
        // A linked worktree's commits uncommit into it, which the oplog can't bring back.
        let undoable = !ids.iter().any(|id| self.in_worktree(project, *id));
        let home = (ids.first())
            .and_then(|id| self.worktree_of(&Selection::Commit(project.clone(), *id)))
            .map(|worktree| worktree.name.clone());
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
                    let shown = match home.clone() {
                        Some(name) => Selection::Worktree(project, name),
                        None => Selection::Uncommitted(project),
                    };
                    store.select(Some(shown), cx);
                }
                store.snack_after(undoable, "Uncommitted", cx)
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
        label: impl Into<SharedString>,
        overlay: Overlay,
        op: impl FnOnce(&Path, &Remap) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let label = label.into();
        let captured = self.workspace(project).map_or_else(HashMap::new, |ws| {
            ws.commits()
                .map(|commit| (commit.id, commit.key.clone()))
                .collect()
        });
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
        let start_label = label.clone();
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
    pub(super) fn start_next(&mut self, cx: &mut Context<Self>) {
        let mut started = false;
        while let Some(ix) =
            (self.queue.iter()).position(|q| !self.running.contains_key(&q.project))
        {
            let Some(next) = self.queue.remove(ix) else {
                break;
            };
            let now = self
                .workspace(&next.project)
                .into_iter()
                .flat_map(|ws| ws.commits());
            let remap = Remap::new(
                &next.captured,
                now.map(|commit| (commit.key.as_str(), commit.id)),
            );
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
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let Ok(true) = this.update(cx, |store, cx| {
                    if store.busy().is_some_and(|(_, t)| t >= SLOW) {
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
        label: SharedString,
        op: impl FnOnce(&Path, &Remap) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
        remap: Remap,
        cx: &mut Context<Self>,
    ) {
        let editing = self
            .workspace(project)
            .is_some_and(|ws| ws.editing.is_some());
        let refused = editing && !WHILE_EDITING.contains(&label.as_ref());
        let path = self.path(project).filter(|_| !refused);
        let Some(path) = path else {
            if refused {
                self.snack("Finish or cancel editing the commit first", cx);
            }
            // `start_next` goes on to this project's next operation.
            self.running.remove(project);
            cx.emit(StoreEvent::Pending(project.clone()));
            return;
        };
        // While editing there are no commits to compare with afterwards; every commit would
        // count as the operation's own and lose its unread mark.
        let before: Option<HashSet<String>> = (!editing).then(|| {
            self.workspace(project).map_or_else(HashSet::new, |ws| {
                ws.commits().map(|commit| commit.key.clone()).collect()
            })
        });
        cx.notify();
        let project = project.clone();
        cx.spawn(async move |this, cx| {
            // Which oplog entry the operation recorded, if it did, for its Undo button.
            let result = backend::run(move || {
                let before = backend::history::oplog_head(&path);
                let heads = backend::worktrees::heads(&path);
                let value = op(&path, &remap)?;
                let recorded =
                    backend::history::oplog_head(&path).filter(|after| Some(*after) != before);
                // The oplog records only the main checkout: what moved a linked worktree,
                // even one stacked on a branch here, can't be undone.
                let moved_worktree = backend::worktrees::heads(&path) != heads;
                Ok((value, recorded, moved_worktree))
            })
            .await;
            this.update(cx, |store, cx| {
                // The reload after it reads just the uncommitted files, or just the commits
                // and branches, if that's all it changed, and what was asked for while it ran.
                let mut reload = Reload::full();
                let mut moved = None;
                if let Some(running) = store.running.get_mut(&project) {
                    running.settling = true;
                    moved = running.moved.take();
                    reload = match running.overlay.rereads {
                        Rereads::Commits => Reload::commits(),
                        Rereads::Files => Reload::files(running.overlay.files_in.clone()),
                    };
                    reload.merge(std::mem::take(&mut running.deferred));
                }
                // Set here rather than at the start, so a load the watcher started
                // mid-operation can't use it up.
                if let Some(before) = before {
                    store.rebaseline.insert(project.clone(), before);
                }
                match result {
                    Ok((value, recorded, moved_worktree)) => {
                        store.last_snapshot = recorded.map(|snapshot| (project.clone(), snapshot));
                        if moved_worktree {
                            store.keep_last_snapshot();
                        }
                        done(store, value, cx)
                    }
                    Err(err) => {
                        // What it would have taken away stays, so the selection goes back.
                        let uncommitted = Selection::Uncommitted(project.clone());
                        if let Some(moved) = moved
                            && store.selection.as_ref() == Some(&uncommitted)
                        {
                            store.select(Some(moved), cx);
                        }
                        store.error(format!("{label} failed"), failure_detail(&err), cx)
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

    /// The longest-running mutation in flight, and for how long, if any.
    pub fn busy(&self) -> Option<(&SharedString, Duration)> {
        (self.running.values())
            .max_by_key(|running| running.started.elapsed())
            .map(|running| (&running.label, running.started.elapsed()))
    }

    /// End `id`'s running mutation once it has loaded what it did, or can't load at all,
    /// and start what waits on it.
    pub(super) fn settled(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        if self.settles(id) {
            self.running.remove(id);
            cx.emit(StoreEvent::Pending(id.clone()));
            self.start_next(cx);
        }
    }

    /// Whether the next load of `id` to start ends its running mutation.
    pub(super) fn settles(&self, id: &ProjectId) -> bool {
        self.running.get(id).is_some_and(|r| r.settling)
    }

    /// Load `reload` of `id` now, unless an operation is changing `id`. Then the reload after
    /// it reads this too, and loading now would only queue in front of that one.
    pub(super) fn load_or_defer(&mut self, id: &ProjectId, reload: Reload, cx: &mut Context<Self>) {
        match (self.running.get_mut(id)).filter(|r| !r.settling && !r.overlay.remote_only) {
            Some(running) => running.deferred.merge(reload),
            None => self.load(id, reload, cx),
        }
    }

    /// Operations quitting now would cut off: the ones running, and the ones waiting.
    pub fn unfinished(&self) -> usize {
        let running = self.running.values().filter(|r| !r.settling);
        running.count() + self.queue.len()
    }

    /// Operations besides the longest-running one: running elsewhere, or waiting.
    pub fn others_busy(&self) -> usize {
        self.running.len().saturating_sub(1) + self.queue.len()
    }

    /// Whether an operation called `label` waits or runs for `project`: for one a repeat of
    /// which would do nothing more, so it isn't queued again.
    fn pending(&self, project: &ProjectId, label: &str) -> bool {
        let running = self.running.get(project).map(|r| &r.label);
        let queued = (self.queue.iter()).filter(|q| q.project == *project);
        running
            .into_iter()
            .chain(queued.map(|q| &q.label))
            .any(|pending| pending == label)
    }

    /// Whether an update from the target waits or runs on `project`.
    pub fn updating(&self, project: &ProjectId) -> bool {
        self.pending(project, UPDATE)
    }

    pub fn fetching(&self, project: &ProjectId) -> bool {
        self.pending(project, FETCH)
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

    /// Whether the file at `path` is leaving the uncommitted files of the linked worktree
    /// named `checkout`, or of the main checkout for `None`.
    pub fn hides_file(&self, project: &ProjectId, checkout: Option<&str>, path: &str) -> bool {
        (self.overlays(project))
            .any(|o| o.files_in.as_deref() == checkout && o.files.contains(path))
    }

    pub fn hides_hunk(
        &self,
        project: &ProjectId,
        checkout: Option<&str>,
        path: &str,
        hunk: HunkKey,
    ) -> bool {
        self.overlays(project).any(|o| {
            o.files_in.as_deref() == checkout
                && o.hunks.get(path).is_some_and(|hunks| hunks.contains(&hunk))
        })
    }

    pub fn hides_branch(&self, project: &ProjectId, ref_name: &str) -> bool {
        self.overlays(project)
            .any(|o| o.branches.contains(ref_name))
    }

    pub fn hides_worktree(&self, project: &ProjectId, name: &str) -> bool {
        self.overlays(project).any(|o| o.worktrees.contains(name))
    }

    /// The hunks, or lines of them, pending mutations take out of files that otherwise stay,
    /// by path: of the uncommitted files of the linked worktree named `checkout` (the main
    /// checkout for `None`), or with a `commit` key, of that commit's.
    pub fn parts_leaving<'a>(
        &'a self,
        project: &'a ProjectId,
        checkout: Option<&str>,
        commit: Option<&str>,
    ) -> HashMap<&'a str, Vec<HunkKey>> {
        let mut parts: HashMap<&str, Vec<HunkKey>> = HashMap::new();
        for overlay in self.overlays(project) {
            let found = match commit {
                Some(commit) => overlay.commit_hunks.get(commit),
                None => (overlay.files_in.as_deref() == checkout).then_some(&overlay.hunks),
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
            .filter(|(ref_name, _)| *ref_name == branch.ref_name)
            .last();
        renamed.map_or(&branch.name, |(_, name)| name)
    }

    /// Whether a pending operation pushes `ref_name`, on its own or to open its pull request.
    pub fn pushing(&self, project: &ProjectId, ref_name: &str) -> bool {
        self.overlays(project)
            .any(|o| o.pushes.as_deref() == Some(ref_name))
    }

    /// Whether a pending operation opens `ref_name`'s pull request.
    pub fn publishing(&self, project: &ProjectId, ref_name: &str) -> bool {
        self.overlays(project)
            .any(|o| o.publishes && o.pushes.as_deref() == Some(ref_name))
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

/// What an update onto `target` would leave conflicted that isn't now.
fn update_conflicts_detail(target: &str, found: &UpdateConflicts) -> String {
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
    detail.push("You can resolve them afterwards, or undo the update.".into());
    detail.join(" ")
}

/// What an update from `target` did: the branches (full refs) it left `conflicted`, and
/// the ones that `landed` and went.
fn updated_message(target: &str, conflicted: &[(String, usize)], landed: &[String]) -> String {
    let mut said = vec![format!("Updated from {target}")];
    for (branch, count) in conflicted {
        let name = match short_ref(branch) {
            "" => "an unnamed branch",
            name => name,
        };
        let count = crate::ui::plural(*count, "conflicted commit");
        said.push(format!("{name} has {count}"));
    }
    let landed: Vec<&str> = landed.iter().map(|name| short_ref(name)).collect();
    match landed.len() {
        0 => {}
        1 => said.push(format!("{} landed and was removed", landed[0])),
        _ => said.push(format!("{} landed and were removed", landed.join(" and "))),
    }
    // A snackbar is one fragment, so its parts are listed rather than sentences.
    said.join(", ")
}

/// Branches (full refs, empty for an unnamed one) with how many of their commits:
/// "feature/a (2 commits), feature/b (1 commit)".
fn commit_counts(branches: &[(String, usize)]) -> String {
    let counts: Vec<String> = (branches.iter())
        .map(|(ref_name, count)| {
            let name = match short_ref(ref_name) {
                "" => "an unnamed branch",
                name => name,
            };
            format!("{name} ({})", crate::ui::plural(*count, "commit"))
        })
        .collect();
    counts.join(", ")
}

/// What a failed operation's card says: the engine's own words, except where those name a
/// commit the user never saw, or leave out what the engine did about it.
fn failure_detail(err: &anyhow::Error) -> String {
    match err.downcast_ref::<but_error::Code>() {
        Some(but_error::Code::ConflictedCommitCheckout) => {
            "That would leave the checked-out commit conflicted, which Git can't check out."
                .to_owned()
        }
        // The engine turns signing off for the project then, and the next commit would
        // quietly go unsigned.
        Some(but_error::Code::CommitSigningFailed) => format!(
            "Signing it failed, which turns commit signing off for this project unless it's \
             on in your global Git config. Check Sign commits in Project settings.\n{}",
            err.root_cause()
        ),
        _ => format!("{err:#}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_conflicted_checkout_says_why_without_a_hash() {
        let err = anyhow::anyhow!(but_error::Code::ConflictedCommitCheckout)
            .context("Refusing to check out conflicted commit 7c20f085")
            .context("Moving the commit");
        assert!(
            failure_detail(&err).starts_with("That would leave the checked-out commit"),
            "the engine's code is found under the context it adds"
        );
        let err = anyhow::anyhow!(but_error::Code::CommitSigningFailed).context("Signing failed");
        assert!(
            failure_detail(&err).contains("turns commit signing off"),
            "the engine turned signing off, which the next commit would otherwise not show"
        );
    }

    #[test]
    fn an_update_says_what_conflicted_and_what_landed() {
        assert_eq!(
            updated_message(
                "origin/main",
                &[("refs/heads/feature-a".into(), 2)],
                &["refs/heads/feature-b".into()]
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
    }

    fn id(n: u8) -> CommitId {
        CommitId(gix::ObjectId::from([n; 20]))
    }

    #[test]
    fn remap_follows_rewritten_commits_by_key() {
        let captured = HashMap::from([(id(1), "a".to_owned()), (id(2), "b".to_owned())]);
        let remap = Remap::new(&captured, [("a", id(11)), ("b", id(12))]);
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
