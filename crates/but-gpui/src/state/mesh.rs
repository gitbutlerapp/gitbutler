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

use super::mutation::Overlay;
use super::{ProjectId, Selection, Store, StoreEvent};
use crate::backend::{
    self,
    mesh::{Checkout, Connection},
};

/// How often the other machines' work is read again. The mesh hears of a change within
/// seconds, and reading it takes a few.
const POLL: Duration = Duration::from_secs(60);

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

#[derive(Default)]
pub struct Mesh {
    pub state: MeshState,
    /// Waiting for the browser to approve this machine.
    pub signing_in: bool,
    /// Each watched project's checkouts on other machines, as last read.
    checkouts: HashMap<ProjectId, Arc<[Checkout]>>,
    /// Projects the daemon was asked to report in this run, so each is asked once.
    asked_to_report: HashSet<PathBuf>,
}

impl Mesh {
    /// Whether `project`'s work elsewhere has been read, or tried, in this run.
    pub fn read(&self, project: &ProjectId) -> bool {
        self.checkouts.contains_key(project)
    }

    pub fn connection(&self) -> Option<&Connection> {
        match &self.state {
            MeshState::Connected(connection) => Some(connection),
            _ => None,
        }
    }
}

impl Store {
    /// Read the mesh every [`POLL`], or at once when something asks with [`Self::read_mesh`]:
    /// the connection, then each watched project's checkouts elsewhere, one at a time.
    pub(super) fn poll_mesh(
        mut asked: mpsc::UnboundedReceiver<()>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let state = match backend::run(backend::mesh::connection).await {
                    Ok(Some(connection)) => MeshState::Connected(connection),
                    Ok(None) => MeshState::SignedOut,
                    Err(err) => MeshState::Unavailable(format!("{err:#}")),
                };
                let Ok(due) = this.update(cx, |store, cx| store.set_mesh_state(state, cx)) else {
                    return;
                };
                for (project, path, report) in due {
                    if report {
                        let path = path.clone();
                        if let Err(err) = backend::run(move || backend::mesh::report(&path)).await {
                            tracing::warn!(%project, "mesh couldn't report the project: {err:#}");
                        }
                    }
                    let read = backend::run(move || backend::mesh::checkouts(&path)).await;
                    tracing::debug!(%project, checkouts = ?read.as_ref().map(Vec::len), "mesh read");
                    let Ok(()) = this.update(cx, |store, cx| match read {
                        Ok(checkouts) => store.set_mesh_checkouts(project, checkouts, cx),
                        Err(err) => {
                            tracing::debug!(%project, "mesh read failed: {err:#}");
                            // Tried, so the page stops saying it's reading; what was read
                            // before stays.
                            store.mesh.checkouts.entry(project).or_default();
                        }
                    }) else {
                        return;
                    };
                }
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
    pub fn read_mesh(&self) {
        self.mesh_requests.unbounded_send(()).ok();
    }

    /// Keep `state`, and say which watched projects to read: each with its path, and whether
    /// the daemon has to be asked to report it first.
    fn set_mesh_state(
        &mut self,
        state: MeshState,
        cx: &mut Context<Self>,
    ) -> Vec<(ProjectId, PathBuf, bool)> {
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
            // Signed out, the navigator lists no Other machines page to be on.
            if let Some(Selection::Machines(project)) = self.selection.clone() {
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
        cx.emit(StoreEvent::MeshRead(project));
        cx.notify();
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
                    Err(err) => {
                        store.error("Couldn't connect to your mesh", format!("{err:#}"), cx)
                    }
                }
                store.mesh.asked_to_report.clear();
                store.read_mesh();
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
                    Err(err) => {
                        store.error("Couldn't disconnect from your mesh", format!("{err:#}"), cx)
                    }
                }
                store.read_mesh();
            })
            .ok();
        })
        .detach();
    }

    /// Bring `branch`, by the mesh's id, from another machine and apply it to `project`'s
    /// workspace. The CLI records it in the oplog, as `but apply` does.
    pub fn mesh_apply(
        &mut self,
        project: &ProjectId,
        mesh_id: String,
        branch: String,
        cx: &mut Context<Self>,
    ) {
        self.run(
            project,
            format!("Bring {branch} here"),
            Overlay::rewriting_main(),
            move |path, _| backend::mesh::apply(path, &mesh_id),
            move |store, applied_as, cx| store.snack(format!("Applied {applied_as}"), cx),
            cx,
        );
    }
}
