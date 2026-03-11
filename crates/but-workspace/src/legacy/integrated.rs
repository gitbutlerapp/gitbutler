//! ### Detecting if a commit is integrated
//!
//! This code is a fork of the [`gitbutler_branch_actions::virtual::IsCommitIntegrated`]

use anyhow::{Context as _, Result};
use but_core::{RepositoryExt, commit::Headers};
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_stack::Target;
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
        target: &Target,
        graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    ) -> anyhow::Result<Self> {
        let remote_head = repo
            .find_reference(&target.branch.to_string())?
            .peel_to_commit()?;
        let upstream_tree_id = remote_head.tree_id()?.detach();
        let upstream_commits = remote_head
            .id()
            .ancestors()
            .with_hidden(Some(target.sha))
            .all()?
            .filter_map(Result::ok)
            .map(|info| info.id)
            .collect_vec();
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|commit_id| {
                repo.find_commit(*commit_id)
                    .ok()
                    .and_then(|c| c.change_id())
                    .map(|cid| cid.to_string())
            })
            .sorted()
            .collect();
        Ok(Self {
            repo,
            graph,
            target_commit_id: target.sha,
            upstream_tree_id,
            upstream_commits,
            upstream_change_ids,
        })
    }

    pub(crate) fn is_integrated(&mut self, commit_id: gix::ObjectId) -> Result<bool> {
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

        let gix_commit = self.repo.find_commit(commit_id)?;
        let gix_commit = gix_commit.decode()?;

        if let Some(change_id) = Headers::try_from_commit_headers(|| gix_commit.extra_headers())
            .and_then(|hdr| hdr.change_id)
            && self
                .upstream_change_ids
                .binary_search(&change_id.to_string())
                .is_ok()
        {
            return Ok(true);
        }

        if self.upstream_commits.binary_search(&commit_id).is_ok() {
            return Ok(true);
        }

        let merge_base_id =
            self.repo
                .merge_base_with_graph(self.target_commit_id, commit_id, self.graph)?;
        if merge_base_id == commit_id {
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
                gix_commit.tree(),
                self.upstream_tree_id,
                Default::default(),
                merge_options,
            )
            .context("failed to merge trees")?;

        if merge_output.has_unresolved_conflicts(conflict_kind) {
            return Ok(false);
        }

        let merge_tree_id = merge_output.tree.write()?.detach();

        let parent_tree = gix_commit
            .parents()
            .next()
            .map(|parent_id| -> Result<gix::ObjectId> {
                let parent = self.repo.find_commit(parent_id)?;
                Ok(parent.tree_id()?.detach())
            })
            .transpose()?;
        if let Some(parent_tree) = parent_tree {
            // if the commit tree is the same as its the parent tree, it must be an empty commit, so dont classify it as integrated
            if gix_commit.tree() == parent_tree {
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
