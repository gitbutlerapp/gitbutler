use std::path;

use anyhow::Result;

use crate::{gb_repository, reader, virtual_branches::state::VirtualBranchesHandle, writer};

use super::Branch;

pub struct BranchWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
    reader: reader::Reader<'writer>,
    state_handle: VirtualBranchesHandle,
}

impl<'writer> BranchWriter<'writer> {
    pub fn new<P: AsRef<path::Path>>(
        repository: &'writer gb_repository::Repository,
        path: P,
    ) -> Result<Self, std::io::Error> {
        let reader = reader::Reader::open(repository.root())?;
        let writer = writer::DirWriter::open(repository.root())?;
        let state_handle = VirtualBranchesHandle::new(path.as_ref());
        Ok(Self {
            repository,
            writer,
            reader,
            state_handle,
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
                // Write in the state file as well
                let _ = self.state_handle.remove_branch(branch.id);
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

        // Write in the state file as well
        self.state_handle.set_branch(branch.clone())?;

        Ok(())
    }
}
