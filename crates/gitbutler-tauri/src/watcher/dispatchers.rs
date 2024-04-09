mod file_change;

use std::path;

use anyhow::{Context, Result};
use gitbutler_core::projects::ProjectId;
use tokio::{
    select,
    sync::mpsc::{channel, Receiver},
    task,
};
use tokio_util::sync::CancellationToken;

use super::events;

#[derive(Clone)]
pub struct Dispatcher {
    file_change_dispatcher: file_change::Dispatcher,
    cancellation_token: CancellationToken,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("{0} not found")]
    PathNotFound(path::PathBuf),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            file_change_dispatcher: file_change::Dispatcher::new(),
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn stop(&self) {
        self.file_change_dispatcher.stop();
    }

    pub fn run<P: AsRef<path::Path>>(
        &self,
        project_id: &ProjectId,
        path: P,
    ) -> Result<Receiver<events::Event>, RunError> {
        let path = path.as_ref();

        let mut file_change_rx = match self.file_change_dispatcher.run(project_id, path) {
            Ok(file_change_rx) => Ok(file_change_rx),
            Err(file_change::RunError::PathNotFound(path)) => Err(RunError::PathNotFound(path)),
            Err(error) => Err(error).context("failed to run file change dispatcher")?,
        }?;

        let (tx, rx) = channel(1);
        let project_id = *project_id;
        let cancellation_token = self.cancellation_token.clone();
        task::spawn(async move {
            loop {
                select! {
                    () = cancellation_token.cancelled() => {
                        break;
                    }
                    Some(event) = file_change_rx.recv() => {
                        if let Err(error) = tx.send(event).await {
                            tracing::error!(%project_id, ?error,"failed to send file change");
                        }
                    }
                }
            }
            tracing::debug!(%project_id, "dispatcher stopped");
        });

        Ok(rx)
    }
}
