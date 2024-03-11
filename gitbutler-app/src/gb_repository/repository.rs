use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Read},
    path, time,
};

#[cfg(target_os = "windows")]
use crate::windows::MetadataShim;
#[cfg(target_family = "unix")]
use std::os::unix::prelude::*;

use anyhow::{anyhow, Context, Result};
use filetime::FileTime;
use fslock::LockFile;
use sha2::{Digest, Sha256};

use crate::{
    deltas, fs, git, project_repository,
    projects::{self, ProjectId},
    reader, sessions,
    sessions::SessionId,
    users,
    virtual_branches::{self, target},
};

pub struct Repository {
    git_repository: git::Repository,
    project: projects::Project,
    lock_path: path::PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("path not found: {0}")]
    ProjectPathNotFound(path::PathBuf),
    #[error(transparent)]
    Git(#[from] git::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error("path has invalid utf-8 bytes: {0}")]
    InvalidUnicodePath(path::PathBuf),
}

impl Repository {
    pub fn open(
        root: &path::Path,
        project_repository: &project_repository::Repository,
        user: Option<&users::User>,
    ) -> Result<Self, Error> {
        let project = project_repository.project();
        let project_objects_path = project.path.join(".git/objects");
        if !project_objects_path.exists() {
            return Err(Error::ProjectPathNotFound(project_objects_path));
        }

        let projects_dir = root.join("projects");
        let path = projects_dir.join(project.id.to_string());

        let lock_path = projects_dir.join(format!("{}.lock", project.id));

        if path.exists() {
            let git_repository = git::Repository::open(path.clone())
                .with_context(|| format!("{}: failed to open git repository", path.display()))?;

            git_repository
                .add_disk_alternate(project_objects_path.to_str().unwrap())
                .context("failed to add disk alternate")?;

            Result::Ok(Self {
                git_repository,
                project: project.clone(),
                lock_path,
            })
        } else {
            std::fs::create_dir_all(&path).context("failed to create project directory")?;

            let git_repository = git::Repository::init_opts(
                &path,
                git2::RepositoryInitOptions::new()
                    .bare(true)
                    .initial_head("refs/heads/current")
                    .external_template(false),
            )
            .with_context(|| format!("{}: failed to initialize git repository", path.display()))?;

            git_repository
                .add_disk_alternate(project_objects_path.to_str().unwrap())
                .context("failed to add disk alternate")?;

            let gb_repository = Self {
                git_repository,
                project: project.clone(),
                lock_path,
            };

            let _lock = gb_repository.lock();
            let session = gb_repository.create_current_session(project_repository)?;
            drop(_lock);

            gb_repository
                .flush_session(project_repository, &session, user)
                .context("failed to run initial flush")?;

            Result::Ok(gb_repository)
        }
    }

    pub fn get_project_id(&self) -> &ProjectId {
        &self.project.id
    }

    fn remote(&self, user: Option<&users::User>) -> Result<Option<(git::Remote, String)>> {
        // only push if logged in
        let access_token = match user {
            Some(user) => user.access_token.clone(),
            None => return Ok(None),
        };

        // only push if project is connected
        let remote_url = match &self.project.api {
            Some(api) => api.git_url.clone(),
            None => return Ok(None),
        };

        let remote = self
            .git_repository
            .remote_anonymous(&remote_url.parse().unwrap())
            .with_context(|| {
                format!(
                    "failed to create anonymous remote for {}",
                    remote_url.as_str()
                )
            })?;

        Ok(Some((remote, access_token)))
    }

    pub fn fetch(&self, user: Option<&users::User>) -> Result<(), RemoteError> {
        let (mut remote, access_token) = match self.remote(user)? {
            Some((remote, access_token)) => (remote, access_token),
            None => return Result::Ok(()),
        };

        let mut callbacks = git2::RemoteCallbacks::new();
        if self.project.omit_certificate_check.unwrap_or(false) {
            callbacks.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
        }
        callbacks.push_update_reference(move |refname, message| {
            tracing::debug!(
                project_id = %self.project.id,
                refname,
                message,
                "pulling reference"
            );
            Result::Ok(())
        });
        callbacks.push_transfer_progress(move |one, two, three| {
            tracing::debug!(
                project_id = %self.project.id,
                "transferred {}/{}/{} objects",
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
            .fetch(&["refs/heads/*:refs/remotes/*"], Some(&mut fetch_opts))
            .map_err(|error| match error {
                git::Error::Network(error) => {
                    tracing::warn!(project_id = %self.project.id, error = %error, "failed to fetch gb repo");
                    RemoteError::Network
                }
                error => RemoteError::Other(error.into()),
            })?;

        tracing::info!(
            project_id = %self.project.id,
            "gb repo fetched",
        );

        Result::Ok(())
    }

    pub fn push(&self, user: Option<&users::User>) -> Result<(), RemoteError> {
        let (mut remote, access_token) = match self.remote(user)? {
            Some((remote, access_token)) => (remote, access_token),
            None => return Ok(()),
        };

        // Set the remote's callbacks
        let mut callbacks = git2::RemoteCallbacks::new();
        if self.project.omit_certificate_check.unwrap_or(false) {
            callbacks.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
        }
        callbacks.push_update_reference(move |refname, message| {
            tracing::debug!(
                project_id = %self.project.id,
                refname,
                message,
                "pushing reference"
            );
            Result::Ok(())
        });
        callbacks.push_transfer_progress(move |current, total, bytes| {
            tracing::debug!(
                project_id = %self.project.id,
                "transferred {}/{}/{} objects",
                current,
                total,
                bytes
            );
        });

        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);
        let auth_header = format!("Authorization: {}", access_token);
        let headers = &[auth_header.as_str()];
        push_options.custom_headers(headers);

        let remote_refspec = format!("refs/heads/current:refs/heads/{}", self.project.id);

        // Push to the remote
        remote
            .push(&[&remote_refspec], Some(&mut push_options)).map_err(|error| match error {
                git::Error::Network(error) => {
                    tracing::warn!(project_id = %self.project.id, error = %error, "failed to push gb repo");
                    RemoteError::Network
                }
                error => RemoteError::Other(error.into()),
            })?;

        tracing::info!(project_id = %self.project.id,  "gb repository pushed");

        Ok(())
    }

    // take branches from the last session and put them into the current session
    fn copy_branches(&self) -> Result<()> {
        let last_session = self
            .get_sessions_iterator()
            .context("failed to get sessions iterator")?
            .next();
        if last_session.is_none() {
            return Ok(());
        }
        let last_session = last_session
            .unwrap()
            .context("failed to read last session")?;
        let last_session_reader = sessions::Reader::open(self, &last_session)
            .context("failed to open last session reader")?;

        let branches = virtual_branches::Iterator::new(&last_session_reader)
            .context("failed to read virtual branches")?
            .collect::<Result<Vec<_>, reader::Error>>()
            .context("failed to read virtual branches")?
            .into_iter()
            .collect::<Vec<_>>();

        let src_target_reader = virtual_branches::target::Reader::new(&last_session_reader);
        let dst_target_writer = virtual_branches::target::Writer::new(self)
            .context("failed to open target writer for current session")?;

        // copy default target
        let default_target = match src_target_reader.read_default() {
            Result::Ok(target) => Ok(Some(target)),
            Err(reader::Error::NotFound) => Ok(None),
            Err(err) => Err(err).context("failed to read default target"),
        }?;
        if let Some(default_target) = default_target.as_ref() {
            dst_target_writer
                .write_default(default_target)
                .context("failed to write default target")?;
        }

        // copy branch targets
        for branch in &branches {
            let target = src_target_reader
                .read(&branch.id)
                .with_context(|| format!("{}: failed to read target", branch.id))?;
            if let Some(default_target) = default_target.as_ref() {
                if *default_target == target {
                    continue;
                }
            }
            dst_target_writer
                .write(&branch.id, &target)
                .with_context(|| format!("{}: failed to write target", branch.id))?;
        }

        let dst_branch_writer = virtual_branches::branch::Writer::new(self)
            .context("failed to open branch writer for current session")?;

        // copy branches that we don't already have
        for branch in &branches {
            dst_branch_writer
                .write(&mut branch.clone())
                .with_context(|| format!("{}: failed to write branch", branch.id))?;
        }

        Ok(())
    }

    fn create_current_session(
        &self,
        project_repository: &project_repository::Repository,
    ) -> Result<sessions::Session> {
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
            id: SessionId::generate(),
            hash: None,
            meta,
        };

        // write session to disk
        sessions::Writer::new(self)
            .context("failed to create session writer")?
            .write(&session)
            .context("failed to write session")?;

        tracing::info!(
            project_id = %self.project.id,
            session_id = %session.id,
            "created new session"
        );

        self.flush_gitbutler_file(&session.id)?;

        Ok(session)
    }

    pub fn lock(&self) -> LockFile {
        let mut lockfile = LockFile::open(&self.lock_path).expect("failed to open lock file");
        lockfile.lock().expect("failed to obtain lock on lock file");
        lockfile
    }

    pub fn mark_active_session(&self) -> Result<()> {
        let current_session = self
            .get_or_create_current_session()
            .context("failed to get current session")?;

        let updated_session = sessions::Session {
            meta: sessions::Meta {
                last_timestamp_ms: time::SystemTime::now()
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                ..current_session.meta
            },
            ..current_session
        };

        sessions::Writer::new(self)
            .context("failed to create session writer")?
            .write(&updated_session)
            .context("failed to write session")?;

        Ok(())
    }

    pub fn get_latest_session(&self) -> Result<Option<sessions::Session>> {
        if let Some(current_session) = self.get_current_session()? {
            Ok(Some(current_session))
        } else {
            let mut sessions_iterator = self.get_sessions_iterator()?;
            sessions_iterator
                .next()
                .transpose()
                .context("failed to get latest session")
        }
    }

    pub fn get_or_create_current_session(&self) -> Result<sessions::Session> {
        let _lock = self.lock();

        let reader = reader::Reader::open(&self.root())?;
        match sessions::Session::try_from(&reader) {
            Result::Ok(session) => Ok(session),
            Err(sessions::SessionError::NoSession) => {
                let project_repository = project_repository::Repository::open(&self.project)
                    .context("failed to open project repository")?;
                let session = self
                    .create_current_session(&project_repository)
                    .context("failed to create current session")?;
                drop(_lock);
                self.copy_branches().context("failed to unpack branches")?;
                Ok(session)
            }
            Err(err) => Err(err).context("failed to read current session"),
        }
    }

    #[cfg(test)]
    pub fn flush(
        &self,
        project_repository: &project_repository::Repository,
        user: Option<&users::User>,
    ) -> Result<Option<sessions::Session>> {
        let current_session = self
            .get_current_session()
            .context("failed to get current session")?;
        if current_session.is_none() {
            return Ok(None);
        }

        let current_session = current_session.unwrap();
        let current_session = self
            .flush_session(project_repository, &current_session, user)
            .context(format!("failed to flush session {}", current_session.id))?;
        Ok(Some(current_session))
    }

    pub fn flush_session(
        &self,
        project_repository: &project_repository::Repository,
        session: &sessions::Session,
        user: Option<&users::User>,
    ) -> Result<sessions::Session> {
        if session.hash.is_some() {
            return Ok(session.clone());
        }

        if !self.root().exists() {
            return Err(anyhow!("nothing to flush"));
        }

        let _lock = self.lock();

        // update last timestamp
        let session_writer =
            sessions::Writer::new(self).context("failed to create session writer")?;
        session_writer.write(session)?;

        let mut tree_builder = self.git_repository.treebuilder(None);

        tree_builder.upsert(
            "session",
            build_session_tree(self).context("failed to build session tree")?,
            git::FileMode::Tree,
        );
        tree_builder.upsert(
            "wd",
            build_wd_tree(self, project_repository)
                .context("failed to build working directory tree")?,
            git::FileMode::Tree,
        );
        tree_builder.upsert(
            "branches",
            build_branches_tree(self).context("failed to build branches tree")?,
            git::FileMode::Tree,
        );

        let tree_id = tree_builder.write().context("failed to write tree")?;

        let commit_oid =
            write_gb_commit(tree_id, self, user).context("failed to write gb commit")?;

        tracing::info!(
            project_id = %self.project.id,
            session_id = %session.id,
            %commit_oid,
            "flushed session"
        );

        session_writer.remove()?;

        let session = sessions::Session {
            hash: Some(commit_oid),
            ..session.clone()
        };

        Ok(session)
    }

    pub fn get_sessions_iterator(&self) -> Result<sessions::SessionsIterator<'_>> {
        sessions::SessionsIterator::new(&self.git_repository)
    }

    pub fn get_current_session(&self) -> Result<Option<sessions::Session>> {
        let _lock = self.lock();
        let reader = reader::Reader::open(&self.root())?;
        match sessions::Session::try_from(&reader) {
            Ok(session) => Ok(Some(session)),
            Err(sessions::SessionError::NoSession) => Ok(None),
            Err(sessions::SessionError::Other(err)) => Err(err),
        }
    }

    pub(crate) fn root(&self) -> std::path::PathBuf {
        self.git_repository.path().join("gitbutler")
    }

    pub(crate) fn session_path(&self) -> std::path::PathBuf {
        self.root().join("session")
    }

    pub(crate) fn session_wd_path(&self) -> std::path::PathBuf {
        self.session_path().join("wd")
    }

    pub fn default_target(&self) -> Result<Option<target::Target>> {
        if let Some(latest_session) = self.get_latest_session()? {
            let latest_session_reader = sessions::Reader::open(self, &latest_session)
                .context("failed to open current session")?;
            let target_reader = target::Reader::new(&latest_session_reader);
            match target_reader.read_default() {
                Result::Ok(target) => Ok(Some(target)),
                Err(reader::Error::NotFound) => Ok(None),
                Err(err) => Err(err.into()),
            }
        } else {
            Ok(None)
        }
    }

    fn flush_gitbutler_file(&self, session_id: &SessionId) -> Result<()> {
        let gb_path = self.git_repository.path();
        let project_id = self.project.id.to_string();
        let gb_file_content = serde_json::json!({
            "sessionId": session_id,
            "repositoryId": project_id,
            "gbPath": gb_path,
            "api": self.project.api,
        });

        let gb_file_path = self.project.path.join(".git/gitbutler.json");
        std::fs::write(&gb_file_path, gb_file_content.to_string())?;

        tracing::debug!("gitbutler file updated: {:?}", gb_file_path);

        Ok(())
    }

    pub fn git_repository(&self) -> &git::Repository {
        &self.git_repository
    }
}

fn build_wd_tree(
    gb_repository: &Repository,
    project_repository: &project_repository::Repository,
) -> Result<git::Oid> {
    match gb_repository
        .git_repository
        .find_reference(&"refs/heads/current".parse().unwrap())
    {
        Result::Ok(reference) => build_wd_tree_from_reference(gb_repository, &reference)
            .context("failed to build wd index"),
        Err(git::Error::NotFound(_)) => build_wd_tree_from_repo(gb_repository, project_repository)
            .context("failed to build wd index"),
        Err(e) => Err(e.into()),
    }
}

fn build_wd_tree_from_reference(
    gb_repository: &Repository,
    reference: &git::Reference,
) -> Result<git::Oid> {
    // start off with the last tree as a base
    let tree = reference.peel_to_tree()?;
    let wd_tree_entry = tree.get_name("wd").unwrap();
    let wd_tree = gb_repository.git_repository.find_tree(wd_tree_entry.id())?;
    let mut index = git::Index::try_from(&wd_tree)?;

    // write updated files on top of the last tree
    for file_path in fs::list_files(gb_repository.session_wd_path(), &[]).with_context(|| {
        format!(
            "failed to session working directory files list files in {}",
            gb_repository.session_wd_path().display()
        )
    })? {
        add_wd_path(
            &mut index,
            &gb_repository.session_wd_path(),
            &file_path,
            gb_repository,
        )
        .with_context(|| {
            format!(
                "failed to add session working directory path {}",
                file_path.display()
            )
        })?;
    }

    let session_reader = reader::Reader::open(&gb_repository.root())?;
    let deltas = deltas::Reader::from(&session_reader)
        .read(None)
        .context("failed to read deltas")?;
    let wd_files = session_reader.list_files(path::Path::new("session/wd"))?;
    let wd_files = wd_files.iter().collect::<HashSet<_>>();

    // if a file has delta, but doesn't exist in wd, it was deleted
    let deleted_files = deltas
        .keys()
        .filter(|key| !wd_files.contains(key))
        .collect::<Vec<_>>();

    for deleted_file in deleted_files {
        index
            .remove_path(deleted_file)
            .context("failed to remove path")?;
    }

    let wd_tree_oid = index
        .write_tree_to(&gb_repository.git_repository)
        .context("failed to write wd tree")?;
    Ok(wd_tree_oid)
}

// build wd index from the working directory files new session wd files
// this is important because we want to make sure session files are in sync with session deltas
fn build_wd_tree_from_repo(
    gb_repository: &Repository,
    project_repository: &project_repository::Repository,
) -> Result<git::Oid> {
    let mut index = git::Index::new()?;

    let mut added: HashMap<String, bool> = HashMap::new();

    // first, add session/wd files. session/wd are written at the same time as deltas, so it's important to add them first
    // to make sure they are in sync with the deltas
    for file_path in fs::list_files(gb_repository.session_wd_path(), &[]).with_context(|| {
        format!(
            "failed to session working directory files list files in {}",
            gb_repository.session_wd_path().display()
        )
    })? {
        if project_repository
            .git_repository
            .is_path_ignored(&file_path)
            .unwrap_or(true)
        {
            continue;
        }

        add_wd_path(
            &mut index,
            &gb_repository.session_wd_path(),
            &file_path,
            gb_repository,
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
    for file_path in fs::list_files(project_repository.root(), &[path::Path::new(".git")])
        .with_context(|| {
            format!(
                "failed to working directory list files in {}",
                project_repository.root().display()
            )
        })?
    {
        if added.contains_key(&file_path.to_string_lossy().to_string()) {
            continue;
        }

        if project_repository
            .git_repository
            .is_path_ignored(&file_path)
            .unwrap_or(true)
        {
            continue;
        }

        add_wd_path(
            &mut index,
            project_repository.root(),
            &file_path,
            gb_repository,
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
    index: &mut git::Index,
    dir: &std::path::Path,
    rel_file_path: &std::path::Path,
    gb_repository: &Repository,
) -> Result<()> {
    let file_path = dir.join(rel_file_path);

    let metadata = std::fs::symlink_metadata(&file_path).context("failed to get metadata for")?;
    let modify_time = FileTime::from_last_modification_time(&metadata);
    let create_time = FileTime::from_creation_time(&metadata).unwrap_or(modify_time);

    // look for files that are bigger than 4GB, which are not supported by git
    // insert a pointer as the blob content instead
    // TODO: size limit should be configurable
    let blob = if metadata.is_symlink() {
        // it's a symlink, make the content the path of the link
        let link_target = std::fs::read_link(&file_path)?;
        // if the link target is inside the project repository, make it relative
        let link_target = link_target.strip_prefix(dir).unwrap_or(&link_target);
        gb_repository.git_repository.blob(
            link_target
                .to_str()
                .ok_or_else(|| Error::InvalidUnicodePath(link_target.into()))?
                .as_bytes(),
        )?
    } else if metadata.len() > 100_000_000 {
        tracing::warn!(
            project_id = %gb_repository.project.id,
            path = %file_path.display(),
            "file too big"
        );

        // get a sha256 hash of the file first
        let sha = sha256_digest(&file_path)?;

        // put togther a git lfs pointer file: https://github.com/git-lfs/git-lfs/blob/main/docs/spec.md
        let mut lfs_pointer = String::from("version https://git-lfs.github.com/spec/v1\n");
        lfs_pointer.push_str("oid sha256:");
        lfs_pointer.push_str(&sha);
        lfs_pointer.push('\n');
        lfs_pointer.push_str("size ");
        lfs_pointer.push_str(&metadata.len().to_string());
        lfs_pointer.push('\n');

        // write the file to the .git/lfs/objects directory
        // create the directory recursively if it doesn't exist
        let lfs_objects_dir = gb_repository.git_repository.path().join("lfs/objects");
        std::fs::create_dir_all(lfs_objects_dir.clone())?;
        let lfs_path = lfs_objects_dir.join(sha);
        std::fs::copy(file_path, lfs_path)?;

        gb_repository.git_repository.blob(lfs_pointer.as_bytes())?
    } else {
        // read the file into a blob, get the object id
        gb_repository.git_repository.blob_path(&file_path)?
    };

    // create a new IndexEntry from the file metadata
    // truncation is ok https://libgit2.org/libgit2/#HEAD/type/git_index_entry
    #[allow(clippy::cast_possible_truncation)]
    index
        .add(&git::IndexEntry {
            ctime: create_time,
            mtime: modify_time,
            dev: metadata.dev() as u32,
            ino: metadata.ino() as u32,
            mode: 33188,
            uid: metadata.uid(),
            gid: metadata.gid(),
            file_size: metadata.len() as u32,
            flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
            flags_extended: 0, // no extended flags
            path: rel_file_path.to_str().unwrap().to_string().into(),
            id: blob,
        })
        .with_context(|| format!("failed to add index entry for {}", rel_file_path.display()))?;

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

fn build_branches_tree(gb_repository: &Repository) -> Result<git::Oid> {
    let mut index = git::Index::new()?;

    let branches_dir = gb_repository.root().join("branches");
    for file_path in
        fs::list_files(&branches_dir, &[]).context("failed to find branches directory")?
    {
        let file_path = std::path::Path::new(&file_path);
        add_file_to_index(
            gb_repository,
            &mut index,
            file_path,
            &branches_dir.join(file_path),
        )
        .context("failed to add branch file to index")?;
    }

    let tree_oid = index
        .write_tree_to(&gb_repository.git_repository)
        .context("failed to write index to tree")?;

    Ok(tree_oid)
}

fn build_session_tree(gb_repository: &Repository) -> Result<git::Oid> {
    let mut index = git::Index::new()?;

    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    for file_path in fs::list_files(
        gb_repository.session_path(),
        &[path::Path::new("wd").to_path_buf()],
    )
    .context("failed to list session files")?
    {
        add_file_to_index(
            gb_repository,
            &mut index,
            &file_path,
            &gb_repository.session_path().join(&file_path),
        )
        .with_context(|| format!("failed to add session file: {}", file_path.display()))?;
    }

    let tree_oid = index
        .write_tree_to(&gb_repository.git_repository)
        .context("failed to write index to tree")?;

    Ok(tree_oid)
}

// this is a helper function for build_gb_tree that takes paths under .git/gb/session and adds them to the in-memory index
fn add_file_to_index(
    gb_repository: &Repository,
    index: &mut git::Index,
    rel_file_path: &std::path::Path,
    abs_file_path: &std::path::Path,
) -> Result<()> {
    let blob = gb_repository.git_repository.blob_path(abs_file_path)?;
    let metadata = abs_file_path.metadata()?;
    let modified_time = FileTime::from_last_modification_time(&metadata);
    let create_time = FileTime::from_creation_time(&metadata).unwrap_or(modified_time);

    // create a new IndexEntry from the file metadata
    // truncation is ok https://libgit2.org/libgit2/#HEAD/type/git_index_entry
    #[allow(clippy::cast_possible_truncation)]
    index
        .add(&git::IndexEntry {
            ctime: create_time,
            mtime: modified_time,
            dev: metadata.dev() as u32,
            ino: metadata.ino() as u32,
            mode: 33188,
            uid: metadata.uid(),
            gid: metadata.gid(),
            file_size: metadata.len() as u32,
            flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
            flags_extended: 0, // no extended flags
            path: rel_file_path.to_str().unwrap().into(),
            id: blob,
        })
        .with_context(|| format!("Failed to add file to index: {}", abs_file_path.display()))?;

    Ok(())
}

// write a new commit object to the repo
// this is called once we have a tree of deltas, metadata and current wd snapshot
// and either creates or updates the refs/heads/current ref
fn write_gb_commit(
    tree_id: git::Oid,
    gb_repository: &Repository,
    user: Option<&users::User>,
) -> Result<git::Oid> {
    let comitter = git::Signature::now("gitbutler", "gitbutler@localhost")?;
    let author = match user {
        None => comitter.clone(),
        Some(user) => git::Signature::try_from(user)?,
    };

    let current_refname: git::Refname = "refs/heads/current".parse().unwrap();

    match gb_repository
        .git_repository
        .find_reference(&current_refname)
    {
        Result::Ok(reference) => {
            let last_commit = reference.peel_to_commit()?;
            let new_commit = gb_repository.git_repository.commit(
                Some(&current_refname),
                &author,                                                   // author
                &comitter,                                                 // committer
                "gitbutler check",                                         // commit message
                &gb_repository.git_repository.find_tree(tree_id).unwrap(), // tree
                &[&last_commit],                                           // parents
            )?;
            Ok(new_commit)
        }
        Err(git::Error::NotFound(_)) => {
            let new_commit = gb_repository.git_repository.commit(
                Some(&current_refname),
                &author,                                                   // author
                &comitter,                                                 // committer
                "gitbutler check",                                         // commit message
                &gb_repository.git_repository.find_tree(tree_id).unwrap(), // tree
                &[],                                                       // parents
            )?;
            Ok(new_commit)
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("network error")]
    Network,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use anyhow::Result;
    use pretty_assertions::assert_eq;

    use crate::tests::{Case, Suite};

    #[test]
    fn test_alternates_file_being_set() -> Result<()> {
        let Case {
            gb_repository,
            project_repository,
            ..
        } = Suite::default().new_case();

        let file_content = std::fs::read_to_string(
            gb_repository
                .git_repository
                .path()
                .join("objects/info/alternates"),
        )?;

        let file_content = PathBuf::from(file_content.trim());
        let project_path = project_repository.path().to_path_buf().join(".git/objects");

        assert_eq!(file_content, project_path);

        Ok(())
    }
}
