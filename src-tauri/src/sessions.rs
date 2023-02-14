use crate::{butler, fs, users::user::User};
use anyhow::{anyhow, Context, Result};
use filetime::FileTime;
use git2::{PushOptions, RemoteCallbacks, Repository};
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

fn parse_reflog_line(line: &str) -> Result<Activity> {
    match line.split("\t").collect::<Vec<&str>>()[..] {
        [meta, message] => {
            let meta_parts = meta.split_whitespace().collect::<Vec<&str>>();
            let timestamp = meta_parts[meta_parts.len() - 2].parse::<u64>()?;

            match message.split(": ").collect::<Vec<&str>>()[..] {
                [entry_type, msg] => Ok(Activity {
                    activity_type: entry_type.to_string(),
                    message: msg.to_string(),
                    timestamp,
                }),
                _ => Err(anyhow!("failed to parse reflog line: {}", line)),
            }
        }
        _ => Err(anyhow!("failed to parse reflog line: {}", line)),
    }
}

impl Session {
    pub fn current(repo: &git2::Repository) -> Result<Option<Self>> {
        let session_path = repo.path().join(butler::dir()).join("session");
        if !session_path.exists() {
            return Ok(None);
        }

        let meta_path = session_path.join("meta");

        let start_path = meta_path.join("start");
        let start_ts = std::fs::read_to_string(start_path.clone())?
            .parse::<u64>()
            .with_context(|| {
                format!(
                    "failed to parse start timestamp from {}",
                    start_path.display()
                )
            })?;

        let last_path = meta_path.join("last");
        let last_ts = std::fs::read_to_string(last_path.clone())?
            .parse::<u64>()
            .with_context(|| {
                format!(
                    "failed to parse last timestamp from {}",
                    last_path.display()
                )
            })?;

        let branch_path = meta_path.join("branch");
        let branch = std::fs::read_to_string(branch_path.clone()).with_context(|| {
            format!("failed to read branch name from {}", branch_path.display())
        })?;

        let commit_path = meta_path.join("commit");
        let commit = std::fs::read_to_string(commit_path.clone()).with_context(|| {
            format!("failed to read commit hash from {}", commit_path.display())
        })?;

        let reflog = std::fs::read_to_string(repo.path().join("logs/HEAD")).with_context(|| {
            format!(
                "failed to read reflog from {}",
                repo.path().join("logs/HEAD").display()
            )
        })?;
        let activity = reflog
            .lines()
            .filter_map(|line| parse_reflog_line(line).ok())
            .filter(|activity| activity.timestamp >= start_ts)
            .collect::<Vec<Activity>>();

        let id_path = meta_path.join("id");
        let id = std::fs::read_to_string(id_path.clone())
            .with_context(|| format!("failed to read session id from {}", id_path.display()))?;

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

    pub fn from_head(repo: &git2::Repository) -> Result<Self> {
        let now_ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let head = repo.head()?;
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

    pub fn from_commit(repo: &git2::Repository, commit: &git2::Commit) -> Result<Self> {
        let tree = commit.tree().with_context(|| {
            format!("failed to get tree from commit {}", commit.id().to_string())
        })?;

        let start_ts = read_as_string(repo, &tree, Path::new("session/meta/start"))?
            .parse::<u64>()
            .with_context(|| {
                format!(
                    "failed to parse start timestamp from commit {}",
                    commit.id().to_string()
                )
            })?;

        let reflog = read_as_string(repo, &tree, Path::new("logs/HEAD")).with_context(|| {
            format!(
                "failed to read reflog from commit {}",
                commit.id().to_string()
            )
        })?;
        let activity = reflog
            .lines()
            .filter_map(|line| parse_reflog_line(line).ok())
            .filter(|activity| activity.timestamp >= start_ts)
            .collect::<Vec<Activity>>();

        Ok(Session {
            id: read_as_string(repo, &tree, Path::new("session/meta/id")).with_context(|| {
                format!(
                    "failed to read session id from commit {}",
                    commit.id().to_string()
                )
            })?,
            hash: Some(commit.id().to_string()),
            meta: Meta {
                start_ts,
                last_ts: read_as_string(repo, &tree, Path::new("session/meta/last"))?
                    .parse::<u64>()
                    .with_context(|| {
                        format!(
                            "failed to parse last timestamp from commit {}",
                            commit.id().to_string()
                        )
                    })?,
                branch: read_as_string(repo, &tree, Path::new("session/meta/branch"))
                    .with_context(|| {
                        format!(
                            "failed to read branch name from commit {}",
                            commit.id().to_string()
                        )
                    })?,
                commit: read_as_string(repo, &tree, Path::new("session/meta/commit"))
                    .with_context(|| {
                        format!(
                            "failed to read commit hash from commit {}",
                            commit.id().to_string()
                        )
                    })?,
            },
            activity,
        })
    }
}

fn write_session(session_path: &Path, session: &Session) -> Result<()> {
    if session.hash.is_some() {
        return Err(anyhow!("cannot write session that is not current"));
    }

    let meta_path = session_path.join("meta");

    std::fs::create_dir_all(meta_path.clone()).with_context(|| {
        format!(
            "failed to create session meta directory {}",
            meta_path.display()
        )
    })?;

    let id_path = meta_path.join("id");
    std::fs::write(id_path.clone(), session.id.clone())
        .with_context(|| format!("failed to write session id to {}", id_path.display()))?;

    let start_path = meta_path.join("start");
    std::fs::write(start_path.clone(), session.meta.start_ts.to_string()).with_context(|| {
        format!(
            "failed to write session start timestamp to {}",
            start_path.display()
        )
    })?;

    let last_path = meta_path.join("last");
    std::fs::write(last_path.clone(), session.meta.last_ts.to_string()).with_context(|| {
        format!(
            "failed to write session last timestamp to {}",
            last_path.display()
        )
    })?;

    let branch_path = meta_path.join("branch");
    std::fs::write(branch_path.clone(), session.meta.branch.clone()).with_context(|| {
        format!(
            "failed to write session branch to {}",
            branch_path.display()
        )
    })?;

    let commit_path = meta_path.join("commit");
    std::fs::write(commit_path.clone(), session.meta.commit.clone()).with_context(|| {
        format!(
            "failed to write session commit to {}",
            commit_path.display()
        )
    })?;

    Ok(())
}

pub fn update_session(repo: &git2::Repository, session: &Session) -> Result<()> {
    log::debug!("{}: Updating current session", repo.path().display());
    let session_path = repo.path().join(butler::dir()).join("session");
    if session_path.exists() {
        write_session(&session_path, session)
    } else {
        Err(anyhow!("session does not exist"))
    }
}

pub fn create_session(repo: &git2::Repository, session: &Session) -> Result<()> {
    log::debug!("{}: Creating current session", repo.path().display());
    let session_path = repo.path().join(butler::dir()).join("session");
    if session_path.exists() {
        Err(anyhow!("session already exists"))
    } else {
        write_session(&session_path, session)
    }
}

fn delete_session(repo: &git2::Repository) -> Result<()> {
    log::debug!("{}: Deleting current session", repo.path().display());
    let session_path = repo.path().join(butler::dir()).join("session");
    if session_path.exists() {
        std::fs::remove_dir_all(session_path)?;
    }
    Ok(())
}

pub fn get(repo: &git2::Repository, id: &str) -> Result<Option<Session>> {
    let list = list(repo)?;
    for session in list {
        if session.id == id {
            return Ok(Some(session));
        }
    }
    Ok(None)
}

pub fn list(repo: &git2::Repository) -> Result<Vec<Session>> {
    let mut sessions = list_persistent(repo)?;
    if let Some(session) = Session::current(repo)? {
        sessions.push(session);
    }
    Ok(sessions)
}

fn list_persistent(repo: &git2::Repository) -> Result<Vec<Session>> {
    let gitbutler_ref =
        repo.find_reference(format!("refs/{}/current", butler::refname()).as_str())?;
    let gitbutler_head = repo.find_commit(gitbutler_ref.target().unwrap())?;

    // list all commits from gitbutler head to the first commit
    let mut walker = repo.revwalk()?;
    walker.push(gitbutler_head.id())?;
    walker.set_sorting(git2::Sort::TIME)?;

    let mut sessions: Vec<Session> = vec![];
    for id in walker {
        let id = id?;
        let commit = repo.find_commit(id).with_context(|| {
            format!(
                "failed to find commit {} in repository {}",
                id.to_string(),
                repo.path().display()
            )
        })?;
        sessions.push(Session::from_commit(repo, &commit)?);
    }

    Ok(sessions)
}

fn read_as_string(repo: &git2::Repository, tree: &git2::Tree, path: &Path) -> Result<String> {
    let tree_entry = tree.get_path(path)?;
    let blob = tree_entry.to_object(repo)?.into_blob().unwrap();
    let contents = String::from_utf8(blob.content().to_vec()).with_context(|| {
        format!(
            "failed to read file {} as string",
            path.to_str().unwrap_or("unknown")
        )
    })?;
    Ok(contents)
}

// return a map of file name -> file content for all files in the beginning of a session.
pub fn list_files(repo: &git2::Repository, session_id: &str) -> Result<HashMap<String, String>> {
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
        _ => return Err(anyhow!("session {} not found", session_id)),
    };

    if session_hash.is_none() {
        return Err(anyhow!("session {} has no hash", session_id));
    }

    let commit_id = git2::Oid::from_str(&session_hash.clone().unwrap())?;
    let commit = repo.find_commit(commit_id)?;

    let tree = commit.tree()?;

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

pub fn flush_current_session(repo: &git2::Repository, user: &Option<User>) -> Result<Session> {
    match user {
        Some(user) => println!("flushing current session for {}", user.name),
        None => println!("No logged in user"),
    }

    let session = Session::current(&repo)?;
    if session.is_none() {
        return Err(anyhow!("session not found"));
    }

    let wd_index = &mut git2::Index::new()?;

    build_wd_index(&repo, wd_index)?;
    let wd_tree = wd_index.write_tree_to(&repo)?;

    let session_index = &mut git2::Index::new()?;
    build_session_index(&repo, session_index)?;
    let session_tree = session_index.write_tree_to(&repo)?;

    let log_index = &mut git2::Index::new()?;
    build_log_index(&repo, log_index)?;
    let log_tree = log_index.write_tree_to(&repo)?;

    let mut tree_builder = repo.treebuilder(None)?;
    tree_builder.insert("session", session_tree, 0o040000)?;
    tree_builder.insert("wd", wd_tree, 0o040000)?;
    tree_builder.insert("logs", log_tree, 0o040000)?;

    let tree = tree_builder.write()?;

    let commit_oid = write_gb_commit(tree, &repo)?;
    log::debug!(
        "{}: wrote gb commit {}",
        repo.workdir().unwrap().display(),
        commit_oid
    );
    delete_session(repo)?;

    push_to_remote(repo, user)?;

    Ok(session.unwrap())
}

// try to push the new gb history head to the remote
// TODO: if we see it is not a FF, pull down the remote, determine order, rewrite the commit line, and push again
fn push_to_remote(repo: &Repository, user: &Option<User>) -> Result<(), git2::Error> {
    // only push if the user is logged in and this project is registered
    let auth_token: String;
    match user {
        Some(user) => {
            println!("getting access token for {}", user.email);
            auth_token = user.access_token.clone();
        }
        None => {
            println!("No logged in user, no push");
            return Ok(());
        }
    }

    let remote_url = "https://test.app.gitbutler.com/git/287bff35-7827-4fc1-aab9-e9732da2b5ec";

    println!("Pushing to remote: {}", remote_url);

    // Create an anonymous remote
    let mut remote = repo.remote_anonymous(remote_url).unwrap();

    // Set the remote's callbacks
    let mut callbacks = RemoteCallbacks::new();
    callbacks.push_update_reference(move |refname, message| {
        println!("Pushing reference '{}': {:?}", refname, message);
        Ok(())
    });
    callbacks.push_transfer_progress(move |one, two, three| {
        println!("Transferred {}/{}/{} objects", one, two, three);
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    let auth_header = format!("Authorization: {}", auth_token);
    let headers = &[auth_header.as_str()];
    push_options.custom_headers(headers);

    // Push to the remote
    let refname = format!("refs/{}/current", butler::refname());
    remote.push(&[refname], Some(&mut push_options));

    Ok(())
}

// build the initial tree from the working directory, not taking into account the gitbutler metadata
// eventually we might just want to run this once and then update it with the files that are changed over time, but right now we're running it every commit
// it ignores files that are in the .gitignore
fn build_wd_index(repo: &git2::Repository, index: &mut git2::Index) -> Result<()> {
    // create a new in-memory git2 index and open the working one so we can cheat if none of the metadata of an entry has changed
    let repo_index = &mut repo.index()?;

    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    let all_files = fs::list_files(repo.workdir().unwrap())?;
    for file in all_files {
        let file_path = Path::new(&file);
        if !repo.is_path_ignored(&file).unwrap_or(true) {
            add_wd_path(index, repo_index, &file_path, &repo).with_context(|| {
                format!(
                    "failed to add working directory path {}",
                    file_path.display()
                )
            })?;
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
) -> Result<()> {
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

fn build_log_index(repo: &git2::Repository, index: &mut git2::Index) -> Result<()> {
    let logs_dir = repo.path().join("logs");
    for log_file in fs::list_files(&logs_dir)? {
        let log_file = Path::new(&log_file);
        add_log_path(repo, index, &log_file)
            .with_context(|| format!("Failed to add log file to index: {}", log_file.display()))?;
    }
    Ok(())
}

fn add_log_path(
    repo: &git2::Repository,
    index: &mut git2::Index,
    rel_file_path: &Path,
) -> Result<()> {
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

fn build_session_index(repo: &git2::Repository, index: &mut git2::Index) -> Result<()> {
    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    let session_dir = repo.path().join(butler::dir()).join("session");
    for session_file in fs::list_files(&session_dir)? {
        let file_path = Path::new(&session_file);
        add_session_path(&repo, index, &file_path).with_context(|| {
            format!(
                "Failed to add session file to index: {}",
                file_path.display()
            )
        })?;
    }

    Ok(())
}

// this is a helper function for build_gb_tree that takes paths under .git/gb/session and adds them to the in-memory index
fn add_session_path(
    repo: &git2::Repository,
    index: &mut git2::Index,
    rel_file_path: &Path,
) -> Result<()> {
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
