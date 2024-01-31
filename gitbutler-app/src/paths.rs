use std::path;

use anyhow::Context;
use tauri::AppHandle;

pub struct LogsDir(path::PathBuf);

impl LogsDir {
    pub fn to_path_buf(&self) -> path::PathBuf {
        self.0.clone()
    }
}

impl TryFrom<&AppHandle> for LogsDir {
    type Error = anyhow::Error;

    fn try_from(app_handle: &AppHandle) -> Result<Self, Self::Error> {
        app_handle
            .path_resolver()
            .app_log_dir()
            .map(Self)
            .context("failed to get app log dir")
    }
}

impl From<&path::PathBuf> for LogsDir {
    fn from(value: &path::PathBuf) -> Self {
        Self::from(value.clone())
    }
}

impl From<path::PathBuf> for LogsDir {
    fn from(value: path::PathBuf) -> Self {
        Self(value)
    }
}

impl From<LogsDir> for path::PathBuf {
    fn from(logs_dir: LogsDir) -> Self {
        logs_dir.0
    }
}
