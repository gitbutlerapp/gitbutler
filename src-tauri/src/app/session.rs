use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};

use crate::{deltas, pty, sessions};

use super::{
    gb_repository,
    reader::{self, Reader},
    writer::{self, Writer},
};

pub struct SessionWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: Box<dyn writer::Writer + 'writer>,
}

impl<'writer> SessionWriter<'writer> {
    pub fn open(
        repository: &'writer gb_repository::Repository,
        session: sessions::Session,
    ) -> Result<Self> {
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

        let writer = writer::DirWriter::open(repository.root());

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

    pub fn write_logs<P: AsRef<std::path::Path>>(&self, path: P, contents: &str) -> Result<()> {
        let path = path.as_ref();
        log::info!(
            "{}: writing logs to {}",
            self.repository.project_id,
            path.display()
        );

        self.writer.write_string(
            &self.repository.logs_path().join(path).to_str().unwrap(),
            contents,
        )?;

        Ok(())
    }

    pub fn write_file<P: AsRef<std::path::Path>>(&self, path: P, contents: &str) -> Result<()> {
        let path = path.as_ref();
        log::info!(
            "{}: writing file to {}",
            self.repository.project_id,
            path.display()
        );

        self.writer.write_string(
            &self.repository.wd_path().join(path).to_str().unwrap(),
            contents,
        )?;

        Ok(())
    }

    pub fn write_deltas<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        deltas: Vec<deltas::Delta>,
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
    repository: &'reader gb_repository::Repository,
    reader: Box<dyn reader::Reader + 'reader>,
}

impl<'reader> SessionReader<'reader> {
    pub fn open(
        repository: &'reader gb_repository::Repository,
        session: sessions::Session,
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
            return Ok(SessionReader {
                reader: Box::new(wd_reader),
                repository,
            });
        }

        let session_hash = if let Some(hash) = session.hash {
            hash
        } else {
            return Err(anyhow!(
                "can not open reader for {} because it has no commit hash nor it is a current session",
                session.id
            ));
        };

        let oid = git2::Oid::from_str(&session_hash)
            .with_context(|| format!("failed to parse commit hash {}", session_hash))?;

        let commit_reader = repository.get_commit_reader(oid)?;
        Ok(SessionReader {
            reader: Box::new(commit_reader),
            repository,
        })
    }

    pub fn files(&self, paths: Option<Vec<&str>>) -> Result<HashMap<String, String>> {
        let files = self
            .reader
            .list_files(&self.repository.wd_path().to_str().unwrap())?;
        let files_with_content = files
            .iter()
            .filter(|file| {
                if let Some(paths) = paths.as_ref() {
                    paths.iter().any(|path| file.starts_with(path))
                } else {
                    true
                }
            })
            .map(|file| {
                let content = self
                    .reader
                    .read_to_string(&self.repository.wd_path().join(file).to_str().unwrap())
                    .unwrap();
                (file.to_string(), content)
            })
            .collect();
        Ok(files_with_content)
    }

    pub fn deltas(&self, paths: Option<Vec<&str>>) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        let files = self
            .reader
            .list_files(&self.repository.deltas_path().to_str().unwrap())?;
        let files_with_content = files
            .iter()
            .filter(|file| {
                if let Some(paths) = paths.as_ref() {
                    paths.iter().any(|path| file.starts_with(path))
                } else {
                    true
                }
            })
            .map(|file| {
                let content = self
                    .reader
                    .read_to_string(&self.repository.deltas_path().join(file).to_str().unwrap())
                    .unwrap();
                let deltas: Vec<deltas::Delta> = serde_json::from_str(&content).unwrap();
                (file.to_string(), deltas)
            })
            .collect();
        Ok(files_with_content)
    }
}
