use crate::reader::{self, SubReader};

use super::Target;

pub struct TargetReader<'reader> {
    reader: &'reader dyn reader::Reader,
}

impl<'reader> TargetReader<'reader> {
    pub fn new(reader: &'reader dyn reader::Reader) -> Self {
        Self { reader }
    }

    pub fn read_default(&self) -> Result<Target, reader::Error> {
        if !self.reader.exists("branches/target") {
            return Err(reader::Error::NotFound);
        }

        let reader: &dyn crate::reader::Reader = &SubReader::new(self.reader, "branches/target");
        Target::try_from(reader)
    }

    pub fn read(&self, id: &str) -> Result<Target, reader::Error> {
        if !self.reader.exists(&format!("branches/{}/target", id)) {
            return self.read_default();
        }

        let reader: &dyn crate::reader::Reader =
            &SubReader::new(self.reader, &format!("branches/{}/target", id));
        Target::try_from(reader)
    }
}

#[cfg(test)]
mod tests {
    use crate::writer::Writer;
    use anyhow::Result;
    use tempfile::tempdir;

    use crate::{
        gb_repository, projects, sessions, storage, users,
        virtual_branches::{branch, target::writer::TargetWriter},
    };

    use super::*;

    static mut TEST_INDEX: usize = 0;

    fn test_branch() -> branch::Branch {
        unsafe {
            TEST_INDEX += 1;
        }
        branch::Branch {
            id: format!("branch_{}", unsafe { TEST_INDEX }),
            name: format!("branch_name_{}", unsafe { TEST_INDEX }),
            applied: true,
            upstream: format!("upstream_{}", unsafe { TEST_INDEX }),
            created_timestamp_ms: unsafe { TEST_INDEX } as u128,
            updated_timestamp_ms: unsafe { TEST_INDEX + 100 } as u128,
            head: git2::Oid::from_str(&format!(
                "0123456789abcdef0123456789abcdef0123456{}",
                unsafe { TEST_INDEX }
            ))
            .unwrap(),
            tree: git2::Oid::from_str(&format!(
                "0123456789abcdef0123456789abcdef012345{}",
                unsafe { TEST_INDEX + 10 }
            ))
            .unwrap(),
            ownership: branch::Ownership {
                files: vec![branch::FileOwnership {
                    file_path: format!("file/{}", unsafe { TEST_INDEX }).into(),
                    hunks: vec![],
                }],
            },
            order: unsafe { TEST_INDEX },
        }
    }

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

    #[test]
    fn test_read_not_found() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
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
        assert_eq!(result.unwrap_err().to_string(), "file not found");

        Ok(())
    }

    #[test]
    fn test_read_deprecated_format() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let writer = crate::writer::DirWriter::open(gb_repo.root());
        writer
            .write_string("branches/target/name", "origin/master")
            .unwrap();
        writer
            .write_string(
                "branches/target/remote",
                "git@github.com:gitbutlerapp/gitbutler-client.git",
            )
            .unwrap();
        writer
            .write_string(
                "branches/target/sha",
                "dd945831869e9593448aa622fa4342bbfb84813d",
            )
            .unwrap();

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;
        let reader = TargetReader::new(&session_reader);

        let read = reader.read_default().unwrap();
        assert_eq!(read.branch_name, "origin/master");
        assert_eq!(read.remote_name, "origin");
        assert_eq!(
            read.remote_url,
            "git@github.com:gitbutlerapp/gitbutler-client.git"
        );
        assert_eq!(
            read.sha,
            git2::Oid::from_str("dd945831869e9593448aa622fa4342bbfb84813d").unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_read_override_target() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let branch = test_branch();

        let target = Target {
            remote_name: "remote".to_string(),
            branch_name: "branch".to_string(),
            remote_url: "remote url".to_string(),
            sha: git2::Oid::from_str("fedcba9876543210fedcba9876543210fedcba98").unwrap(),
            behind: 0,
        };

        let default_target = Target {
            remote_name: "default remote".to_string(),
            branch_name: "default branch".to_string(),
            remote_url: "default remote url".to_string(),
            sha: git2::Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
            behind: 0,
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
