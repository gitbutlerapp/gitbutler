use anyhow::{Context, Result};

use crate::gb_repository;

use super::Record;

pub struct PtyWriter<'writer> {
    repository: &'writer gb_repository::Repository,
}

impl<'writer> PtyWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self> {
        repository
            .get_or_create_current_session()
            .context("failed to create session")?;
        Ok(Self { repository })
    }

    pub fn write(&self, record: &Record) -> Result<()> {
        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("failed to unlock");
        }

        serde_jsonlines::append_json_lines(
            self.repository.session_path().join("pty.jsonl"),
            [record],
        )?;

        tracing::info!(
            "{}: appended pty record to session",
            self.repository.project_id
        );

        Ok(())
    }
}
