use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    os::unix::prelude::MetadataExt,
    sync, time,
};

use anyhow::{anyhow, Context, Ok, Result};
use filetime::FileTime;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{fs, projects, users};

use crate::{
    app::{project_repository, sessions},
    reader::{self, Reader},
};

pub struct Repository {
    pub(crate) project_id: String,
    project_store: projects::Storage,
    users_store: users::Storage,
    pub(crate) git_repository: git2::Repository,
    fslock: sync::Arc<sync::Mutex<fslock::LockFile>>,
}

impl Repository {
    pub fn open<P: AsRef<std::path::Path>>(
        root: P,
        project_id: String,
        project_store: projects::Storage,
        users_store: users::Storage,
    ) -> Result<Self> {
        let project = project_store
            .get_project(&project_id)
            .context("failed to get project")?;
        if project.is_none() {
            return Err(anyhow!("project not found"));
        }
        let project = project.unwrap();

        let project_objects_path = std::path::Path::new(&project.path).join(".git/objects");
        if !project_objects_path.exists() {
            return Err(anyhow!(
                "{}: project objects path does not exist",
                project_objects_path.display()
            ));
        }

        let path = root.as_ref().join("projects").join(project_id.clone());
        let lock_file_path = path.join("lock");

        if path.exists() {
            let git_repository = git2::Repository::open(path.clone())
                .with_context(|| format!("{}: failed to open git repository", path.display()))?;

            git_repository
                .odb()?
                .add_disk_alternate(project_objects_path.to_str().unwrap())
                .context("failed to add disk alternate")?;

            Ok(Self {
                project_id,
                git_repository,
                project_store,
                users_store,
                fslock: sync::Arc::new(sync::Mutex::new(fslock::LockFile::open(&lock_file_path)?)),
            })
        } else {
            let git_repository = git2::Repository::init_opts(
                &path,
                &git2::RepositoryInitOptions::new()
                    .bare(true)
                    .initial_head("refs/heads/current")
                    .external_template(false),
            )
            .with_context(|| format!("{}: failed to initialize git repository", path.display()))?;

            git_repository
                .odb()?
                .add_disk_alternate(project_objects_path.to_str().unwrap())
                .context("failed to add disk alternate")?;

            let gb_repository = Self {
                project_id,
                git_repository,
                project_store,
                users_store,
                fslock: sync::Arc::new(sync::Mutex::new(fslock::LockFile::open(&lock_file_path)?)),
            };

            if gb_repository
                .migrate(&project)
                .context("failed to migrate")?
            {
                log::info!("{}: migrated", gb_repository.project_id);
                return Ok(gb_repository);
            }

            let session = gb_repository
                .create_current_session(&project_repository::Repository::open(&project)?)?;
            gb_repository
                .flush_session(&project_repository::Repository::open(&project)?, &session)
                .context("failed to run initial flush")?;

            Ok(gb_repository)
        }
    }

    pub fn get_project_id(&self) -> &str {
        &self.project_id
    }

    fn remote(&self) -> Result<Option<(git2::Remote, String)>> {
        let user = self.users_store.get().context("failed to get user")?;
        let project = self
            .project_store
            .get_project(&self.project_id)
            .context("failed to get project")?
            .ok_or(anyhow!("project not found"))?;
        let project = project.as_ref();

        // only push if logged in
        let access_token = match user {
            Some(user) => user.access_token.clone(),
            None => return Ok(None),
        };

        // only push if project is connected
        let remote_url = match project.api {
            Some(ref api) => api.git_url.clone(),
            None => return Ok(None),
        };

        let remote = self
            .git_repository
            .remote_anonymous(remote_url.as_str())
            .with_context(|| {
                format!(
                    "failed to create anonymous remote for {}",
                    remote_url.as_str()
                )
            })?;

        Ok(Some((remote, access_token)))
    }

    pub fn fetch(&self) -> Result<bool> {
        let (mut remote, access_token) = match self.remote()? {
            Some((remote, access_token)) => (remote, access_token),
            None => return Ok(false),
        };

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.push_update_reference(move |refname, message| {
            log::info!(
                "{}: pulling reference '{}': {:?}",
                self.project_id,
                refname,
                message
            );
            Result::Ok(())
        });
        callbacks.push_transfer_progress(move |one, two, three| {
            log::info!(
                "{}: transferred {}/{}/{} objects",
                self.project_id,
                one,
                two,
                three
            );
        });

        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        let auth_header = format!("Authorization: {}", access_token);
        let headers = &[auth_header.as_str()];
        fetch_opts.custom_headers(headers);

        remote
            .fetch(
                &["refs/heads/*:refs/remotes/*"],
                Some(&mut fetch_opts),
                None,
            )
            .with_context(|| format!("failed to pull from remote {}", remote.url().unwrap()))?;

        log::info!(
            "{}: fetched from {}",
            self.project_id,
            remote.url().unwrap()
        );

        Ok(true)
    }

    fn push(&self) -> Result<()> {
        let (mut remote, access_token) = match self.remote()? {
            Some((remote, access_token)) => (remote, access_token),
            None => return Ok(()),
        };

        // Set the remote's callbacks
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.push_update_reference(move |refname, message| {
            log::info!(
                "{}: pushing reference '{}': {:?}",
                self.project_id,
                refname,
                message
            );
            Result::Ok(())
        });
        callbacks.push_transfer_progress(move |one, two, three| {
            log::info!(
                "{}: transferred {}/{}/{} objects",
                self.project_id,
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

        let remote_refspec = format!("refs/heads/current:refs/heads/{}", self.project_id);

        // Push to the remote
        remote
            .push(&[remote_refspec], Some(&mut push_options))
            .with_context(|| {
                format!(
                    "failed to push refs/heads/current to {}",
                    remote.url().unwrap()
                )
            })?;

        log::info!("{}: pushed to {}", self.project_id, remote.url().unwrap());

        Ok(())
    }

    fn create_current_session(
        &self,
        project_repository: &project_repository::Repository,
    ) -> Result<sessions::Session> {
        log::info!("{}: creating new session", self.project_id);

        let now_ms = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let meta = match project_repository.get_head() {
            Result::Ok(head) => sessions::Meta {
                start_timestamp_ms: now_ms,
                last_timestamp_ms: now_ms,
                branch: head.name().map(|name| name.to_string()),
                commit: Some(head.peel_to_commit()?.id().to_string()),
            },
            Err(_) => sessions::Meta {
                start_timestamp_ms: now_ms,
                last_timestamp_ms: now_ms,
                branch: None,
                commit: None,
            },
        };

        let session = sessions::Session {
            id: Uuid::new_v4().to_string(),
            hash: None,
            meta,
        };

        // write session to disk
        sessions::Writer::open(&self, &session)?;

        Ok(session)
    }

    pub(crate) fn lock(&self) -> Result<()> {
        self.fslock
            .lock()
            .unwrap()
            .lock()
            .context("failed to lock")?;
        Ok(())
    }

    pub(crate) fn unlock(&self) -> Result<()> {
        self.fslock
            .lock()
            .unwrap()
            .unlock()
            .context("failed to unlock")?;
        Ok(())
    }

    pub fn get_or_create_current_session(&self) -> Result<sessions::Session> {
        match self
            .get_current_session()
            .context("failed to get current session")?
        {
            Some(session) => Ok(session),
            None => {
                let project = self
                    .project_store
                    .get_project(&self.project_id)
                    .context("failed to get project")?;
                if project.is_none() {
                    return Err(anyhow!("project does not exist"));
                }
                let project = project.unwrap();
                let project_repository = project_repository::Repository::open(&project)?;
                let session = self.create_current_session(&project_repository)?;
                Ok(session)
            }
        }
    }

    pub fn flush(&self) -> Result<Option<sessions::Session>> {
        let current_session = self
            .get_current_session()
            .context("failed to get current session")?;
        if current_session.is_none() {
            return Ok(None);
        }

        let project = self
            .project_store
            .get_project(&self.project_id)
            .context("failed to get project")?;
        if project.is_none() {
            return Err(anyhow!("project not found"));
        }
        let project = project.unwrap();

        let current_session = current_session.unwrap();
        let current_session = self.flush_session(
            &project_repository::Repository::open(&project)?,
            &current_session,
        )?;
        Ok(Some(current_session))
    }

    pub fn flush_session(
        &self,
        project_repository: &project_repository::Repository,
        session: &sessions::Session,
    ) -> Result<sessions::Session> {
        if session.hash.is_some() {
            return Ok(session.clone());
        }

        if !self.root().exists() {
            return Err(anyhow!("nothing to flush"));
        }

        // touch session writer to update last timestamp
        sessions::Writer::open(&self, &session)?;

        self.lock()?;
        defer! {
            self.unlock().expect("failed to unlock");
        }

        let wd_tree_oid = build_wd_tree(&self, &project_repository)
            .context("failed to build working directory tree")?;
        let session_tree_oid = build_session_tree(&self).context("failed to build session tree")?;
        let log_tree_oid =
            build_log_tree(&self, &project_repository).context("failed to build logs tree")?;

        let mut tree_builder = self
            .git_repository
            .treebuilder(None)
            .context("failed to create tree builder")?;
        tree_builder
            .insert("session", session_tree_oid, 0o040000)
            .context("failed to insert session tree")?;
        tree_builder
            .insert("wd", wd_tree_oid, 0o040000)
            .context("failed to insert wd tree")?;
        tree_builder
            .insert("logs", log_tree_oid, 0o040000)
            .context("failed to insert logs tree")?;

        let tree = tree_builder.write().context("failed to write tree")?;

        let user = self.users_store.get().context("failed to get user")?;

        let commit_oid =
            write_gb_commit(tree, &self, &user).context("failed to write gb commit")?;

        log::info!(
            "{}: flushed session {} into commit {}",
            self.project_id,
            session.id,
            commit_oid,
        );

        std::fs::remove_dir_all(self.root()).context("failed to remove session directory")?;

        if let Err(e) = self.push() {
            log::error!("{}: failed to push to remote: {:#}", self.project_id, e);
        }

        let session = sessions::Session {
            hash: Some(commit_oid.to_string()),
            ..session.clone()
        };

        Ok(session)
    }

    pub fn get_sessions_iterator<'repository>(
        &'repository self,
    ) -> Result<sessions::SessionsIterator<'repository>> {
        Ok(sessions::SessionsIterator::new(&self.git_repository)?)
    }

    pub fn get_session(&self, session_id: &str) -> Result<sessions::Session> {
        if let Some(oid) = sessions::get_hash_mapping(session_id) {
            let commit = self.git_repository.find_commit(oid)?;
            let reader = reader::CommitReader::from_commit(&self.git_repository, commit)?;
            return Ok(sessions::Session::try_from(reader)?);
        }

        if let Some(session) = self.get_current_session()? {
            if session.id == session_id {
                return Ok(session);
            }
        }

        let mut session_ids_iterator = sessions::SessionsIdsIterator::new(&self.git_repository)?;
        while let Some(ids) = session_ids_iterator.next() {
            match ids {
                Result::Ok((oid, sid)) => {
                    if sid == session_id {
                        let commit = self.git_repository.find_commit(oid)?;
                        let reader =
                            reader::CommitReader::from_commit(&self.git_repository, commit)?;
                        return Ok(sessions::Session::try_from(reader)?);
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Err(anyhow!("session not found"))
    }

    pub fn get_current_session(&self) -> Result<Option<sessions::Session>> {
        let reader = reader::DirReader::open(self.root());
        match sessions::Session::try_from(reader) {
            Result::Ok(session) => Ok(Some(session)),
            Err(sessions::SessionError::NoSession) => Ok(None),
            Err(sessions::SessionError::Err(err)) => Err(err),
        }
    }

    pub(crate) fn root(&self) -> std::path::PathBuf {
        self.git_repository.path().join("gitbutler")
    }

    pub(crate) fn session_path(&self) -> std::path::PathBuf {
        self.root().join("session")
    }

    pub(crate) fn deltas_path(&self) -> std::path::PathBuf {
        self.session_path().join("deltas")
    }

    pub(crate) fn session_wd_path(&self) -> std::path::PathBuf {
        self.session_path().join("wd")
    }

    // migrate old data to the new format.
    // TODO: remove once we think everyone has migrated
    fn migrate(&self, project: &projects::Project) -> Result<bool> {
        if !self
            .migrate_history(project)
            .context("failed to migrate history")?
        {
            Ok(false)
        } else {
            let current_session_dir = std::path::Path::new(project.path.as_str())
                .join(".git")
                .join(format!("gb-{}", project.id));
            if current_session_dir.exists() {
                std::fs::rename(current_session_dir, self.root())
                    .context("failed to rename current session directory")?;
            }
            Ok(true)
        }
    }

    fn migrate_history(&self, project: &projects::Project) -> Result<bool> {
        let refname = format!("refs/gitbutler-{}/current", project.id);
        let repo = git2::Repository::open(&project.path).context("failed to open repository")?;
        let reference = repo.find_reference(&refname);
        match reference {
            Err(e) => {
                if e.code() == git2::ErrorCode::NotFound {
                    log::warn!(
                        "{}: reference {} not found, no migration",
                        project.id,
                        refname
                    );
                    return Ok(false);
                }
                Err(e.into())
            }
            Result::Ok(reference) => {
                let mut walker = repo.revwalk()?;
                walker.push(reference.target().unwrap())?;
                walker.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

                let mut migrated = false;
                for id in walker {
                    let id = id?;
                    let commit = repo.find_commit(id)?;

                    let copy_tree = |tree: git2::Tree| -> Result<git2::Oid> {
                        let mut tree_builder = self.git_repository.treebuilder(None)?;
                        for tree_entry in tree.iter() {
                            let path = tree_entry.name().unwrap();
                            let oid = tree_entry.id();
                            let mode = tree_entry.filemode();
                            tree_builder
                                .insert(path, oid, mode)
                                .context("failed to insert tree entry")?;
                        }
                        let tree_oid = tree_builder.write()?;
                        Ok(tree_oid)
                    };

                    let tree = self.git_repository.find_tree(copy_tree(commit.tree()?)?)?;

                    match self.git_repository.head() {
                        Result::Ok(head) => {
                            let parent = head.peel_to_commit()?;
                            self.git_repository
                                .commit(
                                    Some("HEAD"),
                                    &commit.author(),
                                    &commit.committer(),
                                    &commit.message().unwrap(),
                                    &tree,
                                    &[&parent],
                                )
                                .context("failed to commit")?;
                        }
                        Err(_) => {
                            self.git_repository
                                .commit(
                                    Some("HEAD"),
                                    &commit.author(),
                                    &commit.committer(),
                                    &commit.message().unwrap(),
                                    &tree,
                                    &vec![],
                                )
                                .context("failed to commit")?;
                        }
                    };

                    log::warn!("{}: migrated commit {}", project.id, id);
                    migrated = true
                }

                Ok(migrated)
            }
        }
    }

    pub fn purge(&self) -> Result<()> {
        self.project_store
            .purge(&self.project_id)
            .context("failed to delete project from store")?;
        std::fs::remove_dir_all(self.git_repository.path()).context("failed to remove repository")
    }
}

fn build_wd_tree(
    gb_repository: &Repository,
    project_repository: &project_repository::Repository,
) -> Result<git2::Oid> {
    match gb_repository
        .git_repository
        .find_reference("refs/heads/current")
    {
        Result::Ok(reference) => {
            let index = &mut git2::Index::new()?;
            // build the working directory tree from the current commit
            // and the session files
            let tree = reference.peel_to_tree()?;
            let wd_tree_entry = tree.get_name("wd").unwrap();
            let wd_tree = gb_repository.git_repository.find_tree(wd_tree_entry.id())?;
            index.read_tree(&wd_tree)?;

            let session_wd_reader = reader::DirReader::open(gb_repository.session_wd_path());
            let session_wd_files = session_wd_reader
                .list_files(".")
                .context("failed to read session wd files")?;
            for file_path in session_wd_files {
                let abs_path = gb_repository.session_wd_path().join(&file_path);
                let metadata = abs_path.metadata().with_context(|| {
                    format!("failed to get metadata for {}", abs_path.display())
                })?;
                let mtime = FileTime::from_last_modification_time(&metadata);
                let ctime = FileTime::from_creation_time(&metadata).unwrap_or(mtime);

                let file_content = match session_wd_reader
                    .read_to_string(&file_path)
                    .context("failed to read file")
                {
                    Result::Ok(content) => content,
                    Err(e) => {
                        log::error!(
                            "{}: failed to read file {}: {:#}",
                            gb_repository.project_id,
                            abs_path.display(),
                            e
                        );
                        continue;
                    }
                };

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
                        path: file_path.clone().into(),
                        id: gb_repository.git_repository.blob(file_content.as_bytes())?,
                    })
                    .with_context(|| format!("failed to add index entry for {}", file_path))?;
            }

            let wd_tree_oid = index
                .write_tree_to(&gb_repository.git_repository)
                .with_context(|| format!("failed to write wd tree"))?;
            Ok(wd_tree_oid)
        }
        Err(e) => {
            if e.code() != git2::ErrorCode::NotFound {
                return Err(e.into());
            }
            build_wd_tree_from_repo(gb_repository, project_repository)
                .context("failed to build wd index")
        }
    }
}

// build wd index from the working directory files new session wd files
// this is important because we want to make sure session files are in sync with session deltas
fn build_wd_tree_from_repo(
    gb_repository: &Repository,
    project_repository: &project_repository::Repository,
) -> Result<git2::Oid> {
    let mut index = git2::Index::new()?;

    // create a new in-memory git2 index and open the working one so we can cheat if none of the metadata of an entry has changed
    let repo_index = &mut project_repository
        .git_repository
        .index()
        .with_context(|| format!("failed to open repo index"))?;

    let mut added: HashMap<String, bool> = HashMap::new();

    // first, add session/wd files. session/wd are written at the same time as deltas, so it's important to add them first
    // to make sure they are in sync with the deltas
    for file_path in fs::list_files(gb_repository.session_wd_path()).with_context(|| {
        format!(
            "failed to session working directory files list files in {}",
            gb_repository.session_wd_path().display()
        )
    })? {
        let file_path = std::path::Path::new(&file_path);
        if project_repository
            .git_repository
            .is_path_ignored(&file_path)
            .unwrap_or(true)
        {
            continue;
        }

        add_wd_path(
            &mut index,
            repo_index,
            &gb_repository.session_wd_path(),
            &file_path,
            &gb_repository,
        )
        .with_context(|| {
            format!(
                "failed to add session working directory path {}",
                file_path.display()
            )
        })?;
        added.insert(file_path.to_string_lossy().to_string(), true);
    }

    // finally, add files from the working directory if they aren't already in the index
    for file_path in fs::list_files(&project_repository.root()).with_context(|| {
        format!(
            "failed to working directory list files in {}",
            project_repository.root().display()
        )
    })? {
        if added.contains_key(&file_path.to_string_lossy().to_string()) {
            continue;
        }

        let file_path = std::path::Path::new(&file_path);

        if project_repository
            .git_repository
            .is_path_ignored(&file_path)
            .unwrap_or(true)
        {
            continue;
        }

        add_wd_path(
            &mut index,
            repo_index,
            project_repository.root(),
            &file_path,
            &gb_repository,
        )
        .with_context(|| {
            format!(
                "failed to add working directory path {}",
                file_path.display()
            )
        })?;
    }

    let tree_oid = index
        .write_tree_to(&gb_repository.git_repository)
        .context("failed to write tree to repo")?;
    Ok(tree_oid)
}

// take a file path we see and add it to our in-memory index
// we call this from build_initial_wd_tree, which is smart about using the existing index to avoid rehashing files that haven't changed
// and also looks for large files and puts in a placeholder hash in the LFS format
// TODO: actually upload the file to LFS
fn add_wd_path(
    index: &mut git2::Index,
    repo_index: &mut git2::Index,
    dir: &std::path::Path,
    rel_file_path: &std::path::Path,
    gb_repository: &Repository,
) -> Result<()> {
    let file_path = dir.join(rel_file_path);

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
            index.add(&entry).unwrap();
            log::debug!(
                "{}: added existing entry for {}",
                gb_repository.project_id,
                file_path.display()
            );
            return Ok(());
        }
    }

    // something is different, or not found, so we need to create a new entry

    // look for files that are bigger than 4GB, which are not supported by git
    // insert a pointer as the blob content instead
    // TODO: size limit should be configurable
    let blob = if metadata.len() > 100_000_000 {
        log::debug!(
            "{}: file too big: {}",
            gb_repository.project_id,
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
        let lfs_objects_dir = gb_repository.git_repository.path().join("lfs/objects");
        std::fs::create_dir_all(lfs_objects_dir.clone())?;
        let lfs_path = lfs_objects_dir.join(sha);
        std::fs::copy(file_path, lfs_path)?;

        gb_repository
            .git_repository
            .blob(lfs_pointer.as_bytes())
            .unwrap()
    } else {
        // read the file into a blob, get the object id
        gb_repository.git_repository.blob_path(&file_path)?
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
        .with_context(|| format!("failed to add index entry for {}", rel_file_path.display()))?;

    log::debug!(
        "{}: created index entry for {}",
        gb_repository.project_id,
        rel_file_path.display()
    );

    Ok(())
}

/// calculates sha256 digest of a large file as lowercase hex string via streaming buffer
/// used to calculate the hash of large files that are not supported by git
fn sha256_digest(path: &std::path::Path) -> Result<String> {
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

fn build_log_tree(
    gb_repository: &Repository,
    project_repository: &project_repository::Repository,
) -> Result<git2::Oid> {
    let mut index = git2::Index::new()?;

    let logs_dir = project_repository.git_repository.path().join("logs");
    for file_path in fs::list_files(logs_dir).context("failed to list log files")? {
        add_log_path(
            &std::path::Path::new(&file_path),
            &mut index,
            gb_repository,
            &project_repository,
        )
        .with_context(|| format!("failed to add log file to index: {}", file_path.display()))?;
    }

    let tree_oid = index
        .write_tree_to(&gb_repository.git_repository)
        .context("failed to write index to tree")?;

    Ok(tree_oid)
}

fn add_log_path(
    rel_file_path: &std::path::Path,
    index: &mut git2::Index,
    gb_repository: &Repository,
    project_repository: &project_repository::Repository,
) -> Result<()> {
    let file_path = project_repository
        .git_repository
        .path()
        .join("logs")
        .join(rel_file_path);
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
        id: gb_repository.git_repository.blob_path(&file_path)?,
    })?;

    log::debug!("added log path: {}", file_path.display());

    Ok(())
}

fn build_session_tree(gb_repository: &Repository) -> Result<git2::Oid> {
    let mut index = git2::Index::new()?;

    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    for file_path in
        fs::list_files(&gb_repository.session_path()).context("failed to list session files")?
    {
        let file_path = std::path::Path::new(&file_path);
        add_session_path(&gb_repository, &mut index, &file_path)
            .with_context(|| format!("failed to add session file: {}", file_path.display()))?;
    }

    let tree_oid = index
        .write_tree_to(&gb_repository.git_repository)
        .context("failed to write index to tree")?;

    Ok(tree_oid)
}

// this is a helper function for build_gb_tree that takes paths under .git/gb/session and adds them to the in-memory index
fn add_session_path(
    gb_repository: &Repository,
    index: &mut git2::Index,
    rel_file_path: &std::path::Path,
) -> Result<()> {
    let file_path = gb_repository.session_path().join(rel_file_path);

    let blob = gb_repository.git_repository.blob_path(&file_path)?;
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

    log::debug!("added session path: {}", file_path.display());

    Ok(())
}

// write a new commit object to the repo
// this is called once we have a tree of deltas, metadata and current wd snapshot
// and either creates or updates the refs/heads/current ref
fn write_gb_commit(
    tree_id: git2::Oid,
    gb_repository: &Repository,
    user: &Option<users::User>,
) -> Result<git2::Oid> {
    let comitter = git2::Signature::now("gitbutler", "gitbutler@localhost")?;
    let author = match user {
        None => comitter.clone(),
        Some(user) => git2::Signature::now(user.name.as_str(), user.email.as_str())?,
    };

    match gb_repository
        .git_repository
        .revparse_single("refs/heads/current")
    {
        Result::Ok(obj) => {
            let last_commit = gb_repository.git_repository.find_commit(obj.id())?;
            let new_commit = gb_repository.git_repository.commit(
                Some("refs/heads/current"),
                &author,                                                   // author
                &comitter,                                                 // committer
                "gitbutler check",                                         // commit message
                &gb_repository.git_repository.find_tree(tree_id).unwrap(), // tree
                &[&last_commit],                                           // parents
            )?;
            Ok(new_commit)
        }
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                let new_commit = gb_repository.git_repository.commit(
                    Some("refs/heads/current"),
                    &author,                                                   // author
                    &comitter,                                                 // committer
                    "gitbutler check",                                         // commit message
                    &gb_repository.git_repository.find_tree(tree_id).unwrap(), // tree
                    &[],                                                       // parents
                )?;
                Ok(new_commit)
            } else {
                return Err(e.into());
            }
        }
    }
}
