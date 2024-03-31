use anyhow::anyhow;

use super::Target;
use crate::{
    reader, sessions,
    virtual_branches::{BranchId, VirtualBranchesHandle},
};

pub struct TargetReader<'r> {
    reader: &'r reader::Reader<'r>,
    state_handle: VirtualBranchesHandle,
    use_state_handle: bool,
}

impl<'r> TargetReader<'r> {
    pub fn new(
        reader: &'r sessions::Reader<'r>,
        state_handle: VirtualBranchesHandle,
        use_state_handle: bool,
    ) -> Self {
        Self {
            reader: reader.reader(),
            state_handle,
            use_state_handle,
        }
    }

    pub fn read_default(&self) -> Result<Target, reader::Error> {
        if self.use_state_handle && self.state_handle.file_exists() {
            self.state_handle
                .get_default_target()
                .and_then(|op| op.ok_or(anyhow!("Branch not found")))
                .map_err(|_| reader::Error::NotFound)
        } else {
            Target::try_from(&self.reader.sub("branches/target"))
        }
    }

    /// If the target for the specified branchid is not found, returns the default target
    pub fn read(&self, id: &BranchId) -> Result<Target, reader::Error> {
        if self.use_state_handle && self.state_handle.file_exists() {
            let branch_target = self.state_handle.get_branch_target(id);
            match branch_target {
                Ok(Some(target)) => Ok(target),
                Ok(None) => self.read_default(),
                Err(_) => Err(reader::Error::NotFound),
            }
        } else {
            if !self
                .reader
                .exists(format!("branches/{}/target", id))
                .map_err(reader::Error::from)?
            {
                return self.read_default();
            }

            Target::try_from(&self.reader.sub(format!("branches/{}/target", id)))
        }
    }
}
