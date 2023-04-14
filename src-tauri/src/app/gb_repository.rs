use std::time;

use anyhow::{anyhow, Context, Ok, Result};
use uuid::Uuid;

use crate::{projects, sessions};

use super::{
    project_repository,
    reader::{self, Reader},
    session,
};

pub struct Repository {
    pub(crate) project_id: String,
    project_store: projects::Storage,
    pub(crate) git_repository: git2::Repository,
}

impl Repository {
    pub fn open<P: AsRef<std::path::Path>>(
        root: P,
        project_id: String,
        project_store: projects::Storage,
    ) -> Result<Self> {
        let path = root.as_ref().join(project_id.clone());
        if path.exists() {
            Ok(Self {
                project_id,
                git_repository: git2::Repository::open(path.clone()).with_context(|| {
                    format!("{}: failed to open git repository", path.display())
                })?,
                project_store,
            })
        } else {
            let git_repository = git2::Repository::init_opts(
                &path,
                &git2::RepositoryInitOptions::new()
                    .initial_head("refs/heads/current")
                    .external_template(false),
            )
            .with_context(|| format!("{}: failed to initialize git repository", path.display()))?;

            {
                // TODO: remove this once flushing is fully working
                let mut index = git_repository.index()?;
                let oid = index.write_tree()?;
                let signature = git2::Signature::now("gitbutler", "gitbutler@localhost").unwrap();
                git_repository.commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    "Initial commit",
                    &git_repository.find_tree(oid)?,
                    &[],
                )?;
            }

            let gb_repository = Self {
                project_id,
                git_repository,
                project_store,
            };

            gb_repository.flush()?;
            Ok(gb_repository)
        }
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
            activity: vec![],
        };

        Ok(session)
    }

    pub fn get_session_writer(
        &self,
        session: &sessions::Session,
    ) -> Result<session::SessionWriter> {
        session::SessionWriter::open(&self, &session)
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
                self.get_session_writer(&session)?;
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
        let current_session = current_session.unwrap();

        let project = self
            .project_store
            .get_project(&self.project_id)
            .context("failed to get project")?;
        if project.is_none() {
            return Err(anyhow!("project does not exist"));
        }
        let project = project.unwrap();

        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let project_wd_reader = project_repository.get_wd_reader();

        let current_session_writer = self
            .get_session_writer(&current_session)
            .context("failed to get session writer")?;

        // read from wd
        // write to session
        // create commit

        Err(anyhow!("not implemented"))
    }

    pub fn get_session_reader(&self, session: sessions::Session) -> Result<session::SessionReader> {
        session::SessionReader::open(&self, session)
    }

    pub fn get_sessions_iterator(&self) -> Result<session::SessionsIterator> {
        Ok(session::SessionsIterator::new(&self.git_repository)?)
    }

    pub fn get_current_session(&self) -> Result<Option<sessions::Session>> {
        let reader = reader::DirReader::open(self.root());
        match sessions::Session::try_from(reader) {
            Result::Ok(session) => Ok(Some(session)),
            Err(sessions::SessionError::NoSession) => Ok(None),
            Err(sessions::SessionError::Err(err)) => Err(err),
        }
    }

    pub(crate) fn root(&self) -> &std::path::Path {
        self.git_repository.path().parent().unwrap()
    }

    pub(crate) fn session_path(&self) -> std::path::PathBuf {
        self.git_repository.path().parent().unwrap().join("session")
    }

    pub(crate) fn deltas_path(&self) -> std::path::PathBuf {
        self.session_path().join("deltas")
    }

    pub(crate) fn wd_path(&self) -> std::path::PathBuf {
        self.session_path().join("wd")
    }

    pub(crate) fn logs_path(&self) -> std::path::PathBuf {
        self.git_repository.path().parent().unwrap().join("logs")
    }
}
