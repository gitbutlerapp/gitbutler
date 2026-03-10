//! Provides some slightly higher level tools to help with manipulating commits, in preparation for use in the editor.

use anyhow::{Context, Result, bail};
use but_core::RefMetadata;
use but_core::commit::SignCommit;
use gix::prelude::ObjectIdExt;

use crate::{
    commit::{DateMode, create},
    graph_rebase::{
        Editor, Pick, Selector, Step, ToCommitSelector, ToReferenceSelector,
        util::collect_ordered_parents,
    },
};

impl<M: RefMetadata> Editor<'_, '_, M> {
    /// Returns a reference to the in-memory repository.
    pub fn repo(&self) -> &gix::Repository {
        &self.repo
    }

    /// Finds a commit from inside the editor's in memory repository.
    pub fn find_commit(&self, id: gix::ObjectId) -> Result<but_core::CommitOwned> {
        but_core::Commit::from_id(id.attach(&self.repo)).map(|c| c.detach())
    }

    /// Finds a commit that is selectable in the editor graph and is
    /// found in the editor's repo.
    ///
    /// Returns the normalized selector and the found commit.
    pub fn find_selectable_commit(
        &self,
        selector: impl ToCommitSelector,
    ) -> Result<(Selector, but_core::CommitOwned)> {
        let selector = self
            .history
            .normalize_selector(selector.to_commit_selector(self)?)?;
        let Step::Pick(Pick { id, .. }) = &self.graph[selector.id] else {
            bail!("BUG: Expected pick step from commit selector. This should never happen");
        };
        Ok((selector, self.find_commit(*id)?))
    }

    /// Finds the first pick parent of a reference
    pub fn find_reference_target(
        &self,
        selector: impl ToReferenceSelector,
    ) -> Result<(Selector, but_core::CommitOwned)> {
        let selector = self
            .history
            .normalize_selector(selector.to_reference_selector(self)?)?;

        let parents = collect_ordered_parents(&self.graph, selector.id);
        let first_parent = parents
            .first()
            .context("Failed to find a parent for selected reference in the step graph.")?;

        let Step::Pick(pick) = &self.graph[*first_parent] else {
            bail!("BUG: collect_ordered_parents provided a non-pick return value");
        };

        Ok((
            Selector {
                id: *first_parent,
                revision: self.history.current_revision(),
            },
            self.find_commit(pick.id)?,
        ))
    }

    /// Writes a commit with correct signing to the in memory repository,
    /// without updating the history log.
    pub fn new_commit_untracked(
        &self,
        commit: but_core::CommitOwned,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        create(
            &self.repo,
            commit.inner,
            date_mode,
            SignCommit::LegacyIfSignCommitsEnabled,
        )
    }

    /// Writes a commit with correct signing to the in memory repository.
    pub fn new_commit(
        &mut self,
        commit: but_core::CommitOwned,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        let commit_id = commit.id;
        let new_id = self.new_commit_untracked(commit, date_mode)?;
        self.history.update_mapping(commit_id, new_id);
        Ok(new_id)
    }

    /// Creates a commit with only the signature and author set correctly.
    ///
    /// The ID of the commit is all zeros & the commit hasn't been written into any ODB
    pub fn empty_commit(&self) -> Result<but_core::CommitOwned> {
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

        Ok(but_core::CommitOwned {
            id: gix::ObjectId::null(kind),
            inner: obj,
        })
    }
}
