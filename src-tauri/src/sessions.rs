use crate::{butler, fs};
use filetime::FileTime;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    os::unix::prelude::MetadataExt,
    path::Path,
    time::SystemTime,
};
use uuid::Uuid;

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

    pub fn from_head(repo: &git2::Repository) -> Result<Self, Error> {
        let now_ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let head = repo.head().map_err(|err| Error {
            cause: err.into(),
            message: "Error while getting HEAD".to_string(),
        })?;
        let session = Session {
            id: Uuid::new_v4().to_string(),
            hash: None,
            meta: Meta {
                start_ts: now_ts,
                last_ts: now_ts,
                branch: head.name().unwrap().to_string(),
                commit: head.peel_to_commit().unwrap().id().to_string(),
            },
            activity: vec![],
        };
        create_session(repo, &session)?;
        Ok(session)
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
            ErrorCause::TryFromIntError(err) => Some(err),
            ErrorCause::SessionExistsError => Some(self),
            ErrorCause::SessionNotFound => Some(self),
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
            ErrorCause::TryFromIntError(ref e) => write!(f, "{}: {}", self.message, e),
            ErrorCause::SessionExistsError => write!(f, "{}", self.message),
            ErrorCause::SessionNotFound => write!(f, "{}", self.message),
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
    TryFromIntError(std::num::TryFromIntError),
    GitError(git2::Error),
    SessionExistsError,
    SessionIsNotCurrentError,
    SessionNotFound,
    ParseUtf8Error(std::string::FromUtf8Error),
    ParseActivityError,
}

impl From<std::num::TryFromIntError> for ErrorCause {
    fn from(err: std::num::TryFromIntError) -> Self {
        ErrorCause::TryFromIntError(err)
    }
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
            cause: ErrorCause::SessionNotFound,
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

fn delete_session(repo: &git2::Repository) -> Result<(), std::io::Error> {
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

// return a map of file name -> file content for all files in the beginning of a session.
pub fn list_files(
    repo: &git2::Repository,
    session_id: &str,
) -> Result<HashMap<String, String>, Error> {
    let list = list(repo)?;

    let mut previous_session = None;
    let mut session = None;
    for s in list {
        if s.id == session_id {
            session = Some(s);
            break;
        }
        previous_session = Some(s);
    }

    let session_hash = match (previous_session, session) {
        // if there is a previous session, we want to list the files from the previous session
        (Some(previous_session), Some(_)) => previous_session.hash,
        // if there is no previous session, we use the found session, because it's the first one.
        (None, Some(session)) => session.hash,
        _ => {
            return Err(Error {
                message: format!("Could not find session {}", session_id),
                cause: ErrorCause::SessionNotFound,
            })
        }
    };

    if session_hash.is_none() {
        return Err(Error {
            message: format!("Could not find files for  {}", session_id),
            cause: ErrorCause::SessionNotFound,
        });
    }

    let commit_id = git2::Oid::from_str(&session_hash.clone().unwrap()).map_err(|e| Error {
        message: format!(
            "Could not parse commit id {}",
            session_hash.as_ref().unwrap().to_string()
        ),
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
        if !entry_path.starts_with("wd") {
            return git2::TreeWalkResult::Ok;
        }
        let blob = entry.to_object(repo).and_then(|obj| obj.peel_to_blob());
        let content = blob.map(|blob| blob.content().to_vec());

        files.insert(
            entry_path
                .strip_prefix("wd")
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

pub fn flush_current_session(repo: &git2::Repository) -> Result<Session, Error> {
    let session = Session::current(&repo)?;
    if session.is_none() {
        return Err(Error {
            cause: ErrorCause::SessionNotFound,
            message: "No current session".to_string(),
        });
    }

    let wd_index = &mut git2::Index::new().map_err(|e| Error {
        cause: e.into(),
        message: "Failed to create wd index".to_string(),
    })?;

    build_wd_index(&repo, wd_index).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to build wd index".to_string(),
    })?;
    let wd_tree = wd_index.write_tree_to(&repo).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to write wd tree".to_string(),
    })?;

    let session_index = &mut git2::Index::new().map_err(|e| Error {
        cause: e.into(),
        message: "Failed to create session index".to_string(),
    })?;
    build_session_index(&repo, session_index).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to build session index".to_string(),
    })?;
    let session_tree = session_index.write_tree_to(&repo).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to write session tree".to_string(),
    })?;

    let log_index = &mut git2::Index::new().map_err(|e| Error {
        cause: e.into(),
        message: "Failed to create log index".to_string(),
    })?;
    build_log_index(&repo, log_index).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to build log index".to_string(),
    })?;
    let log_tree = log_index.write_tree_to(&repo).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to write log tree".to_string(),
    })?;

    let mut tree_builder = repo.treebuilder(None).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to create tree builder".to_string(),
    })?;
    tree_builder
        .insert("session", session_tree, 0o040000)
        .map_err(|e| Error {
            cause: e.into(),
            message: "Failed to insert session tree".to_string(),
        })?;
    tree_builder
        .insert("wd", wd_tree, 0o040000)
        .map_err(|e| Error {
            cause: e.into(),
            message: "Failed to insert wd tree".to_string(),
        })?;
    tree_builder
        .insert("logs", log_tree, 0o040000)
        .map_err(|e| Error {
            cause: e.into(),
            message: "Failed to insert log tree".to_string(),
        })?;

    let tree = tree_builder.write().map_err(|e| Error {
        cause: e.into(),
        message: "Failed to write tree".to_string(),
    })?;

    let commit_oid = write_gb_commit(tree, &repo).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to write gb commit".to_string(),
    })?;
    log::debug!(
        "{}: wrote gb commit {}",
        repo.workdir().unwrap().display(),
        commit_oid
    );
    delete_session(repo).map_err(|e| Error {
        cause: e.into(),
        message: "Failed to delete session".to_string(),
    })?;

    Ok(session.unwrap())

    // TODO: try to push the new gb history head to the remote
    // TODO: if we see it is not a FF, pull down the remote, determine order, rewrite the commit line, and push again
}

// build the initial tree from the working directory, not taking into account the gitbutler metadata
// eventually we might just want to run this once and then update it with the files that are changed over time, but right now we're running it every commit
// it ignores files that are in the .gitignore
fn build_wd_index(repo: &git2::Repository, index: &mut git2::Index) -> Result<(), ErrorCause> {
    // create a new in-memory git2 index and open the working one so we can cheat if none of the metadata of an entry has changed
    let repo_index = &mut repo.index()?;

    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    let all_files = fs::list_files(repo.workdir().unwrap())?;
    for file in all_files {
        let file_path = Path::new(&file);
        if !repo.is_path_ignored(&file).unwrap_or(true) {
            add_wd_path(index, repo_index, &file_path, &repo)?;
        }
    }

    Ok(())
}

// take a file path we see and add it to our in-memory index
// we call this from build_initial_wd_tree, which is smart about using the existing index to avoid rehashing files that haven't changed
// and also looks for large files and puts in a placeholder hash in the LFS format
// TODO: actually upload the file to LFS
fn add_wd_path(
    index: &mut git2::Index,
    repo_index: &mut git2::Index,
    rel_file_path: &Path,
    repo: &git2::Repository,
) -> Result<(), ErrorCause> {
    let abs_file_path = repo.workdir().unwrap().join(rel_file_path);
    let file_path = Path::new(&abs_file_path);

    let metadata = file_path.metadata()?;
    let mtime = FileTime::from_last_modification_time(&metadata);
    let ctime = FileTime::from_creation_time(&metadata).unwrap();

    // if we find the entry in the index, we can just use it
    match repo_index.get_path(rel_file_path, 0) {
        // if we find the entry and the metadata of the file has not changed, we can just use the existing entry
        Some(entry) => {
            if entry.mtime.seconds() == i32::try_from(mtime.seconds())?
                && entry.mtime.nanoseconds() == u32::try_from(mtime.nanoseconds()).unwrap()
                && entry.file_size == u32::try_from(metadata.len())?
                && entry.mode == metadata.mode()
            {
                log::debug!("Using existing entry for {}", file_path.display());
                index.add(&entry).unwrap();
                return Ok(());
            }
        }
        None => {
            log::debug!("No entry found for {}", file_path.display());
        }
    };

    // something is different, or not found, so we need to create a new entry

    log::debug!("Adding wd path: {}", file_path.display());

    // look for files that are bigger than 4GB, which are not supported by git
    // insert a pointer as the blob content instead
    // TODO: size limit should be configurable
    let blob = if metadata.len() > 100_000_000 {
        log::debug!(
            "{}: file too big: {}",
            repo.workdir().unwrap().display(),
            file_path.display()
        );

        // get a sha256 hash of the file first
        let sha = sha256_digest(&file_path)?;

        // put togther a git lfs pointer file: https://github.com/git-lfs/git-lfs/blob/main/docs/spec.md
        let mut lfs_pointer = String::from("version https://git-lfs.github.com/spec/v1\n");
        lfs_pointer.push_str("oid sha256:");
        lfs_pointer.push_str(&sha);
        lfs_pointer.push_str("\n");
        lfs_pointer.push_str("size ");
        lfs_pointer.push_str(&metadata.len().to_string());
        lfs_pointer.push_str("\n");

        // write the file to the .git/lfs/objects directory
        // create the directory recursively if it doesn't exist
        let lfs_objects_dir = repo.path().join("lfs/objects");
        std::fs::create_dir_all(lfs_objects_dir.clone())?;
        let lfs_path = lfs_objects_dir.join(sha);
        std::fs::copy(file_path, lfs_path)?;

        repo.blob(lfs_pointer.as_bytes()).unwrap()
    } else {
        // read the file into a blob, get the object id
        repo.blob_path(&file_path)?
    };

    // create a new IndexEntry from the file metadata
    index.add(&git2::IndexEntry {
        ctime: git2::IndexTime::new(
            ctime.seconds().try_into()?,
            ctime.nanoseconds().try_into().unwrap(),
        ),
        mtime: git2::IndexTime::new(
            mtime.seconds().try_into()?,
            mtime.nanoseconds().try_into().unwrap(),
        ),
        dev: metadata.dev().try_into()?,
        ino: metadata.ino().try_into()?,
        mode: metadata.mode(),
        uid: metadata.uid().try_into().unwrap(),
        gid: metadata.gid().try_into().unwrap(),
        file_size: metadata.len().try_into().unwrap(),
        flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
        flags_extended: 0, // no extended flags
        path: rel_file_path.to_str().unwrap().to_string().into(),
        id: blob,
    })?;

    Ok(())
}

/// calculates sha256 digest of a large file as lowercase hex string via streaming buffer
/// used to calculate the hash of large files that are not supported by git
fn sha256_digest(path: &Path) -> Result<String, std::io::Error> {
    let input = File::open(path)?;
    let mut reader = BufReader::new(input);

    let digest = {
        let mut hasher = Sha256::new();
        let mut buffer = [0; 1024];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        hasher.finalize()
    };
    Ok(format!("{:X}", digest))
}

fn build_log_index(repo: &git2::Repository, index: &mut git2::Index) -> Result<(), ErrorCause> {
    let logs_dir = repo.path().join("logs");
    for log_file in fs::list_files(&logs_dir)? {
        let log_file = Path::new(&log_file);
        add_log_path(repo, index, &log_file)?;
    }
    Ok(())
}

fn add_log_path(
    repo: &git2::Repository,
    index: &mut git2::Index,
    rel_file_path: &Path,
) -> Result<(), ErrorCause> {
    let file_path = repo.path().join("logs").join(rel_file_path);
    log::debug!("Adding log path: {}", file_path.display());

    let metadata = file_path.metadata()?;
    let mtime = FileTime::from_last_modification_time(&metadata);
    let ctime = FileTime::from_creation_time(&metadata).unwrap();

    index.add(&git2::IndexEntry {
        ctime: git2::IndexTime::new(
            ctime.seconds().try_into()?,
            ctime.nanoseconds().try_into().unwrap(),
        ),
        mtime: git2::IndexTime::new(
            mtime.seconds().try_into()?,
            mtime.nanoseconds().try_into().unwrap(),
        ),
        dev: metadata.dev().try_into()?,
        ino: metadata.ino().try_into()?,
        mode: metadata.mode(),
        uid: metadata.uid().try_into().unwrap(),
        gid: metadata.gid().try_into().unwrap(),
        file_size: metadata.len().try_into()?,
        flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
        flags_extended: 0, // no extended flags
        path: rel_file_path.to_str().unwrap().to_string().into(),
        id: repo.blob_path(&file_path)?,
    })?;

    Ok(())
}

fn build_session_index(repo: &git2::Repository, index: &mut git2::Index) -> Result<(), ErrorCause> {
    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    let session_dir = repo.path().join(butler::dir()).join("session");
    for session_file in fs::list_files(&session_dir)? {
        let file_path = Path::new(&session_file);
        add_session_path(&repo, index, &file_path)?;
    }

    Ok(())
}

// this is a helper function for build_gb_tree that takes paths under .git/gb/session and adds them to the in-memory index
fn add_session_path(
    repo: &git2::Repository,
    index: &mut git2::Index,
    rel_file_path: &Path,
) -> Result<(), ErrorCause> {
    let file_path = repo
        .path()
        .join(butler::dir())
        .join("session")
        .join(rel_file_path);

    log::debug!("Adding session path: {}", file_path.display());

    let blob = repo.blob_path(&file_path)?;
    let metadata = file_path.metadata()?;
    let mtime = FileTime::from_last_modification_time(&metadata);
    let ctime = FileTime::from_creation_time(&metadata).unwrap();

    // create a new IndexEntry from the file metadata
    index.add(&git2::IndexEntry {
        ctime: git2::IndexTime::new(
            ctime.seconds().try_into()?,
            ctime.nanoseconds().try_into().unwrap(),
        ),
        mtime: git2::IndexTime::new(
            mtime.seconds().try_into()?,
            mtime.nanoseconds().try_into().unwrap(),
        ),
        dev: metadata.dev().try_into()?,
        ino: metadata.ino().try_into()?,
        mode: metadata.mode(),
        uid: metadata.uid().try_into().unwrap(),
        gid: metadata.gid().try_into().unwrap(),
        file_size: metadata.len().try_into()?,
        flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
        flags_extended: 0, // no extended flags
        path: rel_file_path.to_str().unwrap().into(),
        id: blob,
    })?;

    Ok(())
}

// write a new commit object to the repo
// this is called once we have a tree of deltas, metadata and current wd snapshot
// and either creates or updates the refs/gitbutler/current ref
fn write_gb_commit(gb_tree: git2::Oid, repo: &git2::Repository) -> Result<git2::Oid, git2::Error> {
    // find the Oid of the commit that refs/.../current points to, none if it doesn't exist
    let refname = format!("refs/{}/current", butler::refname());
    match repo.revparse_single(refname.as_str()) {
        Ok(obj) => {
            let last_commit = repo.find_commit(obj.id()).unwrap();
            let new_commit = repo.commit(
                Some(refname.as_str()),
                &repo.signature().unwrap(),        // author
                &repo.signature().unwrap(),        // committer
                "gitbutler check",                 // commit message
                &repo.find_tree(gb_tree).unwrap(), // tree
                &[&last_commit],                   // parents
            )?;
            Ok(new_commit)
        }
        Err(_) => {
            let new_commit = repo.commit(
                Some(refname.as_str()),
                &repo.signature().unwrap(),        // author
                &repo.signature().unwrap(),        // committer
                "gitbutler check",                 // commit message
                &repo.find_tree(gb_tree).unwrap(), // tree
                &[],                               // parents
            )?;
            Ok(new_commit)
        }
    }
}
