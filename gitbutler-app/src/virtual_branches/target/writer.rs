use anyhow::{Context, Result};

use crate::{gb_repository, reader, virtual_branches::BranchId, writer};

use super::Target;

pub struct TargetWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
    reader: reader::Reader<'writer>,
}

impl<'writer> TargetWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self, std::io::Error> {
        let reader = reader::Reader::open(&repository.root())?;
        let writer = writer::DirWriter::open(repository.root())?;
        Ok(Self {
            repository,
            writer,
            reader,
        })
    }

    pub fn write_default(&self, target: &Target) -> Result<()> {
        let reader = self.reader.sub("branches/target");
        match Target::try_from(&reader) {
            Ok(existing) if existing.eq(target) => return Ok(()),
            Ok(_) | Err(reader::Error::NotFound) => {}
            Err(e) => return Err(e.into()),
        };

        self.repository.mark_active_session()?;

        let batch = vec![
            writer::BatchTask::Write(
                "branches/target/branch_name",
                format!("{}/{}", target.branch.remote(), target.branch.branch()),
            ),
            writer::BatchTask::Write(
                "branches/target/remote_name",
                target.branch.remote().to_string(),
            ),
            writer::BatchTask::Write("branches/target/remote_url", target.remote_url.clone()),
            writer::BatchTask::Write("branches/target/sha", target.sha.to_string()),
        ];

        self.writer
            .batch(&batch)
            .context("Failed to write default target")?;

        Ok(())
    }

    pub fn write(&self, id: &BranchId, target: &Target) -> Result<()> {
        let reader = self.reader.sub(format!("branches/{}/target", id));
        match Target::try_from(&reader) {
            Ok(existing) if existing.eq(target) => return Ok(()),
            Ok(_) | Err(reader::Error::NotFound) => {}
            Err(e) => return Err(e.into()),
        };

        self.repository
            .mark_active_session()
            .context("Failed to get or create current session")?;

        let batch = vec![
            writer::BatchTask::Write(
                format!("branches/{}/target/branch_name", id),
                format!("{}/{}", target.branch.remote(), target.branch.branch()),
            ),
            writer::BatchTask::Write(
                format!("branches/{}/target/remote_name", id),
                target.branch.remote().to_string(),
            ),
            writer::BatchTask::Write(
                format!("branches/{}/target/remote_url", id),
                target.remote_url.clone(),
            ),
            writer::BatchTask::Write(
                format!("branches/{}/target/sha", id),
                target.sha.to_string(),
            ),
        ];

        self.writer
            .batch(&batch)
            .context("Failed to write target")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use once_cell::sync::Lazy;

    use crate::{
        tests::{Case, Suite},
        virtual_branches::branch,
    };

    use super::{super::Target, *};

    static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    fn test_branch() -> branch::Branch {
        TEST_INDEX.fetch_add(1, Ordering::Relaxed);

        branch::Branch {
            id: BranchId::generate(),
            name: format!("branch_name_{}", TEST_INDEX.load(Ordering::Relaxed)),
            notes: format!("branch_notes_{}", TEST_INDEX.load(Ordering::Relaxed)),
            applied: true,
            created_timestamp_ms: TEST_INDEX.load(Ordering::Relaxed) as u128,
            upstream: Some(
                format!(
                    "refs/remotes/origin/upstream_{}",
                    TEST_INDEX.load(Ordering::Relaxed)
                )
                .parse()
                .unwrap(),
            ),
            upstream_head: None,
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
    fn test_write() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let mut branch = test_branch();
        let target = Target {
            branch: "refs/remotes/remote name/branch name".parse().unwrap(),
            remote_url: "remote url".to_string(),
            sha: "0123456789abcdef0123456789abcdef01234567".parse().unwrap(),
        };

        let branch_writer = branch::Writer::new(&gb_repository)?;
        branch_writer.write(&mut branch)?;

        let target_writer = TargetWriter::new(&gb_repository)?;
        target_writer.write(&branch.id, &target)?;

        let root = gb_repository
            .root()
            .join("branches")
            .join(branch.id.to_string());

        assert_eq!(
            fs::read_to_string(root.join("meta").join("name").to_str().unwrap())
                .context("Failed to read branch name")?,
            branch.name
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("branch_name").to_str().unwrap())
                .context("Failed to read branch target name")?,
            format!("{}/{}", target.branch.remote(), target.branch.branch())
        );
        assert_eq!(
            fs::read_to_string(root.join("target").join("remote_name").to_str().unwrap())
                .context("Failed to read branch target name name")?,
            target.branch.remote()
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
        let Case { gb_repository, .. } = Suite::default().new_case();

        let mut branch = test_branch();
        let target = Target {
            branch: "refs/remotes/remote name/branch name".parse().unwrap(),
            remote_url: "remote url".to_string(),
            sha: "0123456789abcdef0123456789abcdef01234567".parse().unwrap(),
        };

        let branch_writer = branch::Writer::new(&gb_repository)?;
        branch_writer.write(&mut branch)?;
        let target_writer = TargetWriter::new(&gb_repository)?;
        target_writer.write(&branch.id, &target)?;

        let updated_target = Target {
            branch: "refs/remotes/updated remote name/updated branch name"
                .parse()
                .unwrap(),
            remote_url: "updated remote url".to_string(),
            sha: "fedcba9876543210fedcba9876543210fedcba98".parse().unwrap(),
        };

        target_writer.write(&branch.id, &updated_target)?;

        let root = gb_repository
            .root()
            .join("branches")
            .join(branch.id.to_string());

        assert_eq!(
            fs::read_to_string(root.join("target").join("branch_name").to_str().unwrap())
                .context("Failed to read branch target branch name")?,
            format!(
                "{}/{}",
                updated_target.branch.remote(),
                updated_target.branch.branch()
            )
        );

        assert_eq!(
            fs::read_to_string(root.join("target").join("remote_name").to_str().unwrap())
                .context("Failed to read branch target remote name")?,
            updated_target.branch.remote()
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
