mod dispatchers;
mod events;
mod handlers;

use std::{collections::HashMap, path, sync::Arc, time};

pub use events::Event;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        Mutex,
    },
    task,
};
use tokio_util::sync::CancellationToken;

use crate::projects::{self, ProjectId};

#[derive(Clone)]
pub struct Watchers {
    app_handle: AppHandle,
    watchers: Arc<Mutex<HashMap<ProjectId, Watcher>>>,
}

impl TryFrom<&AppHandle> for Watchers {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(value.state::<Self>().inner().clone())
    }
}

impl Watchers {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn watch(&self, project: &projects::Project) -> Result<()> {
        let watcher = Watcher::try_from(&self.app_handle)?;

        let project_id = project.id;
        let project_path = project.path.clone();

        task::spawn({
            let watchers = Arc::clone(&self.watchers);
            let watcher = watcher.clone();
            async move {
                watchers.lock().await.insert(project_id, watcher.clone());
                match watcher.run(&project_path, &project_id).await {
                    Ok(()) => {
                        tracing::debug!(%project_id, "watcher stopped");
                    }
                    Err(RunError::PathNotFound(path)) => {
                        tracing::warn!(%project_id, path = %path.display(), "watcher stopped: project path not found");
                        watchers.lock().await.remove(&project_id);
                    }
                    Err(error) => {
                        tracing::error!(?error, %project_id, "watcher error");
                        watchers.lock().await.remove(&project_id);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn post(&self, event: Event) -> Result<()> {
        let watchers = self.watchers.lock().await;
        if let Some(watcher) = watchers.get(event.project_id()) {
            watcher.post(event).await.context("failed to post event")
        } else {
            Err(anyhow::anyhow!("watcher not found",))
        }
    }

    pub async fn stop(&self, project_id: &ProjectId) -> Result<()> {
        if let Some((_, watcher)) = self.watchers.lock().await.remove_entry(project_id) {
            watcher.stop();
        };
        Ok(())
    }
}

#[derive(Clone)]
struct Watcher {
    inner: Arc<WatcherInner>,
}

impl TryFrom<&AppHandle> for Watcher {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            inner: Arc::new(WatcherInner::try_from(value)?),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("{0} not found")]
    PathNotFound(path::PathBuf),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Watcher {
    pub fn stop(&self) {
        self.inner.stop();
    }

    pub async fn post(&self, event: Event) -> Result<()> {
        self.inner.post(event).await
    }

    pub async fn run<P: AsRef<path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<(), RunError> {
        self.inner.run(path, project_id).await
    }
}

struct WatcherInner {
    handler: handlers::Handler,
    dispatcher: dispatchers::Dispatcher,
    cancellation_token: CancellationToken,

    proxy_tx: Arc<tokio::sync::Mutex<Option<UnboundedSender<Event>>>>,
}

impl TryFrom<&AppHandle> for WatcherInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            handler: handlers::Handler::try_from(value)?,
            dispatcher: dispatchers::Dispatcher::new(),
            cancellation_token: CancellationToken::new(),
            proxy_tx: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }
}

impl WatcherInner {
    pub fn stop(&self) {
        self.cancellation_token.cancel();
    }

    pub async fn post(&self, event: Event) -> Result<()> {
        let tx = self.proxy_tx.lock().await;
        if tx.is_some() {
            tx.as_ref()
                .unwrap()
                .send(event)
                .context("failed to send event")?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("watcher is not started"))
        }
    }

    pub async fn run<P: AsRef<path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<(), RunError> {
        let (proxy_tx, mut proxy_rx) = unbounded_channel();
        self.proxy_tx.lock().await.replace(proxy_tx.clone());

        let dispatcher = self.dispatcher.clone();
        let mut dispatcher_rx = match dispatcher.run(project_id, path.as_ref()) {
            Ok(dispatcher_rx) => Ok(dispatcher_rx),
            Err(dispatchers::RunError::PathNotFound(path)) => Err(RunError::PathNotFound(path)),
            Err(error) => Err(error).context("failed to run dispatcher")?,
        }?;

        proxy_tx
            .send(Event::IndexAll(*project_id))
            .context("failed to send event")?;

        let handle_event = |event: &Event| -> Result<()> {
            task::spawn_blocking({
                let project_id = project_id.to_string();
                let handler = self.handler.clone();
                let tx = proxy_tx.clone();
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

        loop {
            tokio::select! {
                Some(event) = dispatcher_rx.recv() => handle_event(&event)?,
                Some(event) = proxy_rx.recv() => handle_event(&event)?,
                () = self.cancellation_token.cancelled() => {
                    self.dispatcher.stop();
                    break;
                }
            }
        }

        Ok(())
    }
}
