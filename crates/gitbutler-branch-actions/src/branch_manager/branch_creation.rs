use anyhow::{Context, Result, anyhow, bail};
use but_workspace::{
    branch::{
        OnWorkspaceMergeConflict,
        apply::{IntegrationMode, WorkspaceReferenceNaming},
        checkout::UncommitedWorktreeChanges,
    },
    stack_ext::StackExt,
};
use gitbutler_branch::{self, BranchCreateRequest, dedup};
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_commit::{commit_ext::CommitExt, commit_headers::HasCommitHeaders};
use gitbutler_error::error::Marker;
use gitbutler_oplog::SnapshotExt;
use gitbutler_oxidize::{GixRepositoryExt, ObjectIdExt, OidExt, RepoExt};
use gitbutler_project::{AUTO_TRACK_LIMIT_BYTES, access::WorktreeWritePermission};
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{RepositoryExt as _, rebase::gitbutler_merge_commits};
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{BranchOwnershipClaims, Stack, StackId};
use gitbutler_time::time::now_since_unix_epoch_ms;
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommited_changes_with_tree};
use serde::Serialize;
use tracing::instrument;

use super::BranchManager;
use crate::{
    VirtualBranchesExt, hunk::VirtualBranchHunk, integration::update_workspace_commit,
    r#virtual as vbranch,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchFromBranchOutcome {
    pub stack_id: StackId,
    pub unapplied_stacks: Vec<StackId>,
}

impl From<(StackId, Vec<StackId>)> for CreateBranchFromBranchOutcome {
    fn from((stack_id, unapplied_stacks): (StackId, Vec<StackId>)) -> Self {
        Self {
            stack_id,
            unapplied_stacks,
        }
    }
}

impl BranchManager<'_> {
    #[instrument(level = tracing::Level::DEBUG, skip(self, perm), err(Debug))]
    pub fn create_virtual_branch(
        &self,
        create: &BranchCreateRequest,
        perm: &mut WorktreeWritePermission,
    ) -> Result<Stack> {
        let vb_state = self.ctx.project().virtual_branches();
        let default_target = vb_state.get_default_target()?;

        let commit = self
            .ctx
            .repo()
            .find_commit(default_target.sha)
            .context("failed to find default target commit")?;

        let tree = commit
            .tree()
            .context("failed to find default target commit tree")?;

        let mut all_stacks = vb_state
            .list_stacks_in_workspace()
            .context("failed to read virtual branches")?;

        let name = dedup(
            &all_stacks
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            create.name.as_ref().unwrap_or(&"Lane".to_string()),
        );

        _ = self.ctx.snapshot_branch_creation(name.clone(), perm);

        all_stacks.sort_by_key(|branch| branch.order);

        let order = create.order.unwrap_or(vb_state.next_order_index()?);

        let selected_for_changes = if let Some(selected_for_changes) = create.selected_for_changes {
            if selected_for_changes {
                for mut other_branch in vb_state
                    .list_stacks_in_workspace()
                    .context("failed to read virtual branches")?
                {
                    other_branch.selected_for_changes = None;
                    vb_state.set_stack(other_branch.clone())?;
                }
                Some(now_since_unix_epoch_ms())
            } else {
                None
            }
        } else {
            (!all_stacks.iter().any(|b| b.selected_for_changes.is_some()))
                .then_some(now_since_unix_epoch_ms())
        };

        // make space for the new branch
        for (i, branch) in all_stacks.iter().enumerate() {
            let mut branch = branch.clone();
            let new_order = if i < order { i } else { i + 1 };
            if branch.order != new_order {
                branch.order = new_order;
                vb_state.set_stack(branch.clone())?;
            }
        }

        let mut branch = Stack::create(
            self.ctx,
            name.clone(),
            None,
            None,
            None,
            tree.id(),
            default_target.sha,
            order,
            selected_for_changes,
            self.ctx.project().ok_with_force_push.into(),
            false, // disallow duplicate branch names on creation
        )?;

        if let Some(ownership) = create.ownership.clone() {
            let claim = ownership.into();
            vbranch::set_ownership(&vb_state, &mut branch, &claim)
                .context("failed to set ownership")?;
        }

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
        perm: &mut WorktreeWritePermission,
    ) -> Result<(StackId, Vec<StackId>)> {
        // Assume that this is always about 'apply' and hijack the entire method.
        // That way we'd learn what's missing.
        if self.ctx.app_settings().feature_flags.apply3 {
            let (repo, mut meta, graph) = self.ctx.graph_and_meta_mut_and_repo(perm)?;
            let ws = graph.to_workspace()?;
            let target = target.to_string();
            let branch_to_apply = target.as_str().try_into()?;
            let mut out = but_workspace::branch::apply(
                branch_to_apply,
                &ws,
                &repo,
                &mut *meta,
                but_workspace::branch::apply::Options {
                    integration_mode: IntegrationMode::AlwaysMerge,
                    on_workspace_conflict:
                        OnWorkspaceMergeConflict::MaterializeAndReportConflictingStacks,
                    workspace_reference_naming: WorkspaceReferenceNaming::Default,
                    uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                    order: None,
                    new_stack_id: None,
                },
            )?;
            let ws = out.graph.to_workspace()?;
            let applied_branch_stack_id = ws
                .find_segment_and_stack_by_refname(out.applied_branches.pop().context("BUG: must mention the actually applied branch last")?.as_ref())
                .with_context(||
                    format!("BUG: Can't find the branch to apply in workspace, but the 'apply' function should have failed instead \n{out:?}")
                )?
                .0
                .id
                .context("BUG: newly applied stacks should always have a stack id")?;
            return Ok((applied_branch_stack_id, out.conflicting_stack_ids));
        }
        let old_cwd = (!self.ctx.app_settings().feature_flags.cv3)
            .then(|| self.ctx.repo().create_wd_tree(0).map(|tree| tree.id()))
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

        let branch_name = target
            .branch()
            .expect("always a branch reference")
            .to_string();

        let _ = self.ctx.snapshot_branch_creation(branch_name.clone(), perm);

        let vb_state = self.ctx.project().virtual_branches();

        let default_target = vb_state.get_default_target()?;

        if let Refname::Remote(remote_upstream) = target
            && default_target.branch == *remote_upstream
        {
            bail!("cannot create a branch from default target")
        }

        let repo = self.ctx.repo();
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
        let head_commit_tree = head_commit.tree().context("failed to find tree")?;

        let stacks = vb_state
            .list_stacks_in_workspace()
            .context("failed to read virtual branches")?
            .into_iter()
            .collect::<Vec<Stack>>();

        let order = vb_state.next_order_index()?;

        let selected_for_changes = (!stacks.iter().any(|b| b.selected_for_changes.is_some()))
            .then_some(now_since_unix_epoch_ms());

        // add file ownership based off the diff
        let target_commit = repo.find_commit(default_target.sha)?;
        let merge_base_oid = repo.merge_base(target_commit.id(), head_commit.id())?;
        let merge_base_tree = repo.find_commit(merge_base_oid)?.tree()?;

        // do a diff between the head of this branch and the target base
        let diff =
            gitbutler_diff::trees(self.ctx.repo(), &merge_base_tree, &head_commit_tree, true)?;

        // assign ownership to the branch
        let ownership = diff.iter().fold(
            BranchOwnershipClaims::default(),
            |mut ownership, (file_path, file)| {
                for hunk in &file.hunks {
                    ownership.put(
                        format!(
                            "{}:{}",
                            file_path.display(),
                            VirtualBranchHunk::gen_id(hunk.new_start, hunk.new_lines)
                        )
                        .parse()
                        .unwrap(),
                    );
                }
                ownership
            },
        );

        let mut branch = if let Some(mut branch) = vb_state
            .find_by_top_reference_name_where_not_in_workspace(&target.to_string())?
            .or(vb_state.find_by_source_refname_where_not_in_workspace(target)?)
        {
            branch.upstream_head = upstream_branch.is_some().then_some(head_commit.id());
            branch.upstream = upstream_branch; // Used as remote when listing commits.
            branch.ownership = ownership;
            branch.order = order;
            branch.selected_for_changes = selected_for_changes;
            branch.allow_rebasing = self.ctx.project().ok_with_force_push.into();
            branch.in_workspace = true;

            // This seems to ensure that there is at least one head.
            branch.initialize(self.ctx, true)?;
            vb_state.set_stack(branch.clone())?;
            branch
        } else {
            let upstream_head = upstream_branch.is_some().then_some(head_commit.id());
            Stack::create(
                self.ctx,
                branch_name.clone(),
                Some(target.clone()),
                upstream_branch,
                upstream_head,
                head_commit_tree.id(),
                head_commit.id(),
                order,
                selected_for_changes,
                self.ctx.project().ok_with_force_push.into(),
                true, // allow duplicate branch name if created from an existing branch
            )?
        };

        if let (Some(pr_number), Some(head)) = (pr_number, branch.heads(false).last()) {
            branch.set_pr_number(self.ctx, head, Some(pr_number))?;
        }
        branch.set_stack_head(
            &vb_state,
            &repo.to_gix()?,
            head_commit.id(),
            Some(head_commit_tree.id()),
        )?;
        self.ctx.add_branch_reference(&branch)?;

        match self.apply_branch(branch.id, perm, old_workspace, old_cwd) {
            Ok((_, unapplied_stacks)) => Ok((branch.id, unapplied_stacks)),
            Err(err)
                if err
                    .downcast_ref()
                    .is_some_and(|marker: &Marker| *marker == Marker::ProjectConflict) =>
            {
                // if branch conflicts with the workspace, it's ok. keep it unapplied
                Ok((branch.id, vec![]))
            }
            Err(err) => Err(err).context("failed to apply"),
        }
    }
}

/// Holding private methods associated to branch creation
impl BranchManager<'_> {
    #[instrument(level = tracing::Level::DEBUG, skip(self, perm), err(Debug))]
    fn apply_branch(
        &self,
        stack_id: StackId,
        perm: &mut WorktreeWritePermission,
        workspace_state: WorkspaceState,
        old_cwd: Option<git2::Oid>,
    ) -> Result<(String, Vec<StackId>)> {
        let repo = self.ctx.repo();

        let vb_state = self.ctx.project().virtual_branches();
        let default_target = vb_state.get_default_target()?;

        let mut stack = vb_state.get_stack_in_workspace(stack_id)?;

        // calculate the merge base and make sure it's the same as the target commit
        // if not, we need to merge or rebase the branch to get it up to date

        let gix_repo = repo.to_gix()?;
        let merge_base = repo
            .merge_base(default_target.sha, stack.head_oid(&gix_repo)?.to_git2())
            .context(format!(
                "failed to find merge base between {} and {}",
                default_target.sha,
                stack.head_oid(&gix_repo)?
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
            let gix_repo = self.ctx.gix_repo_for_merging_non_persisting()?;
            let merges_cleanly = gix_repo
                .merges_cleanly_compat(
                    merge_base_tree_id,
                    branch_tree_id,
                    uncommited_changes_tree_id,
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

        if merge_base != default_target.sha {
            let mut rebase_output = None;
            let new_head = if stack.allow_rebasing {
                let gix_repo = self.ctx.gix_repo()?;
                let steps = stack.as_rebase_steps(self.ctx, &gix_repo)?;
                let mut rebase =
                    but_rebase::Rebase::new(&gix_repo, default_target.sha.to_gix(), None)?;
                rebase.steps(steps)?;
                rebase.rebase_noops(true);
                let output = rebase.rebase()?;
                rebase_output = Some(output.clone());
                repo.find_commit(output.top_commit.to_git2())?
            } else {
                gitbutler_merge_commits(
                    repo,
                    repo.find_commit(stack.head_oid(&gix_repo)?.to_git2())?,
                    repo.find_commit(default_target.sha)?,
                    &stack.name,
                    default_target.branch.branch(),
                )?
            };

            stack.set_stack_head(
                &vb_state,
                &gix_repo,
                new_head.id(),
                Some(repo.find_real_tree(&new_head, Default::default())?.id()),
            )?;

            if let Some(output) = rebase_output {
                stack.set_heads_from_rebase_output(self.ctx, output.references)?;
            }
        }

        // apply the branch
        vb_state.set_stack(stack.clone())?;

        vbranch::ensure_selected_for_changes(&vb_state)
            .context("failed to ensure selected for changes")?;

        {
            if let Some(wip_commit_to_unapply) = &stack.not_in_workspace_wip_change_id {
                let potential_wip_commit =
                    repo.find_commit(stack.head_oid(&gix_repo)?.to_git2())?;

                // Don't try to undo commit if its conflicted
                if !potential_wip_commit.is_conflicted() {
                    if let Some(headers) = potential_wip_commit.gitbutler_headers()
                        && headers.change_id == wip_commit_to_unapply.clone()
                    {
                        stack = crate::undo_commit::undo_commit(
                            self.ctx,
                            stack.id,
                            stack.head_oid(&gix_repo)?.to_git2(),
                            perm,
                        )?;
                    }

                    stack.not_in_workspace_wip_change_id = None;

                    vb_state.set_stack(stack.clone())?;
                }
            }
        }

        // Permissions here might be wonky, just go with it though.
        let new_workspace = WorkspaceState::create(self.ctx, perm.read_permission())?;
        let res = update_uncommited_changes_with_tree(
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

        Ok((stack.name, unapplied_stacks))
    }
}
