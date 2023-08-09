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
                    log::warn!("{}: sending dispatcher event: {}", project_id, event);
                    if let Err(e) = tx.send(event.clone()).await {
                        log::error!("{}: failed to post event: {:#}", project_id, e);
                    }
                    log::warn!("{}: sent dispatcher event: {}", project_id, event);
                }
            }
        });

        tx.send(Event::IndexAll(project_id.to_string()))
            .await
            .context("failed to send event")?;

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    spawn({
                        let project_id = project_id.to_string();
                        let handler = self.handler.clone();
                        let tx = tx.clone();
                        let event = event.clone();
                        async move {
                            log::warn!("{}: handling event: {}", project_id, event);
                            let start = std::time::Instant::now();
                            match handler.handle(event.clone()).await {
                                Err(error) => log::error!("{}: failed to handle event {} in {:?}: {:#}", project_id, event, start.elapsed(), error),
                                Ok(events) => {
                                    log::warn!("{}: handled event {} in {:?}", project_id, event, start.elapsed()) ;
                                    for event in events {
                                        let start = std::time::Instant::now();
                                        log::warn!("{}: sending response event: {}", project_id, event);
                                        if let Err(e) = tx.send(event.clone()).await {
                                            log::error!("{}: failed to post event {}, waited {:?}: {:#}", project_id, event, start.elapsed(), e);
                                        } else {
                                            log::info!("{}: sent response event, waited {:?}: {}", project_id, start.elapsed(), event);
                                        }
                                    }
                                }
                            }
                        }
                    });
                },
                _ = self.cancellation_token.cancelled() => {
                    if let Err(error) = self.dispatcher.stop() {
                        log::error!("{}: failed to stop dispatcher: {:#}", project_id, error);
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}
