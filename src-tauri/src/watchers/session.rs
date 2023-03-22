use crate::{deltas, events, projects, search, sessions, users};
use anyhow::{Context, Result};
use std::{sync::Arc, time::SystemTime};
use tokio::time::{sleep, Duration};

const FIVE_MINUTES: u128 = Duration::new(5 * 60, 0).as_millis();
const ONE_HOUR: u128 = Duration::new(60 * 60, 0).as_millis();

#[derive(Clone)]
pub struct SessionWatcher {
    projects_storage: projects::Storage,
    users_storage: users::Storage,
    deltas_searcher: search::Deltas,
}

impl SessionWatcher {
    pub fn new(
        projects_storage: projects::Storage,
        users_storage: users::Storage,
        deltas_searcher: search::Deltas,
    ) -> Self {
        Self {
            projects_storage,
            users_storage,
            deltas_searcher,
        }
    }

    async fn run(
        &mut self,
        project_id: &str,
        sender: tokio::sync::mpsc::Sender<events::Event>,
        fslock: Arc<tokio::sync::Mutex<fslock::LockFile>>,
        deltas_storage: &deltas::Store,
        sessions_storage: &sessions::Store,
    ) -> Result<()> {
        match self
            .projects_storage
            .get_project(&project_id)
            .with_context(|| format!("{}: failed to get project", project_id))?
        {
            Some(project) => {
                let user = self
                    .users_storage
                    .get()
                    .with_context(|| format!("{}: failed to get user", project.id))?;

                match session_to_commit(&project, &sessions_storage)
                    .with_context(|| "failed to check for session to comit")?
                {
                    Some(mut session) => {
                        let mut fslock = fslock.lock().await;
                        log::debug!("{}: locking", project.id);
                        fslock.lock().unwrap();
                        log::debug!("{}: locked", project.id);

                        session = sessions_storage
                            .flush(&session, user)
                            .with_context(|| format!("failed to flush session {}", session.id))?;

                        log::debug!("{}: unlocking", project.id);
                        fslock.unlock().unwrap();
                        log::debug!("{}: unlocked", project.id);

                        self.deltas_searcher
                            .index_session(&project, &session, &deltas_storage, &sessions_storage)
                            .with_context(|| format!("failed to index session {}", session.id))?;

                        sender
                            .send(events::Event::session(&project, &session))
                            .await
                            .with_context(|| {
                                format!("failed to send session {} event", session.id)
                            })?;

                        Ok(())
                    }
                    None => Ok(()),
                }
            }
            None => Err(anyhow::anyhow!("project not found")),
        }
    }

    pub fn watch(
        &self,
        sender: tokio::sync::mpsc::Sender<events::Event>,
        project: projects::Project,
        fslock: Arc<tokio::sync::Mutex<fslock::LockFile>>,
        deltas_storage: &deltas::Store,
        sessions_storage: &sessions::Store,
    ) -> Result<()> {
        log::info!("{}: watching sessions in {}", project.id, project.path);

        let shared_self = self.clone();
        let mut self_copy = shared_self.clone();
        let project_id = project.id;

        let shared_storage = deltas_storage.clone();
        let shared_sessions_storage = sessions_storage.clone();
        tauri::async_runtime::spawn(async move {
            let local_self = &mut self_copy;
            let deltas_storage = shared_storage.clone();
            let sessions_storage = shared_sessions_storage.clone();

            loop {
                if let Err(e) = local_self
                    .run(
                        &project_id,
                        sender.clone(),
                        fslock.clone(),
                        &deltas_storage,
                        &sessions_storage,
                    )
                    .await
                {
                    log::error!("{}: error while running git watcher: {:#}", project_id, e);
                }

                sleep(Duration::from_secs(10)).await;
            }
        });

        Ok(())
    }
}

// make sure that the .git/gb/session directory exists (a session is in progress)
// and that there has been no activity in the last 5 minutes (the session appears to be over)
// and the start was at most an hour ago
fn session_to_commit(
    project: &projects::Project,
    sessions_store: &sessions::Store,
) -> Result<Option<sessions::Session>> {
    match sessions_store
        .get_current()
        .with_context(|| format!("{}: failed to get current session", project.id))?
    {
        None => Ok(None),
        Some(current_session) => {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let elapsed_last = now - current_session.meta.last_timestamp_ms;
            let elapsed_start = now - current_session.meta.start_timestamp_ms;

            if (elapsed_last > FIVE_MINUTES) || (elapsed_start > ONE_HOUR) {
                log::info!(
                    "{}: ready to commit {} ({} seconds elapsed, {} seconds since start)",
                    project.id,
                    project.path,
                    elapsed_last / 1000,
                    elapsed_start / 1000
                );
                Ok(Some(current_session))
            } else {
                log::debug!(
                    "{}: not ready to commit {} yet. ({} seconds elapsed, {} seconds since start)",
                    project.id,
                    project.path,
                    elapsed_last / 1000,
                    elapsed_start / 1000
                );
                Ok(None)
            }
        }
    }
}
