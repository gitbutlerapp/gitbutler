use std::collections::HashSet;

use anyhow::Result;

use crate::sessions;

use super::branch::{self, BranchId};

pub struct BranchIterator<'i> {
    branch_reader: branch::Reader<'i>,
    ids: Vec<BranchId>,
}

impl<'i> BranchIterator<'i> {
    pub fn new(session_reader: &'i sessions::Reader<'i>) -> Result<Self> {
        let reader = session_reader.reader();
        let ids_itarator = reader
            .list_files("branches")?
            .into_iter()
            .map(|file_path| {
                file_path
                    .iter()
                    .next()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .filter(|file_path| file_path != "selected")
            .filter(|file_path| file_path != "target");
        let unique_ids: HashSet<String> = ids_itarator.collect();
        let mut ids: Vec<BranchId> = unique_ids
            .into_iter()
            .map(|id| id.parse())
            .filter_map(Result::ok)
            .collect();
        ids.sort();
        Ok(Self {
            branch_reader: branch::Reader::new(session_reader),
            ids,
        })
    }
}

impl Iterator for BranchIterator<'_> {
    type Item = Result<branch::Branch, crate::reader::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ids.is_empty() {
            return None;
        }

        let id = self.ids.remove(0);
        let branch = self.branch_reader.read(&id);
        Some(branch)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::Result;
    use once_cell::sync::Lazy;

    use crate::{
        reader, sessions,
        tests::{Case, Suite},
        virtual_branches::target,
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
                TEST_INDEX.load(Ordering::Relaxed) + 10
            )
            .parse()
            .unwrap(),
            ownership: branch::BranchOwnershipClaims::default(),
            order: TEST_INDEX.load(Ordering::Relaxed),
            selected_for_changes: Some(1),
        }
    }

    static TEST_TARGET_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    fn test_target() -> target::Target {
        target::Target {
            branch: format!(
                "refs/remotes/branch name{}/remote name {}",
                TEST_TARGET_INDEX.load(Ordering::Relaxed),
                TEST_TARGET_INDEX.load(Ordering::Relaxed)
            )
            .parse()
            .unwrap(),
            remote_url: format!("remote url {}", TEST_TARGET_INDEX.load(Ordering::Relaxed)),
            sha: format!(
                "0123456789abcdef0123456789abcdef0123456{}",
                TEST_TARGET_INDEX.load(Ordering::Relaxed)
            )
            .parse()
            .unwrap(),
        }
    }

    #[test]
    fn test_empty_iterator() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let iter = BranchIterator::new(&session_reader)?;

        assert_eq!(iter.count(), 0);

        Ok(())
    }

    #[test]
    fn test_iterate_all() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let target_writer = target::Writer::new(&gb_repository)?;
        target_writer.write_default(&test_target())?;

        let branch_writer = branch::Writer::new(&gb_repository)?;
        let mut branch_1 = test_branch();
        branch_writer.write(&mut branch_1)?;
        let mut branch_2 = test_branch();
        branch_writer.write(&mut branch_2)?;
        let mut branch_3 = test_branch();
        branch_writer.write(&mut branch_3)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let iter =
            BranchIterator::new(&session_reader)?.collect::<Result<Vec<_>, reader::Error>>()?;
        assert_eq!(iter.len(), 3);
        assert!(iter.contains(&branch_1));
        assert!(iter.contains(&branch_2));
        assert!(iter.contains(&branch_3));

        Ok(())
    }
}
