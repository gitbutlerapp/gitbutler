mod dispatchers;
mod events;
mod handlers;

use std::{path, sync::Arc};

pub use events::Event;

use anyhow::{Context, Result};
use tokio::{
    spawn,
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        Mutex,
    },
};
use tokio_util::sync::CancellationToken;

use crate::{bookmarks, deltas, files, projects, search, sessions, users};

#[derive(Clone)]
pub struct Watcher {
    inner: Arc<InnerWatcher>,
}

impl Watcher {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_data_dir: &path::Path,
        project: &projects::Project,
        project_store: &projects::Storage,
        user_store: &users::Storage,
        deltas_searcher: &search::Searcher,
        events_sender: &crate::events::Sender,
        sessions_database: &sessions::Database,
        deltas_database: &deltas::Database,
        files_database: &files::Database,
        bookmarks_database: &bookmarks::Database,
    ) -> Self {
        Self {
            inner: Arc::new(InnerWatcher::new(
                local_data_dir,
                project,
                project_store,
                user_store,
                deltas_searcher,
                events_sender,
                sessions_database,
                deltas_database,
                files_database,
                bookmarks_database,
            )),
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.inner.stop()
    }

    pub async fn post(&self, event: Event) -> Result<()> {
        self.inner.post(event).await
    }

    pub async fn run(&self) -> Result<()> {
        self.inner.run().await
    }
}

struct InnerWatcher {
    project_id: String,
    dispatcher: dispatchers::Dispatcher,
    handler: handlers::Handler,
    cancellation_token: CancellationToken,

    proxy_tx: Arc<Mutex<Option<UnboundedSender<Event>>>>,
}

impl<'watcher> InnerWatcher {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_data_dir: &path::Path,
        project: &projects::Project,
        project_store: &projects::Storage,
        user_store: &users::Storage,
        deltas_searcher: &search::Searcher,
        events_sender: &crate::events::Sender,
        sessions_database: &sessions::Database,
        deltas_database: &deltas::Database,
        files_database: &files::Database,
        bookmarks_database: &bookmarks::Database,
    ) -> Self {
        Self {
            project_id: project.id.clone(),
            dispatcher: dispatchers::Dispatcher::new(project),
            handler: handlers::Handler::new(
                local_data_dir,
                &project.id,
                project_store,
                user_store,
                deltas_searcher,
                events_sender,
                sessions_database,
                deltas_database,
                files_database,
                bookmarks_database,
            ),
            cancellation_token: CancellationToken::new(),
            proxy_tx: Arc::new(Mutex::new(None)),
        }
    }

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

    pub async fn run(&self) -> Result<()> {
        let (tx, mut rx) = unbounded_channel();
        self.proxy_tx.lock().await.replace(tx.clone());

        spawn({
            let dispatcher = self.dispatcher.clone();
            let project_id = self.project_id.clone();
            let tx = tx.clone();
            async move {
                let mut dispatcher_rx = dispatcher.run().expect("failed to start dispatcher");
                while let Some(event) = dispatcher_rx.recv().await {
                    log::warn!("{}: dispatcher event: {}", project_id, event);
                    if let Err(e) = tx.send(event) {
                        log::error!("{}: failed to post event: {:#}", project_id, e);
                    }
                }
            }
        });

        tx.send(Event::IndexAll).context("failed to send event")?;

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    log::warn!("{}: handling: {}", self.project_id, event);
                    let handler = self.handler.clone();
                    let project_id = self.project_id.clone();
                    let tx = tx.clone();
                    spawn(
                        async move {
                         match handler.handle(event).await {
                            Ok(events) => {
                                for event in events {
                                    if let Err(e) = tx.send(event) {
                                        log::error!("{}: failed to post event: {:#}", project_id, e);
                                    }
                                }
                            },
                            Err(err) => log::error!("{}: failed to handle event: {:#}", project_id, err),
                        }
                    });
                },
                _ = self.cancellation_token.cancelled() => {
                    if let Err(error) = self.dispatcher.stop() {
                        log::error!("{}: failed to stop dispatcher: {:#}", self.project_id, error);
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}
