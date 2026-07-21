//! Provides some slightly higher level tools to help with manipulating commits, in preparation for use in the graph edit.

use anyhow::{Context, Result, bail};
use but_core::commit::{Headers, SignCommit, write::DateMode};
use gix::prelude::ObjectIdExt;

use crate::{
    NodeIndex,
    edit::{MutableNodeGraph, ToCommitSelector, ToReferenceSelector},
    node::resolve_to_commit,
};

impl MutableNodeGraph {
    /// Finds a commit from inside the edit's in memory repository.
    pub fn find_commit(&self, id: gix::ObjectId) -> Result<but_core::CommitOwned> {
        but_core::Commit::from_id(id.attach(self.repo())).map(|c| c.detach())
    }

    /// Finds a commit that is selectable in the graph and is found in the
    /// edit's repo.
    ///
    /// Returns the node index and the found commit.
    pub fn find_selectable_commit(
        &self,
        selector: impl ToCommitSelector,
    ) -> Result<(NodeIndex, but_core::CommitOwned)> {
        let index = selector.to_commit_selector(self)?;
        let Some(pick) = self.pick_at(index) else {
            bail!("BUG: Expected a pick from commit selector. This should never happen");
        };
        Ok((index, self.find_commit(pick.id)?))
    }

    /// Finds the first pick parent of a reference
    pub fn find_reference_target(
        &self,
        selector: impl ToReferenceSelector,
    ) -> Result<(NodeIndex, but_core::CommitOwned)> {
        let index = selector.to_reference_selector(self)?;

        let target = resolve_to_commit(self.nodes(), index)
            .context("Failed to find a target for selected reference in the graph.")?;

        let Some(pick) = self.pick_at(target) else {
            bail!("BUG: resolve_to_commit provided a non-pick return value");
        };

        Ok((target, self.find_commit(pick.id)?))
    }

    /// Writes a commit with correct signing to the in memory repository.
    ///
    /// This does not update the commit mappings; a rewrite is only recorded
    /// once the commit is installed into the graph via
    /// [`MutableNodeGraph::replace`] or a rebase.
    pub fn new_commit(
        &self,
        commit: but_core::CommitOwned,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        let change_id = commit.change_id();
        but_core::commit::write::create(
            self.repo(),
            commit.inner,
            date_mode,
            SignCommit::IfSignCommitsEnabled,
            Some(change_id),
        )
    }

    /// Creates a commit with only the signature, author, and headers set correctly.
    ///
    /// The ID of the commit is all zeros & the commit hasn't been written into any ODB
    pub fn empty_commit(&self) -> Result<but_core::CommitOwned> {
        let kind = gix::hash::Kind::Sha1;
        let committer = self
            .repo()
            .committer()
            .transpose()?
            .context("Need committer to be configured when creating a new commit")?
            .into();
        let author = self
            .repo()
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
            extra_headers: (&Headers::from_config(&self.repo().config_snapshot())).into(),
        };

        Ok(but_core::CommitOwned {
            id: gix::ObjectId::null(kind),
            inner: obj,
        })
    }
}
