use std::path::PathBuf;

use anyhow::Result;

use super::Delta;
use crate::{gb_repository, writer};

pub struct DeltasWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> DeltasWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self, std::io::Error> {
        writer::DirWriter::open(repository.root()).map(|writer| Self { repository, writer })
    }

    pub fn write<P: AsRef<std::path::Path>>(&self, path: P, deltas: &Vec<Delta>) -> Result<()> {
        self.repository.mark_active_session()?;

        let _lock = self.repository.lock();

        let path = path.as_ref();
        let raw_deltas = serde_json::to_string(&deltas)?;

        self.writer
            .write_string(PathBuf::from("session/deltas").join(path), &raw_deltas)?;

        tracing::trace!(
            project_id = %self.repository.get_project_id(),
            path = %path.display(),
            "wrote deltas"
        );

        Ok(())
    }

    pub fn remove_wd_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        self.repository.mark_active_session()?;

        let _lock = self.repository.lock();

        let path = path.as_ref();
        self.writer.remove(PathBuf::from("session/wd").join(path))?;

        tracing::trace!(
            project_id = %self.repository.get_project_id(),
            path = %path.display(),
            "deleted session wd file"
        );

        Ok(())
    }

    pub fn write_wd_file<P: AsRef<std::path::Path>>(&self, path: P, contents: &str) -> Result<()> {
        self.repository.mark_active_session()?;

        let _lock = self.repository.lock();

        let path = path.as_ref();
        self.writer
            .write_string(PathBuf::from("session/wd").join(path), contents)?;

        tracing::trace!(
            project_id = %self.repository.get_project_id(),
            path = %path.display(),
            "wrote session wd file"
        );

        Ok(())
    }
}
