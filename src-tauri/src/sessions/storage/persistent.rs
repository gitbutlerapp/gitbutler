use crate::{
    app::reader::{self, Reader},
    fs, projects, sessions, users,
};
use anyhow::{Context, Result};
use filetime::FileTime;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    os::unix::prelude::MetadataExt,
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Store {
    project: projects::Project,
    git_repository: Arc<Mutex<git2::Repository>>,

    files_cache: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
    sessions_cache: Arc<Mutex<Option<Vec<sessions::Session>>>>,
}

impl Store {
    pub fn new(git_repository: Arc<Mutex<git2::Repository>>, project: projects::Project) -> Self {
        Self {
            project,
            git_repository,
            files_cache: Arc::new(Mutex::new(HashMap::new())),
            sessions_cache: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_by_id(&self, session_id: &str) -> Result<Option<sessions::Session>> {
        let sessions_cache = self.sessions_cache.lock().unwrap();
        match sessions_cache.as_ref() {
            Some(sessions) => {
                for session in sessions {
                    if session.id == session_id {
                        return Ok(Some(session.clone()));
                    }
                }
                Ok(None)
            }
            None => self.get_by_id_from_disk(session_id),
        }
    }

    fn get_by_id_from_disk(&self, session_id: &str) -> Result<Option<sessions::Session>> {
        let git_repository = self.git_repository.lock().unwrap();
        let reference = git_repository.find_reference(&self.project.refname())?;
        let head = git_repository.find_commit(reference.target().unwrap())?;
        let mut walker = git_repository.revwalk()?;
        walker.push(head.id())?;
        walker.set_sorting(git2::Sort::TIME)?;

        for commit_id in walker {
            let commit_id = commit_id?;
            let commit = git_repository.find_commit(commit_id)?;
            let reader = reader::CommitReader::from_commit(&git_repository, commit)?;
            if reader.read_to_string("session/meta/id")? == session_id {
                return Ok(Some(sessions::Session::try_from(reader)?));
            }
        }

        Ok(None)
    }

    pub fn list_files(
        &self,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        let mut files_cache = self.files_cache.lock().unwrap();
        let files = match files_cache.get(session_id) {
            Some(files) => files.clone(),
            None => {
                let files = self.list_files_from_disk(session_id)?;
                files_cache.insert(session_id.to_string(), files.clone());
                files
            }
        };
        match paths {
            Some(paths) => {
                let mut filtered_files = HashMap::new();
                for path in paths {
                    if let Some(file) = files.get(path) {
                        filtered_files.insert(path.to_string(), file.to_string());
                    }
                }
                Ok(filtered_files)
            }
            None => Ok(files),
        }
    }

    fn list_files_from_disk(&self, session_id: &str) -> Result<HashMap<String, String>> {
        let git_repository = self.git_repository.lock().unwrap();
        let reference = git_repository.find_reference(&self.project.refname())?;
        let commit = if is_current_session_id(&self.project, session_id)? {
            let head_commit = reference.peel_to_commit()?;
            Some(head_commit)
        } else {
            let head_commit = reference.peel_to_commit()?;
            let mut walker = git_repository.revwalk()?;
            walker.push(head_commit.id())?;
            walker.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

            let mut session_commit = None;
            let mut previous_session_commit = None;
            for commit_id in walker {
                let commit = git_repository.find_commit(commit_id?)?;
                if sessions::id_from_commit(&git_repository, &commit)? == session_id {
                    session_commit = Some(commit);
                    break;
                }
                previous_session_commit = Some(commit.clone());
            }

            match (previous_session_commit, session_commit) {
                // if there is a previous session, we want to list the files from the previous session
                (Some(previous_session_commit), Some(_)) => Some(previous_session_commit),
                // if there is no previous session, we use the found session, because it's the first one.
                (None, Some(session_commit)) => Some(session_commit),
                _ => None,
            }
        };

        if commit.is_none() {
            return Ok(HashMap::new());
        }
        let commit = commit.unwrap();

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
            if "wd".eq(entry_path.to_str().unwrap()) {
                return git2::TreeWalkResult::Ok;
            }

            if entry.kind() == Some(git2::ObjectType::Tree) {
                return git2::TreeWalkResult::Ok;
            }

            let blob = entry
                .to_object(&git_repository)
                .and_then(|obj| obj.peel_to_blob());
            let content = blob.map(|blob| blob.content().to_vec());

            let relpath = entry_path.strip_prefix("wd").unwrap();

            files.insert(
                relpath.to_owned().to_str().unwrap().to_owned(),
                String::from_utf8(content.unwrap_or_default()).unwrap_or_default(),
            );

            git2::TreeWalkResult::Ok
        })?;

        Ok(files)
    }

    // returns list of sessions in reverse chronological order
    // except for the first session. The first created session
    // is special and used to bootstrap the gitbutler state inside a repo.
    // see crate::repositories::inib
    pub fn list(&self, earliest_timestamp_ms: Option<u128>) -> Result<Vec<sessions::Session>> {
        let mut cached_sessions = self.sessions_cache.lock().unwrap();
        let sessions = if let Some(sessions) = cached_sessions.as_ref() {
            sessions.clone().to_vec()
        } else {
            let sessions = self.list_from_disk()?;
            cached_sessions.replace(sessions.clone());
            sessions
        };

        let filtered_sessions = if let Some(earliest_timestamp_ms) = earliest_timestamp_ms {
            sessions
                .into_iter()
                .filter(|session| session.meta.start_timestamp_ms >= earliest_timestamp_ms)
                .collect()
        } else {
            sessions
        };

        Ok(filtered_sessions)
    }

    fn list_from_disk(&self) -> Result<Vec<sessions::Session>> {
        let git_repository = self.git_repository.lock().unwrap();
        let reference = git_repository.find_reference(&self.project.refname())?;
        let head = git_repository.find_commit(reference.target().unwrap())?;

        // list all commits from gitbutler head to the first commit
        let mut walker = git_repository.revwalk()?;
        walker.push(head.id())?;
        walker.set_sorting(git2::Sort::TOPOLOGICAL)?;

        let mut sessions: Vec<sessions::Session> = vec![];
        for id in walker {
            let id = id?;
            let commit = git_repository.find_commit(id)?;
            let reader = reader::CommitReader::from_commit(&git_repository, commit)?;
            let session = sessions::Session::try_from(reader)?;
            sessions.push(session);
        }

        // drop the first session, which is the bootstrap session
        sessions.pop();

        Ok(sessions)
    }

    pub fn flush(
        &self,
        user: Option<users::User>,
        session: &sessions::Session,
    ) -> Result<sessions::Session> {
        let session = self.flush_to_disk(user, session)?;
        let mut cached_sessions = self.sessions_cache.lock().unwrap();
        if let Some(sessions) = cached_sessions.as_mut() {
            sessions.insert(0, session.clone());
        }
        Ok(session)
    }

    fn flush_to_disk(
        &self,
        user: Option<users::User>,
        session: &sessions::Session,
    ) -> Result<sessions::Session> {
        if session.hash.is_some() {
            return Err(anyhow::anyhow!(
                "refuse to flush {} because it already has a hash",
                session.id
            ));
        }

        let git_repository = self.git_repository.lock().unwrap();
        let wd_tree = build_wd_tree(&git_repository, &self.project)
            .with_context(|| "failed to build wd tree for project".to_string())?;

        let session_index =
            &mut git2::Index::new().with_context(|| format!("failed to create session index"))?;
        build_session_index(&git_repository, &self.project, session_index)
            .with_context(|| format!("failed to build session index"))?;
        let session_tree = session_index
            .write_tree_to(&git_repository)
            .with_context(|| format!("failed to write session tree"))?;

        let log_index =
            &mut git2::Index::new().with_context(|| format!("failed to create log index"))?;
        build_log_index(&git_repository, log_index)
            .with_context(|| format!("failed to build log index"))?;
        let log_tree = log_index
            .write_tree_to(&git_repository)
            .with_context(|| format!("failed to write log tree"))?;

        let mut tree_builder = git_repository
            .treebuilder(None)
            .with_context(|| format!("failed to create tree builder"))?;
        tree_builder
            .insert("session", session_tree, 0o040000)
            .with_context(|| format!("failed to insert session tree"))?;
        tree_builder
            .insert("wd", wd_tree, 0o040000)
            .with_context(|| format!("failed to insert wd tree"))?;
        tree_builder
            .insert("logs", log_tree, 0o040000)
            .with_context(|| format!("failed to insert log tree"))?;

        let tree = tree_builder
            .write()
            .with_context(|| format!("failed to write tree"))?;

        let commit_oid = write_gb_commit(tree, &git_repository, user.clone(), &self.project)
            .with_context(|| {
                format!(
                    "failed to write gb commit for {}",
                    git_repository.workdir().unwrap().display()
                )
            })?;

        log::info!(
            "{}: flushed session {} into commit {}",
            self.project.id,
            session.id,
            commit_oid,
        );

        let updated_session = sessions::Session {
            id: session.id.clone(),
            meta: session.meta.clone(),
            activity: session.activity.clone(),
            hash: Some(commit_oid.to_string()),
        };

        if let Err(e) = push_to_remote(&git_repository, user, &self.project) {
            log::error!(
                "{}: failed to push gb commit {} to remote: {:#}",
                self.project.id,
                commit_oid,
                e
            );
        }

        Ok(updated_session)
    }
}

fn is_current_session_id(project: &projects::Project, session_id: &str) -> Result<bool> {
    let current_id_path = project.session_path().join("meta").join("id");
    if !current_id_path.exists() {
        return Ok(false);
    }
    let current_id = std::fs::read_to_string(current_id_path)?;
    return Ok(current_id == session_id);
}

// try to push the new gb history head to the remote
// TODO: if we see it is not a FF, pull down the remote, determine order, rewrite the commit line, and push again
fn push_to_remote(
    repo: &git2::Repository,
    user: Option<users::User>,
    project: &projects::Project,
) -> Result<()> {
    // only push if logged in
    let access_token = match user {
        Some(user) => user.access_token.clone(),
        None => return Ok(()),
    };

    // only push if project is connected
    let remote_url = match project.api {
        Some(ref api) => api.git_url.clone(),
        None => return Ok(()),
    };

    log::info!("pushing {} to {}", project.path, remote_url);

    // Create an anonymous remote
    let mut remote = repo
        .remote_anonymous(remote_url.as_str())
        .with_context(|| {
            format!(
                "failed to create anonymous remote for {}",
                remote_url.as_str()
            )
        })?;

    // Set the remote's callbacks
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.push_update_reference(move |refname, message| {
        log::info!(
            "{}: pushing reference '{}': {:?}",
            project.path,
            refname,
            message
        );
        Ok(())
    });
    callbacks.push_transfer_progress(move |one, two, three| {
        log::info!(
            "{}: transferred {}/{}/{} objects",
            project.path,
            one,
            two,
            three
        );
    });

    let mut push_options = git2::PushOptions::new();
    push_options.remote_callbacks(callbacks);
    let auth_header = format!("Authorization: {}", access_token);
    let headers = &[auth_header.as_str()];
    push_options.custom_headers(headers);

    // Push to the remote
    remote
        .push(&[project.refname()], Some(&mut push_options))
        .with_context(|| {
            format!(
                "failed to push {} to {}",
                project.refname(),
                remote_url.as_str()
            )
        })?;

    Ok(())
}

fn build_wd_tree(repo: &git2::Repository, project: &projects::Project) -> Result<git2::Oid> {
    let wd_index = &mut git2::Index::new()
        .with_context(|| format!("failed to create index for working directory"))?;
    match repo.find_reference(&project.refname()) {
        Ok(reference) => {
            // build the working directory tree from the current commit
            // and the session files
            let commit = reference.peel_to_commit()?;
            let tree = commit.tree()?;
            let wd_tree_entry = tree.get_name("wd").unwrap();
            let wd_tree = repo.find_tree(wd_tree_entry.id())?;
            wd_index.read_tree(&wd_tree)?;

            let session_wd_files = fs::list_files(project.wd_path()).with_context(|| {
                format!("failed to list files in {}", project.wd_path().display())
            })?;
            for file in session_wd_files {
                let abs_path = project.wd_path().join(&file);
                let metadata = abs_path
                    .metadata()
                    .with_context(|| "failed to get metadata for".to_string())?;
                let mtime = FileTime::from_last_modification_time(&metadata);
                let ctime = FileTime::from_creation_time(&metadata).unwrap_or(mtime);

                let file_content = match std::fs::read_to_string(&abs_path) {
                    Ok(content) => content,
                    Err(e) => {
                        log::error!(
                            "{}: failed to read file {}: {:#}",
                            project.id,
                            abs_path.display(),
                            e
                        );
                        continue;
                    }
                };

                wd_index
                    .add(&git2::IndexEntry {
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
                        mode: 33188,
                        uid: metadata.uid().try_into().unwrap(),
                        gid: metadata.gid().try_into().unwrap(),
                        file_size: metadata.len().try_into().unwrap(),
                        flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
                        flags_extended: 0, // no extended flags
                        path: file.to_str().unwrap().to_string().into(),
                        id: repo.blob(file_content.as_bytes())?,
                    })
                    .with_context(|| format!("failed to add index entry for {}", file.display()))?;
            }

            let wd_tree_oid = wd_index
                .write_tree_to(&repo)
                .with_context(|| format!("failed to write wd tree"))?;
            Ok(wd_tree_oid)
        }
        Err(e) => {
            if e.code() != git2::ErrorCode::NotFound {
                return Err(e.into());
            }
            build_wd_index_from_repo(&repo, &project, wd_index)
                .with_context(|| format!("failed to build wd index"))?;
            let wd_tree_oid = wd_index
                .write_tree_to(&repo)
                .with_context(|| format!("failed to write wd tree"))?;
            Ok(wd_tree_oid)
        }
    }
}

// build wd index from the working directory files new session wd files
// this is important because we want to make sure session files are in sync with session deltas
fn build_wd_index_from_repo(
    repo: &git2::Repository,
    project: &projects::Project,
    index: &mut git2::Index,
) -> Result<()> {
    // create a new in-memory git2 index and open the working one so we can cheat if none of the metadata of an entry has changed
    let repo_index = &mut repo
        .index()
        .with_context(|| format!("failed to open repo index"))?;

    let mut added: HashMap<String, bool> = HashMap::new();

    // first, add session/wd files. this is important because we want to make sure session files
    // are in sync with session deltas
    let session_wd_files = fs::list_files(project.wd_path())
        .with_context(|| format!("failed to list files in {}", project.wd_path().display()))?;
    for file in session_wd_files {
        let file_path = Path::new(&file);
        if repo.is_path_ignored(&file).unwrap_or(true) {
            continue;
        }
        add_wd_path(index, repo_index, &project.wd_path(), &file_path, &repo).with_context(
            || {
                format!(
                    "failed to add working directory path {}",
                    file_path.display()
                )
            },
        )?;
        added.insert(file_path.to_string_lossy().to_string(), true);
    }

    // finally, add files from the working directory if they aren't already in the index
    let current_wd_files = fs::list_files(&project.path)
        .with_context(|| format!("failed to list files in {}", project.path))?;

    for file in current_wd_files {
        if added.contains_key(&file.to_string_lossy().to_string()) {
            continue;
        }

        let file_path = Path::new(&file);
        if !repo.is_path_ignored(&file).unwrap_or(true) {
            add_wd_path(
                index,
                repo_index,
                &Path::new(&project.path),
                &file_path,
                &repo,
            )
            .with_context(|| {
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
    dir: &Path,
    rel_file_path: &Path,
    repo: &git2::Repository,
) -> Result<()> {
    let abs_file_path = dir.join(rel_file_path);
    let file_path = Path::new(&abs_file_path);

    let metadata = file_path
        .metadata()
        .with_context(|| "failed to get metadata for".to_string())?;
    let mtime = FileTime::from_last_modification_time(&metadata);
    let ctime = FileTime::from_creation_time(&metadata).unwrap_or(mtime);

    if let Some(entry) = repo_index.get_path(rel_file_path, 0) {
        // if we find the entry and the metadata of the file has not changed, we can just use the existing entry
        if entry.mtime.seconds() == i32::try_from(mtime.seconds())?
            && entry.mtime.nanoseconds() == u32::try_from(mtime.nanoseconds()).unwrap()
            && entry.file_size == u32::try_from(metadata.len())?
            && entry.mode == metadata.mode()
        {
            log::debug!("using existing entry for {}", file_path.display());
            index.add(&entry).unwrap();
            return Ok(());
        }
    }

    // something is different, or not found, so we need to create a new entry

    log::debug!("adding wd path: {}", file_path.display());

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
    index
        .add(&git2::IndexEntry {
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
            mode: 33188,
            uid: metadata.uid().try_into().unwrap(),
            gid: metadata.gid().try_into().unwrap(),
            file_size: metadata.len().try_into().unwrap(),
            flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
            flags_extended: 0, // no extended flags
            path: rel_file_path.to_str().unwrap().to_string().into(),
            id: blob,
        })
        .with_context(|| format!("failed to add index entry for {}", file_path.display()))?;

    Ok(())
}

/// calculates sha256 digest of a large file as lowercase hex string via streaming buffer
/// used to calculate the hash of large files that are not supported by git
fn sha256_digest(path: &Path) -> Result<String> {
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
    if !logs_dir.exists() {
        return Ok(());
    }
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
    let ctime = FileTime::from_creation_time(&metadata).unwrap_or(mtime);

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

fn build_session_index(
    repo: &git2::Repository,
    project: &projects::Project,
    index: &mut git2::Index,
) -> Result<()> {
    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    let session_dir = project.session_path();
    for session_file in fs::list_files(&session_dir)? {
        let file_path = Path::new(&session_file);
        add_session_path(&repo, index, project, &file_path)
            .with_context(|| format!("failed to add session file: {}", file_path.display()))?;
    }

    Ok(())
}

// this is a helper function for build_gb_tree that takes paths under .git/gb/session and adds them to the in-memory index
fn add_session_path(
    repo: &git2::Repository,
    index: &mut git2::Index,
    project: &projects::Project,
    rel_file_path: &Path,
) -> Result<()> {
    let file_path = project.session_path().join(rel_file_path);

    log::debug!("adding session path: {}", file_path.display());

    let blob = repo.blob_path(&file_path)?;
    let metadata = file_path.metadata()?;
    let mtime = FileTime::from_last_modification_time(&metadata);
    let ctime = FileTime::from_creation_time(&metadata).unwrap_or(mtime);

    // create a new IndexEntry from the file metadata
    index
        .add(&git2::IndexEntry {
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
        })
        .with_context(|| {
            format!(
                "Failed to add session file to index: {}",
                file_path.display()
            )
        })?;

    Ok(())
}

// write a new commit object to the repo
// this is called once we have a tree of deltas, metadata and current wd snapshot
// and either creates or updates the refs/gitbutler/current ref
fn write_gb_commit(
    gb_tree: git2::Oid,
    repo: &git2::Repository,
    user: Option<users::User>,
    project: &projects::Project,
) -> Result<git2::Oid> {
    // find the Oid of the commit that refs/.../current points to, none if it doesn't exist
    let refname = project.refname();

    let comitter = git2::Signature::now("gitbutler", "gitbutler@localhost")?;

    let author = match user {
        None => comitter.clone(),
        Some(user) => git2::Signature::now(user.name.as_str(), user.email.as_str())?,
    };

    match repo.revparse_single(refname.as_str()) {
        Ok(obj) => {
            let last_commit = repo.find_commit(obj.id()).unwrap();
            let new_commit = repo.commit(
                Some(refname.as_str()),
                &author,                           // author
                &comitter,                         // committer
                "gitbutler check",                 // commit message
                &repo.find_tree(gb_tree).unwrap(), // tree
                &[&last_commit],                   // parents
            )?;
            Ok(new_commit)
        }
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                let new_commit = repo.commit(
                    Some(refname.as_str()),
                    &author,                           // author
                    &comitter,                         // committer
                    "gitbutler check",                 // commit message
                    &repo.find_tree(gb_tree).unwrap(), // tree
                    &[],                               // parents
                )?;
                Ok(new_commit)
            } else {
                return Err(e.into());
            }
        }
    }
}
