//! File watching, so the UI follows changes made by editors, agents and other GitButler clients.

use std::sync::OnceLock;

use anyhow::{Context as _, Result};
use but_settings::AppSettingsWithDiskSync;

use super::{Project, open, runtime};

/// Keeps a project's watcher alive; dropping it stops watching.
pub struct Watcher {
    _handle: gitbutler_watcher::WatcherHandle,
}

/// Call `on_change` whenever anything GitButler cares about changes in `project`.
/// It runs on a watcher thread, so it should only signal.
pub fn watch(project: &Project, on_change: impl Fn() + Send + Sync + 'static) -> Result<Watcher> {
    let settings = settings()?;
    let watch_mode = gitbutler_watcher::WatchMode::from_env_or_settings(
        &settings.get()?.feature_flags.watch_mode,
        |key| std::env::var(key).ok(),
    );
    let worktree = open(&project.path)?.workdir_or_fail()?;
    let handler = gitbutler_watcher::Handler::new(move |_change| {
        on_change();
        Ok(())
    });
    let _tokio = runtime().enter();
    let handle = gitbutler_watcher::watch_in_background(
        handler,
        worktree,
        project.id.parse().context("Invalid project id")?,
        settings,
        watch_mode,
    )?;
    Ok(Watcher { _handle: handle })
}

fn settings() -> Result<AppSettingsWithDiskSync> {
    static SETTINGS: OnceLock<AppSettingsWithDiskSync> = OnceLock::new();
    if let Some(settings) = SETTINGS.get() {
        return Ok(settings.clone());
    }
    let config_dir = but_path::app_config_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    let settings = AppSettingsWithDiskSync::new_with_customization(config_dir, None)?;
    Ok(SETTINGS.get_or_init(|| settings).clone())
}
