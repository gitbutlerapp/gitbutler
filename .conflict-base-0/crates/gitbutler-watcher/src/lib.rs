//! Implement the file-monitoring agent that informs about changes in interesting locations.
#![deny(unsafe_code)]
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]

use std::path::Path;

use anyhow::Result;
use but_settings::AppSettingsWithDiskSync;
use gitbutler_project::ProjectId;
pub use handler::Handler;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task,
};
use tokio_util::sync::CancellationToken;

mod events;
pub use events::Change;
use gitbutler_filemonitor::InternalEvent;

mod handler;

/// An abstraction over a link to the spawned watcher, which runs in the background.
pub struct WatcherHandle {
    /// The id of the project we are watching.
    project_id: ProjectId,
    signal_flush: UnboundedSender<()>,
    /// A way to tell the background process to stop handling events.
    cancellation_token: CancellationToken,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

impl WatcherHandle {
    /// Return the id of the project we are watching.
    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn flush(&self) -> Result<()> {
        self.signal_flush.send(())?;
        Ok(())
    }
}

/// Run our file watcher processing loop in the background and let `handler` deal with them.
/// Return a handle to the watcher to allow interactions while it's running in the background.
/// Drop the handle to stop the watcher.
///
/// ### How it works
///
/// The watcher is a processing loop that relies on filesystem events. These are aggregated so
/// every ~100ms, the changed paths sorted by 'worktree' and 'git-repository' will be processed,
/// each of these events is handled in its own thread, while being able to spawn additional processing
/// tasks as well.
///
/// This also means that when there are continuous changes to the filesystem, these events might pile
/// up if they take longer to process than the 100ms window between them, causing high-CPU and possibly
/// high-memory. However, the likelihood for this is much lower than it was before the architecture
/// was changed to what it is now, which should be much less wasteful.
pub fn watch_in_background(
    handler: handler::Handler,
    worktree_path: impl AsRef<Path>,
    project_id: ProjectId,
    app_settings: AppSettingsWithDiskSync,
) -> Result<WatcherHandle, anyhow::Error> {
    let (events_out, mut events_in) = unbounded_channel();
    let (flush_tx, mut flush_rx) = unbounded_channel();

    let debounce =
        gitbutler_filemonitor::spawn(project_id, worktree_path.as_ref(), events_out.clone())?;

    let cancellation_token = CancellationToken::new();
    let handle = WatcherHandle {
        project_id,
        signal_flush: flush_tx,
        cancellation_token: cancellation_token.clone(),
    };
    let handle_event =
        move |event: InternalEvent, app_settings: AppSettingsWithDiskSync| -> Result<()> {
            let handler = handler.clone();
            // NOTE: Traditional parallelization (blocking) is required as `tokio::spawn()` on
            //       the `handler.handle()` future isn't `Send` as it keeps non-Send things
            //       across await points. Further, there is a fair share of `sync` IO happening
            //       as well, so nothing can really be done here.
            task::spawn_blocking(move || {
                handler.handle(event, app_settings).ok();
            });
            Ok(())
        };

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(event) = events_in.recv() => handle_event(event, app_settings.clone())?,
                Some(_signal_flush) = flush_rx.recv() => {
                    debounce.flush_nonblocking();
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
