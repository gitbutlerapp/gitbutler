use super::Target;
use crate::{reader, sessions, virtual_branches::BranchId};

pub struct TargetReader<'r> {
    reader: &'r reader::Reader<'r>,
}

impl<'r> TargetReader<'r> {
    pub fn new(reader: &'r sessions::Reader<'r>) -> Self {
        Self {
            reader: reader.reader(),
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
