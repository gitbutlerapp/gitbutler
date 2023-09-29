use anyhow::{Context, Result};

use crate::gb_repository;

use super::Bookmark;

pub struct BookmarksWriter<'writer> {
    repository: &'writer gb_repository::Repository,
}

impl<'writer> BookmarksWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self> {
        repository
            .get_or_create_current_session()
            .context("failed to create session")?;
        Ok(Self { repository })
    }

    pub fn write(&self, bookmark: &Bookmark) -> Result<()> {
        let _lock = self.repository.lock();

        serde_jsonlines::append_json_lines(
            self.repository.session_path().join("bookmarks.jsonl"),
            [bookmark],
        )?;

        tracing::debug!(
            project_id = self.repository.project_id,
            timestamp_ms = bookmark.timestamp_ms,
            "wrote bookmark",
        );

        Ok(())
    }
}
