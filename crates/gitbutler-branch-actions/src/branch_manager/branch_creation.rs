use anyhow::{Context as _, Result, anyhow, bail};
use but_core::{RepositoryExt, worktree::checkout::UncommitedWorktreeChanges};
use but_ctx::access::RepoExclusive;
use but_error::Marker;
use but_oxidize::{ObjectIdExt, OidExt, RepoExt};
use but_workspace::{
    branch::{
        OnWorkspaceMergeConflict,
        apply::{WorkspaceMerge, WorkspaceReferenceNaming},
    },
    legacy::stack_ext::StackExt,
};
use gitbutler_branch::{self, BranchCreateRequest, dedup};
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_oplog::SnapshotExt;
use gitbutler_project::AUTO_TRACK_LIMIT_BYTES;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::RepositoryExt as _;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{Stack, StackId};
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommitted_changes_with_tree};
use serde::Serialize;
use tracing::instrument;

use super::BranchManager;
use crate::{VirtualBranchesExt, integration::update_workspace_commit};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchFromBranchOutcome {
    pub stack_id: StackId,
    pub unapplied_stacks: Vec<StackId>,
    pub unapplied_stacks_short_names: Vec<String>,
}

impl From<(StackId, Vec<StackId>, Vec<String>)> for CreateBranchFromBranchOutcome {
    fn from(
        (stack_id, unapplied_stacks, unapplied_stacks_short_names): (
            StackId,
            Vec<StackId>,
            Vec<String>,
        ),
    ) -> Self {
        Self {
            stack_id,
            unapplied_stacks,
            unapplied_stacks_short_names,
        }
    }
}

impl BranchManager<'_> {
    #[instrument(level = "debug", skip(self, perm), err(Debug))]
    pub fn create_virtual_branch(
        &self,
        create: &BranchCreateRequest,
        perm: &mut RepoExclusive,
    ) -> Result<Stack> {
        let vb_state = self.ctx.virtual_branches();
        let default_target = vb_state.get_default_target()?;

        let mut all_stacks = vb_state
            .list_stacks_in_workspace()
            .context("failed to read virtual branches")?;

        let stack_names: Vec<String> = all_stacks.iter().map(|b| b.name()).collect();
        let stack_name_refs: Vec<&str> = stack_names.iter().map(|s| s.as_str()).collect();
        let name = dedup(
            &stack_name_refs,
            create.name.as_ref().unwrap_or(&"Lane".to_string()),
        );

        _ = self.ctx.snapshot_branch_creation(name.clone(), perm);

        all_stacks.sort_by_key(|branch| branch.order);

        let order = create.order.unwrap_or(vb_state.next_order_index()?);

        // make space for the new branch
        for (i, branch) in all_stacks.iter().enumerate() {
            let mut branch = branch.clone();
            let new_order = if i < order { i } else { i + 1 };
            if branch.order != new_order {
                branch.order = new_order;
                vb_state.set_stack(branch.clone())?;
            }
        }

        let branch = Stack::new_empty(self.ctx, name, default_target.sha, order)?;

        vb_state.set_stack(branch.clone())?;
        self.ctx.add_branch_reference(&branch)?;

        update_workspace_commit(&vb_state, self.ctx, false)?;

        Ok(branch)
    }

    pub fn create_virtual_branch_from_branch(
        &self,
        target: &Refname,
        upstream_branch: Option<RemoteRefname>,
        pr_number: Option<usize>,
        perm: &mut RepoExclusive,
    ) -> Result<(StackId, Vec<StackId>, Vec<String>)> {
        let branch_name = target
            .branch()
            .expect("always a branch reference")
            .to_string();
        let _ = self.ctx.snapshot_branch_creation(branch_name.clone(), perm);

        // Assume that this is always about 'apply' and hijack the entire method.
        // That way we'd learn what's missing.
        if self.ctx.settings.feature_flags.apply3 {
            #[expect(deprecated)] // should have no need for this in modern code anymore
            let (mut meta, ws) = self.ctx.workspace_and_meta_from_head(perm)?;
            let repo = self.ctx.repo.get()?;

            let target = target.to_string();
            let branch_to_apply = target.as_str().try_into()?;
            let mut out = but_workspace::branch::apply(
                branch_to_apply,
                &ws,
                &repo,
                &mut meta,
                but_workspace::branch::apply::Options {
                    workspace_merge: WorkspaceMerge::AlwaysMerge,
                    on_workspace_conflict:
                        OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
                    workspace_reference_naming: WorkspaceReferenceNaming::Default,
                    uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                    order: None,
                    new_stack_id: None,
                },
            )?;
            let ws = out.workspace.into_owned();
            let applied_branch_stack_id = ws
                .find_segment_and_stack_by_refname(out.applied_branches.pop().context("BUG: must mention the actually applied branch last")?.as_ref())
                .context(
                    "BUG: Can't find the branch to apply in workspace, but the 'apply' function should have failed instead"
                )?
                .0
                .id
                .context("BUG: newly applied stacks should always have a stack id")?;
            let conflicted_stack_short_names_for_display = ws
                .metadata
                .as_ref()
                .map(|md| {
                    md.stacks
                        .iter()
                        .filter_map(|s| {
                            out.conflicting_stack_ids
                                .contains(&s.id)
                                .then(|| s.ref_name().map(|rn| rn.shorten().to_string()))
                                .flatten()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            return Ok((
                applied_branch_stack_id,
                out.conflicting_stack_ids,
                conflicted_stack_short_names_for_display,
            ));
        }
        let old_cwd = (!self.ctx.settings.feature_flags.cv3)
            .then(|| {
                self.ctx
                    .git2_repo
                    .get()?
                    .create_wd_tree(0)
                    .map(|tree| tree.id())
            })
            .transpose()?;
        let old_workspace = WorkspaceState::create(self.ctx, perm.read_permission())?;
        // only set upstream if it's not the default target
        let upstream_branch = match upstream_branch {
            Some(upstream_branch) => Some(upstream_branch),
            None => {
                match target {
                    Refname::Other(_) | Refname::Virtual(_) => {
                        // we only support local or remote branches
                        bail!("branch {target} must be a local or remote branch");
                    }
                    Refname::Remote(remote) => Some(remote.clone()),
                    Refname::Local(local) => local.remote().cloned(),
                }
            }
        };

        let vb_state = self.ctx.virtual_branches();

        let default_target = vb_state.get_default_target()?;

        if let Refname::Remote(remote_upstream) = target
            && default_target.branch == *remote_upstream
        {
            bail!("cannot create a branch from default target")
        }

        let repo = self.ctx.git2_repo.get()?;
        let head_reference = repo
            .find_reference(&target.to_string())
            .map_err(|err| match err {
                err if err.code() == git2::ErrorCode::NotFound => {
                    anyhow!("branch {target} was not found")
                }
                err => err.into(),
            })?;
        let head_commit = head_reference
            .peel_to_commit()
            .context("failed to peel to commit")?;

        let order = vb_state.next_order_index()?;

        let mut branch = if let Some(mut branch) = vb_state
            .find_by_top_reference_name_where_not_in_workspace(&target.to_string())?
            .or(vb_state.find_by_source_refname_where_not_in_workspace(target)?)
            && branch.is_initialized()
        {
            branch.upstream = upstream_branch; // Used as remote when listing commits.
            branch.order = order;
            branch.in_workspace = true;

            vb_state.set_stack(branch.clone())?;
            branch
        } else {
            Stack::new_from_existing(
                self.ctx,
                branch_name.clone(),
                Some(target.clone()),
                upstream_branch,
                head_commit.id(),
                order,
            )?
        };

        if let (Some(pr_number), Some(head)) = (pr_number, branch.heads(false).last()) {
            branch.set_pr_number(self.ctx, head, Some(pr_number))?;
        }
        branch.set_stack_head(&vb_state, &(&*repo).to_gix_repo()?, head_commit.id())?;
        self.ctx.add_branch_reference(&branch)?;

        match self.apply_branch(branch.id, perm, old_workspace, old_cwd) {
            Ok((_, unapplied_stacks)) => Ok((branch.id, unapplied_stacks, vec![])),
            Err(err)
                if err
                    .downcast_ref()
                    .is_some_and(|marker: &Marker| *marker == Marker::ProjectConflict) =>
            {
                // if branch conflicts with the workspace, it's ok. keep it unapplied
                Ok((branch.id, vec![], vec![]))
            }
            Err(err) => Err(err).context("failed to apply"),
        }
    }
}

/// Holding private methods associated to branch creation
impl BranchManager<'_> {
    #[instrument(level = "debug", skip(self, perm), err(Debug))]
    fn apply_branch(
        &self,
        stack_id: StackId,
        perm: &mut RepoExclusive,
        workspace_state: WorkspaceState,
        old_cwd: Option<git2::Oid>,
    ) -> Result<(String, Vec<StackId>)> {
        let repo = &*self.ctx.git2_repo.get()?;

        let vb_state = self.ctx.virtual_branches();
        let default_target = vb_state.get_default_target()?;

        let mut stack = vb_state.get_stack_in_workspace(stack_id)?;

        // calculate the merge base and make sure it's the same as the target commit
        // if not, we need to merge or rebase the branch to get it up to date

        let merge_base = repo
            .merge_base(default_target.sha, stack.head_oid(self.ctx)?.to_git2())
            .context(format!(
                "failed to find merge base between {} and {}",
                default_target.sha,
                stack.head_oid(self.ctx)?
            ))?;

        // Branch is out of date, merge or rebase it
        let merge_base_tree_id = repo
            .find_commit(merge_base)
            .context(format!("failed to find merge base commit {merge_base}"))?
            .tree()
            .context("failed to find merge base tree")?
            .id();
        let branch_tree_id = stack.tree(self.ctx)?;

        let mut unapplied_stacks = vec![];

        // We don't support having two branches applied that conflict with each other
        {
            let uncommited_changes_tree_id = repo.create_wd_tree(AUTO_TRACK_LIMIT_BYTES)?.id();
            let gix_repo = self.ctx.clone_repo_for_merging_non_persisting()?;
            let merges_cleanly = gix_repo
                .merges_cleanly(
                    merge_base_tree_id.to_gix(),
                    branch_tree_id.to_gix(),
                    uncommited_changes_tree_id.to_gix(),
                )
                .context("failed to merge trees")?;

            if !merges_cleanly {
                for stack_to_unapply in vb_state
                    .list_stacks_in_workspace()?
                    .iter()
                    .filter(|branch| branch.id != stack_id)
                {
                    unapplied_stacks.push(stack_to_unapply.id);
                    let safe_checkout = old_cwd.is_none();
                    let res =
                        self.unapply(stack_to_unapply.id, perm, false, Vec::new(), safe_checkout);
                    if res.is_err() {
                        stack.in_workspace = false;
                        vb_state.set_stack(stack.clone())?;
                    }
                    res?;
                }
            }
        }

        // Do we need to rebase the branch on top of the default target?
        let gix_repo = self.ctx.repo.get()?;
        let has_change_id = gix_repo
            .find_commit(stack.head_oid(self.ctx)?)?
            .change_id()
            .is_some();
        // If the branch has no change ID for the head commit, we want to rebase it even if the base is the same
        // This way stacking functionality which relies on change IDs will work as expected
        if merge_base != default_target.sha || !has_change_id {
            let steps = stack.as_rebase_steps(self.ctx)?;
            let mut rebase = but_rebase::Rebase::new(&gix_repo, default_target.sha.to_gix(), None)?;
            rebase.steps(steps)?;
            rebase.rebase_noops(true);
            let output = rebase.rebase()?;
            let new_head = repo.find_commit(output.top_commit.to_git2())?;

            stack.set_stack_head(&vb_state, &gix_repo, new_head.id())?;

            stack.set_heads_from_rebase_output(self.ctx, output.references)?;
        }

        // apply the branch
        vb_state.set_stack(stack.clone())?;

        // Permissions here might be wonky, just go with it though.
        let new_workspace = WorkspaceState::create(self.ctx, perm.read_permission())?;
        let res = update_uncommitted_changes_with_tree(
            self.ctx,
            workspace_state,
            new_workspace,
            old_cwd,
            Some(true),
            perm,
        );
        if res.is_err() {
            stack.in_workspace = false;
            vb_state.set_stack(stack.clone())?;
        }
        res?;

        update_workspace_commit(&vb_state, self.ctx, false)?;

        Ok((stack.name(), unapplied_stacks))
    }
}
