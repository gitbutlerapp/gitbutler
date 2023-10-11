use std::path;

use crate::reader::{self, Reader, SubReader};

use super::Branch;

pub struct BranchReader<'reader> {
    reader: &'reader dyn reader::Reader,
}

impl<'reader> BranchReader<'reader> {
    pub fn new(reader: &'reader dyn Reader) -> Self {
        Self { reader }
    }

    pub fn reader(&self) -> &dyn reader::Reader {
        self.reader
    }

    pub fn read(&self, id: &str) -> Result<Branch, reader::Error> {
        if !self
            .reader
            .exists(&path::PathBuf::from(format!("branches/{}", id)))
        {
            return Err(reader::Error::NotFound);
        }

        let single_reader: &dyn crate::reader::Reader =
            &SubReader::new(self.reader, &format!("branches/{}", id));
        Branch::try_from(single_reader)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::Result;
    use once_cell::sync::Lazy;

    use crate::{
        sessions,
        test_utils::{Case, Suite},
        virtual_branches::branch::Ownership,
    };

    use super::{super::Writer, *};

    static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    fn test_branch() -> Branch {
        TEST_INDEX.fetch_add(1, Ordering::Relaxed);

        Branch {
            id: format!("branch_{}", TEST_INDEX.load(Ordering::Relaxed)),
            name: format!("branch_name_{}", TEST_INDEX.load(Ordering::Relaxed)),
            notes: "".to_string(),
            applied: true,
            order: TEST_INDEX.load(Ordering::Relaxed),
            upstream: Some(
                format!(
                    "refs/remotes/origin/upstream_{}",
                    TEST_INDEX.load(Ordering::Relaxed)
                )
                .parse()
                .unwrap(),
            ),
            upstream_head: Some(
                format!(
                    "0123456789abcdef0123456789abcdef0123456{}",
                    TEST_INDEX.load(Ordering::Relaxed)
                )
                .parse()
                .unwrap(),
            ),
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
                TEST_INDEX.load(Ordering::Relaxed) + 10
            )
            .parse()
            .unwrap(),
            ownership: Ownership {
                files: vec![format!("file/{}:1-2", TEST_INDEX.load(Ordering::Relaxed))
                    .parse()
                    .unwrap()],
            },
        }
    }

    #[test]
    fn test_read_not_found() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let reader = BranchReader::new(&session_reader);
        let result = reader.read("not found");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "file not found");

        Ok(())
    }

    #[test]
    fn test_read_override() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let branch = test_branch();

        let writer = Writer::new(&gb_repository);
        writer.write(&branch)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let reader = BranchReader::new(&session_reader);

        assert_eq!(branch, reader.read(&branch.id).unwrap());

        Ok(())
    }
}
