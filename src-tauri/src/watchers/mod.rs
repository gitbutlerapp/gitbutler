mod delta;
mod session;

pub use self::delta::WatcherCollection;
use crate::{events, projects, users};
use anyhow::Result;
use std::sync::mpsc;

pub struct Watcher<'a> {
    session_watcher: session::SessionWatcher,
    delta_watcher: delta::DeltaWatchers<'a>,
}

impl<'a> Watcher<'a> {
    pub fn new(
        watchers: &'a delta::WatcherCollection,
        projects_storage: projects::Storage,
        users_storage: users::Storage,
    ) -> Self {
        let session_watcher = session::SessionWatcher::new(projects_storage, users_storage);
        let delta_watcher = delta::DeltaWatchers::new(watchers);
        Self {
            session_watcher,
            delta_watcher,
        }
    }

    pub fn watch(
        &self,
        sender: mpsc::Sender<events::Event>,
        project: &projects::Project,
    ) -> Result<()> {
        self.delta_watcher.watch(sender.clone(), project.clone())?;
        self.session_watcher
            .watch(sender.clone(), project.clone())?;
        Ok(())
    }

    pub fn unwatch(&self, project: projects::Project) -> Result<()> {
        self.delta_watcher.unwatch(project)?;
        // TODO: how to unwatch session ?
        Ok(())
    }
}
