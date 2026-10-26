//! GitButler's mesh: what the user's other machines have of each watched project, read
//! every minute, and this machine's connection to them. The mesh daemon reports this
//! machine's work itself, watching its files; the app makes sure it reports the projects
//! watched here.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::{FutureExt as _, StreamExt as _, channel::mpsc};
use gpui::{Context, Task};

use super::mutation::{Label, Overlay};
use super::{ProjectId, Section, Selection, Store, StoreEvent};
use crate::backend::{
    self, CommitId,
    mesh::{self, Checkout, Connection},
};

/// How often the other machines' work is read again. The mesh hears of a change within
/// seconds, and reading it takes a few.
const POLL: Duration = Duration::from_secs(60);

/// A branch on another machine: its project, its checkout by the mesh's id, and its name.
pub type MeshBranch = (ProjectId, Arc<str>, String);

/// What's selected of a checkout on another machine: it, one of its branches, or a commit,
/// by the branch it's listed under.
#[derive(Debug, Clone, PartialEq)]
pub enum MeshPick {
    Checkout,
    Branch(String),
    Commit(String, CommitId),
}

impl MeshPick {
    /// The branch picked, or holding the commit picked.
    pub fn branch(&self) -> Option<&str> {
        match self {
            MeshPick::Checkout => None,
            MeshPick::Branch(name) | MeshPick::Commit(name, _) => Some(name),
        }
    }
}

/// This machine's connection to the mesh.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum MeshState {
    /// Not read yet.
    #[default]
    Unknown,
    /// `but-mesh` can't run, and why.
    Unavailable(String),
    SignedOut,
    Connected(Connection),
}

pub struct Mesh {
    pub state: MeshState,
    /// Waiting for the browser to approve this machine.
    pub signing_in: bool,
    /// Each watched project's checkouts on other machines, as last read.
    checkouts: HashMap<ProjectId, Arc<[Checkout]>>,
    /// Each watched project's id in the mesh, once read, which its icon is set by.
    ids: HashMap<ProjectId, String>,
    /// Projects whose icon is being sent to the mesh: a read meanwhile may have the old one.
    sending_icon: HashSet<ProjectId>,
    /// Projects whose icon was picked before the mesh's id for them was read, sent once it is.
    unsent_icon: HashSet<ProjectId>,
    /// Projects the daemon was asked to report in this run, so each is asked once.
    asked_to_report: HashSet<PathBuf>,
    /// Asks the poll to read the mesh again now.
    requests: mpsc::UnboundedSender<()>,
    _poll: Task<()>,
}

impl Mesh {
    /// Start reading the mesh in the background, every [`POLL`] and whenever asked.
    pub(super) fn start(cx: &mut Context<Store>) -> Self {
        let (requests, asked) = mpsc::unbounded();
        Self {
            state: MeshState::default(),
            signing_in: false,
            checkouts: HashMap::new(),
            ids: HashMap::new(),
            sending_icon: HashSet::new(),
            unsent_icon: HashSet::new(),
            asked_to_report: HashSet::new(),
            requests,
            _poll: Store::poll_mesh(asked, cx),
        }
    }

    pub fn connection(&self) -> Option<&Connection> {
        match &self.state {
            MeshState::Connected(connection) => Some(connection),
            _ => None,
        }
    }
}

impl Store {
    /// Read the mesh every [`POLL`], or at once when something asks with [`Self::mesh_read_now`]:
    /// the connection, then each watched project's checkouts elsewhere, one at a time.
    fn poll_mesh(mut asked: mpsc::UnboundedReceiver<()>, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                // Hidden, nothing shows what it reads, so it waits to be asked, as showing the
                // window asks.
                let Ok(hidden) = this.update(cx, |store, _| store.hidden) else {
                    return;
                };
                if hidden {
                    if asked.next().await.is_none() {
                        return;
                    }
                    continue;
                }
                let state = match backend::run(backend::mesh::connection).await {
                    Ok(Some(connection)) => MeshState::Connected(connection),
                    Ok(None) => MeshState::SignedOut,
                    Err(err) => MeshState::Unavailable(format!("{err:#}")),
                };
                let Ok(due) = this.update(cx, |store, cx| store.set_mesh_state(state, cx)) else {
                    return;
                };
                // All at once: each read takes seconds, and one after another the last project
                // waited for all the others.
                let reads = due.into_iter().map(|(project, path, report)| {
                    let (this, mut cx) = (this.clone(), cx.clone());
                    async move {
                        if report {
                            let to_report = path.clone();
                            let reported =
                                backend::run(move || backend::mesh::report(&to_report)).await;
                            // Asked again on the next read, as the daemon may only have been down.
                            if let Err(err) = reported {
                                tracing::debug!(%project, "mesh couldn't report the project: {err:#}");
                                let path = path.clone();
                                (this.update(&mut cx, |store, _| {
                                    store.mesh.asked_to_report.remove(&path)
                                }))
                                .ok();
                            }
                        }
                        let id = project.to_string();
                        let read = backend::run(move || backend::mesh::status(&path, &id)).await;
                        tracing::debug!(%project, checkouts = ?read.as_ref().map(|s| s.checkouts.len()), "mesh read");
                        (this.update(&mut cx, |store, cx| match read {
                            Ok(status) => {
                                if let Some(read) = status.project {
                                    store.set_mesh_project(&project, read, cx);
                                }
                                store.set_mesh_checkouts(project, status.checkouts, cx)
                            }
                            // What was read before stays.
                            Err(err) => tracing::debug!(%project, "mesh read failed: {err:#}"),
                        }))
                        .ok();
                    }
                });
                futures::future::join_all(reads).await;
                let timer = cx.background_executor().timer(POLL);
                futures::select! {
                    _ = timer.fuse() => {}
                    wake = asked.next() => if wake.is_none() { return },
                }
                // Several asks while reading make one more read.
                while asked.try_recv().is_ok() {}
            }
        })
    }

    /// Read the mesh again now, as after watching a project or signing in.
    pub fn mesh_read_now(&self) {
        self.mesh.requests.unbounded_send(()).ok();
    }

    /// Keep `state`, and say which watched projects to read: each with its path, and whether
    /// the daemon has to be asked to report it first.
    fn set_mesh_state(
        &mut self,
        state: MeshState,
        cx: &mut Context<Self>,
    ) -> Vec<(ProjectId, PathBuf, bool)> {
        // A failed check while connected, as on a poor network or with the CLI briefly
        // broken, keeps what was read: the navigator shows it, a little stale, until it answers.
        if let MeshState::Unavailable(why) = &state
            && self.mesh.connection().is_some()
        {
            tracing::debug!("mesh check failed, keeping the last read: {why}");
            return Vec::new();
        }
        if self.mesh.state != state {
            tracing::debug!(?state, "mesh connection");
            self.mesh.state = state;
            cx.notify();
        }
        let Some(connection) = self.mesh.connection() else {
            if !self.mesh.checkouts.is_empty() {
                self.mesh.checkouts.clear();
                cx.notify();
            }
            self.mesh.ids.clear();
            // Signed out, the navigator lists no other machines to be on.
            if let Some(Selection::Mesh(project, ..) | Selection::Section(project, Section::Mesh)) =
                self.selection.clone()
            {
                self.select(Some(Selection::Project(project)), cx);
            }
            return Vec::new();
        };
        let reported: HashSet<PathBuf> = connection.reported.iter().cloned().collect();
        let watched: Vec<_> = (self.projects.iter())
            .filter(|entry| entry.expanded)
            .map(|entry| (entry.id.clone(), entry.project.path.clone()))
            .collect();
        // Projects no longer watched keep nothing read of them.
        let before = self.mesh.checkouts.len();
        (self.mesh.checkouts).retain(|id, _| watched.iter().any(|(watched, _)| watched == id));
        if self.mesh.checkouts.len() != before {
            cx.notify();
        }
        watched
            .into_iter()
            .map(|(id, path)| {
                let report =
                    !reported.contains(&path) && self.mesh.asked_to_report.insert(path.clone());
                (id, path, report)
            })
            .collect()
    }

    fn set_mesh_checkouts(
        &mut self,
        project: ProjectId,
        checkouts: Vec<Checkout>,
        cx: &mut Context<Self>,
    ) {
        let watched = self.project(&project).is_some_and(|entry| entry.expanded);
        if !watched || self.mesh.checkouts.get(&project).map(|c| &c[..]) == Some(&checkouts[..]) {
            return;
        }
        self.mesh
            .checkouts
            .insert(project.clone(), checkouts.into());
        // What was selected there may be gone, finished or pushed: it goes up to what's still
        // there, a commit rewritten to its branch, a branch gone to its checkout, a checkout
        // gone to their heading, and the last one to the project.
        let none_left = self.mesh_checkouts(&project).is_empty();
        if none_left && self.selection == Some(Selection::Section(project.clone(), Section::Mesh)) {
            self.select(Some(Selection::Project(project.clone())), cx);
        }
        if let Some(Selection::Mesh(p, id, pick)) = &self.selection
            && *p == project
        {
            let checkout = self.mesh_checkout(p, id);
            let branch = (checkout.zip(pick.branch()))
                .and_then(|(checkout, name)| checkout.branches.iter().find(|b| b.name == name));
            let mesh = |pick| Selection::Mesh(p.clone(), id.clone(), pick);
            let up = match pick {
                _ if none_left => Some(Selection::Project(p.clone())),
                _ if checkout.is_none() => Some(Selection::Section(p.clone(), Section::Mesh)),
                MeshPick::Checkout => None,
                MeshPick::Branch(_) => branch.is_none().then(|| mesh(MeshPick::Checkout)),
                MeshPick::Commit(name, commit) => match branch {
                    None => Some(mesh(MeshPick::Checkout)),
                    Some(branch) => (!branch.commits.iter().any(|c| c.id == *commit))
                        .then(|| mesh(MeshPick::Branch(name.clone()))),
                },
            };
            if let Some(up) = up {
                self.select(Some(up), cx);
            }
        }
        cx.emit(StoreEvent::MeshRead(project));
        cx.notify();
    }

    /// Keep the mesh's id for `project`, and have its icon and the mesh's agree: the mesh's,
    /// picked here or on but.dev, wins, unless one was picked here before the id was read; one
    /// picked only here is sent to the mesh.
    fn set_mesh_project(
        &mut self,
        project: &ProjectId,
        read: mesh::Project,
        cx: &mut Context<Self>,
    ) {
        self.mesh.ids.insert(project.clone(), read.id);
        let unsent = self.mesh.unsent_icon.remove(project);
        let here = self.project_icon(project).unwrap_or_default();
        if read.icon == here || self.mesh.sending_icon.contains(project) {
            return;
        }
        match read.icon.is_empty() || unsent {
            true => self.mesh_send_icon(project, here.to_owned(), cx),
            false => {
                let (id, icon) = (project.to_string(), read.icon);
                self.set_prefs(|prefs| _ = prefs.project_icons.insert(id, icon), cx);
            }
        }
    }

    /// Send `project`'s `icon` to the mesh, now if it has an id there, or else once a read
    /// gives it one. A failure is only logged: the next read puts back the mesh's.
    pub(super) fn mesh_send_icon(
        &mut self,
        project: &ProjectId,
        icon: String,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.mesh.ids.get(project).cloned() else {
            self.mesh.unsent_icon.insert(project.clone());
            return;
        };
        let project = project.clone();
        self.mesh.sending_icon.insert(project.clone());
        cx.spawn(async move |this, cx| {
            let sent = backend::run(move || mesh::set_project_icon(&id, &icon)).await;
            if let Err(err) = sent {
                tracing::warn!(%project, "couldn't send the project's icon to the mesh: {err:#}");
            }
            this.update(cx, |store, _| {
                store.mesh.sending_icon.remove(&project);
                store.mesh_read_now();
            })
            .ok();
        })
        .detach();
    }

    /// Show `project`'s checkouts on other machines as they were last read, until the mesh
    /// answers, which takes seconds: its section is there with the rest of the card rather
    /// than arriving after it, and the read only changes what changed.
    pub(super) fn mesh_seed(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        if !self.mesh_may_show(project) {
            return;
        }
        let project = project.clone();
        // Off the UI thread: a busy project's read runs to a megabyte.
        cx.spawn(async move |this, cx| {
            let id = project.to_string();
            let Ok(Some(status)) = backend::run(move || Ok(mesh::cached_status(&id))).await else {
                return;
            };
            this.update(cx, |store, cx| {
                // Unless the mesh answered meanwhile, or this machine left it.
                if store.mesh_may_show(&project) {
                    (store.mesh.checkouts).insert(project, status.checkouts.into());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Whether what was last read of `project` may stand in: nothing's been read of it this
    /// run, and this machine is in the mesh, or not known yet not to be.
    fn mesh_may_show(&self, project: &ProjectId) -> bool {
        matches!(
            self.mesh.state,
            MeshState::Unknown | MeshState::Connected(_)
        ) && !self.mesh.checkouts.contains_key(project)
    }

    /// `project`'s checkouts on other machines, as the mesh last said.
    pub fn mesh_checkouts(&self, project: &ProjectId) -> &[Checkout] {
        self.mesh.checkouts.get(project).map_or(&[], |c| &c[..])
    }

    /// `project`'s checkout `id` on another machine.
    pub fn mesh_checkout(&self, project: &ProjectId, id: &str) -> Option<&Checkout> {
        self.mesh_checkouts(project).iter().find(|c| c.id == id)
    }

    /// Connect this machine to the mesh: the browser opens to approve it, and once it's
    /// approved the daemon starts reporting this machine's projects.
    pub fn mesh_sign_in(&mut self, cx: &mut Context<Self>) {
        if self.mesh.signing_in {
            return;
        }
        self.mesh.signing_in = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let signed_in = backend::run(backend::mesh::log_in).await;
            this.update(cx, |store, cx| {
                store.mesh.signing_in = false;
                match signed_in {
                    Ok(()) => store.snack("Connected to your mesh", cx),
                    Err(err) => store.failed("Couldn't connect to your mesh", &err, cx),
                }
                store.mesh.asked_to_report.clear();
                store.mesh_read_now();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Take this machine out of the mesh, stopping its daemon.
    pub fn mesh_sign_out(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let signed_out = backend::run(backend::mesh::log_out).await;
            this.update(cx, |store, cx| {
                match signed_out {
                    Ok(()) => store.snack("Disconnected from your mesh", cx),
                    Err(err) => store.failed("Couldn't disconnect from your mesh", &err, cx),
                }
                store.mesh_read_now();
            })
            .ok();
        })
        .detach();
    }

    /// Bring `branch`, by the mesh's id, from another machine and apply it to `project`'s
    /// workspace. Its commits come over first, outside the project's queue, so a slow network
    /// holds up only this; applying them then takes a turn in the queue, and the CLI records it
    /// in the oplog, as `but apply` does.
    pub fn mesh_apply(
        &mut self,
        project: &ProjectId,
        mesh_id: String,
        branch: String,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.path(project) else {
            return;
        };
        let project = project.clone();
        self.snack(format!("Bringing {branch} over…"), cx);
        cx.spawn(async move |this, cx| {
            let id = mesh_id.clone();
            let fetched = backend::run(move || backend::mesh::fetch(&path, &id, None)).await;
            this.update(cx, |store, cx| match fetched {
                Ok(_) => store.run(
                    &project,
                    Label::new("Bring here", format!("Bring {branch} here")),
                    Overlay::rewriting_main(),
                    move |path, _| backend::mesh::apply(path, &mesh_id),
                    move |store, applied_as, cx| store.snack(format!("Applied {applied_as}"), cx),
                    cx,
                ),
                Err(err) => store.failed(format!("Couldn't bring {branch} here"), &err, cx),
            })
            .ok();
        })
        .detach();
    }
}
