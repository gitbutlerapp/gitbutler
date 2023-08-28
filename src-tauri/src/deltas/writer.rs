use anyhow::{Context, Result};

use crate::{
    gb_repository,
    writer::{self, Writer},
};

use super::Delta;

pub struct DeltasWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> DeltasWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Self {
        let writer = writer::DirWriter::open(repository.root());
        Self { writer, repository }
    }

    pub fn write<P: AsRef<std::path::Path>>(&self, path: P, deltas: &Vec<Delta>) -> Result<()> {
        self.repository
            .get_or_create_current_session()
            .context("failed to create session")?;

        let _lock = self.repository.lock();

        let path = path.as_ref();
        let raw_deltas = serde_json::to_string(&deltas)?;

        self.writer
            .write_string(&format!("session/deltas/{}", path.display()), &raw_deltas)?;

        tracing::info!(
            "{}: wrote deltas for {}",
            self.repository.project_id,
            path.display()
        );

        Ok(())
    }

    pub fn write_wd_file<P: AsRef<std::path::Path>>(&self, path: P, contents: &str) -> Result<()> {
        self.repository
            .get_or_create_current_session()
            .context("failed to create session")?;

        let _lock = self.repository.lock();

        let path = path.as_ref();
        self.writer
            .write_string(&format!("session/wd/{}", path.display()), contents)?;

        tracing::info!(
            "{}: wrote session wd file {}",
            self.repository.project_id,
            path.display()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use crate::{deltas, projects, sessions, users, test_utils};

    use super::*;

    #[test]
    fn write_no_vbranches() -> Result<()> {
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = test_utils::temp_dir();
        let local_app_data = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_app_data);
        let project_store = projects::Storage::from(&local_app_data);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

        let deltas_writer = DeltasWriter::new(&gb_repo);

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;
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
