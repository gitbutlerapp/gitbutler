//! Operations that change a project. They run one at a time, and the ones asked for
//! meanwhile wait in a queue, their overlays hiding what they take away.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use gpui::{Context, SharedString, Task};

use super::{ProjectId, Selection, Store, StoreEvent};
use crate::backend::{self, CommitId, PushState, SnapshotId};
use crate::transfer::short_ref;

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
    /// The branch (full ref) a new commit goes on top of, and its title.
    pub new_commit: Option<(String, SharedString)>,
}

impl Overlay {
    pub fn commits(keys: impl IntoIterator<Item = String>) -> Self {
        Self {
            commits: keys.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Hides the files taken whole; ones with only some hunks taken stay.
    pub fn files<'a>(selected: impl IntoIterator<Item = &'a backend::Selected>) -> Self {
        Self {
            files: selected
                .into_iter()
                .filter(|s| s.hunks.is_none())
                .map(|s| s.file.path.clone())
                .collect(),
            ..Self::default()
        }
    }
}

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
            .context("A commit it needed changed while it waited")
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
        self.run(
            project,
            "Fetch",
            Overlay::default(),
            |path, _| backend::fetch(path),
            |store, (), cx| store.snack("Fetched", cx),
            cx,
        );
    }

    /// Rebase the project's branches onto its target's latest commits, or with `branch`
    /// just the stack holding it.
    pub fn update_from_target(
        &mut self,
        project: &ProjectId,
        branch: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let done = if branch.is_some() {
            "Stack updated from target"
        } else {
            "Updated from target"
        };
        self.run(
            project,
            "Update",
            Overlay::default(),
            move |path, _| backend::update_from_target(path, branch.as_deref()),
            move |store, (), cx| store.snack_undoable(done, cx),
            cx,
        );
    }

    pub fn push(&mut self, project: &ProjectId, ref_name: String, cx: &mut Context<Self>) {
        let force = self
            .workspace(project)
            .and_then(|ws| ws.find_branch(&ref_name))
            .is_some_and(|branch| branch.push == PushState::NeedsForce);
        let name = short_ref(&ref_name).to_owned();
        let pushed = project.clone();
        self.run(
            project,
            format!("Push {name}"),
            Overlay::default(),
            move |path, _| backend::push(path, &ref_name, force),
            move |store, (), cx| {
                store.snack(format!("Pushed {name}"), cx);
                // A push can open or move a pull request's checks.
                store.refresh_forge(&pushed);
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
        self.run(
            project,
            label,
            Overlay::commits(keys),
            move |path, remap| {
                let ids = ids
                    .iter()
                    .map(|id| remap.get(*id))
                    .collect::<Result<Vec<_>>>()?;
                backend::uncommit(path, &ids)
            },
            |store, (), cx| store.snack_undoable("Uncommitted", cx),
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
        self.cut = None;
        self.picked.clear();
        cx.emit(StoreEvent::Pending(project.clone()));
        cx.notify();
        if self.busy.is_none() {
            self.start_next(cx);
        }
    }

    pub(super) fn start_next(&mut self, cx: &mut Context<Self>) {
        let Some(next) = self.queue.pop_front() else {
            return;
        };
        let now = self
            .workspace(&next.project)
            .into_iter()
            .flat_map(|ws| ws.commits());
        let remap = Remap::new(
            &next.captured,
            now.map(|commit| (commit.key.as_str(), commit.id)),
        );
        self.busy = Some(next.label);
        self.running = Some((next.project.clone(), next.overlay, Instant::now()));
        (next.start)(self, remap, cx);
        // Replacing the last one's ticker stops it.
        self.busy_ticker = Self::tick_while_busy(cx);
    }

    /// Redraw while a mutation runs, so the status bar can say how long it's taking.
    fn tick_while_busy(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let Ok(true) = this.update(cx, |store, cx| {
                    let busy = store.running.is_some();
                    if store.busy_for().is_some_and(|t| t >= SLOW) {
                        cx.notify();
                    }
                    busy
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
        let Some(path) = self.path(project) else {
            self.busy = None;
            self.running = None;
            cx.emit(StoreEvent::Pending(project.clone()));
            return self.start_next(cx);
        };
        self.mutating = Some(project.clone());
        let before: HashSet<String> = self.workspace(project).map_or_else(HashSet::new, |ws| {
            ws.commits().map(|commit| commit.key.clone()).collect()
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
                store.mutating = None;
                store.settling = Some(project.clone());
                // Set here rather than at the start, so a load the watcher started
                // mid-operation can't use it up.
                store.rebaseline.insert(project.clone(), before);
                match result {
                    Ok((value, recorded)) => {
                        store.last_snapshot = recorded.map(|snapshot| (project.clone(), snapshot));
                        done(store, value, cx)
                    }
                    Err(err) => store.error(format!("{label} failed"), format!("{err:#}"), cx),
                }
                store.reload(&project, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The key of `project`'s commit `id`, which survives rewrites.
    pub fn key_of(&self, project: &ProjectId, id: CommitId) -> Option<String> {
        let (_, commit) = self.workspace(project)?.find_commit(id)?;
        Some(commit.key.clone())
    }

    pub fn busy_for(&self) -> Option<Duration> {
        self.running
            .as_ref()
            .map(|(_, _, started)| started.elapsed())
    }

    /// Operations quitting now would cut off: the one running, and the ones waiting.
    pub fn unfinished(&self) -> usize {
        usize::from(self.mutating.is_some()) + self.queue.len()
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    fn overlays<'a>(&'a self, project: &'a ProjectId) -> impl Iterator<Item = &'a Overlay> {
        let running = self.running.iter().filter(move |(p, ..)| p == project);
        let queued = self.queue.iter().filter(move |q| q.project == *project);
        running
            .map(|(_, overlay, _)| overlay)
            .chain(queued.map(|q| &q.overlay))
    }

    pub fn hides_commit(&self, project: &ProjectId, key: &str) -> bool {
        self.overlays(project).any(|o| o.commits.contains(key))
    }

    pub fn hides_file(&self, project: &ProjectId, path: &str) -> bool {
        self.overlays(project).any(|o| o.files.contains(path))
    }

    /// The titles of commits pending mutations will make on top of `ref_name`.
    pub fn new_commits(&self, project: &ProjectId, ref_name: &str) -> Vec<SharedString> {
        self.overlays(project)
            .filter_map(|o| o.new_commit.as_ref())
            .filter(|(branch, _)| branch == ref_name)
            .map(|(_, title)| title.clone())
            .collect()
    }

    /// Keep the selection on commits an operation rewrote.
    pub fn follow(&mut self, rewritten: &backend::Rewritten, cx: &mut Context<Self>) {
        if let Some(Selection::Commit(project, id)) = &self.selection
            && let Some(new) = rewritten.get(id)
        {
            let selection = Selection::Commit(project.clone(), *new);
            self.select(Some(selection), cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
