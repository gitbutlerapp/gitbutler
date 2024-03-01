use crate::{reader, sessions, virtual_branches::BranchId};

use super::Target;

pub struct TargetReader<'r> {
    reader: &'r reader::Reader<'r>,
}

impl<'r> TargetReader<'r> {
    pub fn new(reader: &'r sessions::Reader<'r>) -> Self {
        Self {
            reader: reader.reader(),
        }
    }

    pub fn read_default(&self) -> Result<Target, reader::Error> {
        Target::try_from(&self.reader.sub("branches/target"))
    }

    pub fn read(&self, id: &BranchId) -> Result<Target, reader::Error> {
        if !self
            .reader
            .exists(format!("branches/{}/target", id))
            .map_err(reader::Error::from)?
        {
            return self.read_default();
        }

        Target::try_from(&self.reader.sub(format!("branches/{}/target", id)))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::Result;
    use once_cell::sync::Lazy;

    use crate::{
        sessions,
        tests::{Case, Suite},
        virtual_branches::{branch, target::writer::TargetWriter},
    };

    use super::*;

    static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    fn test_branch() -> branch::Branch {
        TEST_INDEX.fetch_add(1, Ordering::Relaxed);

        branch::Branch {
            id: BranchId::generate(),
            name: format!("branch_name_{}", TEST_INDEX.load(Ordering::Relaxed)),
            notes: String::new(),
            applied: true,
            upstream: Some(
                format!(
                    "refs/remotes/origin/upstream_{}",
                    TEST_INDEX.load(Ordering::Relaxed)
                )
                .parse()
                .unwrap(),
            ),
            upstream_head: None,
            created_timestamp_ms: TEST_INDEX.load(Ordering::Relaxed) as u128,
            updated_timestamp_ms: (TEST_INDEX.load(Ordering::Relaxed) + 100) as u128,
            head: format!(
                "0123456789abcdef0123456789abcdef0123456{}",
                TEST_INDEX.load(Ordering::Relaxed)
            )
            .parse()
            .unwrap(),
            tree: format!(
                "0123456789abcdef0123456789abcdef012345{}",
                (TEST_INDEX.load(Ordering::Relaxed) + 10)
            )
            .parse()
            .unwrap(),
            ownership: branch::Ownership {
                files: vec![branch::FileOwnership {
                    file_path: format!("file/{}", TEST_INDEX.load(Ordering::Relaxed)).into(),
                    hunks: vec![],
                }],
            },
            order: TEST_INDEX.load(Ordering::Relaxed),
            selected_for_changes: None,
        }
    }

    #[test]
    fn test_read_not_found() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let reader = TargetReader::new(&session_reader);
        let result = reader.read(&BranchId::generate());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "file not found");

        Ok(())
    }

    #[test]
    fn test_read_deprecated_format() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let writer = crate::writer::DirWriter::open(gb_repository.root())?;
        writer
            .write_string("branches/target/name", "origin/master")
            .unwrap();
        writer
            .write_string(
                "branches/target/remote",
                "git@github.com:gitbutlerapp/gitbutler.git",
            )
            .unwrap();
        writer
            .write_string(
                "branches/target/sha",
                "dd945831869e9593448aa622fa4342bbfb84813d",
            )
            .unwrap();

        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
        let reader = TargetReader::new(&session_reader);

        let read = reader.read_default().unwrap();
        assert_eq!(read.branch.branch(), "master");
        assert_eq!(read.branch.remote(), "origin");
        assert_eq!(read.remote_url, "git@github.com:gitbutlerapp/gitbutler.git");
        assert_eq!(
            read.sha.to_string(),
            "dd945831869e9593448aa622fa4342bbfb84813d"
        );

        Ok(())
    }

    #[test]
    fn test_read_override_target() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let mut branch = test_branch();

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

        let branch_writer = branch::Writer::new(&gb_repository)?;
        branch_writer.write(&mut branch)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let target_writer = TargetWriter::new(&gb_repository)?;
        let reader = TargetReader::new(&session_reader);

        target_writer.write_default(&default_target)?;
        assert_eq!(default_target, reader.read(&branch.id)?);

        target_writer.write(&branch.id, &target)?;
        assert_eq!(target, reader.read(&branch.id)?);

        Ok(())
    }
}
