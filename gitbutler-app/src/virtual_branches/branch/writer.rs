use anyhow::Result;

use crate::{gb_repository, reader, writer};

use super::Branch;

pub struct BranchWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
    reader: reader::Reader<'writer>,
}

impl<'writer> BranchWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self, std::io::Error> {
        let reader = reader::Reader::open(repository.root())?;
        let writer = writer::DirWriter::open(repository.root())?;
        Ok(Self {
            repository,
            writer,
            reader,
        })
    }

    pub fn delete(&self, branch: &Branch) -> Result<()> {
        match self
            .reader
            .sub(format!("branches/{}", branch.id))
            .read("id")
        {
            Ok(_) => {
                self.repository.mark_active_session()?;
                let _lock = self.repository.lock();
                self.writer.remove(format!("branches/{}", branch.id))?;
                Ok(())
            }
            Err(reader::Error::NotFound) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    pub fn write(&self, branch: &mut Branch) -> Result<()> {
        let reader = self.reader.sub(format!("branches/{}", branch.id));
        match Branch::from_reader(&reader) {
            Ok(existing) if existing.eq(branch) => return Ok(()),
            Ok(_) | Err(reader::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }

        self.repository.mark_active_session()?;

        branch.updated_timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis();

        let mut batch = vec![];

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/id", branch.id),
            branch.id.to_string(),
        ));

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/name", branch.id),
            branch.name.clone(),
        ));

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/notes", branch.id),
            branch.notes.clone(),
        ));

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/order", branch.id),
            branch.order.to_string(),
        ));

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/applied", branch.id),
            branch.applied.to_string(),
        ));

        if let Some(upstream) = &branch.upstream {
            batch.push(writer::BatchTask::Write(
                format!("branches/{}/meta/upstream", branch.id),
                upstream.to_string(),
            ));
        } else {
            batch.push(writer::BatchTask::Remove(format!(
                "branches/{}/meta/upstream",
                branch.id
            )));
        }

        if let Some(upstream_head) = &branch.upstream_head {
            batch.push(writer::BatchTask::Write(
                format!("branches/{}/meta/upstream_head", branch.id),
                upstream_head.to_string(),
            ));
        } else {
            batch.push(writer::BatchTask::Remove(format!(
                "branches/{}/meta/upstream_head",
                branch.id
            )));
        }

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/tree", branch.id),
            branch.tree.to_string(),
        ));

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/head", branch.id),
            branch.head.to_string(),
        ));

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/created_timestamp_ms", branch.id),
            branch.created_timestamp_ms.to_string(),
        ));

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/updated_timestamp_ms", branch.id),
            branch.updated_timestamp_ms.to_string(),
        ));

        batch.push(writer::BatchTask::Write(
            format!("branches/{}/meta/ownership", branch.id),
            branch.ownership.to_string(),
        ));

        if let Some(selected_for_changes) = branch.selected_for_changes {
            batch.push(writer::BatchTask::Write(
                format!("branches/{}/meta/selected_for_changes", branch.id),
                selected_for_changes.to_string(),
            ));
        } else {
            batch.push(writer::BatchTask::Remove(format!(
                "branches/{}/meta/selected_for_changes",
                branch.id
            )));
        }

        self.writer.batch(&batch)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use anyhow::Context;
    use once_cell::sync::Lazy;

    use crate::{
        tests::{Case, Suite},
        virtual_branches::branch,
    };

    use self::branch::BranchId;

    use super::*;

    static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    fn test_branch() -> Branch {
        TEST_INDEX.fetch_add(1, Ordering::Relaxed);

        Branch {
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
            ownership: branch::Ownership {
                files: vec![branch::FileOwnership {
                    file_path: format!("file/{}:1-2", TEST_INDEX.load(Ordering::Relaxed)).into(),
                    hunks: vec![],
                }],
            },
            order: TEST_INDEX.load(Ordering::Relaxed),
            selected_for_changes: Some(1),
        }
    }

    #[test]
    fn test_write_branch() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let mut branch = test_branch();

        let writer = BranchWriter::new(&gb_repository)?;
        writer.write(&mut branch)?;

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
            fs::read_to_string(root.join("meta").join("applied").to_str().unwrap())?
                .parse::<bool>()
                .context("Failed to read branch applied")?,
            branch.applied
        );
        assert_eq!(
            fs::read_to_string(root.join("meta").join("upstream").to_str().unwrap())
                .context("Failed to read branch upstream")?,
            branch.upstream.clone().unwrap().to_string()
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

        writer.delete(&branch)?;
        fs::read_dir(root).unwrap_err();

        Ok(())
    }

    #[test]
    fn test_should_create_session() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let mut branch = test_branch();

        let writer = BranchWriter::new(&gb_repository)?;
        writer.write(&mut branch)?;

        assert!(gb_repository.get_current_session()?.is_some());

        Ok(())
    }

    #[test]
    fn test_should_update() -> Result<()> {
        let Case { gb_repository, .. } = Suite::default().new_case();

        let mut branch = test_branch();

        let writer = BranchWriter::new(&gb_repository)?;
        writer.write(&mut branch)?;

        let mut updated_branch = Branch {
            name: "updated_name".to_string(),
            applied: false,
            upstream: Some("refs/remotes/origin/upstream_updated".parse().unwrap()),
            created_timestamp_ms: 2,
            updated_timestamp_ms: 3,
            ownership: branch::Ownership { files: vec![] },
            ..branch.clone()
        };

        writer.write(&mut updated_branch)?;

        let root = gb_repository
            .root()
            .join("branches")
            .join(branch.id.to_string());

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
            updated_branch.upstream.unwrap().to_string()
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
}
