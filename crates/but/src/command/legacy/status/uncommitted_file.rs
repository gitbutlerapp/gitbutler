use bstr::BString;

use crate::{CliId, IdMap, utils::change_source::ChangeSourceId};

/// An uncommitted file with the ID that addresses it.
#[derive(Debug, Clone)]
pub(crate) struct UncommittedFileWithId {
    /// The ID naming the whole file.
    pub cli_id: CliId,
    /// The worktree-relative path of the file.
    pub path: BString,
}

impl UncommittedFileWithId {
    /// The uncommitted files of `source`, ordered by path.
    ///
    /// [`IdMap::uncommitted_files`] holds one entry per source and path, but is keyed
    /// by reverse-hex ID, so the paths need sorting for display. Filtering by source
    /// keeps each checkout's changes under its own heading.
    pub fn in_source(id_map: &IdMap, source: &ChangeSourceId) -> Vec<Self> {
        let mut files: Vec<Self> = id_map
            .uncommitted_files
            .values()
            .filter(|file| file.source == *source)
            .map(|file| Self {
                cli_id: file.to_id(),
                path: file.path().to_owned(),
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        files
    }
}
