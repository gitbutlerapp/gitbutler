use std::borrow::Cow;

use crate::r#virtual as vbranch;
use anyhow::{anyhow, bail, Context, Result};
use gitbutler_branch::{self, dedup, Branch, BranchCreateRequest, BranchId, BranchOwnershipClaims};
use gitbutler_commit::commit_headers::HasCommitHeaders;
use gitbutler_error::error::Marker;
use gitbutler_oplog::SnapshotExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{
    rebase::{cherry_rebase_group, gitbutler_merge_commits},
    LogUntil, RepoActionsExt, RepositoryExt,
};
use gitbutler_time::time::now_since_unix_epoch_ms;
use tracing::instrument;

use super::BranchManager;
use crate::{
    conflicts::{self, RepoConflictsExt},
    hunk::VirtualBranchHunk,
    integration::update_workspace_commit,
    VirtualBranchesExt,
};

impl BranchManager<'_> {
    #[instrument(level = tracing::Level::DEBUG, skip(self, perm), err(Debug))]
    pub fn create_virtual_branch(
        &self,
        create: &BranchCreateRequest,
        perm: &mut WorktreeWritePermission,
    ) -> Result<Branch> {
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

        let now = gitbutler_time::time::now_ms();

        let mut branch = Branch {
            id: BranchId::generate(),
            name: name.clone(),
            notes: String::new(),
            upstream: None,
            upstream_head: None,
            tree: tree.id(),
            head: default_target.sha,
            created_timestamp_ms: now,
            updated_timestamp_ms: now,
            ownership: BranchOwnershipClaims::default(),
            order,
            selected_for_changes,
            allow_rebasing: self.ctx.project().ok_with_force_push.into(),
            in_workspace: true,
            not_in_workspace_wip_change_id: None,
            source_refname: None,
            references: vec![],
        };

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
    ) -> Result<BranchId> {
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
            .collect::<Vec<Branch>>();

        let order = vb_state.next_order_index()?;

        let selected_for_changes = (!virtual_branches
            .iter()
            .any(|b| b.selected_for_changes.is_some()))
        .then_some(now_since_unix_epoch_ms());

        let now = gitbutler_time::time::now_ms();

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

        let branch = if let Ok(Some(mut branch)) =
            vb_state.find_by_source_refname_where_not_in_workspace(target)
        {
            branch.upstream_head = upstream_branch.is_some().then_some(head_commit.id());
            branch.upstream = upstream_branch;
            branch.tree = head_commit_tree.id();
            branch.head = head_commit.id();
            branch.ownership = ownership;
            branch.order = order;
            branch.selected_for_changes = selected_for_changes;
            branch.allow_rebasing = self.ctx.project().ok_with_force_push.into();
            branch.in_workspace = true;

            branch
        } else {
            Branch {
                id: BranchId::generate(),
                name: branch_name.clone(),
                notes: String::new(),
                source_refname: Some(target.clone()),
                upstream_head: upstream_branch.is_some().then_some(head_commit.id()),
                upstream: upstream_branch,
                tree: head_commit_tree.id(),
                head: head_commit.id(),
                created_timestamp_ms: now,
                updated_timestamp_ms: now,
                ownership,
                order,
                selected_for_changes,
                allow_rebasing: self.ctx.project().ok_with_force_push.into(),
                in_workspace: true,
                not_in_workspace_wip_change_id: None,
                references: vec![],
            }
        };

        vb_state.set_branch(branch.clone())?;
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
        branch_id: BranchId,
        perm: &mut WorktreeWritePermission,
    ) -> Result<String> {
        self.ctx.assure_resolved()?;
        self.ctx.assure_unconflicted()?;
        let repo = self.ctx.repository();

        let vb_state = self.ctx.project().virtual_branches();
        let default_target = vb_state.get_default_target()?;

        let mut branch = vb_state.get_branch_in_workspace(branch_id)?;

        let target_commit = repo
            .find_commit(default_target.sha)
            .context("failed to find target commit")?;
        let target_tree = target_commit.tree().context("failed to get target tree")?;

        // calculate the merge base and make sure it's the same as the target commit
        // if not, we need to merge or rebase the branch to get it up to date

        let merge_base = repo
            .merge_base(default_target.sha, branch.head)
            .context(format!(
                "failed to find merge base between {} and {}",
                default_target.sha, branch.head
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
            let mut branch_merged_with_default_target =
                repo.merge_trees(&merge_base_tree, &branch_tree, &target_tree, None)?;

            // If there are conflicts and edit mode is disabled
            if branch_merged_with_default_target.has_conflicts()
                && !self.ctx.project().succeeding_rebases
            {
                // currently we can only deal with the merge problem branch
                for branch in vb_state
                    .list_branches_in_workspace()?
                    .iter()
                    .filter(|branch| branch.id != branch_id)
                {
                    self.save_and_unapply(branch.id, perm)?;
                }

                // apply the branch
                vb_state.set_branch(branch.clone())?;

                // checkout the conflicts
                repo.checkout_index_builder(&mut branch_merged_with_default_target)
                    .allow_conflicts()
                    .conflict_style_merge()
                    .force()
                    .checkout()
                    .context("failed to checkout index")?;

                // mark conflicts

                let conflicts = branch_merged_with_default_target
                    .conflicts()
                    .context("failed to get merge index conflicts")?;
                let mut merge_conflicts = Vec::new();
                for path in conflicts.flatten() {
                    if let Some(ours) = path.our {
                        let path = gix::path::try_from_bstr(Cow::Owned(ours.path.into()))?;
                        merge_conflicts.push(path);
                    }
                }
                conflicts::mark(self.ctx, &merge_conflicts, Some(default_target.sha))?;

                return Ok(branch.name);
            }

            let new_head = if branch.allow_rebasing {
                let commits_to_rebase = repo.l(branch.head, LogUntil::Commit(merge_base))?;

                let head_oid =
                    cherry_rebase_group(repo, default_target.sha, &commits_to_rebase, true)?;

                repo.find_commit(head_oid)?
            } else {
                gitbutler_merge_commits(
                    repo,
                    repo.find_commit(branch.head)?,
                    repo.find_commit(default_target.sha)?,
                    &branch.name,
                    default_target.branch.branch(),
                )?
            };

            branch.head = new_head.id();
            branch.tree = new_head.tree_id();

            vb_state.set_branch(branch.clone())?;
        }

        // apply the branch
        vb_state.set_branch(branch.clone())?;

        vbranch::ensure_selected_for_changes(&vb_state)
            .context("failed to ensure selected for changes")?;

        let final_tree = vb_state
            .list_branches_in_workspace()?
            .into_iter()
            .try_fold(target_tree.clone(), |final_tree, branch| {
                let branch_tree = repo.find_tree(branch.tree)?;
                let mut result = repo.merge_trees(&target_tree, &final_tree, &branch_tree, None)?;
                let final_tree_oid = result.write_tree_to(repo)?;
                repo.find_tree(final_tree_oid)
                    .context("Failed to find tree")
            })?;

        // checkout final_tree into the working directory
        repo.checkout_tree_builder(&final_tree)
            .force()
            .remove_untracked()
            .checkout()
            .context("failed to checkout tree")?;

        update_workspace_commit(&vb_state, self.ctx)?;

        // Look for and handle the vbranch indicator commit
        // TODO: This is not unapplying the WIP commit for some unholy reason.
        // If you can figgure it out I'll buy you a beer.
        {
            if let Some(wip_commit_to_unapply) = branch.not_in_workspace_wip_change_id {
                let potential_wip_commit = repo.find_commit(branch.head)?;

                if let Some(headers) = potential_wip_commit.gitbutler_headers() {
                    if headers.change_id == wip_commit_to_unapply {
                        branch = vbranch::undo_commit(self.ctx, branch.id, branch.head)?;
                    }
                }

                branch.not_in_workspace_wip_change_id = None;

                vb_state.set_branch(branch.clone())?;
            }
        }

        Ok(branch.name)
    }
}
