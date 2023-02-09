use std::path::Path;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    // timestamp of when the session was created
    pub start_ts: u64,
    // timestamp of when the session was last active
    pub last_ts: u64,
    // session branch name
    pub branch: String,
    // session commit hash
    pub commit: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub meta: Meta,
}

impl Session {
    pub fn from_commit(repo: &git2::Repository, commit: &git2::Commit) -> Result<Self, Error> {
        let tree = commit.tree().map_err(|err| Error {
            cause: err.into(),
            message: "Error while getting commit tree".to_string(),
        })?;

        let start = read_as_string(repo, &tree, Path::new("session/meta/start"))?
            .parse::<u64>()
            .map_err(|err| Error {
                cause: ErrorCause::ParseIntError(err),
                message: "Error while parsing start file".to_string(),
            })?;

        let last = read_as_string(repo, &tree, Path::new("session/meta/last"))?
            .parse::<u64>()
            .map_err(|err| Error {
                cause: ErrorCause::ParseIntError(err),
                message: "Error while parsing last file".to_string(),
            })?;

        let branch = read_as_string(repo, &tree, Path::new("session/meta/branch"))?;
        let commit = read_as_string(repo, &tree, Path::new("session/meta/commit"))?;

        Ok(Session {
            meta: Meta {
                start_ts: start,
                last_ts: last,
                branch,
                commit,
            },
        })
    }
}

#[derive(Debug)]
pub struct Error {
    pub cause: ErrorCause,
    pub message: String,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.cause {
            ErrorCause::IOError(err) => Some(err),
            ErrorCause::ParseIntError(err) => Some(err),
            ErrorCause::SessionExistsError => Some(self),
            ErrorCause::SessionDoesNotExistError => Some(self),
            ErrorCause::GitError(err) => Some(err),
            ErrorCause::ParseUtf8Error(err) => Some(err),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.cause {
            ErrorCause::IOError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::ParseIntError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::SessionExistsError => write!(f, "{}", self.message),
            ErrorCause::SessionDoesNotExistError => write!(f, "{}", self.message),
            ErrorCause::GitError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::ParseUtf8Error(ref e) => write!(f, "{}: {}", self.message, e),
        }
    }
}

#[derive(Debug)]
pub enum ErrorCause {
    IOError(std::io::Error),
    ParseIntError(std::num::ParseIntError),
    GitError(git2::Error),
    SessionExistsError,
    SessionDoesNotExistError,
    ParseUtf8Error(std::string::FromUtf8Error),
}

impl From<std::string::FromUtf8Error> for ErrorCause {
    fn from(err: std::string::FromUtf8Error) -> Self {
        ErrorCause::ParseUtf8Error(err)
    }
}

impl From<git2::Error> for ErrorCause {
    fn from(err: git2::Error) -> Self {
        ErrorCause::GitError(err)
    }
}

impl From<std::io::Error> for ErrorCause {
    fn from(err: std::io::Error) -> Self {
        ErrorCause::IOError(err)
    }
}

impl From<std::num::ParseIntError> for ErrorCause {
    fn from(err: std::num::ParseIntError) -> Self {
        ErrorCause::ParseIntError(err)
    }
}

fn write_current_session(session_path: &Path, session: &Session) -> Result<(), Error> {
    let meta_path = session_path.join("meta");

    std::fs::create_dir_all(meta_path.clone()).map_err(|err| Error {
        cause: err.into(),
        message: "Failed to create session directory".to_string(),
    })?;

    let start_path = meta_path.join("start");
    std::fs::write(start_path, session.meta.start_ts.to_string()).map_err(|err| Error {
        cause: err.into(),
        message: "Failed to write session start".to_string(),
    })?;

    let last_path = meta_path.join("last");
    std::fs::write(last_path, session.meta.last_ts.to_string()).map_err(|err| Error {
        cause: err.into(),
        message: "Failed to write session last".to_string(),
    })?;

    let branch_path = meta_path.join("branch");
    std::fs::write(branch_path, session.meta.branch.clone()).map_err(|err| Error {
        cause: err.into(),
        message: "Failed to write session branch".to_string(),
    })?;

    let commit_path = meta_path.join("commit");
    std::fs::write(commit_path, session.meta.commit.clone()).map_err(|err| Error {
        cause: err.into(),
        message: "Failed to write session commit".to_string(),
    })?;

    Ok(())
}

pub fn update_current_session(repo: &git2::Repository, session: &Session) -> Result<(), Error> {
    log::debug!("{}: Updating current session", repo.path().display());
    let session_path = repo.path().join("gb/session");
    if session_path.exists() {
        write_current_session(&session_path, session)
    } else {
        Err(Error {
            cause: ErrorCause::SessionDoesNotExistError,
            message: "Session does not exist".to_string(),
        })
    }
}

pub fn create_current_session(repo: &git2::Repository, session: &Session) -> Result<(), Error> {
    log::debug!("{}: Creating current session", repo.path().display());
    let session_path = repo.path().join("gb/session");
    if session_path.exists() {
        Err(Error {
            cause: ErrorCause::SessionExistsError,
            message: "Session already exists".to_string(),
        })
    } else {
        write_current_session(&session_path, session)
    }
}

pub fn delete_current_session(repo: &git2::Repository) -> Result<(), std::io::Error> {
    log::debug!("{}: Deleting current session", repo.path().display());
    let session_path = repo.path().join("gb/session");
    if session_path.exists() {
        std::fs::remove_dir_all(session_path)?;
    }
    Ok(())
}

pub fn get_current_session(repo: &git2::Repository) -> Result<Option<Session>, Error> {
    let session_path = repo.path().join("gb/session");
    if !session_path.exists() {
        return Ok(None);
    }

    let meta_path = session_path.join("meta");

    let start_path = meta_path.join("start");
    let start_ts = std::fs::read_to_string(start_path)
        .map_err(|e| Error {
            cause: e.into(),
            message: "failed to read session start".to_string(),
        })?
        .parse::<u64>()
        .map_err(|e| Error {
            cause: e.into(),
            message: "failed to parse session start".to_string(),
        })?;

    let last_path = meta_path.join("last");
    let last_ts = std::fs::read_to_string(last_path)
        .map_err(|e| Error {
            cause: e.into(),
            message: "failed to read session last".to_string(),
        })?
        .parse::<u64>()
        .map_err(|e| Error {
            cause: e.into(),
            message: "failed to parse session last".to_string(),
        })?;

    let branch_path = meta_path.join("branch");
    let branch = std::fs::read_to_string(branch_path).map_err(|e| Error {
        cause: e.into(),
        message: "failed to read branch".to_string(),
    })?;

    let commit_path = meta_path.join("commit");
    let commit = std::fs::read_to_string(commit_path).map_err(|e| Error {
        cause: e.into(),
        message: "failed to read commit".to_string(),
    })?;

    Ok(Some(Session {
        meta: Meta {
            start_ts,
            last_ts,
            branch,
            commit,
        },
    }))
}

pub fn list_sessions(repo: &git2::Repository) -> Result<Vec<Session>, Error> {
    match repo.revparse_single("refs/gitbutler/current") {
        Err(_) => Ok(vec![]),
        Ok(object) => {
            let gitbutler_head = repo.find_commit(object.id()).map_err(|err| Error {
                cause: err.into(),
                message: "Failed to find gitbutler head".to_string(),
            })?;
            // list all commits from gitbutler head to the first commit
            let mut walker = repo.revwalk().map_err(|err| Error {
                cause: err.into(),
                message: "Failed to create revwalk".to_string(),
            })?;
            walker.push(gitbutler_head.id()).map_err(|err| Error {
                cause: err.into(),
                message: "Failed to push gitbutler head".to_string(),
            })?;
            walker.set_sorting(git2::Sort::TIME).map_err(|err| Error {
                cause: err.into(),
                message: "Failed to set sorting".to_string(),
            })?;

            let mut sessions: Vec<Session> = vec![];
            for id in walker {
                let id = id.map_err(|err| Error {
                    cause: err.into(),
                    message: "Failed to get commit id".to_string(),
                })?;
                let commit = repo.find_commit(id).map_err(|err| Error {
                    cause: err.into(),
                    message: "Failed to find commit".to_string(),
                })?;
                sessions.push(Session::from_commit(repo, &commit)?);
            }
            Ok(sessions)
        }
    }
}

fn read_as_string(
    repo: &git2::Repository,
    tree: &git2::Tree,
    path: &Path,
) -> Result<String, Error> {
    match tree.get_path(path) {
        Ok(tree_entry) => {
            let blob = tree_entry
                .to_object(repo)
                .map_err(|err| Error {
                    cause: err.into(),
                    message: "Error while getting tree entry object".to_string(),
                })?
                .into_blob()
                .unwrap();
            let contents = String::from_utf8(blob.content().to_vec()).map_err(|err| Error {
                cause: err.into(),
                message: "Error while parsing blob as utf8".to_string(),
            })?;
            Ok(contents)
        }
        Err(err) => {
            return Err(Error {
                cause: err.into(),
                message: "Error while getting tree entry".to_string(),
            })
        }
    }
}
