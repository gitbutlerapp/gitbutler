//! Provides some slightly higher level tools to help with manipulating commits, in preparation for use in the editor.

use anyhow::Result;
use gix::prelude::ObjectIdExt;

use crate::{
    commit::{DateMode, create},
    graph_rebase::Editor,
};

impl Editor {
    /// Finds a commit from inside the editor's in memory repository.
    pub fn find_commit(&self, id: gix::ObjectId) -> Result<but_core::Commit<'_>> {
        but_core::Commit::from_id(id.attach(&self.repo))
    }

    /// Writes a commit with correct signing to the in memory repository.
    pub fn write_commit(
        &self,
        commit: but_core::Commit<'_>,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        create(&self.repo, commit.inner, date_mode)
    }
}
