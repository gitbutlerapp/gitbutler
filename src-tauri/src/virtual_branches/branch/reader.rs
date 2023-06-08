use crate::reader::{self, Reader};

use super::Branch;

pub struct BranchReader<'reader> {
    reader: &'reader dyn reader::Reader,
}

#[derive(thiserror::Error, Debug)]
pub enum BranchReadError {
    #[error("Branch not found")]
    NotFound,
    #[error("Failed to read {0}: {1}")]
    ReadError(String, reader::Error),
}

impl<'reader> BranchReader<'reader> {
    pub fn new(reader: &'reader dyn Reader) -> Self {
        Self { reader }
    }

    pub fn read_selected(&self) -> Result<Option<String>, BranchReadError> {
        match self.reader.read_string("branches/selected") {
            Ok(selected) => Ok(Some(selected)),
            Err(reader::Error::NotFound) => Ok(None),
            Err(e) => Err(BranchReadError::ReadError("selected".to_string(), e)),
        }
    }

    pub fn read(&self, id: &str) -> Result<Branch, BranchReadError> {
        if !self.reader.exists(&format!("branches/{}", id)) {
            return Err(BranchReadError::NotFound);
        }

        let branch = Branch {
            id: id.to_string(),
            name: self
                .reader
                .read_string(&format!("branches/{}/meta/name", id))
                .map_err(|e| BranchReadError::ReadError("name".to_string(), e))?,
            applied: self
                .reader
                .read_bool(&format!("branches/{}/meta/applied", id))
                .map_err(|e| BranchReadError::ReadError("applied".to_string(), e))?,
            upstream: self
                .reader
                .read_string(&format!("branches/{}/meta/upstream", id))
                .map_err(|e| BranchReadError::ReadError("upstream".to_string(), e))?,
            created_timestamp_ms: self
                .reader
                .read_u128(&format!("branches/{}/meta/created_timestamp_ms", id))
                .map_err(|e| BranchReadError::ReadError("created_timestamp_ms".to_string(), e))?,
            updated_timestamp_ms: self
                .reader
                .read_u128(&format!("branches/{}/meta/updated_timestamp_ms", id))
                .map_err(|e| BranchReadError::ReadError("updated_timestamp_ms".to_string(), e))?,
        };
        Ok(branch)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tempfile::tempdir;

    use crate::{gb_repository, projects, sessions, storage, users};

    use super::{super::Writer, *};

    fn test_repository() -> Result<git2::Repository> {
        let path = tempdir()?.path().to_str().unwrap().to_string();
        let repository = git2::Repository::init(path)?;
        let mut index = repository.index()?;
        let oid = index.write_tree()?;
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &repository.find_tree(oid)?,
            &[],
        )?;
        Ok(repository)
    }

    fn test_project(repository: &git2::Repository) -> Result<projects::Project> {
        let project = projects::Project::from_path(
            repository
                .path()
                .parent()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
        )?;
        Ok(project)
    }

    #[test]
    fn test_read_not_found() -> Result<()> {
        let repository = test_repository()?;
        let project = test_project(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;

        let reader = BranchReader::new(&session_reader);
        let result = reader.read("not found");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Branch not found");

        Ok(())
    }

    #[test]
    fn test_read_override_target() -> Result<()> {
        let repository = test_repository()?;
        let project = test_project(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let branch = Branch {
            id: "id".to_string(),
            name: "name".to_string(),
            applied: true,
            upstream: "upstream".to_string(),
            created_timestamp_ms: 0,
            updated_timestamp_ms: 1,
        };

        let writer = Writer::new(&gb_repo);
        writer.write(&branch)?;

        let session = gb_repo.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;

        let reader = BranchReader::new(&session_reader);

        assert_eq!(branch, reader.read(&branch.id)?);

        Ok(())
    }

    #[test]
    fn test_read_selected() -> Result<()> {
        let repository = test_repository()?;
        let project = test_project(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;
        let reader = BranchReader::new(&session_reader);
        let writer = Writer::new(&gb_repo);

        assert_eq!(None, reader.read_selected()?);

        writer.write_selected(Some("test"))?;
        assert_eq!(Some("test".to_string()), reader.read_selected()?);

        writer.write_selected(Some("updated"))?;
        assert_eq!(Some("updated".to_string()), reader.read_selected()?);

        Ok(())
    }
}
