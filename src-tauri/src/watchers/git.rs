use crate::{butler, events, projects::Project, sessions};
use git2::Repository;
use std::{
    thread,
    time::{Duration, SystemTime},
};

#[derive(Debug)]
pub enum WatchError {
    GitError(git2::Error),
    IOError(std::io::Error),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::GitError(e) => write!(f, "Git error: {}", e),
            WatchError::IOError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl From<git2::Error> for WatchError {
    fn from(error: git2::Error) -> Self {
        Self::GitError(error)
    }
}

impl From<std::io::Error> for WatchError {
    fn from(error: std::io::Error) -> Self {
        Self::IOError(error)
    }
}

const FIVE_MINUTES: u64 = Duration::new(5 * 60, 0).as_secs();
const ONE_HOUR: u64 = Duration::new(60 * 60, 0).as_secs();

pub fn watch<R: tauri::Runtime>(
    window: tauri::Window<R>,
    project: Project,
) -> Result<(), WatchError> {
    let repo = git2::Repository::open(&project.path)?;
    thread::spawn(move || loop {
        match repo.revparse_single(format!("refs/{}/current", butler::refname()).as_str()) {
            Ok(_) => {}
            Err(_) => {
                // make sure all the files are tracked by gitbutler session
                if sessions::Session::from_head(&repo).is_err() {
                    log::error!(
                        "Error while creating session for {}",
                        repo.workdir().unwrap().display()
                    );
                }
                if sessions::flush_current_session(&repo).is_err() {
                    log::error!(
                        "Error while flushing current session for {}",
                        repo.workdir().unwrap().display()
                    );
                }
            }
        }

        match check_for_changes(&repo) {
            Ok(Some(session)) => {
                events::session(&window, &project, &session);
            }
            Ok(None) => {}
            Err(error) => {
                log::error!(
                    "Error while checking {} for changes: {}",
                    repo.workdir().unwrap().display(),
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
    repo: &Repository,
) -> Result<Option<sessions::Session>, Box<dyn std::error::Error>> {
    if ready_to_commit(repo)? {
        Ok(Some(sessions::flush_current_session(repo)?))
    } else {
        Ok(None)
    }
}

// make sure that the .git/gb/session directory exists (a session is in progress)
// and that there has been no activity in the last 5 minutes (the session appears to be over)
// and the start was at most an hour ago
fn ready_to_commit(repo: &Repository) -> Result<bool, Box<dyn std::error::Error>> {
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
