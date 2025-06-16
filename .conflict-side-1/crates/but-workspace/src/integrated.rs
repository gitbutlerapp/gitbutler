//! ### Detecting if a commit is integrated
//!
//! This code is a fork of the [`gitbutler_branch_actions::virtual::IsCommitIntegrated`]

use crate::ref_info::function::remote_and_local_target_ids;
use anyhow::anyhow;
use anyhow::{Context, Result};
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_oxidize::{GixRepositoryExt, OidExt, git2_to_gix_object_id, gix_to_git2_oid};
use gitbutler_repo::{
    RepositoryExt as _,
    logging::{LogUntil, RepositoryExt},
};
use gitbutler_stack::Target;
use gix::prelude::ObjectIdExt;
use itertools::Itertools;

pub(crate) struct IsCommitIntegrated<'repo, 'cache, 'graph> {
    repo: &'repo gix::Repository,
    pub graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    target_commit_id: gix::ObjectId,
    upstream_tree_id: gix::ObjectId,
    upstream_commits: Vec<gix::ObjectId>,
    upstream_change_ids: Vec<String>,
}

impl<'repo, 'cache, 'graph> IsCommitIntegrated<'repo, 'cache, 'graph> {
    /// **IMPORTANT**: `repo` must use in-memory objects!
    /// See [`Self::new_with_gix()`] for the pure-gix version.
    pub(crate) fn new(
        repo: &'repo gix::Repository,
        git2_repo: &git2::Repository,
        target: &Target,
        graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    ) -> anyhow::Result<Self> {
        let remote_branch = git2_repo
            .maybe_find_branch_by_refname(&target.branch.clone().into())?
            .ok_or(anyhow!("failed to get branch"))?;
        let remote_head = remote_branch.get().peel_to_commit()?;
        let upstream_tree_id = git2_repo.find_commit(remote_head.id())?.tree_id();

        let upstream_commits =
            git2_repo.log(remote_head.id(), LogUntil::Commit(target.sha), true)?;
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|commit| commit.change_id())
            .sorted()
            .collect();
        let upstream_commits = upstream_commits
            .iter()
            .map(|commit| commit.id().to_gix())
            .sorted()
            .collect();
        Ok(Self {
            repo,
            graph,
            target_commit_id: git2_to_gix_object_id(target.sha),
            upstream_tree_id: git2_to_gix_object_id(upstream_tree_id),
            upstream_commits,
            upstream_change_ids,
        })
    }

    /// Like [new](Self::new), but avoids 'old' types in favor of more basic types from which everything else can be computed.
    ///
    /// `target_ref_name` is the *remote tracking branch* of the target branch. Without it, we can't check if anything is integrated.
    /// **IMPORTANT**: `repo` must use in-memory objects!
    pub(crate) fn new_with_gix(
        repo: &'repo gix::Repository,
        target_ref_name: &gix::refs::FullNameRef,
        graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    ) -> anyhow::Result<Self> {
        let (remote_target_id, local_target_id) =
            remote_and_local_target_ids(repo, target_ref_name)?
                .map(|(remote_id, local_id)| (Some(remote_id), Some(local_id)))
                .unwrap_or_default();
        let upstream_commits = remote_target_id
            .zip(local_target_id)
            .map(
                |(remote_target_id, local_target_id)| -> anyhow::Result<Vec<_>> {
                    Ok(remote_target_id
                        .attach(repo)
                        .ancestors()
                        .with_hidden(Some(local_target_id))
                        .all()?
                        .filter_map(Result::ok)
                        .map(|info| info.id)
                        .collect())
                },
            )
            .transpose()?
            .unwrap_or_default();
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|id| but_core::Commit::from_id(id.attach(repo)).ok())
            .filter_map(|commit| commit.headers().map(|hdr| hdr.change_id))
            .sorted()
            .collect();
        let upstream_tree_id = remote_target_id.and_then(|id| {
            id.attach(repo)
                .object()
                .ok()?
                .peel_to_commit()
                .ok()?
                .tree_id()
                .ok()
                .map(|id| id.detach())
        });
        Ok(Self {
            repo,
            graph,
            target_commit_id: remote_target_id.unwrap_or_else(|| repo.object_hash().null()),
            upstream_tree_id: upstream_tree_id.unwrap_or_else(|| repo.object_hash().null()),
            upstream_commits,
            upstream_change_ids,
        })
    }

    pub(crate) fn is_integrated(&mut self, commit: &git2::Commit<'_>) -> Result<bool> {
        if self.target_commit_id == git2_to_gix_object_id(commit.id()) {
            // could not be integrated if heads are the same.
            return Ok(false);
        }

        // TODO: this relies on knowing that we update the workspace, notice that something is
        //       integrated, and setting the archive flag (probably). Now it's easy to imagine
        //       somebody fetching and FF-merging the target branch, and we should still be able
        //       to detect that something was integrated.
        //       So this would have to be removed.
        if self.upstream_commits.is_empty() {
            // could not be integrated - there is nothing new upstream.
            return Ok(false);
        }

        if let Some(change_id) = commit.change_id() {
            if self.upstream_change_ids.binary_search(&change_id).is_ok() {
                return Ok(true);
            }
        }

        if self
            .upstream_commits
            .binary_search(&commit.id().to_gix())
            .is_ok()
        {
            return Ok(true);
        }

        let merge_base_id = self.repo.merge_base_with_graph(
            self.target_commit_id,
            git2_to_gix_object_id(commit.id()),
            self.graph,
        )?;
        if gix_to_git2_oid(merge_base_id).eq(&commit.id()) {
            // if merge branch is the same as branch head and there are upstream commits
            // then it's integrated
            return Ok(true);
        }

        let merge_base_tree_id = self.repo.find_commit(merge_base_id)?.tree_id()?;
        // TODO: why this this fail in one of our tests? Are there wrong assumptions in general,
        //       or is this us having picked the wrong upstream_tree_id? `upstream_tree_id` seems
        //       to be correct though, so it's the merge_base tree comparison that's not really
        //       what it is supposed to be (anymore, or maybe ever?).
        // if merge_base_tree_id == self.upstream_tree_id {
        //     // if merge base is the same as upstream tree, then it's integrated
        //     return Ok(true);
        // }

        // try to merge our tree into the upstream tree
        let (merge_options, conflict_kind) = self.repo.merge_options_no_rewrites_fail_fast()?;
        let mut merge_output = self
            .repo
            .merge_trees(
                merge_base_tree_id,
                git2_to_gix_object_id(commit.tree_id()),
                self.upstream_tree_id,
                Default::default(),
                merge_options,
            )
            .context("failed to merge trees")?;

        if merge_output.has_unresolved_conflicts(conflict_kind) {
            return Ok(false);
        }

        let merge_tree_id = merge_output.tree.write()?.detach();

        let parent_tree = commit.parents().next().map(|c| c.tree_id());
        if let Some(parent_tree) = parent_tree {
            // if the commit tree is the same as its the parent tree, it must be an empty commit, so dont classify it as integrated
            if commit.tree_id() == parent_tree {
                return Ok(false);
            }
        }

        // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
        // then the vbranch is fully merged
        Ok(merge_tree_id == self.upstream_tree_id)
    }

    /// The `gix` version of [`Self::is_integrated`].
    pub(crate) fn is_integrated_gix(&mut self, commit_id: gix::ObjectId) -> Result<bool> {
        if self.target_commit_id == commit_id {
            // could not be integrated if heads are the same.
            return Ok(false);
        }

        // TODO: this relies on knowing that we update the workspace, notice that something is
        //       integrated, and setting the archive flag (probably). Now it's easy to imagine
        //       somebody fetching and FF-merging the target branch, and we should still be able
        //       to detect that something was integrated.
        //       So this would have to be removed.
        if self.upstream_commits.is_empty() {
            // could not be integrated - there is nothing new upstream.
            return Ok(false);
        }

        let commit = but_core::Commit::from_id(commit_id.attach(self.repo))?;
        if let Some(change_id) = commit.headers().map(|hdr| hdr.change_id) {
            if self.upstream_change_ids.binary_search(&change_id).is_ok() {
                return Ok(true);
            }
        }

        if self.upstream_commits.binary_search(&commit_id).is_ok() {
            return Ok(true);
        }

        let merge_base_id =
            self.repo
                .merge_base_with_graph(self.target_commit_id, commit_id, self.graph)?;
        if merge_base_id.eq(&commit_id) {
            // if merge branch is the same as branch head and there are upstream commits
            // then it's integrated
            return Ok(true);
        }

        let merge_base_tree_id = self.repo.find_commit(merge_base_id)?.tree_id()?;
        // TODO: why this this fail in one of our tests? Are there wrong assumptions in general,
        //       or is this us having picked the wrong upstream_tree_id? `upstream_tree_id` seems
        //       to be correct though, so it's the merge_base tree comparison that's not really
        //       what it is supposed to be (anymore, or maybe ever?).
        // if merge_base_tree_id == self.upstream_tree_id {
        //     // if merge base is the same as upstream tree, then it's integrated
        //     return Ok(true);
        // }

        // try to merge our tree into the upstream tree
        let (merge_options, conflict_kind) = self.repo.merge_options_no_rewrites_fail_fast()?;
        let mut merge_output = self
            .repo
            .merge_trees(
                merge_base_tree_id,
                commit.tree,
                self.upstream_tree_id,
                Default::default(),
                merge_options,
            )
            .context("failed to merge trees")?;

        if merge_output.has_unresolved_conflicts(conflict_kind) {
            return Ok(false);
        }

        let merge_tree_id = merge_output.tree.write()?.detach();

        let first_parent_tree_id = commit
            .parents
            .first()
            .and_then(|parent_id| self.repo.find_commit(*parent_id).ok()?.tree_id().ok());
        if let Some(first_parent_tree_id) = first_parent_tree_id {
            // if the commit tree is the same as it's the parent tree, it must be an empty commit, so don't classify it as integrated
            if commit.tree == first_parent_tree_id {
                return Ok(false);
            }
        }

        // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
        // then the vbranch is fully merged
        Ok(merge_tree_id == self.upstream_tree_id)
    }
}

pub(crate) type MergeBaseCommitGraph<'repo, 'cache> = gix::revwalk::Graph<
    'repo,
    'cache,
    gix::revision::plumbing::graph::Commit<gix::revision::plumbing::merge_base::Flags>,
>;
