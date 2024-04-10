mod events;
pub use events::Event;
use events::InternalEvent;

mod file_monitor;
mod handler;
pub use handler::Handler;

use std::path::Path;
use std::{collections::HashMap, sync::Arc, time};

use anyhow::{Context, Result};
use futures::executor::block_on;
use gitbutler_core::projects::{self, Project, ProjectId};
use tauri::AppHandle;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task,
};
use tokio_util::sync::CancellationToken;
use tracing::instrument;

/// Note that this type is managed in Tauri and thus needs to be send and sync.
#[derive(Clone)]
pub struct Watchers {
    /// NOTE: This handle is required for this type to be self-contained as it's used by `core` through a trait.
    app_handle: AppHandle,
    // NOTE: This is a `tokio` mutex as this needs to lock a hashmap currently from within async.
    watchers: Arc<tokio::sync::Mutex<HashMap<ProjectId, WatcherHandle>>>,
}

impl Watchers {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            watchers: Default::default(),
        }
    }

    #[instrument(skip(self, project), err)]
    pub fn watch(&self, project: &projects::Project) -> Result<()> {
        let handler = handler::Handler::from_app(&self.app_handle)?;

        let project_id = project.id;
        let project_path = project.path.clone();

        match watch_in_background(handler, project_path, project_id) {
            Ok(handle) => {
                block_on(self.watchers.lock()).insert(project_id, handle);
            }
            Err(err) => {
                tracing::error!(?err, %project_id, "watcher error");
            }
        }
        Ok(())
    }

    pub async fn post(&self, event: Event) -> Result<()> {
        let watchers = self.watchers.lock().await;
        if let Some(handle) = watchers.get(event.project_id()) {
            handle.post(event).await.context("failed to post event")
        } else {
            Err(anyhow::anyhow!("watcher not found",))
        }
    }

    pub async fn stop(&self, project_id: &ProjectId) {
        self.watchers.lock().await.remove(project_id);
    }
}

#[async_trait::async_trait]
impl gitbutler_core::projects::Watchers for Watchers {
    fn watch(&self, project: &Project) -> Result<()> {
        Watchers::watch(self, project)
    }

    async fn stop(&self, id: ProjectId) {
        Watchers::stop(self, &id).await
    }

    async fn fetch_gb_data(&self, id: ProjectId) -> Result<()> {
        self.post(Event::FetchGitbutlerData(id)).await
    }

    async fn push_gb_data(&self, id: ProjectId) -> Result<()> {
        self.post(Event::PushGitbutlerData(id)).await
    }
}

/// An abstraction over a link to the spawned watcher, which runs in the background.
struct WatcherHandle {
    tx: UnboundedSender<InternalEvent>,
    cancellation_token: CancellationToken,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

impl WatcherHandle {
    pub async fn post(&self, event: Event) -> Result<()> {
        self.tx.send(event.into()).context("failed to send event")?;
        Ok(())
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
fn watch_in_background(
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
