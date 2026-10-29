//! File watching, so the UI follows changes made by editors, agents and other GitButler clients.

use anyhow::{Context as _, Result};

use super::settings::app_settings;
use super::{Project, open, runtime};

/// Keeps a project's watcher alive; dropping it stops watching.
pub struct Watcher {
    _monitor: gitbutler_filemonitor::FileMonitorHandle,
}

/// Call `on_change` whenever files change in `project`'s worktree or git directory, with
/// whether the git directory was among them, the checkout whose files changed, and the
/// worktree's files that changed, by their path in it. It runs on a background task, so it
/// should only signal.
///
/// This uses the file monitor alone, not `gitbutler_watcher`, which works out status and
/// hunk assignments under the exclusive repository lock on every event: the app reloads
/// what it shows itself, and the lock would hold up the user's own commits while an
/// agent keeps saving.
pub fn watch(
    project: &Project,
    on_change: impl Fn(bool, super::Checkout, Vec<std::sync::Arc<str>>) + Send + Sync + 'static,
) -> Result<Watcher> {
    let settings = app_settings()?;
    let watch_mode = gitbutler_filemonitor::WatchMode::from_env_or_settings(
        &settings.get()?.feature_flags.watch_mode,
        |key| std::env::var(key).ok(),
    );
    let worktree = open(&project.path)?.workdir_or_fail()?;
    let path = project.path.clone();
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let _tokio = runtime().enter();
    let monitor = gitbutler_filemonitor::spawn(
        project.id.parse().context("Invalid project id")?,
        &worktree,
        move || linked_worktrees(&path),
        events,
        watch_mode,
    )?;
    // Ends when the monitor is dropped, with the sender it holds.
    runtime().spawn(async move {
        use gitbutler_filemonitor::{Checkout, InternalEvent};
        while let Some(event) = received.recv().await {
            let (git, checkout, paths) = match event {
                InternalEvent::GitFilesChange(_, checkout, _) => (true, checkout, Vec::new()),
                InternalEvent::ProjectFilesChange(_, checkout, paths) => (false, checkout, paths),
            };
            let paths = (paths.iter())
                .map(|path| path.to_string_lossy().into())
                .collect();
            let checkout = match checkout {
                Checkout::Main => super::Checkout::Main,
                Checkout::Linked(name) => super::Checkout::Linked(name),
            };
            on_change(git, checkout, paths);
        }
    });
    Ok(Watcher { _monitor: monitor })
}

/// The linked worktrees to watch too, when GitButler's worktree support is on.
fn linked_worktrees(path: &std::path::Path) -> Result<Vec<gitbutler_filemonitor::LinkedWorktree>> {
    Ok((open(path)?.worktrees_with_state()?.into_iter())
        .filter(|worktree| !worktree.archived)
        .map(|worktree| gitbutler_filemonitor::LinkedWorktree {
            name: worktree.name,
            workdir: worktree.path,
        })
        .collect())
}
