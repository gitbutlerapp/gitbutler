use std::{fs, path, time};

use anyhow::{anyhow, Context, Result};

use crate::{
    app::{
        reader::{self, Reader},
        session,
        writer::{self, Writer},
    },
    projects,
};

use super::sessions;

#[derive(Clone)]
pub struct Storage {
    project_storage: projects::Storage,
    root: path::PathBuf,
}

impl Storage {
    pub fn new<P: AsRef<path::Path>>(root: P, project_storage: projects::Storage) -> Self {
        Self {
            project_storage,
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn get_lock(&self, project_id: &str) -> Result<fslock::LockFile> {
        fs::create_dir_all(self.root_path(project_id)).context("failed to create session dir")?;
        let lock_file_path = self.root_path(project_id).join("lock");
        fslock::LockFile::open(&lock_file_path)
            .with_context(|| format!("{}: failed to open lock file", lock_file_path.display()))
    }

    pub fn write(&self, project_id: &str, session: &sessions::Session) -> Result<()> {
        if session.hash.is_some() {
            return Err(anyhow!("cannot write session with hash"));
        }

        self.get_lock(project_id)?.lock()?;

        let session_reader = reader::DirReader::open(self.root_path(project_id));
        if let Ok(session_id) = session_reader.read_to_string("gitbutler/session/meta/id") {
            if session_id != session.id {
                return Err(anyhow!("session already exists"));
            }
        }

        let session_writer = writer::DirWriter::open(self.root_path(project_id));
        session_writer
            .write_string(
                "session/meta/last",
                time::SystemTime::now()
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    .to_string()
                    .as_str(),
            )
            .context("failed to write session meta last")?;

        session_writer
            .write_string("session/meta/id", session.id.as_str())
            .context("failed to write session meta id")?;

        session_writer
            .write_string(
                "session/meta/start",
                session.meta.start_timestamp_ms.to_string().as_str(),
            )
            .context("failed to write session meta start")?;

        if let Some(branch) = session.meta.branch.as_ref() {
            session_writer
                .write_string("session/meta/branch", branch.as_str())
                .context("failed to write session meta branch")?;
        }

        if let Some(commit) = session.meta.commit.as_ref() {
            session_writer
                .write_string("session/meta/commit", commit.as_str())
                .context("failed to write session meta commit")?;
        }

        Ok(())
    }

    pub fn get_current(&self, project_id: &str) -> Result<Option<sessions::Session>> {
        let reader = reader::DirReader::open(self.root_path(project_id));
        match sessions::Session::try_from(reader) {
            Result::Ok(session) => Ok(Some(session)),
            Err(sessions::SessionError::NoSession) => Ok(None),
            Err(sessions::SessionError::Err(err)) => Err(err),
        }
    }

    fn root_path(&self, project_id: &str) -> path::PathBuf {
        self.root
            .join("projects")
            .join(project_id)
            .join("gitbutler")
    }

    fn open_git_repository(&self, project_id: &str) -> Result<git2::Repository> {
        let project = self
            .project_storage
            .get_project(project_id)?
            .ok_or_else(|| anyhow!("{}: project does not exist", project_id))?;

        let git_repository = git2::Repository::open(self.root.join("projects").join(&project.id))
            .context("failed to open git repository")?;

        let project_objects_path = std::path::Path::new(&project.path).join(".git/objects");
        if project_objects_path.exists() {
            git_repository
                .odb()?
                .add_disk_alternate(project_objects_path.to_str().unwrap())
                .context("failed to add disk alternate")?;
        }

        return Ok(git_repository);
    }

    pub fn get_by_id(&self, project_id: &str, id: &str) -> Result<Option<sessions::Session>> {
        if let Some(oid) = session::get_hash_mapping(id) {
            let git_repository = self
                .open_git_repository(project_id)
                .context("failed to open git repository")?;
            let commit = git_repository.find_commit(oid)?;
            let reader = reader::CommitReader::from_commit(&git_repository, commit)?;
            return Ok(Some(sessions::Session::try_from(reader)?));
        }

        if let Some(session) = self.get_current(project_id)? {
            if session.id == id {
                return Ok(Some(session));
            }
        }

        let git_repository = self
            .open_git_repository(project_id)
            .context("failed to open git repository")?;
        let mut session_ids_iterator = session::SessionsIdsIterator::new(&git_repository)?;
        while let Some(ids) = session_ids_iterator.next() {
            match ids {
                Result::Ok((oid, sid)) => {
                    if sid == id {
                        let commit = git_repository.find_commit(oid)?;
                        let reader = reader::CommitReader::from_commit(&git_repository, commit)?;
                        return Ok(Some(sessions::Session::try_from(reader)?));
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok(None)
    }

    pub fn list(
        &self,
        project_id: &str,
        earliest_timestamp_ms: Option<u128>,
    ) -> Result<Vec<sessions::Session>> {
        let git_repository = self
            .open_git_repository(project_id)
            .context("failed to open git repository")?;
        let mut iterator = session::SessionsIterator::new(&git_repository)?;
        let mut sessions = vec![];
        while let Some(session) = iterator.next() {
            if let Err(e) = session {
                return Err(e);
            }

            if let Some(earliest_timestamp_ms) = earliest_timestamp_ms {
                if session.as_ref().unwrap().meta.start_timestamp_ms < earliest_timestamp_ms {
                    break;
                }
            }

            sessions.push(session.unwrap());
        }

        if let Some(current_session) = self.get_current(project_id)? {
            sessions.insert(0, current_session);
        }

        Ok(sessions)
    }
}
