use anyhow::{Context as _, Result, anyhow, bail};
use but_core::RepositoryExt;
use but_ctx::Context;
use but_oxidize::{ObjectIdExt, OidExt};
use but_rebase::RebaseStep;
use but_workspace::legacy::stack_ext::StackExt;
use gitbutler_branch::BranchUpdateRequest;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_reference::Refname;
use gitbutler_repo::{
    RepositoryExt as _,
    logging::{LogUntil, RepositoryExt as _},
};
use gitbutler_stack::{Stack, StackId, Target};
use itertools::Itertools;
use serde::Serialize;

use crate::VirtualBranchesExt;

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    /// The name of the remote to which the branches were pushed.
    pub remote: String,
    /// The list of pushed branches and their corresponding remote refnames.
    pub branch_to_remote: Vec<(String, Refname)>,
    /// The list of branches with their before/after commit SHAs.
    /// Format: (branch_name, before_sha, after_sha)
    /// SHAs are stored as hex strings for serialization
    pub branch_sha_updates: Vec<(String, String, String)>,
}

impl From<but_workspace::ui::Author> for crate::author::Author {
    fn from(value: but_workspace::ui::Author) -> Self {
        crate::author::Author {
            name: value.name,
            email: value.email,
            gravatar_url: value.gravatar_url,
        }
    }
}

pub fn update_stack(ctx: &Context, update: &BranchUpdateRequest) -> Result<Stack> {
    let vb_state = ctx.virtual_branches();
    let mut stack = vb_state.get_stack_in_workspace(update.id.context("BUG(opt-stack-id)")?)?;

    if let Some(order) = update.order {
        stack.order = order;
    };

    vb_state.set_stack(stack.clone())?;
    Ok(stack)
}

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
    upstream_commits: Vec<git2::Oid>,
    upstream_change_ids: Vec<String>,
}

impl<'repo, 'cache, 'graph> IsCommitIntegrated<'repo, 'cache, 'graph> {
    pub(crate) fn new(
        ctx: &'repo Context,
        target: &Target,
        gix_repo: &'repo gix::Repository,
        graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    ) -> anyhow::Result<Self> {
        let git2_repo = &*ctx.git2_repo.get()?;
        let remote_branch = git2_repo
            .maybe_find_branch_by_refname(&target.branch.clone().into())?
            .ok_or(anyhow!("failed to get branch"))?;
        let remote_head = remote_branch.get().peel_to_commit()?;
        let upstream_tree_id = git2_repo.find_commit(remote_head.id())?.tree_id();

        let upstream_commits = git2_repo.log(remote_head.id(), LogUntil::Commit(target.sha), true)?;
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|commit| {
                gix_repo
                    .find_commit(commit.id().to_gix())
                    .ok()
                    .and_then(|c| c.change_id())
                    .map(|c| c.to_string())
            })
            .sorted()
            .collect();
        let upstream_commits = upstream_commits.iter().map(|commit| commit.id()).sorted().collect();
        Ok(Self {
            gix_repo,
            graph,
            target_commit_id: target.sha.to_gix(),
            upstream_tree_id: upstream_tree_id.to_gix(),
            upstream_commits,
            upstream_change_ids,
        })
    }
}

impl IsCommitIntegrated<'_, '_, '_> {
    pub(crate) fn is_integrated(&mut self, commit: &git2::Commit) -> Result<bool> {
        if self.target_commit_id == commit.id().to_gix() {
            // could not be integrated if heads are the same.
            return Ok(false);
        }

        if self.upstream_commits.is_empty() {
            // could not be integrated - there is nothing new upstream.
            return Ok(false);
        }

        let gix_commit = self.gix_repo.find_commit(commit.id().to_gix())?;

        if let Some(change_id) = gix_commit.change_id()
            && self.upstream_change_ids.binary_search(&change_id.to_string()).is_ok()
        {
            return Ok(true);
        }

        if self.upstream_commits.binary_search(&commit.id()).is_ok() {
            return Ok(true);
        }

        let merge_base_id =
            self.gix_repo
                .merge_base_with_graph(self.target_commit_id, commit.id().to_gix(), self.graph)?;
        if merge_base_id.to_git2().eq(&commit.id()) {
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
                commit.tree_id().to_gix(),
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

// changes a commit message for commit_oid, rebases everything above it, updates branch head if successful
pub(crate) fn update_commit_message(
    ctx: &Context,
    stack_id: StackId,
    commit_id: git2::Oid,
    message: &str,
) -> Result<git2::Oid> {
    if message.is_empty() {
        bail!("commit message can not be empty");
    }
    let vb_state = ctx.virtual_branches();
    let default_target = vb_state.get_default_target()?;

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    let branch_commit_oids = ctx.git2_repo.get()?.l(
        stack.head_oid(ctx)?.to_git2(),
        LogUntil::Commit(default_target.sha),
        false,
    )?;

    if !branch_commit_oids.contains(&commit_id) {
        bail!("commit {commit_id} not in the branch");
    }

    let mut steps = stack.as_rebase_steps(ctx)?;
    // Update the commit message
    for step in steps.iter_mut() {
        if let RebaseStep::Pick {
            commit_id: id,
            new_message,
        } = step
            && *id == commit_id.to_gix()
        {
            *new_message = Some(message.into());
        }
    }
    let merge_base = stack.merge_base(ctx)?;
    let output = {
        let repo = ctx.repo.get()?;
        let mut rebase = but_rebase::Rebase::new(&repo, Some(merge_base), None)?;
        rebase.rebase_noops(false);
        rebase.steps(steps)?;
        rebase.rebase()?
    };

    let new_head = output.top_commit.to_git2();
    stack.set_stack_head(&vb_state, &*ctx.repo.get()?, new_head)?;
    stack.set_heads_from_rebase_output(ctx, output.references)?;

    crate::integration::update_workspace_commit(&vb_state, ctx, false)
        .context("failed to update gitbutler workspace")?;

    output
        .commit_mapping
        .iter()
        .find_map(|(_base, old, new)| (*old == commit_id.to_gix()).then_some(new.to_git2()))
        .ok_or(anyhow!("Failed to find the updated commit id after rebasing"))
}
