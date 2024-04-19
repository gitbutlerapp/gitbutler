use std::collections::HashSet;

use anyhow::Result;

use super::{
    branch::{self, BranchId},
    VirtualBranchesHandle,
};
use crate::sessions;

pub struct BranchIterator<'i> {
    branch_reader: branch::Reader<'i>,
    ids: Vec<BranchId>,
}

impl<'i> BranchIterator<'i> {
    pub fn new(
        session_reader: &'i sessions::Reader<'i>,
        state_handle: VirtualBranchesHandle,
        use_state_handle: bool,
    ) -> Result<Self> {
        let unique_ids: HashSet<String> = if use_state_handle && state_handle.file_exists() {
            state_handle.list_branch_ids()?
        } else {
            session_reader
                .reader()
                .list_files("branches")?
                .into_iter()
                .map(|file_path| {
                    file_path
                        .iter()
                        .next()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                })
                .filter(|file_path| file_path != "selected")
                .filter(|file_path| file_path != "target")
                .collect()
        };
        let mut ids: Vec<BranchId> = unique_ids
            .into_iter()
            .map(|id| id.parse())
            .filter_map(Result::ok)
            .collect();
        ids.sort();
        Ok(Self {
            branch_reader: branch::Reader::new(session_reader, state_handle, use_state_handle),
            ids,
        })
    }
}

impl Iterator for BranchIterator<'_> {
    type Item = Result<branch::Branch, crate::reader::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ids.is_empty() {
            return None;
        }

        let id = self.ids.remove(0);
        let branch = self.branch_reader.read(&id);
        Some(branch)
    }
}
