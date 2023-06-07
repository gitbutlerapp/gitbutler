mod dispatchers;
mod events;
mod handlers;

use std::path;

pub use events::Event;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{bookmarks, deltas, files, projects, search, sessions, users};

pub struct Watcher {
    project_id: String,
    dispatcher: dispatchers::Dispatcher,
    handler: handlers::Handler,
    cancellation_token: CancellationToken,
}

impl<'watcher> Watcher {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_data_dir: path::PathBuf,
        project: &projects::Project,
        project_store: projects::Storage,
        user_store: users::Storage,
        deltas_searcher: search::Searcher,
        cancellation_token: CancellationToken,
        events_sender: crate::events::Sender,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
        files_database: files::Database,
        bookmarks_database: bookmarks::Database,
    ) -> Result<Self> {
        Ok(Self {
            project_id: project.id.clone(),
            dispatcher: dispatchers::Dispatcher::new(project.id.clone(), project.path.clone()),
            handler: handlers::Handler::new(
                local_data_dir,
                project.id.clone(),
                project_store,
                user_store,
                deltas_searcher,
                events_sender,
                sessions_database,
                deltas_database,
                files_database,
                bookmarks_database,
            ),
            cancellation_token,
        })
    }

    pub async fn start(&self, mut proxy: mpsc::UnboundedReceiver<events::Event>) -> Result<()> {
        let (events_tx, mut events_rx) = mpsc::unbounded_channel();
        let dispatcher = self.dispatcher.clone();
        let project_id = self.project_id.clone();
        let etx = events_tx.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = dispatcher.start(etx.clone()).await {
                log::error!("{}: failed to start dispatcher: {:#}", project_id, e);
            }
        });

        loop {
            tokio::select! {
                Some(event) = proxy.recv() => {
                    if let Err(e) = events_tx.send(event) {
                        log::error!("{}: failed to post event: {:#}", self.project_id, e);
                    }
                },
                Some(event) = events_rx.recv() => {
                    let project_id = self.project_id.clone();
                    let handler = self.handler.clone();
                    let events_tx = events_tx.clone();
                    tauri::async_runtime::spawn(async move {
                        match handler.handle(event).await {
                            Ok(events) => {
                                for event in events {
                                    if let Err(e) = events_tx.send(event) {
                                        log::error!("{}: failed to post event: {:#}", project_id, e);
                                    }
                                }
                            },
                            Err(err) => log::error!("{}: failed to handle event: {:#}", project_id, err),
                        }
                    });
                },
                _ = self.cancellation_token.cancelled() => {
                    if let Err(e) = self.dispatcher.stop() {
                        log::error!("{}: failed to stop dispatcher: {:#}", self.project_id, e);
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}
