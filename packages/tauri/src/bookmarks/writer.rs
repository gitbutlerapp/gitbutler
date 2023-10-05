use anyhow::Result;

use crate::gb_repository;

use super::Bookmark;

pub struct BookmarksWriter<'writer> {
    repository: &'writer gb_repository::Repository,
}

impl<'writer> BookmarksWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self> {
        Ok(Self { repository })
    }

    pub fn write(&self, bookmark: &Bookmark) -> Result<()> {
        self.repository.mark_active_session()?;

        let _lock = self.repository.lock();

        serde_jsonlines::append_json_lines(
            self.repository.session_path().join("bookmarks.jsonl"),
            [bookmark],
        )?;

        tracing::debug!(
            project_id = self.repository.project.id,
            timestamp_ms = bookmark.timestamp_ms,
            "wrote bookmark",
        );

        Ok(())
    }
}
