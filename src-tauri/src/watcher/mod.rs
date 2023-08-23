mod dispatchers;
mod events;
mod handlers;

use std::{path, sync::Arc};

pub use events::Event;

use anyhow::{Context, Result};
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

    proxy_tx: Arc<Mutex<Option<UnboundedSender<Event>>>>,
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
                .context("failed to send event")?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("watcher is not started"))
        }
    }

    pub async fn run<P: AsRef<path::Path>>(&self, path: P, project_id: &str) -> Result<()> {
        let (proxy_tx, mut proxy_rx) = unbounded_channel();
        self.proxy_tx.lock().await.replace(proxy_tx.clone());

        let dispatcher = self.dispatcher.clone();
        let mut dispatcher_rx = dispatcher
            .run(project_id, path.as_ref())
            .context("failed to run dispatcher")?;

        proxy_tx
            .send(Event::IndexAll(project_id.to_string()))
            .context("failed to send event")?;

        let handle_event = |event: &Event| -> Result<()> {
            task::Builder::new()
                .name(&format!("handle {}", event))
                .spawn_blocking({
                    let project_id = project_id.to_string();
                    let handler = self.handler.clone();
                    let tx = proxy_tx.clone();
                    let event = event.clone();
                    move || match handler.handle(&event) {
                        Err(error) => tracing::error!(
                            "{}: failed to handle event {}: {:#}",
                            project_id,
                            event,
                            error
                        ),
                        Ok(events) => {
                            for e in events {
                                if let Err(e) = tx.send(e.clone()) {
                                    tracing::error!(
                                        "{}: failed to post event {}: {:#}",
                                        project_id,
                                        event,
                                        e
                                    );
                                } else {
                                    tracing::info!(
                                        "{}: sent response event: {}",
                                        project_id,
                                        event
                                    );
                                }
                            }
                        }
                    }
                })?;
            Ok(())
        };

        loop {
            tokio::select! {
                Some(event) = dispatcher_rx.recv() => handle_event(&event)?,
                Some(event) = proxy_rx.recv() => handle_event(&event)?,
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
