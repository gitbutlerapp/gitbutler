use anyhow::Result;

use crate::{gb_repository, writer};

use super::Delta;

pub struct DeltasWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> DeltasWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self, std::io::Error> {
        writer::DirWriter::open(repository.root()).map(|writer| Self { writer, repository })
    }

    pub fn write<P: AsRef<std::path::Path>>(&self, path: P, deltas: &Vec<Delta>) -> Result<()> {
        self.repository.mark_active_session()?;

        let _lock = self.repository.lock();

        let path = path.as_ref();
        let raw_deltas = serde_json::to_string(&deltas)?;

        self.writer
            .write_string(&format!("session/deltas/{}", path.display()), &raw_deltas)?;

        tracing::debug!(
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
        self.writer
            .remove(format!("session/wd/{}", path.display()))?;

        tracing::debug!(
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
            .write_string(&format!("session/wd/{}", path.display()), contents)?;

        tracing::debug!(
            project_id = %self.repository.get_project_id(),
            path = %path.display(),
            "wrote session wd file"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use crate::{
        deltas, sessions,
        tests::{Case, Suite},
    };

    use super::*;

    #[test]
    fn write_no_vbranches() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let deltas_writer = DeltasWriter::new(&gb_repository)?;

        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);

        let path = "test.txt";
        let deltas = vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, " world".to_string()))],
                timestamp_ms: 0,
            },
        ];

        deltas_writer.write(path, &deltas).unwrap();

        assert_eq!(deltas_reader.read_file(path).unwrap(), Some(deltas));
        assert_eq!(deltas_reader.read_file("not found").unwrap(), None);

        Ok(())
    }
}
