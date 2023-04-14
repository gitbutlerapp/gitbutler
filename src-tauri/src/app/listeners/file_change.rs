use crate::{
    app::{
        gb_repository, project_repository,
        reader::{self, Reader},
        writer::Writer,
    },
    deltas, projects,
};
use anyhow::{Context, Result};

pub struct Listener<'listener> {
    project_id: String,
    project_repository: &'listener project_repository::Repository,
    gb_repository: &'listener gb_repository::Repository,
}

impl<'listener> Listener<'listener> {
    pub fn new(
        project: &projects::Project,
        project_repository: &'listener project_repository::Repository,
        gb_repository: &'listener gb_repository::Repository,
    ) -> Self {
        Self {
            project_id: project.id.clone(),
            project_repository,
            gb_repository,
        }
    }

    fn get_current_file_content(&self, path: &std::path::Path) -> Result<Option<String>> {
        if self.project_repository.is_path_ignored(path)? {
            return Ok(None);
        }

        let reader = self.project_repository.get_wd_reader();

        let path = path.to_str().unwrap();
        if reader.size(path)? > 100_000 {
            log::warn!("{}: ignoring large file: {}", self.project_id, path);
            return Ok(None);
        }

        match reader.read(path)? {
            reader::Content::UTF8(content) => Ok(Some(content)),
            reader::Content::Binary(_) => {
                log::warn!("{}: ignoring non-utf8 file: {}", self.project_id, path);
                return Ok(None);
            }
        }
    }

    fn get_latest_file_contents(&self, path: &std::path::Path) -> Result<Option<String>> {
        let path = path.to_str().unwrap();
        let gb_head_reader = self
            .gb_repository
            .get_head_reader()
            .with_context(|| "failed to get gb head reader")?;
        let project_head_reader = self
            .project_repository
            .get_head_reader()
            .with_context(|| "failed to get project head reader")?;
        let reader = if gb_head_reader.exists(path) {
            gb_head_reader
        } else if project_head_reader.exists(path) {
            project_head_reader
        } else {
            return Ok(None);
        };
        if reader.size(path)? > 100_000 {
            log::warn!("{}: ignoring large file: {}", self.project_id, path);
            return Ok(None);
        }
        match reader.read(path)? {
            reader::Content::UTF8(content) => Ok(Some(content)),
            reader::Content::Binary(_) => {
                log::warn!("{}: ignoring non-utf8 file: {}", self.project_id, path);
                return Ok(None);
            }
        }
    }

    fn get_current_deltas(&self, path: &std::path::Path) -> Result<Option<Vec<deltas::Delta>>> {
        let reader = self.gb_repository.get_wd_reader();
        let deltas_path = self.gb_repository.deltas_path().join(path);
        match reader.read_to_string(deltas_path.to_str().unwrap()) {
            Ok(content) => Ok(Some(serde_json::from_str(&content)?)),
            Err(reader::Error::NotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn register<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let current_file_content = match self
            .get_current_file_content(&path)
            .with_context(|| "failed to get current file content")?
        {
            Some(content) => content,
            None => return Ok(()),
        };

        let latest_file_content = self
            .get_latest_file_contents(&path)
            .with_context(|| "failed to get latest file content")?;

        let current_deltas = self
            .get_current_deltas(&path)
            .with_context(|| "failed to get current deltas")?;

        let mut text_doc = match (latest_file_content, current_deltas) {
            (Some(latest_contents), Some(deltas)) => {
                deltas::TextDocument::new(Some(&latest_contents), deltas)?
            }
            (Some(latest_contents), None) => {
                deltas::TextDocument::new(Some(&latest_contents), vec![])?
            }
            (None, Some(deltas)) => deltas::TextDocument::new(None, deltas)?,
            (None, None) => deltas::TextDocument::new(None, vec![])?,
        };

        if !text_doc.update(&current_file_content)? {
            log::debug!(
                "{}: {} no new deltas, ignoring",
                self.project_id,
                path.display()
            );
            return Ok(());
        }

        log::info!("{}: {} changed", self.project_id, path.display());

        let writer = self.gb_repository.get_wd_writer();

        // save current deltas
        writer
            .write_string(
                self.gb_repository
                    .deltas_path()
                    .join(path)
                    .to_str()
                    .unwrap(),
                &serde_json::to_string(&text_doc.get_deltas())?,
            )
            .with_context(|| "failed to write deltas")?;

        // save file contents corresponding to the deltas
        writer
            .write_string(
                self.gb_repository.wd_path().join(path).to_str().unwrap(),
                &current_file_content,
            )
            .with_context(|| "failed to write file content")?;

        Ok(())
    }
}
