use anyhow::{Context as _, Result};
use but_core::RepositoryExt;
use gitbutler_commit::commit_ext::CommitExt;
use itertools::Itertools;

type MergeBaseCommitGraph<'repo, 'cache> = gix::revwalk::Graph<
    'repo,
    'cache,
    gix::revision::plumbing::graph::Commit<gix::revision::plumbing::merge_base::Flags>,
>;

pub(crate) struct IsCommitIntegrated<'repo, 'cache, 'graph> {
    gix_repo: &'repo gix::Repository,
    graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    target_commit_id: gix::ObjectId,
    upstream_tree_id: gix::ObjectId,
    upstream_commits: Vec<gix::ObjectId>,
    upstream_change_ids: Vec<String>,
}

impl<'repo, 'cache, 'graph> IsCommitIntegrated<'repo, 'cache, 'graph> {
    pub(crate) fn new_with_target(
        target_ref_name: &gix::refs::FullNameRef,
        target_base_oid: gix::ObjectId,
        gix_repo: &'repo gix::Repository,
        graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    ) -> anyhow::Result<Self> {
        let remote_head = gix_repo
            .find_reference(target_ref_name)?
            .peel_to_commit()?
            .id;
        let upstream_tree_id = gix_repo.find_commit(remote_head)?.tree_id()?.detach();
        let upstream_commits = commit_ids_until(gix_repo, remote_head, target_base_oid)?;
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|commit_id| {
                gix_repo
                    .find_commit(*commit_id)
                    .ok()
                    .and_then(|c| c.change_id())
                    .map(|c| c.to_string())
            })
            .sorted()
            .collect();
        let upstream_commits = upstream_commits.into_iter().sorted().collect();
        Ok(Self {
            gix_repo,
            graph,
            target_commit_id: target_base_oid,
            upstream_tree_id,
            upstream_commits,
            upstream_change_ids,
        })
    }
}

impl IsCommitIntegrated<'_, '_, '_> {
    pub(crate) fn is_integrated(&mut self, commit_id: gix::ObjectId) -> Result<bool> {
        if self.target_commit_id == commit_id {
            // could not be integrated if heads are the same.
            return Ok(false);
        }

        if self.upstream_commits.is_empty() {
            // could not be integrated - there is nothing new upstream.
            return Ok(false);
        }

        let gix_commit = self.gix_repo.find_commit(commit_id)?;

        if let Some(change_id) = gix_commit.change_id()
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
            self.gix_repo
                .merge_base_with_graph(self.target_commit_id, commit_id, self.graph)?;
        if merge_base_id == commit_id {
            // if merge branch is the same as branch head and there are upstream commits
            // then it's integrated
            return Ok(true);
        }

        let merge_base_tree_id = self.gix_repo.find_commit(merge_base_id)?.tree_id()?;
        if merge_base_tree_id == self.upstream_tree_id {
            // if merge base is the same as upstream tree, then it's integrated
            return Ok(true);
        }

        // try to merge our tree into the upstream tree
        let (merge_options, conflict_kind) = self.gix_repo.merge_options_no_rewrites_fail_fast()?;
        let mut merge_output = self
            .gix_repo
            .merge_trees(
                merge_base_tree_id,
                gix_commit.tree_id()?,
                self.upstream_tree_id,
                Default::default(),
                merge_options,
            )
            .context("failed to merge trees")?;

        if merge_output.has_unresolved_conflicts(conflict_kind) {
            return Ok(false);
        }

        let merge_tree_id = merge_output.tree.write()?.detach();

        // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
        // then the vbranch is fully merged
        Ok(merge_tree_id == self.upstream_tree_id)
    }
}

fn commit_ids_until(
    repo: &gix::Repository,
    from: gix::ObjectId,
    stop_before: gix::ObjectId,
) -> Result<Vec<gix::ObjectId>> {
    use gix::prelude::ObjectIdExt as _;

    from.attach(repo)
        .ancestors()
        .with_hidden(Some(stop_before))
        .all()?
        .map(|info| Ok(info?.id))
        .collect()
}
