//! Provides some slightly higher level tools to help with manipulating commits, in preparation for use in the editor.

use anyhow::{Context, Result, bail};
use but_core::commit::SignCommit;
use but_core::{RefMetadata, commit::Headers};
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

    /// Like [`Self::set_merge_base_override`], but for the checkout of the linked
    /// worktree named `worktree_name`, so changes consumed *from that worktree*
    /// cancel out during its checkout instead of reappearing as uncommitted changes.
    ///
    /// Errors when the editor has no checkout recorded for `worktree_name` - the
    /// worktree is unknown, archived, on a detached `HEAD`, or worktree tips weren't
    /// seeded into the graph (feature flag off). Callers should invoke this before
    /// mutating the graph so bad input fails with zero mutation.
    pub fn set_worktree_merge_base_override(
        &mut self,
        worktree_name: &gix::bstr::BStr,
        tree_id: gix::ObjectId,
    ) -> Result<()> {
        for checkout in &mut self.checkouts {
            match checkout {
                super::Checkout::Worktree {
                    worktree_name: name,
                    merge_base_override,
                    ..
                } if name == worktree_name => {
                    *merge_base_override = Some(tree_id);
                    return Ok(());
                }
                super::Checkout::Head { .. } | super::Checkout::Worktree { .. } => {}
            }
        }
        bail!(
            "No checkout is recorded for a linked worktree named {worktree_name} - \
             it is unknown, archived, on a detached HEAD, or worktree tips weren't seeded into the graph"
        );
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

        Ok((self.new_selector(*first_parent), self.find_commit(pick.id)?))
    }

    /// Writes a commit with correct signing to the in memory repository.
    ///
    /// This does not update the commit mappings; a rewrite is only recorded
    /// once the commit is installed into the graph via [`Editor::replace`] or
    /// a rebase.
    pub fn new_commit(
        &self,
        commit: but_core::CommitOwned,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        create(
            &self.repo,
            commit.inner,
            date_mode,
            SignCommit::IfSignCommitsEnabled,
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
}
