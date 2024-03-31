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
        Target::try_from(&self.reader.sub("branches/target"))
    }

    pub fn read(&self, id: &BranchId) -> Result<Target, reader::Error> {
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
