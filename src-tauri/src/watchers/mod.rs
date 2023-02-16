mod delta;
mod git;

pub use self::delta::WatcherCollection;
use crate::projects;
use anyhow::Result;
use tauri;

pub struct Watcher<'a> {
    git_watcher: git::GitWatcher,
    delta_watcher: delta::DeltaWatchers<'a>,
}

impl<'a> Watcher<'a> {
    pub fn new(
        watchers: &'a delta::WatcherCollection,
        projects_storage: projects::Storage,
    ) -> Self {
        let git_watcher = git::GitWatcher::new(projects_storage);
        let delta_watcher = delta::DeltaWatchers::new(watchers);
        Self {
            git_watcher,
            delta_watcher,
        }
    }

    pub fn watch(&self, window: tauri::Window, project: &projects::Project) -> Result<()> {
        self.delta_watcher.watch(window.clone(), project.clone())?;
        self.git_watcher.watch(window.clone(), project.id.clone())?;
        Ok(())
    }

    pub fn unwatch(&self, project: projects::Project) -> Result<()> {
        self.delta_watcher.unwatch(project)?;
        // TODO: how to unwatch git ?
        Ok(())
    }
}
