//! Implement the file-monitoring agent that informs about changes in interesting locations.
#![deny(unsafe_code, rust_2018_idioms)]
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]
#![feature(slice_as_chunks)]

mod events;
use events::InternalEvent;
pub use events::{Action, Change};

mod file_monitor;
mod handler;
pub use handler::Handler;

use std::path::Path;
use std::time;

use anyhow::{Context, Result};
use gitbutler_core::projects::ProjectId;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task,
};
use tokio_util::sync::CancellationToken;

/// An abstraction over a link to the spawned watcher, which runs in the background.
pub struct WatcherHandle {
    /// A way to post events and interact with the actual handler in the background.
    tx: UnboundedSender<InternalEvent>,
    /// The id of the project we are watching.
    project_id: ProjectId,
    /// A way to tell the background process to stop handling events.
    cancellation_token: CancellationToken,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

impl WatcherHandle {
    /// Post an `action` for the watcher to perform.
    pub async fn post(&self, action: Action) -> Result<()> {
        self.tx
            .send(action.into())
            .context("failed to send event")?;
        Ok(())
    }

    /// Return the id of the project we are watching.
    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }
}

/// Run our file watcher processing loop in the background and let `handler` deal with them.
/// Return a handle to the watcher to allow interactions while it's running in the background.
/// Drop the handle to stop the watcher.
///
/// ### Important
///
/// It runs in such a way that each filesystem event is processed concurrently with others, which is why
/// spamming massive amounts of events should be avoided!
pub fn watch_in_background(
    handler: handler::Handler,
    path: impl AsRef<Path>,
    project_id: ProjectId,
) -> Result<WatcherHandle, anyhow::Error> {
    let (events_out, mut events_in) = unbounded_channel();

    file_monitor::spawn(project_id, path.as_ref(), events_out.clone())?;
    handler.reindex(project_id)?;

    let cancellation_token = CancellationToken::new();
    let handle = WatcherHandle {
        tx: events_out,
        project_id,
        cancellation_token: cancellation_token.clone(),
    };
    let handle_event = move |event: InternalEvent| -> Result<()> {
        let handler = handler.clone();
        // NOTE: Traditional parallelization (blocking) is required as `tokio::spawn()` on
        //       the `handler.handle()` future isn't `Send` as it keeps non-Send things
        //       across await points. Further, there is a fair share of `sync` IO happening
        //       as well, so nothing can really be done here.
        task::spawn_blocking(move || {
            futures::executor::block_on(async move {
                handler.handle(event, time::SystemTime::now()).await.ok();
            });
        });
        Ok(())
    };

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(event) = events_in.recv() => handle_event(event)?,
                () = cancellation_token.cancelled() => {
                    break;
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    });

    Ok(handle)
}
