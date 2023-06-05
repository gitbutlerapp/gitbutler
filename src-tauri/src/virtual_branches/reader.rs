use crate::{
    reader::{self, Reader},
    sessions,
};

use super::{Branch, Target};

pub struct BranchReader<'reader> {
    sessions_reader: &'reader sessions::Reader<'reader>,
}

#[derive(thiserror::Error, Debug)]
pub enum BranchReadError {
    #[error("Branch not found")]
    NotFound,
    #[error("Failed to read {0}: {1}")]
    ReadError(String, reader::Error),
}

impl<'reader> BranchReader<'reader> {
    pub fn new(sessions_reader: &'reader sessions::Reader<'reader>) -> Self {
        Self { sessions_reader }
    }

    pub fn read(&self, id: &str) -> Result<Branch, BranchReadError> {
        if !self.sessions_reader.exists(&format!("branches/{}", id)) {
            return Err(BranchReadError::NotFound);
        }

        let branch = Branch {
            id: id.to_string(),
            name: self
                .sessions_reader
                .read_string(&format!("branches/{}/meta/name", id))
                .map_err(|e| BranchReadError::ReadError("name".to_string(), e))?,
            target: Target {
                name: self
                    .sessions_reader
                    .read_string(&format!("branches/{}/target/name", id))
                    .map_err(|e| BranchReadError::ReadError("target.name".to_string(), e))?,
                remote: self
                    .sessions_reader
                    .read_string(&format!("branches/{}/target/remote", id))
                    .map_err(|e| BranchReadError::ReadError("target.remote".to_string(), e))?,
                sha: self
                    .sessions_reader
                    .read_string(&format!("branches/{}/target/sha", id))
                    .map_err(|e| BranchReadError::ReadError("target.sha".to_string(), e))?
                    .parse()
                    .map_err(|e| {
                        BranchReadError::ReadError(
                            "target.sha".to_string(),
                            reader::Error::IOError(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                e,
                            )),
                        )
                    })?,
            },
            applied: self
                .sessions_reader
                .read_bool(&format!("branches/{}/meta/applied", id))
                .map_err(|e| BranchReadError::ReadError("applied".to_string(), e))?,
            upstream: self
                .sessions_reader
                .read_string(&format!("branches/{}/meta/upstream", id))
                .map_err(|e| BranchReadError::ReadError("upstream".to_string(), e))?,
            created_timestamp_ms: self
                .sessions_reader
                .read_u128(&format!("branches/{}/meta/created_timestamp_ms", id))
                .map_err(|e| BranchReadError::ReadError("created_timestamp_ms".to_string(), e))?,
            updated_timestamp_ms: self
                .sessions_reader
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

    use crate::{gb_repository, projects, storage, users};

    use super::{
        super::{Target, Writer},
        *,
    };

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
    fn test_read() -> Result<()> {
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
            target: Target {
                name: "target_name".to_string(),
                remote: "target_remote".to_string(),
                sha: git2::Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
            },
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
}
