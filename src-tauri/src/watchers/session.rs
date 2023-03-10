use crate::{events, projects, search, sessions, users};
use anyhow::{Context, Result};
use git2::Repository;
use std::{
    sync::mpsc,
    thread,
    time::{Duration, SystemTime},
};

const FIVE_MINUTES: u128 = Duration::new(5 * 60, 0).as_millis();
const ONE_HOUR: u128 = Duration::new(60 * 60, 0).as_millis();

#[derive(Clone)]
pub struct SessionWatcher {
    projects_storage: projects::Storage,
    users_storage: users::Storage,
    deltas_searcher: search::Deltas,
}

impl<'a> SessionWatcher {
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

    fn run(&mut self, project_id: &str, sender: mpsc::Sender<events::Event>) -> Result<()> {
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

                match self.check_for_changes(&project, &user)? {
                    Some(session) => {
                        sender
                            .send(events::Event::session(&project, &session))
                            .with_context(|| {
                                format!("{}: failed to send session event", project.id)
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
        sender: mpsc::Sender<events::Event>,
        project: projects::Project,
    ) -> Result<()> {
        log::info!("{}: watching sessions in {}", project.id, project.path);

        let shared_self = self.clone();
        let mut self_copy = shared_self.clone();
        let project_id = project.id;

        tauri::async_runtime::spawn_blocking(move || loop {
            let local_self = &mut self_copy;

            if let Err(e) = local_self.run(&project_id, sender.clone()) {
                log::error!("{}: error while running git watcher: {:#}", project_id, e);
            }

            thread::sleep(Duration::from_secs(10));
        });

        Ok(())
    }

    // main thing called in a loop to check for changes and write our custom commit data
    // it will commit only if there are changes and the session is either idle for 5 minutes or is over an hour old
    // or if the repository is new to gitbutler.
    // currently it looks at every file in the wd, but we should probably just look at the ones that have changed when we're certain we can get everything
    // - however, it does compare to the git index so we don't actually have to read the contents of every file, so maybe it's not too slow unless in huge repos
    // - also only does the file comparison on commit, so it's not too bad
    //
    // returns a commited session if created
    fn check_for_changes(
        &mut self,
        project: &projects::Project,
        user: &Option<users::User>,
    ) -> Result<Option<sessions::Session>> {
        let repo = git2::Repository::open(project.path.clone())
            .with_context(|| format!("{}: failed to open repository", project.id))?;
        match session_to_commit(&repo, project)
            .with_context(|| "failed to check for session to comit")?
        {
            None => Ok(None),
            Some(mut session) => {
                session
                    .flush(&repo, user, project)
                    .with_context(|| format!("failed to flush session {}", session.id))?;
                self.deltas_searcher
                    .index_session(&repo, &project, &session)
                    .with_context(|| format!("failed to index session {}", session.id))?;
                Ok(Some(session))
            }
        }
    }
}

// make sure that the .git/gb/session directory exists (a session is in progress)
// and that there has been no activity in the last 5 minutes (the session appears to be over)
// and the start was at most an hour ago
fn session_to_commit(
    repo: &Repository,
    project: &projects::Project,
) -> Result<Option<sessions::Session>> {
    match sessions::Session::current(repo, project)
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
