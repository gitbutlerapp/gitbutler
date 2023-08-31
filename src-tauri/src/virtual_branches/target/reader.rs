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

    use crate::{
        gb_repository, projects, sessions, test_utils, users,
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
            notes: "".to_string(),
            applied: true,
            upstream: Some(
                format!("refs/remotes/origin/upstream_{}", unsafe { TEST_INDEX })
                    .parse()
                    .unwrap(),
            ),
            created_timestamp_ms: unsafe { TEST_INDEX } as u128,
            updated_timestamp_ms: unsafe { TEST_INDEX + 100 } as u128,
            head: format!("0123456789abcdef0123456789abcdef0123456{}", unsafe {
                TEST_INDEX
            })
            .parse()
            .unwrap(),
            tree: format!("0123456789abcdef0123456789abcdef012345{}", unsafe {
                TEST_INDEX + 10
            })
            .parse()
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

    #[test]
    fn test_read_not_found() -> Result<()> {
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = test_utils::temp_dir();
        let local_repo_path = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_repo_path);
        let project_store = projects::Storage::from(&local_repo_path);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = test_utils::temp_dir();
        let local_data_path = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_path);
        let project_store = projects::Storage::from(&local_data_path);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

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
        assert_eq!(read.branch.branch(), "origin/master");
        assert_eq!(read.branch.remote(), "origin");
        assert_eq!(
            read.remote_url,
            "git@github.com:gitbutlerapp/gitbutler-client.git"
        );
        assert_eq!(
            read.sha.to_string(),
            "dd945831869e9593448aa622fa4342bbfb84813d"
        );

        Ok(())
    }

    #[test]
    fn test_read_override_target() -> Result<()> {
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = test_utils::temp_dir();
        let local_app_data = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_app_data);
        let project_store = projects::Storage::from(&local_app_data);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

        let branch = test_branch();

        let target = Target {
            branch: "refs/remotes/remote/branch".parse().unwrap(),
            remote_url: "remote url".to_string(),
            sha: "fedcba9876543210fedcba9876543210fedcba98".parse().unwrap(),
        };

        let default_target = Target {
            branch: "refs/remotes/default remote/default branch"
                .parse()
                .unwrap(),
            remote_url: "default remote url".to_string(),
            sha: "0123456789abcdef0123456789abcdef01234567".parse().unwrap(),
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
