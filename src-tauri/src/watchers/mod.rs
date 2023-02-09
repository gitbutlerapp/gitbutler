mod delta;
mod git;

pub use self::delta::get_meta_commit;
pub use self::delta::WatcherCollection;
use crate::projects::Project;
use serde::Serialize;
use tauri::{Runtime, Window};

#[derive(Debug)]
pub enum WatchError {
    WatchDeltaError(delta::WatchError),
    WatchGitError(git::WatchError),
}

impl From<delta::WatchError> for WatchError {
    fn from(error: delta::WatchError) -> Self {
        WatchError::WatchDeltaError(error)
    }
}

impl From<git::WatchError> for WatchError {
    fn from(error: git::WatchError) -> Self {
        WatchError::WatchGitError(error)
    }
}

impl Serialize for WatchError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::WatchDeltaError(error) => write!(f, "watch delta error: {}", error),
            WatchError::WatchGitError(error) => write!(f, "watch git error: {}", error),
        }
    }
}

pub fn watch<R: Runtime>(
    window: Window<R>,
    watchers: &WatcherCollection,
    project: &Project,
) -> Result<(), WatchError> {
    self::delta::watch(window, watchers, project.clone())?;
    self::git::watch(project.clone())?;
    Ok(())
}

#[derive(Debug)]
pub enum UnwatchError {
    DeltaError(delta::UnwatchError),
}

impl std::fmt::Display for UnwatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnwatchError::DeltaError(error) => write!(f, "unwatch delta error: {}", error),
        }
    }
}

impl From<delta::UnwatchError> for UnwatchError {
    fn from(error: delta::UnwatchError) -> Self {
        UnwatchError::DeltaError(error)
    }
}

impl Serialize for UnwatchError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}

pub fn unwatch(watchers: &WatcherCollection, project: Project) -> Result<(), UnwatchError> {
    delta::unwatch(watchers, project)?;
    // TODO: how to unwatch git ?
    Ok(())
}
