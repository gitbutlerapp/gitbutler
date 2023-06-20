use anyhow::{Context, Result};

use crate::{
    gb_repository,
    writer::{self, Writer},
};

use super::Branch;

pub struct BranchWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> BranchWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Self {
        Self {
            repository,
            writer: writer::DirWriter::open(repository.root()),
        }
    }

    pub fn write_selected(&self, id: &Option<String>) -> Result<()> {
        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("Failed to unlock repository");
        }

        if let Some(id) = id {
            self.writer
                .write_string("branches/selected", id)
                .context("Failed to write selected branch")?;
        } else {
            self.writer
                .remove("branches/selected")
                .context("Failed to remove selected branch")?;
        }

        Ok(())
    }

    pub fn write(&self, branch: &Branch) -> Result<()> {
        self.repository
            .get_or_create_current_session()
            .context("Failed to get or create current session")?;

        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("Failed to unlock repository");
        }

        self.writer
            .write_string(&format!("branches/{}/id", branch.id), &branch.id)
            .context("Failed to write branch id")?;

        self.writer
            .write_string(&format!("branches/{}/meta/name", branch.id), &branch.name)
            .context("Failed to write branch name")?;

        self.writer
            .write_bool(
                &format!("branches/{}/meta/applied", branch.id),
                &branch.applied,
            )
            .context("Failed to write branch applied")?;
        self.writer
            .write_string(
                &format!("branches/{}/meta/upstream", branch.id),
                &branch.upstream,
            )
            .context("Failed to write branch upstream")?;
        self.writer
            .write_string(
                &format!("branches/{}/meta/tree", branch.id),
                &branch.tree.to_string(),
            )
            .context("Failed to write branch tree")?;
        self.writer
            .write_string(
                &format!("branches/{}/meta/head", branch.id),
                &branch.head.to_string(),
            )
            .context("Failed to write branch head")?;
        self.writer
            .write_u128(
                &format!("branches/{}/meta/created_timestamp_ms", branch.id),
                &branch.created_timestamp_ms,
            )
            .context("Failed to write branch created timestamp")?;
        self.writer
            .write_u128(
                &format!("branches/{}/meta/updated_timestamp_ms", branch.id),
                &branch.updated_timestamp_ms,
            )
            .context("Failed to write branch updated timestamp")?;
        // convert ownership to string by joining Vec<String> with newlines
        let ownership = branch
            .ownership
            .iter()
            .map(|user| user.to_string())
            .collect::<Vec<String>>()
            .join("\n");

        self.writer
            .write_string(
                &format!("branches/{}/meta/ownership", branch.id),
                &ownership,
            )
            .context("Failed to write branch ownership")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::{projects, storage, users};

    use super::*;

    static mut TEST_INDEX: usize = 0;

    fn test_branch() -> Branch {
        unsafe {
            TEST_INDEX += 1;
        }
        Branch {
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
            ownership: vec![format!("file/{}", unsafe { TEST_INDEX })],
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
    fn test_write_branch() -> Result<()> {
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

        let writer = BranchWriter::new(&gb_repo);
        writer.write(&branch)?;

        let root = gb_repo.root().join("branches").join(&branch.id);

        assert_eq!(
            fs::read_to_string(root.join("meta").join("name").to_str().unwrap())
                .context("Failed to read branch name")?,
            branch.name
        );
        assert_eq!(
            fs::read_to_string(root.join("meta").join("applied").to_str().unwrap())?
                .parse::<bool>()
                .context("Failed to read branch applied")?,
            branch.applied
        );
        assert_eq!(
            fs::read_to_string(root.join("meta").join("upstream").to_str().unwrap())
                .context("Failed to read branch upstream")?,
            branch.upstream
        );
        assert_eq!(
            fs::read_to_string(
                root.join("meta")
                    .join("created_timestamp_ms")
                    .to_str()
                    .unwrap()
            )
            .context("Failed to read branch created timestamp")?
            .parse::<u128>()
            .context("Failed to parse branch created timestamp")?,
            branch.created_timestamp_ms
        );
        assert_eq!(
            fs::read_to_string(
                root.join("meta")
                    .join("updated_timestamp_ms")
                    .to_str()
                    .unwrap()
            )
            .context("Failed to read branch updated timestamp")?
            .parse::<u128>()
            .context("Failed to parse branch updated timestamp")?,
            branch.updated_timestamp_ms
        );

        Ok(())
    }

    #[test]
    fn test_should_create_session() -> Result<()> {
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

        let writer = BranchWriter::new(&gb_repo);
        writer.write(&branch)?;

        assert!(gb_repo.get_current_session()?.is_some());

        Ok(())
    }

    #[test]
    fn test_should_update() -> Result<()> {
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

        let writer = BranchWriter::new(&gb_repo);
        writer.write(&branch)?;

        let updated_branch = Branch {
            name: "updated_name".to_string(),
            applied: false,
            upstream: "updated_upstream".to_string(),
            created_timestamp_ms: 2,
            updated_timestamp_ms: 3,
            ownership: vec![],
            ..branch.clone()
        };

        writer.write(&updated_branch)?;

        let root = gb_repo.root().join("branches").join(&branch.id);

        assert_eq!(
            fs::read_to_string(root.join("meta").join("name").to_str().unwrap())
                .context("Failed to read branch name")?,
            updated_branch.name
        );
        assert_eq!(
            fs::read_to_string(root.join("meta").join("applied").to_str().unwrap())?
                .parse::<bool>()
                .context("Failed to read branch applied")?,
            updated_branch.applied
        );
        assert_eq!(
            fs::read_to_string(root.join("meta").join("upstream").to_str().unwrap())
                .context("Failed to read branch upstream")?,
            updated_branch.upstream
        );
        assert_eq!(
            fs::read_to_string(
                root.join("meta")
                    .join("created_timestamp_ms")
                    .to_str()
                    .unwrap()
            )
            .context("Failed to read branch created timestamp")?
            .parse::<u128>()
            .context("Failed to parse branch created timestamp")?,
            updated_branch.created_timestamp_ms
        );
        assert_eq!(
            fs::read_to_string(
                root.join("meta")
                    .join("updated_timestamp_ms")
                    .to_str()
                    .unwrap()
            )
            .context("Failed to read branch updated timestamp")?
            .parse::<u128>()
            .context("Failed to parse branch updated timestamp")?,
            updated_branch.updated_timestamp_ms
        );

        Ok(())
    }

    #[test]
    fn test_write_selected() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let writer = BranchWriter::new(&gb_repo);

        assert!(!gb_repo.root().join("branches").join("selected").exists());

        writer.write_selected(&None)?;
        assert!(!gb_repo.root().join("branches").join("selected").exists());

        writer.write_selected(&Some("123".to_string()))?;
        assert_eq!(
            fs::read_to_string(
                gb_repo
                    .root()
                    .join("branches")
                    .join("selected")
                    .to_str()
                    .unwrap()
            )?,
            "123"
        );

        writer.write_selected(&None)?;
        assert!(!gb_repo.root().join("branches").join("selected").exists());

        Ok(())
    }
}
