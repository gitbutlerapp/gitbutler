use std::collections::HashSet;

use anyhow::Result;

use crate::reader;

use super::branch;

pub struct BranchIterator<'iterator> {
    branch_reader: branch::Reader<'iterator>,
    ids: Vec<String>,
}

impl<'iterator> BranchIterator<'iterator> {
    pub fn new(sessions_reader: &'iterator dyn reader::Reader) -> Result<Self> {
        let ids_itarator = sessions_reader
            .list_files("branches")?
            .into_iter()
            .map(|file| file.split('/').next().unwrap().to_string());
        let unique_ids: HashSet<String> = ids_itarator.collect();
        let mut ids: Vec<String> = unique_ids.into_iter().collect();
        ids.sort();
        Ok(Self {
            branch_reader: branch::Reader::new(sessions_reader),
            ids,
        })
    }
}

impl<'iterator> Iterator for BranchIterator<'iterator> {
    type Item = Result<branch::Branch, branch::ReadError>;

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
    use tempfile::tempdir;

    use crate::{gb_repository, projects, sessions, storage, users, virtual_branches::target};

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
        }
    }

    static mut TEST_TARGET_INDEX: usize = 0;

    fn test_target() -> target::Target {
        target::Target {
            name: format!("target_name_{}", unsafe { TEST_TARGET_INDEX }),
            remote: format!("remote_{}", unsafe { TEST_TARGET_INDEX }),
            sha: git2::Oid::from_str(&format!(
                "0123456789abcdef0123456789abcdef0123456{}",
                unsafe { TEST_TARGET_INDEX }
            ))
            .unwrap(),
        }
    }

    #[test]
    fn test_empty_iterator() -> Result<()> {
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

        let iter = BranchIterator::new(&session_reader)?;

        assert_eq!(iter.count(), 0);

        Ok(())
    }

    #[test]
    fn test_iterate_all() -> Result<()> {
        let repository = test_repository()?;
        let project = test_project(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let target_writer = target::Writer::new(&gb_repo);
        target_writer.write_default(&test_target())?;

        let branch_writer = branch::Writer::new(&gb_repo);
        let branch_1 = test_branch();
        branch_writer.write(&branch_1)?;
        let branch_2 = test_branch();
        branch_writer.write(&branch_2)?;
        let branch_3 = test_branch();
        branch_writer.write(&branch_3)?;

        let session = gb_repo.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;

        let mut iter = BranchIterator::new(&session_reader)?;
        assert_eq!(iter.next().unwrap().unwrap(), branch_1);
        assert_eq!(iter.next().unwrap().unwrap(), branch_2);
        assert_eq!(iter.next().unwrap().unwrap(), branch_3);

        Ok(())
    }
}
