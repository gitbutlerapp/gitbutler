use std::time;

use anyhow::{anyhow, Context, Result};

use crate::{
    gb_repository,
    reader::{self, Reader},
    writer::{self, Writer},
};

use super::Session;

pub struct SessionWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> SessionWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Self {
        let writer = writer::DirWriter::open(repository.root());
        SessionWriter { repository, writer }
    }

    pub fn write(&self, session: &Session) -> Result<()> {
        if session.hash.is_some() {
            return Err(anyhow!("can not open writer for a session with a hash"));
        }

        let reader = reader::DirReader::open(self.repository.root());

        let current_session_id = reader.read_string("session/meta/id");

        if current_session_id.is_ok() && !current_session_id.as_ref().unwrap().eq(&session.id) {
            return Err(anyhow!(
                "{}: can not open writer for {} because a writer for {} is still open",
                self.repository.project_id,
                session.id,
                current_session_id.unwrap()
            ));
        }

        self.writer
            .write_string(
                "session/meta/last",
                time::SystemTime::now()
                    .duration_since(time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    .to_string()
                    .as_str(),
            )
            .context("failed to write last timestamp")?;

        if current_session_id.is_ok() && current_session_id.as_ref().unwrap().eq(&session.id) {
            return Ok(());
        }

        self.writer
            .write_string(
                self.repository
                    .session_path()
                    .join("meta")
                    .join("id")
                    .to_str()
                    .unwrap(),
                session.id.as_str(),
            )
            .context("failed to write id")?;

        self.writer
            .write_string(
                "session/meta/start",
                session.meta.start_timestamp_ms.to_string().as_str(),
            )
            .context("failed to write start timestamp")?;

        if let Some(branch) = session.meta.branch.as_ref() {
            self.writer
                .write_string("session/meta/branch", branch)
                .context("failed to write branch")?;
        }

        if let Some(commit) = session.meta.commit.as_ref() {
            self.writer
                .write_string("session/meta/commit", commit)
                .context("failed to write commit")?;
        }

        Ok(())
    }
}
