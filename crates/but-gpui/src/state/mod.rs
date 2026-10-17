//! App state shared by every view: the projects with their loaded workspaces, the
//! selection, what's been cut, and the operation in flight.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use gpui::{Context, EventEmitter, SharedString, Subscription, Task};

use crate::backend::watch::Watcher;
use crate::backend::{Branch, Commit, CommitId, Project, Selected, SnapshotId, Workspace};
use crate::transfer::short_ref;

mod background;
mod load;
mod mutation;
mod saved;
mod seen;

use load::{Change, Reload};
pub use mutation::{Overlay, Remap, SAVE_EDIT, SLOW, STOP_EDITING};
use mutation::{Queued, Running};
pub use saved::{Pane, Prefs, saved_window_place};
use saved::{Restored, Saved};
use seen::Seen;
pub use seen::{Review, worktree_review_scope};

pub type ProjectId = SharedString;

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Project(ProjectId),
    Uncommitted(ProjectId),
    Branch(ProjectId, String),
    Commit(ProjectId, CommitId),
    /// The branches outside the workspace, to apply or delete.
    Branches(ProjectId),
    /// The operation log, to look back at and restore.
    History(ProjectId),
    /// A linked worktree, by name, and its uncommitted changes.
    Worktree(ProjectId, Arc<str>),
}

impl Selection {
    pub fn project(&self) -> &ProjectId {
        match self {
            Selection::Project(project)
            | Selection::Uncommitted(project)
            | Selection::Branch(project, _)
            | Selection::Commit(project, _)
            | Selection::Branches(project)
            | Selection::History(project)
            | Selection::Worktree(project, _) => project,
        }
    }
}

/// What ⌘X picked up, waiting for ⌘V on a destination.
#[derive(Debug, Clone)]
pub enum Cut {
    Commit(ProjectId, CommitId),
    /// A commit to put a copy of somewhere else: a cherry-pick.
    Pick(ProjectId, CommitId),
    /// A branch to stack on another, with the branches above it.
    Branch(ProjectId, String),
    /// Files from the worktree (`from: None`), down to the hunks checked, or from a commit.
    Files {
        project: ProjectId,
        from: Option<CommitId>,
        /// Shared, since every row a drag passes over works out what a drop would do.
        files: Arc<[Selected]>,
    },
}

impl Cut {
    pub fn project(&self) -> &ProjectId {
        match self {
            Cut::Commit(project, _)
            | Cut::Pick(project, _)
            | Cut::Branch(project, _)
            | Cut::Files { project, .. } => project,
        }
    }

    /// "Copied" for a cherry-pick, which leaves the commit where it is; "Cut" otherwise.
    pub fn verb(&self) -> &'static str {
        if matches!(self, Cut::Pick(..)) {
            "Copied"
        } else {
            "Cut"
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Cut::Commit(_, id) | Cut::Pick(_, id) => format!("commit {}", id.short()),
            Cut::Branch(_, ref_name) => short_ref(ref_name).to_owned(),
            Cut::Files { files, .. } if files.len() == 1 => files[0].file.path.clone(),
            Cut::Files { files, .. } => format!("{} files", files.len()),
        }
    }
}

#[derive(Debug)]
pub enum StoreEvent {
    /// A project's workspace was (re)loaded.
    Loaded(ProjectId),
    SelectionChanged,
    /// Files of a project were marked reviewed or not, which folds or unfolds them.
    Reviewed(ProjectId),
    /// An operation on a project was queued, started or finished, which hides or shows
    /// what it takes away.
    Pending(ProjectId),
}

/// An error card's button: its label, and what it runs.
pub type Retry = (
    SharedString,
    std::rc::Rc<dyn Fn(&mut Store, &mut Context<Store>)>,
);

/// Something to tell the user, shown the way GitButler's other apps do.
pub enum Notice {
    /// A one-line result at the bottom centre. `undo` is the oplog entry the operation
    /// recorded, which the snackbar's Undo restores.
    Snackbar {
        message: SharedString,
        undo: Option<(ProjectId, SnapshotId)>,
    },
    /// A failure: a card at the bottom right with the error kept for copying, and what
    /// to try instead, if anything.
    Error {
        title: SharedString,
        detail: SharedString,
        retry: Option<Retry>,
    },
}

pub struct Toast {
    pub id: usize,
    pub notice: Notice,
}

pub struct ProjectEntry {
    pub project: Project,
    pub id: ProjectId,
    pub expanded: bool,
    pub workspace: Option<Workspace>,
    pub error: Option<SharedString>,
    loading: bool,
    /// What to read again once the running load lands, if anything was asked for meanwhile.
    queued: Option<Reload>,
    watcher: Option<Watcher>,
}

impl ProjectEntry {
    pub fn is_loading(&self) -> bool {
        self.loading && self.workspace.is_none()
    }
}

pub struct Store {
    pub projects: Vec<ProjectEntry>,
    pub projects_loaded: bool,
    pub selection: Option<Selection>,
    pub cut: Option<Cut>,
    /// Commits picked along with the selected one (⌘- or ⇧-click) to squash or uncommit
    /// together: two or more, or none, all in the selected commit's project.
    pub picked: Vec<CommitId>,
    /// What the pointer is dragging. Only meaningful while GPUI has a drag active.
    pub dragging: Option<Cut>,
    /// Pane widths the user dragged away from their defaults.
    widths: HashMap<Pane, f32>,
    /// Where the window is, saved for the next launch.
    window: Option<saved::WindowPlace>,
    /// What was open when the app last quit, until the first project listing uses it.
    restored: Restored,
    /// The saved state as last written, so unchanged state isn't written again.
    last_saved: Vec<u8>,
    /// File lists show a tree of directories rather than a flat list.
    pub file_tree: bool,
    pub prefs: Prefs,
    pub toasts: Vec<Toast>,
    /// The mutation in flight, if any. Mutations run one at a time, each until the reload
    /// after it lands, so the next one sees what it did.
    running: Option<Running>,
    busy_ticker: Task<()>,
    /// Mutations asked for while one ran, in order.
    queue: VecDeque<Queued>,
    /// The project a finished mutation waits on. Its next load to start ends `running`.
    settling: Option<ProjectId>,
    /// The project an operation is changing right now. Its own reload follows and sees all
    /// of it, so the watcher's reloads for it would only queue in front.
    mutating: Option<ProjectId>,
    /// The oplog entry the last mutation recorded, which the Undo offered for it restores.
    last_snapshot: Option<(ProjectId, SnapshotId)>,
    next_toast: usize,
    seen: Seen,
    /// The commits each project had before the app's last operation on it. Commits that
    /// appear after it are the app's own doing, so they load as already seen.
    rebaseline: HashMap<ProjectId, HashSet<String>>,
    save_task: Task<()>,
    save_queued: bool,
    _save_on_quit: Subscription,
    /// Where the watchers send what changed.
    changes: mpsc::UnboundedSender<Change>,
    _change_listener: Task<()>,
    _forge_poll: Task<()>,
    /// When each project was last fetched in the background.
    fetched: HashMap<ProjectId, Instant>,
    /// When each project's background fetch last failed, as on credentials the user turned
    /// down or being offline; it waits a while before asking again, unless a fetch the user
    /// asks for works.
    fetch_failed: HashMap<ProjectId, Instant>,
    _auto_fetch: Task<()>,
    forge_requests: mpsc::UnboundedSender<ProjectId>,
    /// Projects waiting for a forge refresh, so each waits only once.
    forge_queued: HashSet<ProjectId>,
    /// Projects whose last forge refresh failed, which the minute poll leaves alone.
    forge_failed: HashSet<ProjectId>,
    _forge_worker: Task<()>,
}

impl EventEmitter<StoreEvent> for Store {}

impl Store {
    /// Projects listed and nothing loading or mutating, for the dev script's timings.
    #[cfg(feature = "dev-script")]
    pub fn is_settled(&self) -> bool {
        self.projects_loaded
            && self.running.is_none()
            && self.queue.is_empty()
            && self
                .projects
                .iter()
                .all(|p| !p.loading && p.queued.is_none())
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        let (changes, changed) = mpsc::unbounded();
        let (forge_requests, requested) = mpsc::unbounded();
        let saved = Saved::load();
        let mut store = Self {
            projects: Vec::new(),
            projects_loaded: false,
            selection: None,
            cut: None,
            picked: Vec::new(),
            dragging: None,
            toasts: Vec::new(),
            settling: None,
            mutating: None,
            running: None,
            busy_ticker: Task::ready(()),
            queue: VecDeque::new(),
            last_snapshot: None,
            next_toast: 0,
            seen: saved.seen,
            widths: saved.widths,
            file_tree: saved.file_tree,
            prefs: saved.prefs,
            window: saved.window,
            restored: Restored {
                expanded: saved.expanded,
                selection: saved.selection,
            },
            last_saved: Vec::new(),
            rebaseline: HashMap::new(),
            save_task: Task::ready(()),
            save_queued: false,
            _save_on_quit: cx.on_app_quit(|store, _| {
                store.save();
                async {}
            }),
            changes,
            _change_listener: Self::reload_on_change(changed, cx),
            forge_requests,
            forge_queued: HashSet::new(),
            forge_failed: HashSet::new(),
            _forge_worker: cx
                .spawn(async move |this, cx| Self::forge_worker(this, requested, cx).await),
            _forge_poll: Self::poll_forge(cx),
            fetched: HashMap::new(),
            fetch_failed: HashMap::new(),
            _auto_fetch: Self::fetch_in_background(cx),
        };
        store.load_projects(cx);
        store
    }

    pub fn project(&self, id: &str) -> Option<&ProjectEntry> {
        self.projects.iter().find(|entry| entry.id == id)
    }

    fn project_mut(&mut self, id: &str) -> Option<&mut ProjectEntry> {
        self.projects.iter_mut().find(|entry| entry.id == id)
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.project(id)?.workspace.as_ref()
    }

    /// `project`'s commit `id`, and the branch holding it.
    pub fn find_commit(&self, project: &str, id: CommitId) -> Option<(&Branch, &Commit)> {
        self.workspace(project)?.find_commit(id)
    }

    pub fn path(&self, id: &str) -> Option<PathBuf> {
        Some(self.project(id)?.project.path.clone())
    }

    /// The branch `selection` is, or the one holding the commit it is.
    pub fn branch_for(&self, selection: &Selection) -> Option<&Branch> {
        let workspace = self.workspace(selection.project())?;
        match selection {
            Selection::Branch(_, ref_name) => workspace.find_branch(ref_name),
            Selection::Commit(_, id) => workspace.find_commit(*id).map(|(branch, _)| branch),
            Selection::Project(_)
            | Selection::Uncommitted(_)
            | Selection::Branches(_)
            | Selection::History(_)
            | Selection::Worktree(..) => None,
        }
    }

    /// The projects open in the navigator.
    fn expanded_ids(&self) -> Vec<ProjectId> {
        (self.projects.iter())
            .filter(|entry| entry.expanded)
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// Keep the selection pointing at something that still exists after `id` reloaded.
    /// Another project's selection waits for its own reload: it may be what an operation
    /// there just made.
    fn validate_selection(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.clone().filter(|s| s.project() == id) else {
            return;
        };
        let Some(workspace) = self.workspace(selection.project()) else {
            return;
        };
        let still_there = match &selection {
            Selection::Project(_)
            | Selection::Uncommitted(_)
            | Selection::Branches(_)
            | Selection::History(_) => true,
            Selection::Branch(_, ref_name) => workspace.find_branch(ref_name).is_some(),
            Selection::Commit(_, id) => workspace.find_commit(*id).is_some(),
            Selection::Worktree(_, name) => workspace.worktrees.iter().any(|w| w.name == *name),
        };
        if !still_there {
            self.select(
                Some(Selection::Uncommitted(selection.project().clone())),
                cx,
            );
        }
    }

    pub fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        if !self.picked.is_empty() {
            self.picked.clear();
            cx.notify();
        }
        if self.selection == selection {
            return;
        }
        if let Some(Selection::Commit(project, id)) = &selection
            && let Some((_, commit)) = self.find_commit(project, *id)
        {
            let key = commit.key.clone();
            self.seen
                .commits
                .entry(project.to_string())
                .or_default()
                .insert(key);
        }
        self.selection = selection;
        self.save_soon(cx);
        cx.emit(StoreEvent::SelectionChanged);
        cx.notify();
    }

    /// ⌘-click picks `id` along with the selected commit, or drops it from the picks;
    /// ⇧-click picks every commit from the selected one to `id` in `range`, the commits in
    /// the order they're shown. The clicked commit becomes the selection while it's picked.
    pub fn pick(
        &mut self,
        project: &ProjectId,
        id: CommitId,
        range: Option<Vec<CommitId>>,
        cx: &mut Context<Self>,
    ) {
        let at = match &self.selection {
            Some(Selection::Commit(p, at)) if p == project => *at,
            _ => return self.select(Some(Selection::Commit(project.clone(), id)), cx),
        };
        let mut picked = std::mem::take(&mut self.picked);
        if picked.is_empty() {
            picked.push(at);
        }
        if let Some(order) = range {
            let (Some(from), Some(to)) = (
                order.iter().position(|c| *c == at),
                order.iter().position(|c| *c == id),
            ) else {
                self.picked = picked;
                return;
            };
            for commit in &order[from.min(to)..=from.max(to)] {
                if !picked.contains(commit) {
                    picked.push(*commit);
                }
            }
        } else if let Some(ix) = picked.iter().position(|c| *c == id) {
            picked.remove(ix);
        } else {
            picked.push(id);
        }
        let next = match picked.contains(&id) {
            true => id,
            false => picked.last().copied().unwrap_or(at),
        };
        self.select(Some(Selection::Commit(project.clone(), next)), cx);
        if picked.len() > 1 {
            self.picked = picked;
        }
        cx.notify();
    }

    pub fn snack(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.notify_user(
            Notice::Snackbar {
                message: message.into(),
                undo: None,
            },
            cx,
        );
    }

    /// A snackbar offering to undo the mutation that just ran.
    pub fn snack_undoable(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let undo = self.last_snapshot.clone();
        self.notify_user(
            Notice::Snackbar {
                message: message.into(),
                undo,
            },
            cx,
        );
    }

    pub fn error(
        &mut self,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.notify_user(
            Notice::Error {
                title: title.into(),
                detail: detail.into(),
                retry: None,
            },
            cx,
        );
    }

    fn notify_user(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let id = self.next_toast;
        self.next_toast += 1;
        let linger = match notice {
            Notice::Snackbar { undo: None, .. } => Duration::from_secs(3),
            Notice::Snackbar { undo: Some(_), .. } => Duration::from_secs(6),
            Notice::Error { .. } => Duration::from_secs(15),
        };
        // One snackbar at a time; errors stack.
        if matches!(notice, Notice::Snackbar { .. }) {
            self.toasts
                .retain(|toast| !matches!(toast.notice, Notice::Snackbar { .. }));
        }
        self.toasts.push(Toast { id, notice });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(linger).await;
            this.update(cx, |store, cx| {
                store.toasts.retain(|toast| toast.id != id);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn dismiss_toast(&mut self, id: usize, cx: &mut Context<Self>) {
        self.toasts.retain(|toast| toast.id != id);
        cx.notify();
    }

    pub fn set_cut(&mut self, cut: Option<Cut>, cx: &mut Context<Self>) {
        self.cut = cut;
        cx.notify();
    }
}
