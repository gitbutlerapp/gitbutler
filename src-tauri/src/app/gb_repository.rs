use super::{reader, writer};
use crate::{projects, sessions};
use anyhow::{anyhow, Context, Ok, Result};

pub struct Repository {
    pub(crate) project_id: String,
    pub(crate) git_repository: git2::Repository,
}

impl Repository {
    pub fn open<P: AsRef<std::path::Path>>(root: P, project: &projects::Project) -> Result<Self> {
        let root = root.as_ref();
        let path = root.join(&project.id);
        let git_repository = if path.exists() {
            git2::Repository::open(path)
                .with_context(|| format!("{}: failed to open git repository", project.path))?
        } else {
            // TODO: flush first session instead

            let git_repository = git2::Repository::init_opts(
                &path,
                &git2::RepositoryInitOptions::new()
                    .initial_head("refs/heads/current")
                    .external_template(false),
            )
            .with_context(|| format!("{}: failed to initialize git repository", project.path))?;

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
            project_id: project.id.clone(),
            git_repository,
        })
    }

    pub fn sessions(&self) -> Result<Vec<sessions::Session>> {
        Err(anyhow!("TODO"))
    }

    pub fn get_wd_writer(&self) -> writer::DirWriter {
        writer::DirWriter::open(self.root())
    }

    pub fn get_wd_reader(&self) -> reader::DirReader {
        reader::DirReader::open(self.root())
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
