use crate::{branch_trees::checkout_branch_trees, r#virtual as vbranch};
use anyhow::{anyhow, bail, Context, Result};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch::{self, dedup};
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_commit::{commit_ext::CommitExt, commit_headers::HasCommitHeaders};
use gitbutler_error::error::Marker;
use gitbutler_oplog::SnapshotExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{
    rebase::{cherry_rebase_group, gitbutler_merge_commits},
    LogUntil, RepoActionsExt, RepositoryExt,
};
use gitbutler_stack::{BranchOwnershipClaims, Stack, StackId};
use gitbutler_stack_api::StackExt;
use gitbutler_time::time::now_since_unix_epoch_ms;
use tracing::instrument;

use super::BranchManager;
use crate::{
    conflicts::RepoConflictsExt, hunk::VirtualBranchHunk, integration::update_workspace_commit,
    VirtualBranchesExt,
};

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
            .repository()
            .find_commit(default_target.sha)
            .context("failed to find default target commit")?;

        let tree = commit
            .tree()
            .context("failed to find default target commit tree")?;

        let mut all_virtual_branches = vb_state
            .list_branches_in_workspace()
            .context("failed to read virtual branches")?;

        let name = dedup(
            &all_virtual_branches
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            create
                .name
                .as_ref()
                .unwrap_or(&"Virtual branch".to_string()),
        );

        _ = self
            .ctx
            .project()
            .snapshot_branch_creation(name.clone(), perm);

        all_virtual_branches.sort_by_key(|branch| branch.order);

        let order = create.order.unwrap_or(vb_state.next_order_index()?);

        let selected_for_changes = if let Some(selected_for_changes) = create.selected_for_changes {
            if selected_for_changes {
                for mut other_branch in vb_state
                    .list_branches_in_workspace()
                    .context("failed to read virtual branches")?
                {
                    other_branch.selected_for_changes = None;
                    vb_state.set_branch(other_branch.clone())?;
                }
                Some(now_since_unix_epoch_ms())
            } else {
                None
            }
        } else {
            (!all_virtual_branches
                .iter()
                .any(|b| b.selected_for_changes.is_some()))
            .then_some(now_since_unix_epoch_ms())
        };

        // make space for the new branch
        for (i, branch) in all_virtual_branches.iter().enumerate() {
            let mut branch = branch.clone();
            let new_order = if i < order { i } else { i + 1 };
            if branch.order != new_order {
                branch.order = new_order;
                vb_state.set_branch(branch.clone())?;
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
        );

        if let Some(ownership) = &create.ownership {
            vbranch::set_ownership(&vb_state, &mut branch, ownership)
                .context("failed to set ownership")?;
        }

        vb_state.set_branch(branch.clone())?;
        self.ctx.add_branch_reference(&branch)?;

        Ok(branch)
    }

    pub fn create_virtual_branch_from_branch(
        &self,
        target: &Refname,
        upstream_branch: Option<RemoteRefname>,
        perm: &mut WorktreeWritePermission,
    ) -> Result<StackId> {
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

        let _ = self
            .ctx
            .project()
            .snapshot_branch_creation(branch_name.clone(), perm);

        let vb_state = self.ctx.project().virtual_branches();

        let default_target = vb_state.get_default_target()?;

        if let Refname::Remote(remote_upstream) = target {
            if default_target.branch == *remote_upstream {
                bail!("cannot create a branch from default target")
            }
        }

        let repo = self.ctx.repository();
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

        let virtual_branches = vb_state
            .list_branches_in_workspace()
            .context("failed to read virtual branches")?
            .into_iter()
            .collect::<Vec<Stack>>();

        let order = vb_state.next_order_index()?;

        let selected_for_changes = (!virtual_branches
            .iter()
            .any(|b| b.selected_for_changes.is_some()))
        .then_some(now_since_unix_epoch_ms());

        // add file ownership based off the diff
        let target_commit = repo.find_commit(default_target.sha)?;
        let merge_base_oid = repo.merge_base(target_commit.id(), head_commit.id())?;
        let merge_base_tree = repo.find_commit(merge_base_oid)?.tree()?;

        // do a diff between the head of this branch and the target base
        let diff =
            gitbutler_diff::trees(self.ctx.repository(), &merge_base_tree, &head_commit_tree)?;

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

        let mut branch = if let Ok(Some(mut branch)) =
            vb_state.find_by_source_refname_where_not_in_workspace(target)
        {
            branch.upstream_head = upstream_branch.is_some().then_some(head_commit.id());
            branch.upstream = upstream_branch;
            branch.ownership = ownership;
            branch.order = order;
            branch.selected_for_changes = selected_for_changes;
            branch.allow_rebasing = self.ctx.project().ok_with_force_push.into();
            branch.in_workspace = true;

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
            )
        };

        branch.set_stack_head(self.ctx, head_commit.id(), Some(head_commit_tree.id()))?;
        self.ctx.add_branch_reference(&branch)?;

        match self.apply_branch(branch.id, perm) {
            Ok(_) => Ok(branch.id),
            Err(err)
                if err
                    .downcast_ref()
                    .map_or(false, |marker: &Marker| *marker == Marker::ProjectConflict) =>
            {
                // if branch conflicts with the workspace, it's ok. keep it unapplied
                Ok(branch.id)
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
        branch_id: StackId,
        perm: &mut WorktreeWritePermission,
    ) -> Result<String> {
        self.ctx.assure_resolved()?;
        self.ctx.assure_unconflicted()?;
        let repo = self.ctx.repository();

        let vb_state = self.ctx.project().virtual_branches();
        let default_target = vb_state.get_default_target()?;

        let mut branch = vb_state.get_branch_in_workspace(branch_id)?;

        // calculate the merge base and make sure it's the same as the target commit
        // if not, we need to merge or rebase the branch to get it up to date

        let merge_base = repo
            .merge_base(default_target.sha, branch.head())
            .context(format!(
                "failed to find merge base between {} and {}",
                default_target.sha,
                branch.head()
            ))?;

        // Branch is out of date, merge or rebase it
        let merge_base_tree = repo
            .find_commit(merge_base)
            .context(format!("failed to find merge base commit {}", merge_base))?
            .tree()
            .context("failed to find merge base tree")?;

        let branch_tree = repo
            .find_tree(branch.tree)
            .context("failed to find branch tree")?;

        // We don't support having two branches applied that conflict with each other
        {
            let uncommited_changes_tree = repo.create_wd_tree()?;
            let branch_merged_with_other_applied_branches = repo
                .merge_trees(
                    &merge_base_tree,
                    &branch_tree,
                    &uncommited_changes_tree,
                    None,
                )
                .context("failed to merge trees")?;

            if branch_merged_with_other_applied_branches.has_conflicts() {
                for branch in vb_state
                    .list_branches_in_workspace()?
                    .iter()
                    .filter(|branch| branch.id != branch_id)
                {
                    self.save_and_unapply(branch.id, perm)?;
                }
            }
        }

        // Do we need to rebase the branch on top of the default target?
        if merge_base != default_target.sha {
            let new_head = if branch.allow_rebasing {
                let commits_to_rebase = repo.l(branch.head(), LogUntil::Commit(merge_base))?;

                let head_oid = cherry_rebase_group(repo, default_target.sha, &commits_to_rebase)?;

                repo.find_commit(head_oid)?
            } else {
                gitbutler_merge_commits(
                    repo,
                    repo.find_commit(branch.head())?,
                    repo.find_commit(default_target.sha)?,
                    &branch.name,
                    default_target.branch.branch(),
                )?
            };

            branch.set_stack_head(
                self.ctx,
                new_head.id(),
                Some(repo.find_real_tree(&new_head, Default::default())?.id()),
            )?;
        }

        // apply the branch
        vb_state.set_branch(branch.clone())?;

        vbranch::ensure_selected_for_changes(&vb_state)
            .context("failed to ensure selected for changes")?;

        {
            if let Some(wip_commit_to_unapply) = &branch.not_in_workspace_wip_change_id {
                let potential_wip_commit = repo.find_commit(branch.head())?;

                // Don't try to undo commit if its conflicted
                if !potential_wip_commit.is_conflicted() {
                    if let Some(headers) = potential_wip_commit.gitbutler_headers() {
                        if headers.change_id == wip_commit_to_unapply.clone() {
                            branch = crate::undo_commit::undo_commit(
                                self.ctx,
                                branch.id,
                                branch.head(),
                            )?;
                        }
                    }

                    branch.not_in_workspace_wip_change_id = None;

                    vb_state.set_branch(branch.clone())?;
                }
            }
        }

        // Now that we've added a branch to the workspace, lets merge together all the trees
        checkout_branch_trees(self.ctx, perm)?;

        update_workspace_commit(&vb_state, self.ctx)?;

        Ok(branch.name)
    }
}
