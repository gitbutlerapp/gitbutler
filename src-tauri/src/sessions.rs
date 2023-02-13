use crate::butler;
use serde::Serialize;
use std::{collections::HashMap, path::Path};

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    // if hash is not set, the session is not saved aka current
    pub hash: Option<String>,
    pub meta: Meta,
    pub activity: Vec<Activity>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    #[serde(rename = "type")]
    pub activity_type: String,
    pub timestamp: u64,
    pub message: String,
}

fn parse_reflog_line(line: &str) -> Result<Activity, Error> {
    match line.split("\t").collect::<Vec<&str>>()[..] {
        [meta, message] => {
            let meta_parts = meta.split_whitespace().collect::<Vec<&str>>();
            let timestamp = meta_parts[meta_parts.len() - 2]
                .parse::<u64>()
                .map_err(|err| Error {
                    cause: ErrorCause::ParseIntError(err),
                    message: "Error while parsing reflog timestamp".to_string(),
                })?;

            match message.split(": ").collect::<Vec<&str>>()[..] {
                [entry_type, msg] => Ok(Activity {
                    activity_type: entry_type.to_string(),
                    message: msg.to_string(),
                    timestamp,
                }),
                _ => Err(Error {
                    cause: ErrorCause::ParseActivityError,
                    message: "Error parsing reflog activity message".to_string(),
                }),
            }
        }
        _ => Err(Error {
            cause: ErrorCause::ParseActivityError,
            message: "Error while parsing reflog activity".to_string(),
        }),
    }
}

impl Session {
    pub fn current(repo: &git2::Repository) -> Result<Option<Self>, Error> {
        let session_path = repo.path().join(butler::dir()).join("session");
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

        let reflog = std::fs::read_to_string(repo.path().join("logs/HEAD")).map_err(|e| Error {
            cause: e.into(),
            message: "failed to read reflog".to_string(),
        })?;
        let activity = reflog
            .lines()
            .filter_map(|line| parse_reflog_line(line).ok())
            .filter(|activity| activity.timestamp >= start_ts)
            .collect::<Vec<Activity>>();

        let id_path = meta_path.join("id");
        let id = std::fs::read_to_string(id_path).map_err(|e| Error {
            cause: e.into(),
            message: "failed to read session id".to_string(),
        })?;

        Ok(Some(Session {
            id,
            hash: None,
            activity,
            meta: Meta {
                start_ts,
                last_ts,
                branch,
                commit,
            },
        }))
    }

    pub fn from_commit(repo: &git2::Repository, commit: &git2::Commit) -> Result<Self, Error> {
        let tree = commit.tree().map_err(|err| Error {
            cause: err.into(),
            message: "Error while getting commit tree".to_string(),
        })?;

        let id =
            read_as_string(repo, &tree, Path::new("session/meta/id")).map_err(|err| Error {
                cause: err.cause,
                message: format!("Error while reading session id: {}", err.message),
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

        let reflog = read_as_string(repo, &tree, Path::new("logs/HEAD"))?;
        let activity = reflog
            .lines()
            .filter_map(|line| parse_reflog_line(line).ok())
            .filter(|activity| activity.timestamp >= start)
            .collect::<Vec<Activity>>();

        Ok(Session {
            id,
            hash: Some(commit.id().to_string()),
            meta: Meta {
                start_ts: start,
                last_ts: last,
                branch: read_as_string(repo, &tree, Path::new("session/meta/branch"))?,
                commit: read_as_string(repo, &tree, Path::new("session/meta/commit"))?,
            },
            activity,
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
            ErrorCause::ParseActivityError => Some(self),
            ErrorCause::SessionIsNotCurrentError => Some(self),
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
            ErrorCause::SessionIsNotCurrentError => write!(f, "{}", self.message),
            ErrorCause::GitError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::ParseUtf8Error(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::ParseActivityError => write!(f, "{}", self.message),
        }
    }
}

#[derive(Debug)]
pub enum ErrorCause {
    IOError(std::io::Error),
    ParseIntError(std::num::ParseIntError),
    GitError(git2::Error),
    SessionExistsError,
    SessionIsNotCurrentError,
    SessionDoesNotExistError,
    ParseUtf8Error(std::string::FromUtf8Error),
    ParseActivityError,
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

fn write_session(session_path: &Path, session: &Session) -> Result<(), Error> {
    if session.hash.is_some() {
        return Err(Error {
            cause: ErrorCause::SessionIsNotCurrentError,
            message: "can only write current sessions (without hash)".to_string(),
        });
    }

    let meta_path = session_path.join("meta");

    std::fs::create_dir_all(meta_path.clone()).map_err(|err| Error {
        cause: err.into(),
        message: "Failed to create session directory".to_string(),
    })?;

    let id_path = meta_path.join("id");
    std::fs::write(id_path, session.id.clone()).map_err(|err| Error {
        cause: err.into(),
        message: "Failed to write session id".to_string(),
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

pub fn update_session(repo: &git2::Repository, session: &Session) -> Result<(), Error> {
    log::debug!("{}: Updating current session", repo.path().display());
    let session_path = repo.path().join(butler::dir()).join("session");
    if session_path.exists() {
        write_session(&session_path, session)
    } else {
        Err(Error {
            cause: ErrorCause::SessionDoesNotExistError,
            message: "Session does not exist".to_string(),
        })
    }
}

pub fn create_session(repo: &git2::Repository, session: &Session) -> Result<(), Error> {
    log::debug!("{}: Creating current session", repo.path().display());
    let session_path = repo.path().join(butler::dir()).join("session");
    if session_path.exists() {
        Err(Error {
            cause: ErrorCause::SessionExistsError,
            message: "Session already exists".to_string(),
        })
    } else {
        write_session(&session_path, session)
    }
}

pub fn delete_session(repo: &git2::Repository) -> Result<(), std::io::Error> {
    log::debug!("{}: Deleting current session", repo.path().display());
    let session_path = repo.path().join(butler::dir()).join("session");
    if session_path.exists() {
        std::fs::remove_dir_all(session_path)?;
    }
    Ok(())
}

pub fn get(repo: &git2::Repository, id: &str) -> Result<Option<Session>, Error> {
    let list = list(repo)?;
    for session in list {
        if session.id == id {
            return Ok(Some(session));
        }
    }
    Ok(None)
}

pub fn list(repo: &git2::Repository) -> Result<Vec<Session>, Error> {
    let mut sessions = list_persistent(repo)?;
    if let Some(session) = Session::current(repo)? {
        sessions.push(session);
    }
    Ok(sessions)
}

fn list_persistent(repo: &git2::Repository) -> Result<Vec<Session>, Error> {
    match repo.revparse_single(format!("refs/{}/current", butler::refname()).as_str()) {
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

// return a map of file name -> file content for the given session
pub fn list_files(
    repo: &git2::Repository,
    session_id: &str,
) -> Result<HashMap<String, String>, Error> {
    let session = match get(repo, session_id)? {
        Some(session) => session,
        None => Err(Error {
            message: format!("Could not find session {}", session_id),
            cause: ErrorCause::SessionDoesNotExistError,
        })?,
    };

    match session.hash {
        // if the session is committed, list the files from the session commit
        Some(hash) => list_session_files(repo, &hash),
        // if the session is not committed, list the files from the session base commit
        None => list_commit_files(repo, &session.meta.commit),
    }
}

fn list_commit_files(
    repo: &git2::Repository,
    repo_commit_hash: &str,
) -> Result<HashMap<String, String>, Error> {
    let commit_id = git2::Oid::from_str(repo_commit_hash).map_err(|e| Error {
        message: format!("Could not parse commit id {}", repo_commit_hash),
        cause: e.into(),
    })?;

    let commit = repo.find_commit(commit_id).map_err(|e| Error {
        message: format!("Could not find commit {}", commit_id),
        cause: e.into(),
    })?;

    let tree = commit.tree().map_err(|e| Error {
        message: format!("Could not get tree for commit {}", commit.id()),
        cause: e.into(),
    })?;

    let mut files = HashMap::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
        if entry.name().is_none() {
            return git2::TreeWalkResult::Ok;
        }
        let path = Path::new(root).join(entry.name().unwrap());
        let contents = read_as_string(repo, &tree, &path).unwrap();
        files.insert(path.to_str().unwrap().to_string(), contents);
        git2::TreeWalkResult::Ok
    })
    .map_err(|e| Error {
        message: format!("Could not walk tree for commit {}", commit.id()),
        cause: e.into(),
    })?;

    Ok(files)
}

fn list_session_files(
    repo: &git2::Repository,
    session_hash: &str,
) -> Result<HashMap<String, String>, Error> {
    let commit_id = git2::Oid::from_str(session_hash).map_err(|e| Error {
        message: format!("Could not parse commit id {}", session_hash),
        cause: e.into(),
    })?;

    let commit = repo.find_commit(commit_id).map_err(|e| Error {
        message: format!("Could not find commit {}", commit_id),
        cause: e.into(),
    })?;

    let tree = commit.tree().map_err(|e| Error {
        message: format!("Could not get tree for commit {}", commit.id()),
        cause: e.into(),
    })?;

    let mut files = HashMap::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
        if entry.name().is_none() {
            return git2::TreeWalkResult::Ok;
        }
        let entry_path = Path::new(root).join(entry.name().unwrap());
        if !entry_path.starts_with("session/wd") {
            return git2::TreeWalkResult::Ok;
        }
        let blob = entry.to_object(repo).and_then(|obj| obj.peel_to_blob());
        let content = blob.map(|blob| blob.content().to_vec());

        files.insert(
            entry_path
                .strip_prefix("session/wd")
                .unwrap()
                .to_owned()
                .to_str()
                .unwrap()
                .to_owned(),
            String::from_utf8(content.unwrap_or_default()).unwrap_or_default(),
        );

        git2::TreeWalkResult::Ok
    })
    .map_err(|e| Error {
        message: format!("Could not walk tree for commit {}", commit.id()),
        cause: e.into(),
    })?;

    Ok(files)
}

#[test]
fn test_parse_reflog_line() {
    let test_cases = vec![
        (
            "9ea641990993cb60c7d89c41606f6b457adb9681 3f2657e0d1eae57f58d7734aae10310a861de8e8 Nikita Galaiko <nikita@galaiko.rocks> 1676275740 +0100	commit: try sturdy mac dev certificate",
            Activity{ activity_type: "commit".to_string(), timestamp: 1676275740, message: "try sturdy mac dev certificate".to_string() }
        ),
        (
            "999bc2f0194ea001f71ba65b5422a742b5e66d9f bb98b5411d597fdede63053c190260a38d459ecb Nikita Galaiko <nikita@galaiko.rocks> 1675428111 +0100	checkout: moving from production-build to master",
            Activity{ activity_type: "checkout".to_string(), timestamp: 1675428111, message: "moving from production-build to master".to_string() },
        ),
        (
            "0000000000000000000000000000000000000000 9aa96f488fbdb8f7b15151d9d2e47690d3b21b46 Nikita Galaiko <nikita@galaiko.rocks> 1675176957 +0100	commit (initial): simple tauri example",
            Activity{ activity_type: "commit (initial)".to_string(), timestamp: 1675176957, message: "simple tauri example".to_string() },
        ),
        (
            "d083bb9213fc5e0bb6d07c2c6c1eae5be483be25 dc870a80fddb843583baa36cb637c5c820b1e863 Nikita Galaiko <nikita@galaiko.rocks> 1675425613 +0100	commit (amend): build app with github actions",
            Activity{ activity_type: "commit (amend)".to_string(), timestamp: 1675425613, message: "build app with github actions".to_string() },
        ),
        (
            "2843be38a72ac8418c7e5c5630cba3c4803916d1 fbb7a9356484b948bde4c7ee9fdeb6439edff8c0 Nikita Galaiko <nikita@galaiko.rocks> 1676274883 +0100	pull: Fast-forward",
            Activity{ activity_type: "pull".to_string(), timestamp: 1676274883, message: "Fast-forward".to_string() },
        ),
        (
            "3f2657e0d1eae57f58d7734aae10310a861de8e8 3f2657e0d1eae57f58d7734aae10310a861de8e8 Nikita Galaiko <nikita@galaiko.rocks> 1676277401 +0100	reset: moving to HEAD",
            Activity{ activity_type: "reset".to_string() , timestamp: 1676277401, message: "moving to HEAD".to_string() },
        ),
        (
            "9a831ba2fa07aa6a399bbb498e8effd913cec2e0 add94e65594e4c240b0f6b03973a3be3ff594306 Nikita Galaiko <nikita@galaiko.rocks> 1676039997 +0100	pull --rebase (start): checkout add94e65594e4c240b0f6b03973a3be3ff594306",
            Activity{ activity_type: "pull --rebase (start)".to_string(), timestamp: 1676039997, message: "checkout add94e65594e4c240b0f6b03973a3be3ff594306".to_string() },
        ),
        (
            "add94e65594e4c240b0f6b03973a3be3ff594306 bcc93167c068649868aa3df4999ba154468a62b5 Nikita Galaiko <nikita@galaiko.rocks> 1676039997 +0100	pull --rebase (pick): make app run in background",
            Activity{ activity_type: "pull --rebase (pick)".to_string(), timestamp: 1676039997, message: "make app run in background".to_string() },
        ),
        (
            "bcc93167c068649868aa3df4999ba154468a62b5 bcc93167c068649868aa3df4999ba154468a62b5 Nikita Galaiko <nikita@galaiko.rocks> 1676039997 +0100	pull --rebase (finish): returning to refs/heads/master",
            Activity{ activity_type: "pull --rebase (finish)".to_string(), timestamp: 1676039997, message: "returning to refs/heads/master".to_string() },
        )
    ];

    for (line, expected) in test_cases {
        let actual = parse_reflog_line(line);
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), expected);
    }
}
