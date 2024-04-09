mod events;
mod file_monitor;
pub mod handlers;

use std::{collections::HashMap, path, sync::Arc, time};

use anyhow::{Context, Result};
pub use events::Event;
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
        let handler = handlers::Handler::from_app(&self.app_handle)?;

        let project_id = project.id;
        let project_path = project.path.clone();

        match spawn(handler, project_path, project_id) {
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

    pub async fn stop(&self, project_id: &ProjectId) -> Result<()> {
        if let Some(token) = self.watchers.lock().await.remove(project_id) {
            token.stop();
        };
        Ok(())
    }
}

#[async_trait::async_trait]
impl gitbutler_core::projects::Watchers for Watchers {
    fn watch(&self, project: &Project) -> Result<()> {
        Watchers::watch(self, project)
    }

    async fn stop(&self, id: ProjectId) -> Result<()> {
        Watchers::stop(self, &id).await
    }

    async fn fetch(&self, id: ProjectId) -> Result<()> {
        self.post(Event::FetchGitbutlerData(id)).await
    }

    async fn push(&self, id: ProjectId) -> Result<()> {
        self.post(Event::PushGitbutlerData(id)).await
    }
}

/// An abstraction over a link to the spawned watcher, which runs in the background.
struct WatcherHandle {
    tx: UnboundedSender<Event>,
    cancellation_token: CancellationToken,
}

impl WatcherHandle {
    pub fn stop(&self) {
        self.cancellation_token.cancel();
    }

    pub async fn post(&self, event: Event) -> Result<()> {
        self.tx.send(event).context("failed to send event")?;
        Ok(())
    }
}

/// Run our file watcher processing loop in the background.
///
/// It runs in such a way that each filesystem event is processed in a new thread, and a handler
/// may return additional events that are then processed in their own threads as well. Effectively,
/// everything is auto-parallelized in the tokio thread pool.
fn spawn<P: AsRef<path::Path>>(
    handler: handlers::Handler,
    path: P,
    project_id: ProjectId,
) -> Result<WatcherHandle, anyhow::Error> {
    let (proxy_tx, mut proxy_rx) = unbounded_channel();

    let mut dispatcher_rx = file_monitor::spawn(project_id, path.as_ref())?;
    proxy_tx
        .send(Event::IndexAll(project_id))
        .context("failed to send event")?;

    let cancellation_token = CancellationToken::new();
    let handle = WatcherHandle {
        tx: proxy_tx.clone(),
        cancellation_token: cancellation_token.clone(),
    };
    let handle_event = move |event: &Event| -> Result<()> {
        let handler = handler.clone();
        let project_id = project_id.to_string();
        let tx = proxy_tx.clone();
        task::spawn_blocking({
            let event = event.clone();
            move || {
                futures::executor::block_on(async move {
                    match handler.handle(&event, time::SystemTime::now()).await {
                        Err(error) => tracing::error!(
                            project_id,
                            %event,
                            ?error,
                            "failed to handle event",
                        ),
                        Ok(events) => {
                            for e in events {
                                if let Err(error) = tx.send(e.clone()) {
                                    tracing::error!(
                                        project_id,
                                        %event,
                                        ?error,
                                        "failed to post event",
                                    );
                                } else {
                                    tracing::debug!(
                                        project_id,
                                        %event,
                                        "sent response event",
                                    );
                                }
                            }
                        }
                    }
                });
            }
        });
        Ok(())
    };

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(event) = dispatcher_rx.recv() => handle_event(&event)?,
                Some(event) = proxy_rx.recv() => handle_event(&event)?,
                () = cancellation_token.cancelled() => {
                    break;
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    });

    Ok(handle)
}
