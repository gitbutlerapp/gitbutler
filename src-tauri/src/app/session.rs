use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};

use crate::{deltas, pty, sessions};

use super::{
    gb_repository,
    reader::{self, CommitReader, Reader},
    writer::{self, Writer},
};

pub struct SessionWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: Box<dyn writer::Writer + 'writer>,
}

impl<'writer> SessionWriter<'writer> {
    pub fn open(
        repository: &'writer gb_repository::Repository,
        session: &sessions::Session,
    ) -> Result<Self> {
        if session.hash.is_some() {
            return Err(anyhow!("can not open writer for a session with a hash"));
        }

        let reader = reader::DirReader::open(repository.root());

        let current_session_id = reader.read_to_string(
            repository
                .session_path()
                .join("meta")
                .join("id")
                .to_str()
                .unwrap(),
        );

        if current_session_id.is_ok() && !current_session_id.as_ref().unwrap().eq(&session.id) {
            return Err(anyhow!(
                "{}: can not open writer for {} because a writer for {} is still open",
                repository.project_id,
                session.id,
                current_session_id.unwrap()
            ));
        }

        let writer = writer::DirWriter::open(repository.root().to_path_buf());

        writer
            .write_string(
                repository
                    .session_path()
                    .join("meta")
                    .join("last")
                    .to_str()
                    .unwrap(),
                &session.meta.last_timestamp_ms.to_string(),
            )
            .with_context(|| "failed to write last timestamp")?;

        if current_session_id.is_ok() && current_session_id.as_ref().unwrap().eq(&session.id) {
            let writer = SessionWriter {
                repository: &repository,
                writer: Box::new(writer),
            };
            return Ok(writer);
        }

        writer
            .write_string(
                repository
                    .session_path()
                    .join("meta")
                    .join("id")
                    .to_str()
                    .unwrap(),
                session.id.as_str(),
            )
            .with_context(|| "failed to write id")?;

        writer
            .write_string(
                repository
                    .session_path()
                    .join("meta")
                    .join("start")
                    .to_str()
                    .unwrap(),
                session.meta.start_timestamp_ms.to_string().as_str(),
            )
            .with_context(|| "failed to write start timestamp")?;

        if let Some(branch) = session.meta.branch.as_ref() {
            writer
                .write_string(
                    repository
                        .session_path()
                        .join("meta")
                        .join("branch")
                        .to_str()
                        .unwrap(),
                    branch,
                )
                .with_context(|| "failed to write branch")?;
        }

        if let Some(commit) = session.meta.commit.as_ref() {
            writer
                .write_string(
                    repository
                        .session_path()
                        .join("meta")
                        .join("commit")
                        .to_str()
                        .unwrap(),
                    commit,
                )
                .with_context(|| "failed to write commit")?;
        }

        let writer = SessionWriter {
            repository: &repository,
            writer: Box::new(writer),
        };

        Ok(writer)
    }

    pub fn append_pty(&self, record: &pty::Record) -> Result<()> {
        log::info!(
            "{}: writing pty record to pty.jsonl",
            self.repository.project_id
        );

        serde_json::to_string(record)?;

        serde_jsonlines::append_json_lines(
            &self.repository.session_path().join("pty.jsonl"),
            [record],
        )?;

        Ok(())
    }

    pub fn write_session_wd_file<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        contents: &str,
    ) -> Result<()> {
        let path = path.as_ref();
        log::info!(
            "{}: writing delta wd file to {}",
            self.repository.project_id,
            path.display()
        );

        self.writer.write_string(
            &self
                .repository
                .session_wd_path()
                .join(path)
                .to_str()
                .unwrap(),
            contents,
        )?;

        Ok(())
    }

    pub fn write_deltas<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        deltas: &Vec<deltas::Delta>,
    ) -> Result<()> {
        let path = path.as_ref();
        log::info!(
            "{}: writing deltas to {}",
            self.repository.project_id,
            path.display()
        );

        let raw_deltas = serde_json::to_string(&deltas)?;

        self.writer.write_string(
            &self.repository.deltas_path().join(path).to_str().unwrap(),
            &raw_deltas,
        )?;

        Ok(())
    }
}

pub struct SessionReader<'reader> {
    // reader for the current session. commit or wd
    reader: Box<dyn reader::Reader + 'reader>,
    // reader for the previous session's commit
    previous_reader: Option<CommitReader<'reader>>,
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
    pub fn open(
        repository: &'reader gb_repository::Repository,
        session: &sessions::Session,
    ) -> Result<Self> {
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
                previous_reader: Some(CommitReader::from_commit(
                    &repository.git_repository,
                    head_commit,
                )?),
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
        let parents_count = commit.parent_count();
        let commit_reader =
            reader::CommitReader::from_commit(&repository.git_repository, commit.clone())?;

        let previous_reader = if parents_count > 0 {
            Some(reader::CommitReader::from_commit(
                &repository.git_repository,
                commit.parent(0)?,
            )?)
        } else {
            None
        };

        Ok(SessionReader {
            reader: Box::new(commit_reader),
            previous_reader,
        })
    }

    pub fn files(&self, paths: Option<Vec<&str>>) -> Result<HashMap<String, String>> {
        match &self.previous_reader {
            None => Ok(HashMap::new()),
            Some(previous_reader) => {
                let files = previous_reader.list_files("wd")?;
                let mut files_with_content = HashMap::new();
                for file_path in files {
                    if let Some(paths) = paths.as_ref() {
                        if !paths.iter().any(|path| file_path.starts_with(path)) {
                            continue;
                        }
                    }
                    match previous_reader
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
        }
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
            Ok(content) => Ok(Some(serde_json::from_str(&content)?)),
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
                if !paths.iter().any(|path| file_path.starts_with(path)) {
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

pub struct SessionsIterator<'iterator> {
    git_repository: &'iterator git2::Repository,
    iter: git2::Revwalk<'iterator>,
}

impl<'iterator> SessionsIterator<'iterator> {
    pub(crate) fn new(git_repository: &'iterator git2::Repository) -> Result<Self> {
        let mut iter = git_repository
            .revwalk()
            .context("failed to create revwalk")?;
        iter.push_head().context("failed to push HEAD to revwalk")?;
        iter.set_sorting(git2::Sort::TOPOLOGICAL)
            .context("failed to set sorting")?;
        Ok(Self {
            git_repository,
            iter,
        })
    }
}

impl<'iterator> Iterator for SessionsIterator<'iterator> {
    type Item = Result<sessions::Session>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Result::Ok(oid)) => {
                let commit = match self.git_repository.find_commit(oid) {
                    Result::Ok(commit) => commit,
                    Err(err) => return Some(Err(err.into())),
                };

                if commit.parent_count() == 0 {
                    // skip initial commit, as it's impossible to get a list of files from it
                    // it's only used to bootstrap the history
                    return None;
                }

                let commit_reader =
                    match reader::CommitReader::from_commit(self.git_repository, commit) {
                        Result::Ok(commit_reader) => commit_reader,
                        Err(err) => return Some(Err(err)),
                    };
                let session = match sessions::Session::try_from(commit_reader) {
                    Result::Ok(session) => session,
                    Err(sessions::SessionError::NoSession) => return None,
                    Err(err) => return Some(Err(err.into())),
                };
                Some(Ok(session))
            }
            Some(Err(err)) => Some(Err(err.into())),
            None => None,
        }
    }
}

pub struct SessionsIdsIterator<'iterator> {
    git_repository: &'iterator git2::Repository,
    iter: git2::Revwalk<'iterator>,
}

impl<'iterator> SessionsIdsIterator<'iterator> {
    pub(crate) fn new(git_repository: &'iterator git2::Repository) -> Result<Self> {
        let mut iter = git_repository
            .revwalk()
            .context("failed to create revwalk")?;
        iter.push_head().context("failed to push HEAD to revwalk")?;
        iter.set_sorting(git2::Sort::TOPOLOGICAL)
            .context("failed to set sorting")?;
        Ok(Self {
            git_repository,
            iter,
        })
    }
}

impl<'iterator> Iterator for SessionsIdsIterator<'iterator> {
    type Item = Result<(git2::Oid, String)>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Result::Ok(oid)) => {
                let commit = match self.git_repository.find_commit(oid) {
                    Result::Ok(commit) => commit,
                    Err(err) => return Some(Err(err.into())),
                };
                let commit_reader =
                    match reader::CommitReader::from_commit(self.git_repository, commit) {
                        Result::Ok(commit_reader) => commit_reader,
                        Err(err) => return Some(Err(err)),
                    };
                match commit_reader.read_to_string("session/meta/id") {
                    Ok(sid) => Some(Ok((oid, sid))),
                    Err(e) => Some(Err(e.into())),
                }
            }
            Some(Err(err)) => Some(Err(err.into())),
            None => None,
        }
    }
}
