mod delta;
mod git;

pub use self::delta::WatcherCollection;
use crate::projects::project::Project;
use anyhow::Result;
use tauri::{Runtime, Window};

pub fn watch<R: Runtime>(
    window: Window<R>,
    watchers: &WatcherCollection,
    project: &Project,
) -> Result<()> {
    self::delta::watch(window.clone(), watchers, project.clone())?;
    self::git::watch(window.clone(), project.clone())?;
    Ok(())
}

pub fn unwatch(watchers: &WatcherCollection, project: Project) -> Result<()> {
    delta::unwatch(watchers, project)?;
    // TODO: how to unwatch git ?
    Ok(())
}
