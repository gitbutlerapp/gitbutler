mod delta;
mod session;

use crate::{events, projects, search, users};
use anyhow::Result;
use std::sync::mpsc;

pub struct Watcher {
    session_watcher: session::SessionWatcher,
    delta_watcher: delta::DeltaWatchers,
}

impl Watcher {
    pub fn new(
        projects_storage: projects::Storage,
        users_storage: users::Storage,
        deltas_searcher: search::Deltas,
    ) -> Self {
        let session_watcher = session::SessionWatcher::new(projects_storage, users_storage, deltas_searcher);
        let delta_watcher = delta::DeltaWatchers::new();
        Self {
            session_watcher,
            delta_watcher,
        }
    }

    pub fn watch(
        &mut self,
        sender: mpsc::Sender<events::Event>,
        project: &projects::Project,
    ) -> Result<()> {
        self.delta_watcher.watch(sender.clone(), project.clone())?;
        self.session_watcher
            .watch(sender.clone(), project.clone())?;
        Ok(())
    }

    pub fn unwatch(&mut self, project: projects::Project) -> Result<()> {
        self.delta_watcher.unwatch(project)?;
        // TODO: how to unwatch session ?
        Ok(())
    }
}
