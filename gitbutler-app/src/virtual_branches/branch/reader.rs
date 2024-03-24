use crate::{reader, sessions};

use super::{Branch, BranchId};

pub struct BranchReader<'r> {
    reader: &'r reader::Reader<'r>,
}

impl<'r> BranchReader<'r> {
    pub fn new(reader: &'r sessions::Reader<'r>) -> Self {
        Self {
            reader: reader.reader(),
        }
    }

    pub fn read(&self, id: &BranchId) -> Result<Branch, reader::Error> {
        Branch::from_reader(&self.reader.sub(format!("branches/{}", id)))
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
        virtual_branches::branch::BranchOwnershipClaims,
    };

    use super::{super::Writer, *};

    static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    fn test_branch() -> Branch {
        TEST_INDEX.fetch_add(1, Ordering::Relaxed);

        Branch {
            id: BranchId::generate(),
            name: format!("branch_name_{}", TEST_INDEX.load(Ordering::Relaxed)),
            notes: String::new(),
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
            ownership: BranchOwnershipClaims {
                claims: vec![format!("file/{}:1-2", TEST_INDEX.load(Ordering::Relaxed))
                    .parse()
                    .unwrap()],
            },
            selected_for_changes: Some(1),
        }
    }

    #[test]
    fn test_read_not_found() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let reader = BranchReader::new(&session_reader);
        let result = reader.read(&BranchId::generate());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "file not found");

        Ok(())
    }

    #[test]
    fn test_read_override() -> Result<()> {
        let Case {
            gb_repository,
            project,
            ..
        } = Suite::default().new_case();

        let mut branch = test_branch();

        let writer = Writer::new(&gb_repository, project.gb_dir())?;
        writer.write(&mut branch)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let reader = BranchReader::new(&session_reader);

        assert_eq!(branch, reader.read(&branch.id).unwrap());

        Ok(())
    }
}
