use anyhow::{Context, Result};

use super::Target;
use crate::{
    gb_repository, reader,
    virtual_branches::{state::VirtualBranchesHandle, BranchId},
    writer,
};

pub struct TargetWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
    reader: reader::Reader<'writer>,
    state_handle: VirtualBranchesHandle,
}

impl<'writer> TargetWriter<'writer> {
    pub fn new(
        repository: &'writer gb_repository::Repository,
        state_handle: VirtualBranchesHandle,
    ) -> Result<Self, std::io::Error> {
        let reader = reader::Reader::open(&repository.root())?;
        let writer = writer::DirWriter::open(repository.root())?;
        Ok(Self {
            repository,
            writer,
            reader,
            state_handle,
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

        // Write in the state file as well
        let _ = self.state_handle.set_default_target(target.clone());

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

        // Write in the state file as well
        let _ = self.state_handle.set_branch_target(*id, target.clone());

        Ok(())
    }
}
