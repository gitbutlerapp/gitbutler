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
        self.repository
            .get_or_create_current_session()
            .context("Failed to get or create current session")?;

        let _lock = self.repository.lock();

        self.writer
            .write_string("branches/target/branch_name", &target.branch_name)
            .context("Failed to write default target branch name")?;
        self.writer
            .write_string("branches/target/remote_name", &target.remote_name)
            .context("Failed to write default target remote name ")?;
        self.writer
            .write_string("branches/target/remote_url", &target.remote_url)
            .context("Failed to write default target remote name ")?;
        self.writer
            .write_string("branches/target/sha", &target.sha.to_string())
            .context("Failed to write default target sha")?;
        Ok(())
    }

    pub fn write(&self, id: &str, target: &Target) -> Result<()> {
        self.repository
            .get_or_create_current_session()
            .context("Failed to get or create current session")?;

        let _lock = self.repository.lock();

        self.writer
            .write_string(
                &format!("branches/{}/target/branch_name", id),
                &target.branch_name,
            )
            .context("Failed to write branch target branch name")?;
        self.writer
            .write_string(
                &format!("branches/{}/target/remote_name", id),
                &target.remote_name,
            )
            .context("Failed to write branch target remote")?;
        self.writer
            .write_string(
                &format!("branches/{}/target/remote_url", id),
                &target.remote_url,
            )
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

    use crate::{projects, test_utils, users, virtual_branches::branch};

    use super::{super::Target, *};

    static mut TEST_INDEX: usize = 0;

    fn test_branch() -> branch::Branch {
        unsafe {
            TEST_INDEX += 1;
        }
        branch::Branch {
            id: format!("branch_{}", unsafe { TEST_INDEX }),
            name: format!("branch_name_{}", unsafe { TEST_INDEX }),
            notes: format!("branch_notes_{}", unsafe { TEST_INDEX }),
            applied: true,
            created_timestamp_ms: unsafe { TEST_INDEX } as u128,
            upstream: Some(
                format!("refs/remotes/origin/upstream_{}", unsafe { TEST_INDEX })
                    .parse()
                    .unwrap(),
            ),
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

    #[test]
    fn test_write() -> Result<()> {
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = test_utils::temp_dir();
        let local_repo_path = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_repo_path);
        let project_store = projects::Storage::from(&local_repo_path);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

        let branch = test_branch();
        let target = Target {
            branch_name: "branch name".to_string(),
            remote_name: "remote name".to_string(),
            remote_url: "remote url".to_string(),
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
            fs::read_to_string(root.join("target").join("branch_name").to_str().unwrap())
                .context("Failed to read branch target name")?,
            target.branch_name
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("remote_name").to_str().unwrap())
                .context("Failed to read branch target name name")?,
            target.remote_name
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("remote_url").to_str().unwrap())
                .context("Failed to read branch target remote url")?,
            target.remote_url
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
            branch.upstream.unwrap().to_string()
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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = test_utils::temp_dir();
        let local_repo_path = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_repo_path);
        let project_store = projects::Storage::from(&local_repo_path);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

        let branch = test_branch();
        let target = Target {
            remote_name: "remote name".to_string(),
            branch_name: "branch name".to_string(),
            remote_url: "remote url".to_string(),
            sha: git2::Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
        };

        let branch_writer = branch::Writer::new(&gb_repo);
        branch_writer.write(&branch)?;
        let target_writer = TargetWriter::new(&gb_repo);
        target_writer.write(&branch.id, &target)?;

        let updated_target = Target {
            remote_name: "updated remote name".to_string(),
            branch_name: "updated branch name".to_string(),
            remote_url: "updated remote url".to_string(),
            sha: git2::Oid::from_str("fedcba9876543210fedcba9876543210fedcba98").unwrap(),
        };

        target_writer.write(&branch.id, &updated_target)?;

        let root = gb_repo.root().join("branches").join(&branch.id);

        assert_eq!(
            fs::read_to_string(root.join("target").join("branch_name").to_str().unwrap())
                .context("Failed to read branch target branch name")?,
            updated_target.branch_name
        );

        assert_eq!(
            fs::read_to_string(root.join("target").join("remote_name").to_str().unwrap())
                .context("Failed to read branch target remote name")?,
            updated_target.remote_name
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("remote_url").to_str().unwrap())
                .context("Failed to read branch target remote url")?,
            updated_target.remote_url
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("sha").to_str().unwrap())
                .context("Failed to read branch target sha")?,
            updated_target.sha.to_string()
        );

        Ok(())
    }
}
