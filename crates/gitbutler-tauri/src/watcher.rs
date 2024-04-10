mod events;
mod file_monitor;
pub mod handlers;

use std::{collections::HashMap, path, sync::Arc, time};

use anyhow::{Context, Result};
pub use events::Event;
use gitbutler_core::projects::{self, Project, ProjectId};
use tauri::AppHandle;
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        Mutex,
    },
    task,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Watchers {
    app_handle: AppHandle,
    watchers: Arc<Mutex<HashMap<ProjectId, Watcher>>>,
}

impl Watchers {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn watch(&self, project: &projects::Project) -> Result<()> {
        let watcher = Watcher::from_app(&self.app_handle)?;

        let project_id = project.id;
        let project_path = project.path.clone();

        task::spawn({
            let watchers = Arc::clone(&self.watchers);
            async move {
                watchers.lock().await.insert(project_id, watcher.clone());
                match watcher.run(&project_path, project_id).await {
                    Ok(()) => {
                        tracing::debug!(%project_id, "watcher stopped");
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

#[derive(Clone)]
struct Watcher {
    handler: handlers::Handler,
    cancellation_token: CancellationToken,

    proxy_tx: Arc<tokio::sync::Mutex<Option<UnboundedSender<Event>>>>,
}

impl Watcher {
    pub fn from_app(app: &AppHandle) -> std::result::Result<Self, anyhow::Error> {
        Ok(Self {
            handler: handlers::Handler::from_app(app)?,
            cancellation_token: CancellationToken::new(),
            proxy_tx: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }
}

impl Watcher {
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
        project_id: ProjectId,
    ) -> Result<(), anyhow::Error> {
        let (proxy_tx, mut proxy_rx) = unbounded_channel();
        self.proxy_tx.lock().await.replace(proxy_tx.clone());

        let mut dispatcher_rx = file_monitor::spawn(project_id, path.as_ref())?;
        proxy_tx
            .send(Event::IndexAll(project_id))
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
                    break;
                }
            }
        }

        Ok(())
    }
}
