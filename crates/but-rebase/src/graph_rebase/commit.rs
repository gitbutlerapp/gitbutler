//! Provides some slightly higher level tools to help with manipulating commits, in preparation for use in the editor.

use anyhow::{Context as _, Result};
use gix::prelude::ObjectIdExt;

use crate::{
    commit::{DateMode, create},
    graph_rebase::Editor,
};

impl Editor {
    /// Returns a reference to the in-memory repository.
    pub fn repo(&self) -> &gix::Repository {
        &self.repo
    }

    /// Finds a commit from inside the editor's in memory repository.
    pub fn find_commit(&self, id: gix::ObjectId) -> Result<but_core::Commit<'_>> {
        but_core::Commit::from_id(id.attach(&self.repo))
    }

    /// Writes a commit with correct signing to the in memory repository.
    pub fn new_commit(
        &self,
        commit: but_core::Commit<'_>,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        // TODO(GB-983): As part of moving to only signing at the materializing
        // step, this should have sign_if_configured false here.
        create(&self.repo, commit.inner, date_mode, true)
    }

    /// Creates a commit with only the signature and author set correctly.
    ///
    /// The ID of the commit is all zeros & the commit hasn't been written into any ODB
    pub fn empty_commit(&self) -> Result<but_core::Commit<'_>> {
        let kind = gix::hash::Kind::Sha1;
        let committer = self
            .repo
            .committer()
            .transpose()?
            .context("Need committer to be configured when creating a new commit")?
            .into();
        let author = self
            .repo
            .committer()
            .transpose()?
            .context("Need author to be configured when creating a new commit")?
            .into();
        let obj = gix::objs::Commit {
            tree: gix::ObjectId::empty_tree(kind),
            parents: vec![].into(),
            committer,
            author,
            encoding: None,
            message: b"".into(),
            extra_headers: vec![],
        };

        Ok(but_core::Commit::<'_> {
            id: gix::ObjectId::null(kind).attach(&self.repo),
            inner: obj,
        })
    }
}
