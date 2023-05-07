use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};

use crate::{
    app::{
        gb_repository,
        reader::{self, CommitReader, Reader},
    },
    deltas,
};

use super::Session;

pub struct SessionReader<'reader> {
    // reader for the current session. commit or wd
    reader: Box<dyn reader::Reader + 'reader>,
    // reader for the previous session's commit
    previous_reader: reader::CommitReader<'reader>,
}

impl Reader for SessionReader<'_> {
    fn read(&self, file_path: &str) -> Result<reader::Content, reader::Error> {
        self.reader.read(file_path)
    }

    fn list_files(&self, dir_path: &str) -> Result<Vec<String>> {
        self.reader.list_files(dir_path)
    }

    fn exists(&self, file_path: &str) -> bool {
        self.reader.exists(file_path)
    }

    fn size(&self, file_path: &str) -> Result<usize> {
        self.reader.size(file_path)
    }
}

impl<'reader> SessionReader<'reader> {
    pub fn open(repository: &'reader gb_repository::Repository, session: &Session) -> Result<Self> {
        let wd_reader = reader::DirReader::open(repository.root());

        let current_session_id = wd_reader.read_to_string(
            &repository
                .session_path()
                .join("meta")
                .join("id")
                .to_str()
                .unwrap(),
        );
        if current_session_id.is_ok() && current_session_id.as_ref().unwrap() == &session.id {
            let head_commit = repository.git_repository.head()?.peel_to_commit()?;
            return Ok(SessionReader {
                reader: Box::new(wd_reader),
                previous_reader: CommitReader::from_commit(
                    &repository.git_repository,
                    head_commit,
                )?,
            });
        }

        let session_hash = if let Some(hash) = &session.hash {
            hash
        } else {
            return Err(anyhow!(
                "can not open reader for {} because it has no commit hash nor it is a current session",
                session.id
            ));
        };

        let oid = git2::Oid::from_str(&session_hash)
            .with_context(|| format!("failed to parse commit hash {}", session_hash))?;

        let commit = repository
            .git_repository
            .find_commit(oid)
            .context("failed to get commit")?;
        let commit_reader =
            reader::CommitReader::from_commit(&repository.git_repository, commit.clone())?;

        Ok(SessionReader {
            reader: Box::new(commit_reader),
            previous_reader: reader::CommitReader::from_commit(
                &repository.git_repository,
                commit.parent(0)?,
            )?,
        })
    }

    pub fn files(&self, paths: Option<Vec<&str>>) -> Result<HashMap<String, String>> {
        let files = self.previous_reader.list_files("wd")?;
        let mut files_with_content = HashMap::new();
        for file_path in files {
            if let Some(paths) = paths.as_ref() {
                if !paths.iter().any(|path| file_path.eq(path)) {
                    continue;
                }
            }
            match self
                .previous_reader
                .read(
                    std::path::Path::new("wd")
                        .join(file_path.clone())
                        .to_str()
                        .unwrap(),
                )
                .with_context(|| format!("failed to read {}", file_path))?
            {
                reader::Content::UTF8(content) => {
                    files_with_content.insert(file_path.clone(), content);
                }
                reader::Content::Binary(_) => {}
            }
        }

        Ok(files_with_content)
    }

    pub fn file_deltas<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<Option<Vec<deltas::Delta>>> {
        let path = path.as_ref();
        let file_deltas_path = std::path::Path::new("session/deltas").join(path);
        match self
            .reader
            .read_to_string(file_deltas_path.to_str().unwrap())
        {
            Ok(content) => {
                if content.is_empty() {
                    // this is a leftover from some bug, shouldn't happen anymore
                    Ok(None)
                } else {
                    Ok(Some(serde_json::from_str(&content)?))
                }
            }
            Err(reader::Error::NotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn deltas(&self, paths: Option<Vec<&str>>) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        let deltas_dir = std::path::Path::new("session/deltas");
        let files = self.reader.list_files(deltas_dir.to_str().unwrap())?;
        let mut result = HashMap::new();
        for file_path in files {
            if let Some(paths) = paths.as_ref() {
                if !paths.iter().any(|path| file_path.eq(path)) {
                    continue;
                }
            }
            if let Some(deltas) = self.file_deltas(file_path.clone())? {
                result.insert(file_path, deltas);
            }
        }
        Ok(result)
    }
}
