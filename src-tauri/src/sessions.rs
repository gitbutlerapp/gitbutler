use std::path::Path;

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

pub struct Session {
    pub meta: Meta,
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
        }
    }
}

#[derive(Debug)]
pub enum ErrorCause {
    IOError(std::io::Error),
    ParseIntError(std::num::ParseIntError),
    SessionExistsError,
    SessionDoesNotExistError,
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

pub fn update_current_session(project_path: &Path, session: &Session) -> Result<(), Error> {
    let session_path = project_path.join(".git/gb/session");
    if session_path.exists() {
        write_current_session(&session_path, session)
    } else {
        Err(Error {
            cause: ErrorCause::SessionDoesNotExistError,
            message: "Session does not exist".to_string(),
        })
    }
}

pub fn create_current_session(project_path: &Path, session: &Session) -> Result<(), Error> {
    log::debug!("{}: Creating current session", project_path.display());
    let session_path = project_path.join(".git/gb/session");
    if session_path.exists() {
        Err(Error {
            cause: ErrorCause::SessionExistsError,
            message: "Session already exists".to_string(),
        })
    } else {
        write_current_session(&session_path, session)
    }
}

pub fn delete_current_session(project_path: &Path) -> Result<(), std::io::Error> {
    log::debug!("{}: Deleting current session", project_path.display());
    let session_path = project_path.join(".git/gb/session");
    if session_path.exists() {
        std::fs::remove_dir_all(session_path)?;
    }
    Ok(())
}

pub fn get_current_session(project_path: &Path) -> Result<Option<Session>, Error> {
    let session_path = project_path.join(".git/gb/session");
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
