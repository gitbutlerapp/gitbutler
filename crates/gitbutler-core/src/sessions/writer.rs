use std::time;

use anyhow::{anyhow, Context, Result};

use super::Session;
use crate::{gb_repository, reader, writer};

pub struct SessionWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> SessionWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self, std::io::Error> {
        writer::DirWriter::open(repository.root())
            .map(|writer| SessionWriter { repository, writer })
    }

    pub fn remove(&self) -> Result<()> {
        self.writer.remove("session")?;

        tracing::debug!(
            project_id = %self.repository.get_project_id(),
            "deleted session"
        );

        Ok(())
    }

    pub fn write(&self, session: &Session) -> Result<()> {
        if session.hash.is_some() {
            return Err(anyhow!("can not open writer for a session with a hash"));
        }

        let reader = reader::Reader::open(&self.repository.root())
            .context("failed to open current session reader")?;

        let current_session_id =
            if let Ok(reader::Content::UTF8(current_session_id)) = reader.read("session/meta/id") {
                Some(current_session_id)
            } else {
                None
            };

        if current_session_id.is_some()
            && current_session_id.as_ref() != Some(&session.id.to_string())
        {
            return Err(anyhow!(
                "{}: can not open writer for {} because a writer for {} is still open",
                self.repository.get_project_id(),
                session.id,
                current_session_id.unwrap()
            ));
        }

        let mut batch = vec![writer::BatchTask::Write(
            "session/meta/last",
            time::SystemTime::now()
                .duration_since(time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string(),
        )];

        if current_session_id.is_some()
            && current_session_id.as_ref() == Some(&session.id.to_string())
        {
            self.writer
                .batch(&batch)
                .context("failed to write last timestamp")?;
            return Ok(());
        }

        batch.push(writer::BatchTask::Write(
            "session/meta/id",
            session.id.to_string(),
        ));
        batch.push(writer::BatchTask::Write(
            "session/meta/start",
            session.meta.start_timestamp_ms.to_string(),
        ));

        if let Some(branch) = session.meta.branch.as_ref() {
            batch.push(writer::BatchTask::Write(
                "session/meta/branch",
                branch.to_string(),
            ));
        } else {
            batch.push(writer::BatchTask::Remove("session/meta/branch"));
        }

        if let Some(commit) = session.meta.commit.as_ref() {
            batch.push(writer::BatchTask::Write(
                "session/meta/commit",
                commit.to_string(),
            ));
        } else {
            batch.push(writer::BatchTask::Remove("session/meta/commit"));
        }

        self.writer
            .batch(&batch)
            .context("failed to write session meta")?;

        Ok(())
    }
}
