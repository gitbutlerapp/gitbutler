use super::{Branch, BranchId};
use crate::{reader, sessions};

pub struct BranchReader<'r> {
    reader: &'r reader::Reader<'r>,
}

impl<'r> BranchReader<'r> {
    pub fn new(reader: &'r sessions::Reader<'r>) -> Self {
        Self {
            reader: reader.reader(),
        }
    }

    pub fn read(&self, id: &BranchId) -> Result<Branch, reader::Error> {
        Branch::from_reader(&self.reader.sub(format!("branches/{}", id)))
    }
}
