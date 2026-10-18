//! Operations that change a project. A project's operations run one at a time, and the ones
//! asked for meanwhile wait in a queue, their overlays hiding what they take away.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use gpui::{Context, SharedString, Task};

use super::{ProjectId, Reload, Selection, Store, StoreEvent};
use crate::backend::upstream::{UpdateConflicts, UpdateOutcome};
use crate::backend::{self, CommitId, Destination, HunkKey, Selected, SnapshotId, short_ref};

/// How long a mutation runs before the status bar says how long it's taking.
pub const SLOW: Duration = Duration::from_millis(800);

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
    /// Files leaving a commit whole, by path, under the commit's key.
    pub commit_files: HashMap<String, HashSet<String>>,
    /// The branch (full ref) a new commit goes on top of, and its title.
    pub new_commit: Option<(String, SharedString)>,
    /// It changes only uncommitted files, so the reload after it reads just those.
    pub files_only: bool,
    /// The branch (full ref) it pushes, with the ones below it.
    pub pushes: Option<String>,
    /// It opens a pull request for the branch it pushes.
    pub publishes: bool,
    /// It only talks to the remote or the forge and moves refs, leaving the worktree and
    /// the commits alone, so reloads the watcher starts meanwhile go ahead.
    pub remote_only: bool,
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

    /// Hides the files taken whole out of the commit with key `commit`.
    pub fn commit_files<'a>(
        commit: Option<String>,
        selected: impl IntoIterator<Item = &'a backend::Selected>,
    ) -> Self {
        let Some(commit) = commit else {
            return Self::default();
        };
        let files = (selected.into_iter())
            .filter(|s| s.hunks.is_none())
            .map(|s| s.file.path.clone())
            .collect();
        Self {
            commit_files: HashMap::from([(commit, files)]),
            ..Self::default()
        }
    }
}

const FETCH: &str = "Fetch";
const UPDATE: &str = "Update";

/// The commits an operation captured, found again by key when it runs, as operations queued
/// before it may have rewritten them.
pub struct Remap(HashMap<CommitId, CommitId>);

impl Remap {
    fn new<'a>(
        captured: &HashMap<CommitId, String>,
        now: impl IntoIterator<Item = (&'a str, CommitId)>,
    ) -> Self {
        let now: HashMap<&str, CommitId> = now.into_iter().collect();
        Self(
            captured
                .iter()
                .filter_map(|(id, key)| Some((*id, *now.get(key.as_str())?)))
                .collect(),
        )
    }

    pub fn get(&self, id: CommitId) -> Result<CommitId> {
        self.0
            .get(&id)
            .copied()
            .context("A commit changed before this could run. Try again.")
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
    start: Start,
}

/// Starts a queued operation, with its commits as they are by then.
type Start = Box<dyn FnOnce(&mut Store, Remap, &mut Context<Store>)>;

/// A project's operation running now, from when it starts until the reload after it lands.
pub(super) struct Running {
    label: SharedString,
    overlay: Overlay,
    started: Instant,
    /// It finished and waits for its reload. The next load to start ends it when it lands.
    settling: bool,
    /// Reloads asked for while it ran, which the reload after it reads too.
    deferred: Reload,
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
        self.run(
            project,
            label,
            Overlay::default(),
            move |path, _| backend::history::undo(path, redo),
            move |store, undone, cx| match undone {
                Some(operation) => store.snack(format!("{done}: {operation}"), cx),
                None => store.snack(nothing, cx),
            },
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

    pub fn fetch(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
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
        let pulled = project.clone();
        self.run(
            project,
            "Pull from the remote",
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
    /// card that says so offers to push anyway.
    pub fn push(
        &mut self,
        project: &ProjectId,
        ref_name: String,
        protect: bool,
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
                move |path, _| match backend::push(path, &ref_name, protect, run_hooks) {
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
                    move |store, cx| store.push(&pushed, ref_name.clone(), false, cx),
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
        self.run(
            project,
            label,
            Overlay::default(),
            move |path, _| backend::worktrees::set_archived(path, &name, archived),
            move |store, (), cx| store.snack(done, cx),
            cx,
        );
    }

    /// Move commit `id` into a new branch of its own, and select that branch.
    pub fn move_to_new_branch(
        &mut self,
        project: &ProjectId,
        id: CommitId,
        cx: &mut Context<Self>,
    ) {
        let shown = project.clone();
        self.run(
            project,
            "Move to a new branch",
            Overlay::commits(self.key_of(project, id)),
            move |path, remap| backend::move_to_new_branch(path, remap.get(id)?),
            move |store, (rewritten, branch), cx| {
                store.follow(&rewritten, cx);
                store.select(Some(Selection::Branch(shown, branch)), cx);
                store.snack_undoable("Moved to a new branch", cx)
            },
            cx,
        );
    }

    /// Add uncommitted `files` to commit `into`, then call `then` once that worked.
    pub fn amend(
        &mut self,
        project: &ProjectId,
        into: CommitId,
        files: Arc<[Selected]>,
        then: impl FnOnce(&mut Store, &mut Context<Store>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let run_hooks = self.runs_hooks(project);
        // Which commit, as it's often not on screen.
        let amended = (self.find_commit(project, into)).map_or("Amended".into(), |(_, commit)| {
            format!("Amended \"{}\"", crate::ui::commit_title(&commit.title))
        });
        self.run(
            project,
            format!("Amend {}", into.short()),
            Overlay::files(files.iter()),
            move |path, remap| backend::amend(path, remap.get(into)?, &files, run_hooks),
            move |store, (rewritten, left_out, hook_failed), cx| {
                store.follow(&rewritten, cx);
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
        let project = project.clone();
        self.run(
            &project.clone(),
            label,
            Overlay::commits(keys.clone()),
            move |path, remap| {
                let ids = ids
                    .iter()
                    .map(|id| remap.get(*id))
                    .collect::<Result<Vec<_>>>()?;
                backend::uncommit(path, &ids)
            },
            move |store, (), cx| {
                // The selected commit's changes are uncommitted now; show them there.
                if let Some(Selection::Commit(selected, id)) = &store.selection
                    && *selected == project
                    && store
                        .key_of(selected, *id)
                        .is_some_and(|key| keys.contains(&key))
                {
                    store.select(Some(Selection::Uncommitted(project)), cx);
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
        let id = project.clone();
        let start_label = label.clone();
        self.queue.push_back(Queued {
            project: project.clone(),
            label,
            overlay,
            captured,
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
                    settling: false,
                    deferred: Reload::default(),
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
        // While `but resolve` has a commit checked out, only fetching runs: it leaves the
        // worktree alone.
        let editing = self
            .workspace(project)
            .is_some_and(|ws| ws.editing.is_some());
        let refused = editing && label.as_ref() != FETCH;
        let path = self.path(project).filter(|_| !refused);
        let Some(path) = path else {
            if refused {
                self.snack("Run `but resolve finish` or `but resolve cancel` first", cx);
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
                let value = op(&path, &remap)?;
                let recorded =
                    backend::history::oplog_head(&path).filter(|after| Some(*after) != before);
                Ok((value, recorded))
            })
            .await;
            this.update(cx, |store, cx| {
                // The reload after it reads just the uncommitted files if that's all it
                // changed, and what was asked for while it ran.
                let mut reload = Reload::full();
                if let Some(running) = store.running.get_mut(&project) {
                    running.settling = true;
                    if running.overlay.files_only {
                        reload = Reload::files();
                    }
                    reload.merge(std::mem::take(&mut running.deferred));
                }
                // Set here rather than at the start, so a load the watcher started
                // mid-operation can't use it up.
                if let Some(before) = before {
                    store.rebaseline.insert(project.clone(), before);
                }
                match result {
                    Ok((value, recorded)) => {
                        store.last_snapshot = recorded.map(|snapshot| (project.clone(), snapshot));
                        done(store, value, cx)
                    }
                    Err(err) => store.error(format!("{label} failed"), format!("{err:#}"), cx),
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

    /// Whether an update from the target waits or runs on `project`.
    fn updating(&self, project: &ProjectId) -> bool {
        let running = self.running.get(project).map(|r| &r.label);
        let queued = (self.queue.iter()).filter(|q| q.project == *project);
        running
            .into_iter()
            .chain(queued.map(|q| &q.label))
            .any(|label| label == UPDATE)
    }

    fn overlays<'a>(&'a self, project: &'a ProjectId) -> impl Iterator<Item = &'a Overlay> {
        let queued = self.queue.iter().filter(move |q| q.project == *project);
        (self.running.get(project).map(|r| &r.overlay))
            .into_iter()
            .chain(queued.map(|q| &q.overlay))
    }

    pub fn hides_commit(&self, project: &ProjectId, key: &str) -> bool {
        self.overlays(project).any(|o| o.commits.contains(key))
    }

    pub fn hides_file(&self, project: &ProjectId, path: &str) -> bool {
        self.overlays(project).any(|o| o.files.contains(path))
    }

    pub fn hides_hunk(&self, project: &ProjectId, path: &str, hunk: HunkKey) -> bool {
        self.overlays(project)
            .any(|o| o.hunks.get(path).is_some_and(|hunks| hunks.contains(&hunk)))
    }

    /// Whether the file at `path` is leaving the commit with key `commit`.
    pub fn hides_commit_file(&self, project: &ProjectId, commit: &str, path: &str) -> bool {
        self.overlays(project)
            .any(|o| (o.commit_files.get(commit)).is_some_and(|files| files.contains(path)))
    }

    /// The titles of commits pending mutations will make on top of `ref_name`.
    pub fn new_commits(&self, project: &ProjectId, ref_name: &str) -> Vec<SharedString> {
        self.overlays(project)
            .filter_map(|o| o.new_commit.as_ref())
            .filter(|(branch, _)| branch == ref_name)
            .map(|(_, title)| title.clone())
            .collect()
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
            "" => "An unnamed branch",
            name => name,
        };
        let count = crate::ui::plural(*count, "conflicted commit");
        said.push(format!("{name} has {count}"));
    }
    let landed: Vec<&str> = landed.iter().map(|name| short_ref(name)).collect();
    match landed.len() {
        0 => {}
        1 => said.push(format!("{} landed and was removed", landed[0])),
        _ => said.push(format!("{} landed and were removed", landed.join(", "))),
    }
    said.join(". ")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_says_what_conflicted_and_what_landed() {
        assert_eq!(
            updated_message(
                "origin/main",
                &[("refs/heads/feature-a".into(), 2)],
                &["refs/heads/feature-b".into()]
            ),
            "Updated from origin/main. feature-a has 2 conflicted commits. \
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
}
