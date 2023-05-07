use std::time;

use anyhow::{anyhow, Context, Result};

use crate::{
    app::{
        self, gb_repository,
        reader::{self, Reader},
        writer::{self, Writer},
    },
    pty,
};

use super::Session;

pub struct SessionWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> SessionWriter<'writer> {
    pub fn open(repository: &'writer gb_repository::Repository, session: &Session) -> Result<Self> {
        if session.hash.is_some() {
            return Err(anyhow!("can not open writer for a session with a hash"));
        }

        repository.lock()?;
        defer! {
            repository.unlock().expect("failed to unlock");
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
                time::SystemTime::now()
                    .duration_since(time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    .to_string()
                    .as_str(),
            )
            .with_context(|| "failed to write last timestamp")?;

        if current_session_id.is_ok() && current_session_id.as_ref().unwrap().eq(&session.id) {
            let writer = SessionWriter {
                repository: &repository,
                writer,
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
            writer,
        };

        Ok(writer)
    }

    pub fn append_pty(&self, record: &pty::Record) -> Result<()> {
        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("failed to unlock");
        }

        serde_jsonlines::append_json_lines(
            &self.repository.session_path().join("pty.jsonl"),
            [record],
        )?;

        log::info!(
            "{}: appended pty record to session",
            self.repository.project_id
        );

        Ok(())
    }

    pub fn write_session_wd_file<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        contents: &str,
    ) -> Result<()> {
        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("failed to unlock");
        }

        let path = path.as_ref();
        self.writer.write_string(
            &self
                .repository
                .session_wd_path()
                .join(path)
                .to_str()
                .unwrap(),
            contents,
        )?;

        log::info!(
            "{}: wrote session wd file {}",
            self.repository.project_id,
            path.display()
        );

        Ok(())
    }

    pub fn write_deltas<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        deltas: &Vec<app::Delta>,
    ) -> Result<()> {
        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("failed to unlock");
        }

        let path = path.as_ref();

        let raw_deltas = serde_json::to_string(&deltas)?;

        self.writer.write_string(
            &self.repository.deltas_path().join(path).to_str().unwrap(),
            &raw_deltas,
        )?;

        log::info!(
            "{}: wrote deltas for {}",
            self.repository.project_id,
            path.display()
        );

        Ok(())
    }
}
