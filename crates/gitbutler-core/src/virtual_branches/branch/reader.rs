use anyhow::anyhow;

use super::{Branch, BranchId};
use crate::{reader, sessions, virtual_branches::VirtualBranchesHandle};

pub struct BranchReader<'r> {
    reader: &'r reader::Reader<'r>,
    state_handle: VirtualBranchesHandle,
    use_state_handle: bool,
}

impl<'r> BranchReader<'r> {
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

    pub fn read(&self, id: &BranchId) -> Result<Branch, reader::Error> {
        if self.use_state_handle && self.state_handle.file_exists() {
            self.state_handle
                .get_branch(id)
                .and_then(|op| op.ok_or(anyhow!("Branch not found")))
                .map_err(|_| reader::Error::NotFound)
        } else {
            Branch::from_reader(&self.reader.sub(format!("branches/{}", id)))
        }
    }
}
