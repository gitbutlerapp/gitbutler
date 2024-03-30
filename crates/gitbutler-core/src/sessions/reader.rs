use std::{collections::HashMap, path};

use anyhow::{anyhow, Context, Result};

use super::Session;
use crate::{gb_repository, reader};

pub struct SessionReader<'reader> {
    // reader for the current session. commit or wd
    reader: reader::Reader<'reader>,
    // reader for the previous session's commit
    previous_reader: reader::Reader<'reader>,
}

#[derive(thiserror::Error, Debug)]
pub enum FileError {
    #[error(transparent)]
    Reader(#[from] reader::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl<'reader> SessionReader<'reader> {
    pub fn reader(&self) -> &reader::Reader<'reader> {
        &self.reader
    }

    pub fn open(repository: &'reader gb_repository::Repository, session: &Session) -> Result<Self> {
        let wd_reader = reader::Reader::open(&repository.root())?;

        if let Ok(reader::Content::UTF8(current_session_id)) = wd_reader.read("session/meta/id") {
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
        filter: Option<&[&path::Path]>,
    ) -> Result<HashMap<path::PathBuf, reader::Content>, FileError> {
        let wd_dir = path::Path::new("wd");
        let mut paths = self.previous_reader.list_files(wd_dir)?;
        if let Some(filter) = filter {
            paths = paths
                .into_iter()
                .filter(|file_path| filter.iter().any(|path| file_path.eq(path)))
                .collect::<Vec<_>>();
        }
        paths = paths.iter().map(|path| wd_dir.join(path)).collect();
        let files = self
            .previous_reader
            .batch(&paths)
            .context("failed to batch read")?;

        let files = files.into_iter().collect::<Result<Vec<_>, _>>()?;

        Ok(paths
            .into_iter()
            .zip(files)
            .filter_map(|(path, file)| {
                path.strip_prefix(wd_dir)
                    .ok()
                    .map(|path| (path.to_path_buf(), file))
            })
            .collect::<HashMap<_, _>>())
    }

    pub fn file<P: AsRef<path::Path>>(&self, path: P) -> Result<reader::Content, reader::Error> {
        let path = path.as_ref();
        self.previous_reader
            .read(std::path::Path::new("wd").join(path))
    }
}
