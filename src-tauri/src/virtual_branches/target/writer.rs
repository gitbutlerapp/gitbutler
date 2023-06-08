use anyhow::{Context, Result};

use crate::{
    gb_repository,
    writer::{self, Writer},
};

use super::Target;

pub struct TargetWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> TargetWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Self {
        Self {
            repository,
            writer: writer::DirWriter::open(repository.root()),
        }
    }

    pub fn write_default(&self, target: &Target) -> Result<()> {
        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("Failed to unlock repository");
        }

        self.writer
            .write_string("branches/target/name", &target.name)
            .context("Failed to write default target name")?;
        self.writer
            .write_string("branches/target/remote", &target.remote)
            .context("Failed to write default target remote")?;
        self.writer
            .write_string("branches/target/sha", &target.sha.to_string())
            .context("Failed to write default target sha")?;
        Ok(())
    }

    pub fn write(&self, id: &str, target: &Target) -> Result<()> {
        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("Failed to unlock repository");
        }

        self.writer
            .write_string(&format!("branches/{}/target/name", id), &target.name)
            .context("Failed to write branch target name")?;
        self.writer
            .write_string(&format!("branches/{}/target/remote", id), &target.remote)
            .context("Failed to write branch target remote")?;
        self.writer
            .write_string(
                &format!("branches/{}/target/sha", id),
                &target.sha.to_string(),
            )
            .context("Failed to write branch target sha")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::{projects, storage, users, virtual_branches::branch};

    use super::{super::Target, *};

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
    fn test_write() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let branch = branch::Branch {
            id: "branch_id".to_string(),
            name: "name".to_string(),
            applied: true,
            upstream: "upstream".to_string(),
            created_timestamp_ms: 0,
            updated_timestamp_ms: 1,
        };
        let target = Target {
            name: "target_name".to_string(),
            remote: "target_remote".to_string(),
            sha: git2::Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
        };

        let branch_writer = branch::Writer::new(&gb_repo);
        branch_writer.write(&branch)?;

        let target_writer = TargetWriter::new(&gb_repo);
        target_writer.write(&branch.id, &target)?;

        let root = gb_repo.root().join("branches").join(&branch.id);

        assert_eq!(
            fs::read_to_string(root.join("meta").join("name").to_str().unwrap())
                .context("Failed to read branch name")?,
            branch.name
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("name").to_str().unwrap())
                .context("Failed to read branch target name")?,
            target.name
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("remote").to_str().unwrap())
                .context("Failed to read branch target remote")?,
            target.remote
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("sha").to_str().unwrap())
                .context("Failed to read branch target sha")?,
            target.sha.to_string()
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

        let branch = branch::Branch {
            id: "branch_id".to_string(),
            name: "name".to_string(),
            applied: true,
            upstream: "upstream".to_string(),
            created_timestamp_ms: 0,
            updated_timestamp_ms: 1,
        };
        let target = Target {
            name: "target_name".to_string(),
            remote: "target_remote".to_string(),
            sha: git2::Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
        };

        let branch_writer = branch::Writer::new(&gb_repo);
        branch_writer.write(&branch)?;
        let target_writer = TargetWriter::new(&gb_repo);
        target_writer.write(&branch.id, &target)?;

        let updated_target = Target {
            name: "updated_target_name".to_string(),
            remote: "updated_target_remote".to_string(),
            sha: git2::Oid::from_str("fedcba9876543210fedcba9876543210fedcba98").unwrap(),
        };

        target_writer.write(&branch.id, &updated_target)?;

        let root = gb_repo.root().join("branches").join(&branch.id);

        assert_eq!(
            fs::read_to_string(root.join("target").join("name").to_str().unwrap())
                .context("Failed to read branch target name")?,
            updated_target.name
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("remote").to_str().unwrap())
                .context("Failed to read branch target remote")?,
            updated_target.remote
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("sha").to_str().unwrap())
                .context("Failed to read branch target sha")?,
            updated_target.sha.to_string()
        );

        Ok(())
    }
}
