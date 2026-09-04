//! Provides some slightly higher level tools to help with manipulating commits, in preparation for use in the editor.

use anyhow::{Context, Result, bail};
use but_core::commit::SignCommit;
use but_core::{RefMetadata, commit::Headers};
use gix::prelude::ObjectIdExt;

use crate::{
    commit::{DateMode, create},
    graph_rebase::{CommitIndex, Editor, RefIndex},
};

impl<M: RefMetadata> Editor<'_, M> {
    /// Returns a reference to the in-memory repository.
    pub fn repo(&self) -> &gix::Repository {
        &self.repo
    }

    /// Set a merge-base override for checkout so that consumed worktree
    /// changes don't reappear as uncommitted after materialization.
    pub fn set_merge_base_override(&mut self, tree_id: gix::ObjectId) {
        for checkout in &mut self.checkouts {
            match checkout {
                super::Checkout::Head {
                    merge_base_override,
                    ..
                } => {
                    *merge_base_override = Some(tree_id);
                }
                super::Checkout::Worktree { .. } => {}
            }
        }
    }

    /// Like [`Self::set_merge_base_override()`], but for the checkout of the linked
    /// worktree named `worktree_name`, whose changes were consumed.
    ///
    /// Fails if that worktree has no checkout recorded in this editor - it is unknown,
    /// archived, or worktree tips weren't seeded into the graph - so callers can bail
    /// before mutating the step graph.
    pub fn set_worktree_merge_base_override(
        &mut self,
        worktree_name: &gix::bstr::BStr,
        tree_id: gix::ObjectId,
    ) -> Result<()> {
        for checkout in &mut self.checkouts {
            if let super::Checkout::Worktree {
                worktree_name: name,
                merge_base_override,
                ..
            } = checkout
                && name == worktree_name
            {
                *merge_base_override = Some(tree_id);
                return Ok(());
            }
        }
        bail!("Worktree {worktree_name} has no checkout recorded in the editor")
    }

    /// Finds a commit from inside the editor's in memory repository.
    pub fn find_commit(&self, id: gix::ObjectId) -> Result<but_core::CommitOwned> {
        but_core::Commit::from_id(id.attach(&self.repo)).map(|c| c.detach())
    }

    /// Load the full commit the commit at `commit` currently holds from the editor's
    /// repository — the payload-loading twin of [`Editor::spec_of`](crate::graph_rebase::Editor::spec_of).
    pub fn commit_of(&self, commit: CommitIndex) -> Result<but_core::CommitOwned> {
        let Some(id) = self.store.commit_id(commit) else {
            bail!("The addressed commit was removed");
        };
        self.find_commit(id)
    }

    /// Finds the first commit parent of a reference
    pub fn target_of(&self, reference: RefIndex) -> Result<(CommitIndex, but_core::CommitOwned)> {
        let first_parent = self
            .store
            .resolve_to_commit(reference)
            .context("Failed to find a parent for selected reference in the commit graph.")?;

        let Some(id) = self.store.commit_id(first_parent) else {
            bail!("BUG: resolve_to_commit returned a non-commit entry");
        };

        Ok((first_parent, self.find_commit(id)?))
    }

    /// Writes a commit with correct signing to the in memory repository.
    ///
    /// This does not update the commit mappings; a rewrite is only recorded
    /// once the commit is installed into the graph via [`Editor::replace_commit`] or
    /// a rebase.
    pub fn new_commit(
        &self,
        commit: but_core::CommitOwned,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        let change_id = commit.change_id();
        create(
            &self.repo,
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
            extra_headers: (&Headers::from_config(&self.repo.config_snapshot())).into(),
        };

        Ok(but_core::CommitOwned {
            id: gix::ObjectId::null(kind),
            inner: obj,
        })
    }

    /// Write an empty merge-commit object carrying `message` — no parents yet; the
    /// caller wires them. Authorship is kept (a merge synthesizes existing work).
    /// The date mode is `CommitterKeepAuthorKeep`, not the `Update` that
    /// [`Self::new_squashed_commit`] uses: the template here is a fresh
    /// [`Self::empty_commit`] whose committer time is already now, whereas a squash
    /// copies a stale existing commit and must refresh it.
    pub fn new_merge_commit(
        &self,
        message: impl Into<gix::bstr::BString>,
    ) -> Result<gix::ObjectId> {
        let mut commit = self.empty_commit()?;
        commit.message = message.into();
        self.new_commit(commit, DateMode::CommitterKeepAuthorKeep)
    }
}
