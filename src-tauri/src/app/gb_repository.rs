use std::time;

use anyhow::{anyhow, Context, Ok, Result};
use uuid::Uuid;

use crate::{projects, sessions};

use super::{project_repository, reader, session};

pub struct Repository {
    pub(crate) project_id: String,
    project_store: projects::Storage,
    git_repository: git2::Repository,
}

impl Repository {
    pub fn open<P: AsRef<std::path::Path>>(
        root: P,
        project_id: String,
        project_store: projects::Storage,
    ) -> Result<Self> {
        let path = root.as_ref().join(project_id.clone());
        let git_repository = if path.exists() {
            git2::Repository::open(path.clone())
                .with_context(|| format!("{}: failed to open git repository", path.display()))?
        } else {
            // TODO: flush first session instead

            let git_repository = git2::Repository::init_opts(
                &path,
                &git2::RepositoryInitOptions::new()
                    .initial_head("refs/heads/current")
                    .external_template(false),
            )
            .with_context(|| format!("{}: failed to initialize git repository", path.display()))?;

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

            git_repository
        };
        Ok(Self {
            project_id,
            git_repository,
            project_store,
        })
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

    pub fn get_current_session_writer(&self) -> Result<session::SessionWriter> {
        match self
            .get_current_session()
            .context("failed to get current session")?
        {
            Some(session) => Ok(session::SessionWriter::open(&self, session)?),
            None => {
                let project = self
                    .project_store
                    .get_project(&self.project_id)
                    .context("failed to get project")?;
                if project.is_none() {
                    return Err(anyhow!("project {} does not exist", self.project_id));
                }
                let project = project.unwrap();
                let project_repository = project_repository::Repository::open(&project)?;
                let session = self.create_current_session(&project_repository)?;
                Ok(session::SessionWriter::open(&self, session)?)
            }
        }
    }

    pub fn get_wd_reader(&self) -> reader::DirReader {
        reader::DirReader::open(self.root())
    }

    pub fn get_commit_reader(&self, oid: git2::Oid) -> Result<reader::CommitReader> {
        let commit = self
            .git_repository
            .find_commit(oid)
            .context("failed to get commit")?;
        let reader = reader::CommitReader::from_commit(&self.git_repository, commit)?;
        Ok(reader)
    }

    pub fn get_head_reader(&self) -> Result<reader::CommitReader> {
        let head = self.git_repository.head().context("failed to get HEAD")?;
        let commit = head.peel_to_commit().context("failed to get HEAD commit")?;
        let reader = reader::CommitReader::from_commit(&self.git_repository, commit)?;
        Ok(reader)
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
