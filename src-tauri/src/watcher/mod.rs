mod dispatchers;
mod events;
mod handlers;

use std::{path, sync::Arc};

pub use events::Event;

use anyhow::{Context, Result};
use tauri::AppHandle;
use tokio::{
    spawn,
    sync::{
        mpsc::{channel, Sender},
        Mutex,
    },
    time::{timeout, Duration},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Watcher {
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

impl Watcher {
    pub fn stop(&self) -> Result<()> {
        self.inner.stop()
    }

    pub async fn post(&self, event: Event) -> Result<()> {
        self.inner.post(event).await
    }

    pub async fn run<P: AsRef<path::Path>>(&self, path: P, project_id: &str) -> Result<()> {
        self.inner.run(path, project_id).await
    }
}

struct WatcherInner {
    handler: handlers::Handler,
    dispatcher: dispatchers::Dispatcher,
    cancellation_token: CancellationToken,

    proxy_tx: Arc<Mutex<Option<Sender<Event>>>>,
}

impl TryFrom<&AppHandle> for WatcherInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            handler: handlers::Handler::try_from(value)?,
            dispatcher: dispatchers::Dispatcher::new(),
            cancellation_token: CancellationToken::new(),
            proxy_tx: Arc::new(Mutex::new(None)),
        })
    }
}

impl WatcherInner {
    pub fn stop(&self) -> Result<()> {
        self.cancellation_token.cancel();
        Ok(())
    }

    pub async fn post(&self, event: Event) -> Result<()> {
        let tx = self.proxy_tx.lock().await;
        if tx.is_some() {
            tx.as_ref()
                .unwrap()
                .send(event)
                .await
                .context("failed to send event")?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("watcher is not started"))
        }
    }

    pub async fn run<P: AsRef<path::Path>>(&self, path: P, project_id: &str) -> Result<()> {
        let (tx, mut rx) = channel(1);
        self.proxy_tx.lock().await.replace(tx.clone());

        spawn({
            let dispatcher = self.dispatcher.clone();
            let project_id = project_id.to_string();
            let project_path = path.as_ref().to_path_buf();
            let tx = tx.clone();
            async move {
                let mut dispatcher_rx = dispatcher
                    .run(&project_id, &project_path)
                    .expect("failed to start dispatcher");
                while let Some(event) = dispatcher_rx.recv().await {
                    let span =
                        tracing::info_span!("proxying event from dispatcher", source = %event);
                    let _guard = span.enter();
                    if let Err(e) = tx.send(event.clone()).await {
                        tracing::error!("{}: failed to post event: {:#}", project_id, e);
                    }
                    drop(_guard);
                }
            }
        });

        tx.send(Event::IndexAll(project_id.to_string()))
            .await
            .context("failed to send event")?;

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    let handle_task = spawn({
                        let project_id = project_id.to_string();
                        let handler = self.handler.clone();
                        let tx = tx.clone();
                        let event = event.clone();
                        async move {
                            match handler.handle(&event).await {
                                Err(error) => tracing::error!("{}: failed to handle event {}: {:#}", project_id, event, error),
                                Ok(events) => {
                                    for e in events {
                                        if let Err(e) = tx.send(e.clone()).await {
                                            tracing::error!("{}: failed to post event {}: {:#}", project_id, event, e);
                                        } else {
                                            tracing::info!("{}: sent response event: {}", project_id, event);
                                        }
                                    }
                                }
                            }
                        }
                    });

                    spawn({
                        let project_id = project_id.to_string();
                        let event = event.clone();
                        let handle_task_timeout = Duration::from_secs(30);
                        async move {
                            if timeout(handle_task_timeout, handle_task).await.is_err() {
                                tracing::error!("{}: {} timedout after {:?}", project_id, event, handle_task_timeout);
                            }
                        }
                    });
                },
                _ = self.cancellation_token.cancelled() => {
                    if let Err(error) = self.dispatcher.stop() {
                        tracing::error!("{}: failed to stop dispatcher: {:#}", project_id, error);
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}
