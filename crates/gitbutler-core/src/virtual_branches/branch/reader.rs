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
        Branch::from_reader(&self.reader.sub(format!("branches/{}", id)))
    }
}
