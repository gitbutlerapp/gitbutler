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
        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("failed to unlock");
        }

        serde_jsonlines::append_json_lines(
            self.repository.session_path().join("bookmarks.jsonl"),
            [bookmark],
        )?;

        log::info!(
            "{}: wrote bookmark {}",
            self.repository.project_id,
            bookmark.timestamp_ms
        );

        Ok(())
    }
}
