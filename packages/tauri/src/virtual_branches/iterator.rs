use std::{collections::HashSet, path};

use anyhow::Result;

use crate::reader;

use super::branch;

pub struct BranchIterator<'iterator> {
    branch_reader: branch::Reader<'iterator>,
    ids: Vec<String>,
}

impl<'iterator> BranchIterator<'iterator> {
    pub fn new(reader: &'iterator dyn reader::Reader) -> Result<Self> {
        let ids_itarator = reader
            .list_files(&path::PathBuf::from("branches"))?
            .into_iter()
            .map(|file_path| {
                file_path
                    .display()
                    .to_string()
                    .split('/')
                    .next()
                    .unwrap()
                    .to_string()
            })
            .filter(|file_path| file_path != "selected")
            .filter(|file_path| file_path != "target");
        let unique_ids: HashSet<String> = ids_itarator.collect();
        let mut ids: Vec<String> = unique_ids.into_iter().collect();
        ids.sort();
        Ok(Self {
            branch_reader: branch::Reader::new(reader),
            ids,
        })
    }
}

impl<'iterator> Iterator for BranchIterator<'iterator> {
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
    use anyhow::Result;

    use crate::{
        sessions,
        test_utils::{Case, Suite},
        virtual_branches::target,
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
            ownership: branch::Ownership::default(),
            order: unsafe { TEST_INDEX },
        }
    }

    static mut TEST_TARGET_INDEX: usize = 0;

    fn test_target() -> target::Target {
        target::Target {
            branch: format!(
                "refs/remotes/branch name{}/remote name {}",
                unsafe { TEST_TARGET_INDEX },
                unsafe { TEST_TARGET_INDEX }
            )
            .parse()
            .unwrap(),
            remote_url: format!("remote url {}", unsafe { TEST_TARGET_INDEX }),
            sha: format!("0123456789abcdef0123456789abcdef0123456{}", unsafe {
                TEST_TARGET_INDEX
            })
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

        let target_writer = target::Writer::new(&gb_repository);
        target_writer.write_default(&test_target())?;

        let branch_writer = branch::Writer::new(&gb_repository);
        let branch_1 = test_branch();
        branch_writer.write(&branch_1)?;
        let branch_2 = test_branch();
        branch_writer.write(&branch_2)?;
        let branch_3 = test_branch();
        branch_writer.write(&branch_3)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;

        let mut iter = BranchIterator::new(&session_reader)?;
        assert_eq!(iter.next().unwrap().unwrap(), branch_1);
        assert_eq!(iter.next().unwrap().unwrap(), branch_2);
        assert_eq!(iter.next().unwrap().unwrap(), branch_3);

        Ok(())
    }
}
