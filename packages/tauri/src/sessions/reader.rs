use std::{collections::HashMap, path};

use anyhow::{anyhow, Context, Result};

use crate::{
    gb_repository, git,
    reader::{self, CommitReader, Reader},
};

use super::Session;

pub struct SessionReader<'reader> {
    // reader for the current session. commit or wd
    reader: Box<dyn reader::Reader + 'reader>,
    // reader for the previous session's commit
    previous_reader: reader::CommitReader<'reader>,
}

impl Reader for SessionReader<'_> {
    fn read(&self, file_path: &path::Path) -> Result<reader::Content, reader::Error> {
        self.reader.read(file_path)
    }

    fn list_files(&self, dir_path: &path::Path) -> Result<Vec<path::PathBuf>> {
        self.reader.list_files(dir_path)
    }

    fn is_dir(&self, file_path: &path::Path) -> bool {
        self.reader.is_dir(file_path)
    }

    fn exists(&self, file_path: &path::Path) -> bool {
        self.reader.exists(file_path)
    }
}

impl<'reader> SessionReader<'reader> {
    pub fn open(repository: &'reader gb_repository::Repository, session: &Session) -> Result<Self> {
        let wd_reader = reader::DirReader::open(repository.root());

        if let Ok(reader::Content::UTF8(current_session_id)) =
            wd_reader.read(&repository.session_path().join("meta").join("id"))
        {
            if current_session_id == session.id {
                let head_commit = repository.git_repository.head()?.peel_to_commit()?;
                return Ok(SessionReader {
                    reader: Box::new(wd_reader),
                    previous_reader: CommitReader::from_commit(
                        &repository.git_repository,
                        &head_commit,
                    )?,
                });
            }
        }

        let session_hash = if let Some(hash) = &session.hash {
            hash
        } else {
            return Err(anyhow!(
                "can not open reader for {} because it has no commit hash nor it is a current session",
                session.id
            ));
        };

        let oid: git::Oid = session_hash
            .parse()
            .context(format!("failed to parse commit hash {}", session_hash))?;

        let commit = repository
            .git_repository
            .find_commit(oid)
            .context("failed to get commit")?;
        let commit_reader = reader::CommitReader::from_commit(&repository.git_repository, &commit)?;

        Ok(SessionReader {
            reader: Box::new(commit_reader),
            previous_reader: reader::CommitReader::from_commit(
                &repository.git_repository,
                &commit.parent(0)?,
            )?,
        })
    }

    pub fn files(
        &self,
        paths: &Option<Vec<path::PathBuf>>,
    ) -> Result<HashMap<path::PathBuf, reader::Content>> {
        let files = self.previous_reader.list_files(path::Path::new("wd"))?;
        let mut files_with_content = HashMap::new();
        for file_path in files {
            if let Some(paths) = paths.as_ref() {
                if !paths.iter().any(|path| file_path.eq(path)) {
                    continue;
                }
            }
            files_with_content.insert(file_path.clone(), self.file(&file_path)?);
        }

        Ok(files_with_content)
    }

    pub fn file<P: AsRef<path::Path>>(&self, path: P) -> Result<reader::Content, reader::Error> {
        let path = path.as_ref();
        self.previous_reader
            .read(&std::path::Path::new("wd").join(path))
    }
}
