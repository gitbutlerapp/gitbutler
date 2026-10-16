//! File watching, so the UI follows changes made by editors, agents and other GitButler clients.

use std::sync::OnceLock;

use anyhow::{Context as _, Result};
use but_settings::AppSettingsWithDiskSync;

use super::{Project, open, runtime};

/// Keeps a project's watcher alive; dropping it stops watching.
pub struct Watcher {
    _monitor: gitbutler_filemonitor::FileMonitorHandle,
}

/// Call `on_change` whenever files change in `project`'s worktree or git directory, with
/// whether the git directory was among them. It runs on a background task, so it should
/// only signal.
///
/// This uses the file monitor alone, not `gitbutler_watcher`, which works out status and
/// hunk assignments under the exclusive repository lock on every event: the app reloads
/// what it shows itself, and the lock would hold up the user's own commits while an
/// agent keeps saving.
pub fn watch(
    project: &Project,
    on_change: impl Fn(bool) + Send + Sync + 'static,
) -> Result<Watcher> {
    let settings = settings()?;
    let watch_mode = gitbutler_filemonitor::WatchMode::from_env_or_settings(
        &settings.get()?.feature_flags.watch_mode,
        |key| std::env::var(key).ok(),
    );
    let worktree = open(&project.path)?.workdir_or_fail()?;
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let _tokio = runtime().enter();
    let monitor = gitbutler_filemonitor::spawn(
        project.id.parse().context("Invalid project id")?,
        &worktree,
        || Ok(Vec::new()),
        events,
        watch_mode,
    )?;
    // Ends when the monitor is dropped, with the sender it holds.
    runtime().spawn(async move {
        while let Some(event) = received.recv().await {
            on_change(matches!(
                event,
                gitbutler_filemonitor::InternalEvent::GitFilesChange(..)
            ));
        }
    });
    Ok(Watcher { _monitor: monitor })
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
