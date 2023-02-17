use crate::{butler, events, projects, sessions, users};
use anyhow::Result;
use git2::Repository;
use std::{
    thread,
    time::{Duration, SystemTime},
};

const FIVE_MINUTES: u64 = Duration::new(5 * 60, 0).as_secs();
const ONE_HOUR: u64 = Duration::new(60 * 60, 0).as_secs();

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

    pub fn watch(&self, window: tauri::Window, project_id: String) -> Result<()> {
        self.init(project_id.clone())?;

        let shared_self = std::sync::Arc::new(self.clone());
        let self_copy = shared_self.clone();

        thread::spawn(move || loop {
            let local_self = &self_copy;

            let project = local_self.projects_storage.get_project(&project_id);
            if project.is_err() {
                log::error!(
                    "Error while getting project {} for git watcher: {}",
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

            let user = local_self.users_storage.get();
            if user.is_err() {
                log::error!(
                    "Error while getting user for git watcher: {}",
                    user.err().unwrap()
                );
                continue;
            };
            let user = user.unwrap();

            match local_self.check_for_changes(&project, &user) {
                Ok(Some(session)) => {
                    events::session(&window, &project, &session);
                }
                Ok(None) => {}
                Err(error) => {
                    log::error!(
                        "Error while checking {} for changes: {}",
                        project.path,
                        error
                    );
                }
            }

            thread::sleep(Duration::from_secs(10));
        });

        Ok(())
    }

    // if the repo is new to gitbutler, we need to make sure that all the files are tracked by gitbutler
    fn init(&self, project_id: String) -> Result<()> {
        let project = match self.projects_storage.get_project(&project_id)? {
            Some(project) => Ok(project),
            None => Err(anyhow::anyhow!("Project {} not found", project_id)),
        }?;
        let repo = git2::Repository::open(&project.path)?;
        let user = self.users_storage.get()?;

        if repo
            .revparse_single(format!("refs/{}/current", butler::refname()).as_str())
            .is_err()
        {
            // make sure all the files are tracked by gitbutler session
            sessions::Session::from_head(&repo)?;
            sessions::flush_current_session(&repo, &user, &project)?;
        };

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
        if ready_to_commit(&repo)? {
            Ok(Some(sessions::flush_current_session(
                &repo, &user, &project,
            )?))
        } else {
            Ok(None)
        }
    }
}

// make sure that the .git/gb/session directory exists (a session is in progress)
// and that there has been no activity in the last 5 minutes (the session appears to be over)
// and the start was at most an hour ago
fn ready_to_commit(repo: &Repository) -> Result<bool> {
    if let Some(current_session) = sessions::Session::current(repo)? {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u64;

        let elapsed_last = now - current_session.meta.last_ts;
        let elapsed_start = now - current_session.meta.start_ts;

        // TODO: uncomment
        if (elapsed_last > FIVE_MINUTES) || (elapsed_start > ONE_HOUR) {
            Ok(true)
        } else {
            log::debug!(
                "Not ready to commit {} yet. ({} seconds elapsed, {} seconds since start)",
                repo.workdir().unwrap().display(),
                elapsed_last,
                elapsed_start
            );
            Ok(false)
        }
    } else {
        log::debug!(
            "No current session for {}",
            repo.workdir().unwrap().display()
        );
        Ok(false)
    }
}
