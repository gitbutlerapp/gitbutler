use crate::reader;

use super::Target;

pub struct TargetReader<'reader> {
    reader: &'reader dyn reader::Reader,
}

#[derive(thiserror::Error, Debug)]
pub enum TargetReadError {
    #[error("Target not found")]
    NotFound,
    #[error("Failed to read {0}: {1}")]
    ReadError(String, reader::Error),
}

impl<'reader> TargetReader<'reader> {
    pub fn new(reader: &'reader dyn reader::Reader) -> Self {
        Self { reader }
    }

    fn read_default(&self) -> Result<Target, TargetReadError> {
        if !self.reader.exists("branches/target") {
            return Err(TargetReadError::NotFound);
        }

        let name = self
            .reader
            .read_string("branches/target/name")
            .map_err(|e| TargetReadError::ReadError("default target.name".to_string(), e))?;
        let remote = self
            .reader
            .read_string("branches/target/remote")
            .map_err(|e| TargetReadError::ReadError("default target.remote".to_string(), e))?;
        let sha = self
            .reader
            .read_string("branches/target/sha")
            .map_err(|e| TargetReadError::ReadError("default target.sha".to_string(), e))?
            .parse()
            .map_err(|e| {
                TargetReadError::ReadError(
                    "default target.sha".to_string(),
                    reader::Error::IOError(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?;
        Ok(Target { name, remote, sha })
    }

    pub fn read(&self, id: &str) -> Result<Target, TargetReadError> {
        if !self.reader.exists(&format!("branches/{}/target", id)) {
            return self.read_default();
        }

        let name = self
            .reader
            .read_string(&format!("branches/{}/target/name", id))
            .map_err(|e| TargetReadError::ReadError("target.name".to_string(), e))?;
        let remote = self
            .reader
            .read_string(&format!("branches/{}/target/remote", id))
            .map_err(|e| TargetReadError::ReadError("target.remote".to_string(), e))?;
        let sha = self
            .reader
            .read_string(&format!("branches/{}/target/sha", id))
            .map_err(|e| TargetReadError::ReadError("target.sha".to_string(), e))?
            .parse()
            .map_err(|e| {
                TargetReadError::ReadError(
                    "target.sha".to_string(),
                    reader::Error::IOError(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?;
        Ok(Target { name, remote, sha })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tempfile::tempdir;

    use crate::{
        gb_repository, projects, storage, users,
        virtual_branches::{branch, target::writer::TargetWriter}, sessions,
    };

    use super::*;

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

        let reader = TargetReader::new(&session_reader);
        let result = reader.read("not found");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Target not found");

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

        let branch = branch::Branch {
            id: "id".to_string(),
            name: "name".to_string(),
            applied: true,
            upstream: "upstream".to_string(),
            created_timestamp_ms: 0,
            updated_timestamp_ms: 1,
        };

        let target = Target {
            name: "target".to_string(),
            remote: "remote".to_string(),
            sha: git2::Oid::from_str("fedcba9876543210fedcba9876543210fedcba98").unwrap(),
        };

        let default_target = Target {
            name: "default_target".to_string(),
            remote: "default_remote".to_string(),
            sha: git2::Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
        };

        let branch_writer = branch::Writer::new(&gb_repo);
        branch_writer.write(&branch)?;

        let session = gb_repo.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;

        let target_writer = TargetWriter::new(&gb_repo);
        let reader = TargetReader::new(&session_reader);

        target_writer.write_default(&default_target)?;
        assert_eq!(default_target, reader.read(&branch.id)?);

        target_writer.write(&branch.id, &target)?;
        assert_eq!(target, reader.read(&branch.id)?);

        Ok(())
    }
}
