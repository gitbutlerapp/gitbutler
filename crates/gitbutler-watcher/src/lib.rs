//! Implement the file-monitoring agent that informs about changes in interesting locations.
#![deny(unsafe_code)]
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]

use std::{collections::BTreeSet, path::Path};

use anyhow::Result;
use but_ctx::ProjectHandleOrLegacyProjectId;
use but_settings::AppSettingsWithDiskSync;
pub use handler::Handler;
use tokio::{
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
    task,
};
use tokio_util::sync::CancellationToken;

mod events;

pub use events::Change;
use gitbutler_filemonitor::{FileMonitorHandle, InternalEvent};

mod handler;

/// Re-export for convenience
pub use gitbutler_filemonitor::WatchMode;

/// An abstraction over a link to the spawned watcher, which runs in the background.
pub struct WatcherHandle {
    /// The id of the project we are watching.
    project_id: ProjectHandleOrLegacyProjectId,
    /// Must be dropped synchronously so disconnecting its command channel unblocks
    /// `BlockingPool::shutdown` during Tokio runtime teardown.
    file_monitor: FileMonitorHandle,
    /// A way to tell the async event-dispatch task to stop.
    cancellation_token: CancellationToken,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
        // file_monitor drops here, disconnecting cmd_rx and unblocking the spawn_blocking task.
    }
}

impl WatcherHandle {
    /// Return the id of the project we are watching.
    pub fn project_id(&self) -> &ProjectHandleOrLegacyProjectId {
        &self.project_id
    }

    pub fn flush(&self) -> Result<()> {
        self.file_monitor.flush()
    }
}

/// Run our file watcher processing loop in the background and let `handler` deal with them.
/// Return a handle to the watcher to allow interactions while it's running in the background.
/// Drop the handle to stop the watcher.
///
/// ### How it works
///
/// The watcher is a processing loop that relies on filesystem events. These are aggregated by the
/// file monitor every ~100ms, then coalesced again here before dispatch. Handling a change can
/// require a full worktree refresh, so dispatch stays sequential to avoid piling up expensive scans
/// when a generated directory emits continuous filesystem events.
pub fn watch_in_background(
    handler: handler::Handler,
    worktree_path: impl AsRef<Path>,
    project_id: ProjectHandleOrLegacyProjectId,
    app_settings: AppSettingsWithDiskSync,
    watch_mode: WatchMode,
) -> Result<WatcherHandle, anyhow::Error> {
    let (events_out, mut events_in) = unbounded_channel();

    let file_monitor = gitbutler_filemonitor::spawn(
        project_id.clone(),
        worktree_path.as_ref(),
        events_out.clone(),
        watch_mode,
    )?;

    let cancellation_token = CancellationToken::new();
    let handle = WatcherHandle {
        project_id: project_id.clone(),
        file_monitor,
        cancellation_token: cancellation_token.clone(),
    };
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(event) = events_in.recv() => {
                    let mut events = CoalescedEvents::from(event);
                    events.drain(&mut events_in);

                    for event in events.into_events(&project_id) {
                        let handler = handler.clone();
                        let app_settings = app_settings.clone();
                        task::spawn_blocking(move || {
                            handler.handle(event, app_settings).ok();
                        })
                        .await
                        .ok();
                    }
                }
                () = cancellation_token.cancelled() => {
                    tracing::debug!(%project_id, "stopped watcher");
                    break;
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    });

    Ok(handle)
}

struct CoalescedEvents {
    git_paths: BTreeSet<std::path::PathBuf>,
    project_paths: BTreeSet<std::path::PathBuf>,
}

impl CoalescedEvents {
    fn from(event: InternalEvent) -> Self {
        let mut events = CoalescedEvents {
            git_paths: BTreeSet::new(),
            project_paths: BTreeSet::new(),
        };
        events.add(event);
        events
    }

    fn drain(&mut self, events_in: &mut UnboundedReceiver<InternalEvent>) {
        while let Ok(event) = events_in.try_recv() {
            self.add(event);
        }
    }

    fn add(&mut self, event: InternalEvent) {
        match event {
            InternalEvent::GitFilesChange(_, paths) => self.git_paths.extend(paths),
            InternalEvent::ProjectFilesChange(_, paths) => self.project_paths.extend(paths),
        }
    }

    fn into_events(self, project_id: &ProjectHandleOrLegacyProjectId) -> Vec<InternalEvent> {
        let git_paths: Vec<_> = self.git_paths.into_iter().collect();
        let project_paths: Vec<_> = self.project_paths.into_iter().collect();
        let git_refreshes_worktree = git_paths
            .iter()
            .any(|path| path.to_str() == Some(gitbutler_filemonitor::INDEX));

        let mut events = Vec::with_capacity(2);
        if !git_paths.is_empty() {
            events.push(InternalEvent::GitFilesChange(project_id.clone(), git_paths));
        }
        if !project_paths.is_empty() && !git_refreshes_worktree {
            events.push(InternalEvent::ProjectFilesChange(
                project_id.clone(),
                project_paths,
            ));
        }
        events
    }
}
