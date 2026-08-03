use bstr::BString;

use crate::{IdMap, id::ShortId};

/// An uncommitted file with the short ID that addresses it.
#[derive(Debug, Clone)]
pub(crate) struct UncommittedFileWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// The worktree-relative path of the file.
    pub path: BString,
}

impl UncommittedFileWithId {
    /// All uncommitted files, ordered by path.
    ///
    /// [`IdMap::uncommitted_files`] already holds exactly one entry per path, but is keyed
    /// by reverse-hex ID, so the paths need sorting for display.
    pub fn all_by_path(id_map: &IdMap) -> Vec<Self> {
        let mut files: Vec<Self> = id_map
            .uncommitted_files
            .values()
            .map(|file| Self {
                short_id: file.short_id.clone(),
                path: file.path().to_owned(),
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        files
    }
}
