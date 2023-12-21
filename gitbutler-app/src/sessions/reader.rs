use std::{collections::HashMap, path};

use anyhow::{anyhow, Context, Result};

use crate::{gb_repository, reader};

use super::Session;

pub struct SessionReader<'reader> {
    // reader for the current session. commit or wd
    reader: reader::Reader<'reader>,
    // reader for the previous session's commit
    previous_reader: reader::Reader<'reader>,
}

impl<'reader> SessionReader<'reader> {
    pub fn reader(&self) -> &reader::Reader<'reader> {
        &self.reader
    }

    pub fn open(repository: &'reader gb_repository::Repository, session: &Session) -> Result<Self> {
        let wd_reader = reader::Reader::open(&repository.root())?;

        if let Ok(reader::Content::UTF8(current_session_id)) =
            wd_reader.read(repository.session_path().join("meta").join("id"))
        {
            if current_session_id == session.id.to_string() {
                let head_commit = repository.git_repository().head()?.peel_to_commit()?;
                return Ok(SessionReader {
                    reader: wd_reader,
                    previous_reader: reader::Reader::from_commit(
                        repository.git_repository(),
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

        let commit = repository
            .git_repository()
            .find_commit(*session_hash)
            .context("failed to get commit")?;
        let commit_reader = reader::Reader::from_commit(repository.git_repository(), &commit)?;

        Ok(SessionReader {
            reader: commit_reader,
            previous_reader: reader::Reader::from_commit(
                repository.git_repository(),
                &commit.parent(0)?,
            )?,
        })
    }

    pub fn files(
        &self,
        paths: Option<&[path::PathBuf]>,
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
            .read(std::path::Path::new("wd").join(path))
    }
}
