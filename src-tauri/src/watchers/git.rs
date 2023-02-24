use crate::{events, projects, sessions, users};
use anyhow::{Context, Result};
use git2::Repository;
use std::{
    sync::mpsc,
    thread,
    time::{Duration, SystemTime},
};

const FIVE_MINUTES: u128 = Duration::new(5 * 60, 0).as_millis();
const ONE_HOUR: u128 = Duration::new(60 * 60, 0).as_millis();

#[derive(Debug, Clone)]
pub struct GitWatcher {
    projects_storage: projects::Storage,
    users_storage: users::Storage,
}

impl GitWatcher {
    pub fn new(projects_storage: projects::Storage, users_storage: users::Storage) -> Self {
        Self {
            projects_storage,
            users_storage,
        }
    }

    pub fn watch(
        &self,
        sender: mpsc::Sender<events::Event>,
        project: projects::Project,
    ) -> Result<()> {
        log::info!("Watching git for {}", project.path);

        let shared_self = std::sync::Arc::new(self.clone());
        let self_copy = shared_self.clone();
        let project_id = project.id;

        thread::spawn(move || loop {
            let local_self = &self_copy;

            let project = local_self.projects_storage.get_project(&project_id);
            if project.is_err() {
                log::error!(
                    "Error while getting project {} for git watcher: {:#}",
                    project_id,
                    project.err().unwrap()
                );
                continue;
            };
            let project = project.unwrap();

            if project.is_none() {
                log::error!("Project {} not found", project_id);
                continue;
            }
            let project = project.unwrap();

            log::info!("Checking for session to commit in {}", project.path);

            let user = local_self.users_storage.get();
            if user.is_err() {
                log::error!(
                    "Error while getting user for git watcher: {:#}",
                    user.err().unwrap()
                );
                continue;
            };
            let user = user.unwrap();

            match local_self.check_for_changes(&project, &user) {
                Ok(Some(session)) => {
                    match sender.send(events::Event::session(&project, &session)) {
                        Err(e) => log::error!("filed to send session event: {:#}", e),
                        Ok(_) => {}
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    log::error!(
                        "Error while checking {} for changes: {:#}",
                        project.path,
                        error
                    );
                }
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
        &self,
        project: &projects::Project,
        user: &Option<users::User>,
    ) -> Result<Option<sessions::Session>> {
        let repo = git2::Repository::open(project.path.clone())?;
        match session_to_commit(&repo, project)
            .with_context(|| "Error while checking for session to commit")?
        {
            None => Ok(None),
            Some(mut session) => {
                session
                    .flush(&repo, user, project)
                    .with_context(|| "Error while flushing session")?;
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
    match sessions::Session::current(repo, project)? {
        None => {
            log::debug!(
                "No current session to commit for {}",
                repo.workdir().unwrap().display()
            );
            Ok(None)
        }
        Some(current_session) => {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let elapsed_last = now - current_session.meta.last_timestamp_ms;
            let elapsed_start = now - current_session.meta.start_timestamp_ms;

            if (elapsed_last > FIVE_MINUTES) || (elapsed_start > ONE_HOUR) {
                Ok(Some(current_session))
            } else {
                log::debug!(
                    "Not ready to commit {} yet. ({} seconds elapsed, {} seconds since start)",
                    repo.workdir().unwrap().display(),
                    elapsed_last / 1000,
                    elapsed_start / 1000
                );
                Ok(None)
            }
        }
    }
}
