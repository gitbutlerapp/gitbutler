use std::{collections::HashMap, path::PathBuf, vec};

use crate::{VirtualBranchesExt, hunk::VirtualBranchHunk, status::get_applied_status_cached};
use anyhow::{Context as _, Result, anyhow, bail};
use but_core::RepositoryExt;
use but_ctx::Context;
use but_oxidize::{ObjectIdExt, OidExt, git2_to_gix_object_id, gix_to_git2_oid};
use but_rebase::RebaseStep;
use but_workspace::legacy::stack_ext::StackExt;
use gitbutler_branch::{BranchUpdateRequest, dedup};
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_diff::GitHunk;
use gitbutler_project::AUTO_TRACK_LIMIT_BYTES;
use gitbutler_reference::{Refname, RemoteRefname, normalize_branch_name};
use gitbutler_repo::{
    RepositoryExt as _,
    logging::{LogUntil, RepositoryExt as _},
};
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{
    BranchOwnershipClaims, Stack, StackId, Target, VirtualBranchesHandle, reconcile_claims,
};
use gitbutler_time::time::now_since_unix_epoch_ms;
use itertools::Itertools;
use serde::Serialize;

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    /// The name of the remote to which the branches were pushed.
    pub remote: String,
    /// The list of pushed branches and their corresponding remote refnames.
    pub branch_to_remote: Vec<(String, Refname)>,
}

fn find_base_tree<'a>(
    repo: &'a git2::Repository,
    branch_commit: &'a git2::Commit<'a>,
    target_commit: &'a git2::Commit<'a>,
) -> Result<git2::Tree<'a>> {
    // find merge base between target_commit and branch_commit
    let merge_base = repo
        .merge_base(target_commit.id(), branch_commit.id())
        .context("failed to find merge base")?;
    // turn oid into a commit
    let merge_base_commit = repo
        .find_commit(merge_base)
        .context("failed to find merge base commit")?;
    let base_tree = merge_base_commit
        .tree()
        .context("failed to get base tree object")?;
    Ok(base_tree)
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
    let vb_state = ctx.legacy_project.virtual_branches();
    let mut stack = vb_state.get_stack_in_workspace(update.id.context("BUG(opt-stack-id)")?)?;

    if let Some(ownership) = update.ownership.clone() {
        let claim = ownership.into();
        set_ownership(&vb_state, &mut stack, &claim).context("failed to set ownership")?;
    }

    if let Some(name) = &update.name {
        let all_virtual_branches = vb_state
            .list_stacks_in_workspace()
            .context("failed to read virtual branches")?;

        ctx.delete_branch_reference(&stack)?;

        stack.name = dedup(
            &all_virtual_branches
                .iter()
                .filter(|b| Some(b.id) != update.id)
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            name,
        );

        ctx.add_branch_reference(&stack)?;
    };

    if let Some(updated_upstream) = &update.upstream {
        let default_target = vb_state.get_default_target()?;
        let upstream_remote = match default_target.push_remote_name {
            Some(remote) => remote.clone(),
            None => default_target.branch.remote().to_owned(),
        };

        let remote_branch = format!(
            "refs/remotes/{}/{}",
            upstream_remote,
            normalize_branch_name(updated_upstream)?
        )
        .parse::<RemoteRefname>()
        .unwrap();
        stack.upstream = Some(remote_branch);
    };

    if let Some(notes) = update.notes.clone() {
        stack.notes = notes;
    };

    if let Some(order) = update.order {
        stack.order = order;
    };

    if let Some(selected_for_changes) = update.selected_for_changes {
        stack.selected_for_changes = if selected_for_changes {
            for mut other_branch in vb_state
                .list_stacks_in_workspace()
                .context("failed to read virtual branches")?
                .into_iter()
                .filter(|b| b.id != stack.id)
            {
                other_branch.selected_for_changes = None;
                vb_state.set_stack(other_branch.clone())?;
            }
            Some(now_since_unix_epoch_ms())
        } else {
            None
        };
    };

    if let Some(allow_rebasing) = update.allow_rebasing {
        stack.allow_rebasing = allow_rebasing;
    };

    vb_state.set_stack(stack.clone())?;
    Ok(stack)
}

pub(crate) fn ensure_selected_for_changes(vb_state: &VirtualBranchesHandle) -> Result<()> {
    let mut stacks = vb_state
        .list_stacks_in_workspace()
        .context("failed to list branches")?;

    if stacks.is_empty() {
        println!("no applied branches");
        return Ok(());
    }

    if stacks.iter().any(|b| b.selected_for_changes.is_some()) {
        println!("some branches already selected for changes");
        return Ok(());
    }

    stacks.sort_by_key(|branch| branch.order);

    stacks[0].selected_for_changes = Some(now_since_unix_epoch_ms());
    vb_state.set_stack(stacks[0].clone())?;
    Ok(())
}

pub(crate) fn set_ownership(
    vb_state: &VirtualBranchesHandle,
    target_branch: &mut Stack,
    ownership: &BranchOwnershipClaims,
) -> Result<()> {
    if target_branch.ownership.eq(ownership) {
        // nothing to update
        return Ok(());
    }

    let stacks = vb_state
        .list_stacks_in_workspace()
        .context("failed to read virtual branches")?;

    let mut claim_outcomes = reconcile_claims(stacks, target_branch, &ownership.claims)?;
    for claim_outcome in &mut claim_outcomes {
        if !claim_outcome.removed_claims.is_empty() {
            vb_state
                .set_stack(claim_outcome.updated_branch.clone())
                .context("failed to write ownership for branch".to_string())?;
        }
    }

    // Updates the claiming branch that was passed as mutable state with the new ownership claims
    // TODO: remove mutable reference to target_branch
    target_branch.ownership = ownership.clone();

    Ok(())
}

pub type BranchStatus = HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>;
pub type VirtualBranchHunksByPathMap = HashMap<PathBuf, Vec<VirtualBranchHunk>>;

pub fn commit(
    ctx: &Context,
    stack_id: StackId,
    message: &str,
    ownership: Option<&BranchOwnershipClaims>,
) -> Result<git2::Oid> {
    // get the files to commit
    let diffs = gitbutler_diff::workdir(
        &*ctx.git2_repo.get()?,
        but_workspace::legacy::remerged_workspace_commit_v2(ctx)?,
    )?;
    let statuses = get_applied_status_cached(ctx, None, &diffs)
        .context("failed to get status by branch")?
        .branches;

    let (ref mut branch, files) = statuses
        .into_iter()
        .find(|(stack, _)| stack.id == stack_id)
        .with_context(|| format!("stack {stack_id} not found"))?;

    let gix_repo = ctx.repo.get()?;

    let tree_oid = if let Some(ownership) = ownership {
        let files = files.into_iter().filter_map(|file| {
            let hunks = file
                .hunks
                .into_iter()
                .filter(|hunk| {
                    let hunk: GitHunk = hunk.clone().into();
                    ownership
                        .claims
                        .iter()
                        .find(|f| f.file_path.eq(&file.path))
                        .is_some_and(|f| {
                            f.hunks.iter().any(|h| {
                                h.start == hunk.new_start
                                    && h.end == hunk.new_start + hunk.new_lines
                            })
                        })
                })
                .collect::<Vec<_>>();
            if hunks.is_empty() {
                None
            } else {
                Some((file.path, hunks))
            }
        });
        gitbutler_diff::write::hunks_onto_commit(ctx, branch.head_oid(&gix_repo)?.to_git2(), files)?
    } else {
        let files = files
            .into_iter()
            .map(|file| (file.path, file.hunks))
            .collect::<Vec<(PathBuf, Vec<VirtualBranchHunk>)>>();
        gitbutler_diff::write::hunks_onto_commit(ctx, branch.head_oid(&gix_repo)?.to_git2(), files)?
    };

    let git_repo = &*ctx.git2_repo.get()?;
    let parent_commit = git_repo
        .find_commit(branch.head_oid(&gix_repo)?.to_git2())
        .context(format!(
            "failed to find commit {:?}",
            branch.head_oid(&gix_repo)
        ))?;
    let tree = git_repo
        .find_tree(tree_oid)
        .context(format!("failed to find tree {tree_oid:?}"))?;

    let commit_oid = ctx.commit(message, &tree, &[&parent_commit], None)?;

    let vb_state = ctx.legacy_project.virtual_branches();
    branch.set_stack_head(&vb_state, &gix_repo, commit_oid, Some(tree_oid))?;

    crate::integration::update_workspace_commit(&vb_state, ctx, false)
        .context("failed to update gitbutler workspace")?;

    Ok(commit_oid)
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

        let upstream_commits =
            git2_repo.log(remote_head.id(), LogUntil::Commit(target.sha), true)?;
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|commit| commit.change_id())
            .sorted()
            .collect();
        let upstream_commits = upstream_commits
            .iter()
            .map(|commit| commit.id())
            .sorted()
            .collect();
        Ok(Self {
            gix_repo,
            graph,
            target_commit_id: git2_to_gix_object_id(target.sha),
            upstream_tree_id: git2_to_gix_object_id(upstream_tree_id),
            upstream_commits,
            upstream_change_ids,
        })
    }
}

impl IsCommitIntegrated<'_, '_, '_> {
    pub(crate) fn is_integrated(&mut self, commit: &git2::Commit) -> Result<bool> {
        if self.target_commit_id == git2_to_gix_object_id(commit.id()) {
            // could not be integrated if heads are the same.
            return Ok(false);
        }

        if self.upstream_commits.is_empty() {
            // could not be integrated - there is nothing new upstream.
            return Ok(false);
        }

        if let Some(change_id) = commit.change_id()
            && self.upstream_change_ids.binary_search(&change_id).is_ok()
        {
            return Ok(true);
        }

        if self.upstream_commits.binary_search(&commit.id()).is_ok() {
            return Ok(true);
        }

        let merge_base_id = self.gix_repo.merge_base_with_graph(
            self.target_commit_id,
            git2_to_gix_object_id(commit.id()),
            self.graph,
        )?;
        if gix_to_git2_oid(merge_base_id).eq(&commit.id()) {
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

        // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
        // then the vbranch is fully merged
        Ok(merge_tree_id == self.upstream_tree_id)
    }
}

pub fn is_remote_branch_mergeable(ctx: &Context, branch_name: &RemoteRefname) -> Result<bool> {
    let vb_state = ctx.legacy_project.virtual_branches();

    let default_target = vb_state.get_default_target()?;
    let git2_repo = &*ctx.git2_repo.get()?;
    let target_commit = git2_repo
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let branch = git2_repo
        .maybe_find_branch_by_refname(&branch_name.into())?
        .ok_or(anyhow!("branch not found"))?;
    let branch_oid = branch.get().target().context("detached head")?;
    let branch_commit = git2_repo
        .find_commit(branch_oid)
        .context("failed to find branch commit")?;

    let base_tree = find_base_tree(git2_repo, &branch_commit, &target_commit)?;

    let wd_tree = git2_repo.create_wd_tree(AUTO_TRACK_LIMIT_BYTES)?;

    let branch_tree = branch_commit.tree().context("failed to find branch tree")?;
    let gix_repo_in_memory = ctx.clone_repo_for_merging()?.with_object_memory();
    let (merge_options_fail_fast, conflict_kind) =
        gix_repo_in_memory.merge_options_no_rewrites_fail_fast()?;
    let mergeable = !gix_repo_in_memory
        .merge_trees(
            git2_to_gix_object_id(base_tree.id()),
            git2_to_gix_object_id(branch_tree.id()),
            git2_to_gix_object_id(wd_tree.id()),
            Default::default(),
            merge_options_fail_fast,
        )
        .context("failed to merge trees")?
        .has_unresolved_conflicts(conflict_kind);

    Ok(mergeable)
}

// create and insert a blank commit (no tree change) either above or below a commit
// if offset is positive, insert below, if negative, insert above
// return map of the updated commit ids
pub(crate) fn insert_blank_commit(
    ctx: &Context,
    stack_id: StackId,
    commit_oid: git2::Oid,
    offset: i32,
    message: Option<&str>,
) -> Result<(gix::ObjectId, Vec<(gix::ObjectId, gix::ObjectId)>)> {
    let vb_state = ctx.legacy_project.virtual_branches();

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    // find the commit to offset from
    let git2_repo = &*ctx.git2_repo.get()?;
    let mut commit = git2_repo
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    if offset > 0 {
        commit = commit.parent(0).context("failed to find parent")?;
    }

    let repo = git2_repo;
    let message = message.unwrap_or_default();

    let commit_tree = repo.find_real_tree(&commit, Default::default()).unwrap();
    let blank_commit_oid =
        ctx.commit(message, &commit_tree, &[&commit], Some(Default::default()))?;

    let merge_base = stack.merge_base(ctx)?;
    let repo = ctx.repo.get()?;
    let steps = stack.as_rebase_steps(ctx, &repo)?;
    let mut updated_steps = vec![];
    for step in steps.iter() {
        updated_steps.push(step.clone());
        if let RebaseStep::Pick { commit_id, .. } = step
            && commit_id == &commit.id().to_gix()
        {
            updated_steps.push(RebaseStep::Pick {
                commit_id: blank_commit_oid.to_gix(),
                new_message: None,
            });
        }
    }
    // if the  commit is the merge_base, then put the blank commit at the beginning
    if commit.id().to_gix() == merge_base {
        updated_steps.insert(
            0,
            RebaseStep::Pick {
                commit_id: blank_commit_oid.to_gix(),
                new_message: None,
            },
        );
    }

    let mut rebase = but_rebase::Rebase::new(&repo, merge_base, None)?;
    rebase.steps(updated_steps)?;
    rebase.rebase_noops(false);
    let output = rebase.rebase()?;
    let commit_map = output
        .commit_mapping
        .into_iter()
        .map(|(_, old, new)| (old, new))
        .collect::<Vec<_>>();
    stack.set_heads_from_rebase_output(ctx, output.references)?;

    stack.set_stack_head(&vb_state, &repo, output.top_commit.to_git2(), None)?;

    crate::integration::update_workspace_commit(&vb_state, ctx, false)
        .context("failed to update gitbutler workspace")?;

    let blank_commit_id = commit_map
        .iter()
        .find_map(|(old, new)| {
            if *old == blank_commit_oid.to_gix() {
                Some(*new)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow!("failed to find the blank commit id after rebasing"))?;

    Ok((blank_commit_id, commit_map))
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
    let vb_state = ctx.legacy_project.virtual_branches();
    let default_target = vb_state.get_default_target()?;
    let gix_repo = ctx.repo.get()?;

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    let branch_commit_oids = ctx.git2_repo.get()?.l(
        stack.head_oid(&gix_repo)?.to_git2(),
        LogUntil::Commit(default_target.sha),
        false,
    )?;

    if !branch_commit_oids.contains(&commit_id) {
        bail!("commit {commit_id} not in the branch");
    }

    let pushed_commit_oids = stack.upstream_head.map_or_else(
        || Ok(vec![]),
        |upstream_head| {
            ctx.git2_repo
                .get()?
                .l(upstream_head, LogUntil::Commit(default_target.sha), false)
        },
    )?;

    if pushed_commit_oids.contains(&commit_id) && !stack.allow_rebasing {
        // updating the message of a pushed commit will cause a force push that is not allowed
        bail!("force push not allowed");
    }

    let mut steps = stack.as_rebase_steps(ctx, &gix_repo)?;
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
    let mut rebase = but_rebase::Rebase::new(&gix_repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    let output = rebase.rebase()?;

    let new_head = output.top_commit.to_git2();
    stack.set_stack_head(&vb_state, &gix_repo, new_head, None)?;
    stack.set_heads_from_rebase_output(ctx, output.references)?;

    crate::integration::update_workspace_commit(&vb_state, ctx, false)
        .context("failed to update gitbutler workspace")?;

    output
        .commit_mapping
        .iter()
        .find_map(|(_base, old, new)| (*old == commit_id.to_gix()).then_some(new.to_git2()))
        .ok_or(anyhow!(
            "Failed to find the updated commit id after rebasing"
        ))
}
