mod delta;
mod git;

pub use self::delta::WatcherCollection;
use crate::projects;
use anyhow::Result;
use tauri::{Runtime, Window};

pub fn watch<R: Runtime>(
    window: Window<R>,
    watchers: &WatcherCollection,
    project: &projects::Project,
) -> Result<()> {
    self::delta::watch(window.clone(), watchers, project.clone())?;
    self::git::watch(window.clone(), project.clone())?;
    Ok(())
}

pub fn unwatch(watchers: &WatcherCollection, project: projects::Project) -> Result<()> {
    delta::unwatch(watchers, project)?;
    // TODO: how to unwatch git ?
    Ok(())
}
