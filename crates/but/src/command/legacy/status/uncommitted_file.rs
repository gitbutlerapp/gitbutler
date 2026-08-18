use bstr::BString;

use crate::{IdMap, id::ShortId, utils::change_source::ChangeSourceId};

/// An uncommitted file with the short ID that addresses it.
#[derive(Debug, Clone)]
pub(crate) struct UncommittedFileWithId {
    /// The short ID.
    pub short_id: ShortId,
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
                short_id: file.short_id.clone(),
                path: file.path().to_owned(),
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        files
    }
}
