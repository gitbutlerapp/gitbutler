use super::BranchManager;
use crate::{
    conflicts::{self, RepoConflictsExt},
    ensure_selected_for_changes,
    integration::update_gitbutler_integration,
    set_ownership, undo_commit, VirtualBranchHunk, VirtualBranchesExt,
};
use anyhow::{anyhow, bail, Context, Result};
use gitbutler_branch::{
    branch::{self, BranchCreateRequest, BranchId},
    dedup::dedup,
    diff,
    ownership::BranchOwnershipClaims,
};
use gitbutler_commit::commit_headers::HasCommitHeaders;
use gitbutler_error::error::Marker;
use gitbutler_oplog::snapshot::Snapshot;
use gitbutler_reference::Refname;
use gitbutler_repo::{rebase::cherry_rebase, RepoActions, RepositoryExt};
use gitbutler_time::time::now_since_unix_epoch_ms;

impl BranchManager<'_> {
    pub fn create_virtual_branch(&self, create: &BranchCreateRequest) -> Result<branch::Branch> {
        let vb_state = self.project_repository.project().virtual_branches();

        let default_target = vb_state.get_default_target()?;

        let commit = self
            .project_repository
            .repo()
            .find_commit(default_target.sha)
            .context("failed to find default target commit")?;

        let tree = commit
            .tree()
            .context("failed to find defaut target commit tree")?;

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
            .project_repository
            .project()
            .snapshot_branch_creation(name.clone());

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

        let mut branch = branch::Branch {
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
            allow_rebasing: self.project_repository.project().ok_with_force_push.into(),
            applied: true,
            in_workspace: true,
            not_in_workspace_wip_change_id: None,
            source_refname: None,
        };

        if let Some(ownership) = &create.ownership {
            set_ownership(&vb_state, &mut branch, ownership).context("failed to set ownership")?;
        }

        vb_state.set_branch(branch.clone())?;
        self.project_repository.add_branch_reference(&branch)?;

        Ok(branch)
    }

    pub fn create_virtual_branch_from_branch(&self, upstream: &Refname) -> Result<BranchId> {
        // only set upstream if it's not the default target
        let upstream_branch = match upstream {
            Refname::Other(_) | Refname::Virtual(_) => {
                // we only support local or remote branches
                bail!("branch {upstream} must be a local or remote branch");
            }
            Refname::Remote(remote) => Some(remote.clone()),
            Refname::Local(local) => local.remote().cloned(),
        };

        let branch_name = upstream
            .branch()
            .expect("always a branch reference")
            .to_string();

        let _ = self
            .project_repository
            .project()
            .snapshot_branch_creation(branch_name.clone());

        let vb_state = self.project_repository.project().virtual_branches();

        let default_target = vb_state.get_default_target()?;

        if let Refname::Remote(remote_upstream) = upstream {
            if default_target.branch == *remote_upstream {
                bail!("cannot create a branch from default target")
            }
        }

        let repo = self.project_repository.repo();
        let head_reference =
            repo.find_reference(&upstream.to_string())
                .map_err(|err| match err {
                    err if err.code() == git2::ErrorCode::NotFound => {
                        anyhow!("branch {upstream} was not found")
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
            .collect::<Vec<branch::Branch>>();

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
        let diff = diff::trees(
            self.project_repository.repo(),
            &merge_base_tree,
            &head_commit_tree,
        )?;

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
            vb_state.find_by_source_refname_where_not_in_workspace(upstream)
        {
            branch.upstream_head = upstream_branch.is_some().then_some(head_commit.id());
            branch.upstream = upstream_branch;
            branch.tree = head_commit_tree.id();
            branch.head = head_commit.id();
            branch.ownership = ownership;
            branch.order = order;
            branch.selected_for_changes = selected_for_changes;
            branch.allow_rebasing = self.project_repository.project().ok_with_force_push.into();
            branch.applied = true;
            branch.in_workspace = true;

            branch
        } else {
            branch::Branch {
                id: BranchId::generate(),
                name: branch_name.clone(),
                notes: String::new(),
                source_refname: Some(upstream.clone()),
                upstream_head: upstream_branch.is_some().then_some(head_commit.id()),
                upstream: upstream_branch,
                tree: head_commit_tree.id(),
                head: head_commit.id(),
                created_timestamp_ms: now,
                updated_timestamp_ms: now,
                ownership,
                order,
                selected_for_changes,
                allow_rebasing: self.project_repository.project().ok_with_force_push.into(),
                applied: true,
                in_workspace: true,
                not_in_workspace_wip_change_id: None,
            }
        };

        vb_state.set_branch(branch.clone())?;
        self.project_repository.add_branch_reference(&branch)?;

        match self.apply_branch(branch.id) {
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
    fn apply_branch(&self, branch_id: BranchId) -> Result<String> {
        self.project_repository.assure_resolved()?;
        self.project_repository.assure_unconflicted()?;
        let repo = self.project_repository.repo();

        let vb_state = self.project_repository.project().virtual_branches();
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
        if merge_base != default_target.sha {
            // Branch is out of date, merge or rebase it
            let merge_base_tree = repo
                .find_commit(merge_base)
                .context(format!("failed to find merge base commit {}", merge_base))?
                .tree()
                .context("failed to find merge base tree")?;

            let branch_tree = repo
                .find_tree(branch.tree)
                .context("failed to find branch tree")?;

            let mut merge_index = repo
                .merge_trees(&merge_base_tree, &branch_tree, &target_tree, None)
                .context("failed to merge trees")?;

            if merge_index.has_conflicts() {
                // currently we can only deal with the merge problem branch
                for branch in vb_state
                    .list_branches_in_workspace()?
                    .iter()
                    .filter(|branch| branch.id != branch_id)
                {
                    self.convert_to_real_branch(branch.id, Default::default())?;
                }

                // apply the branch
                vb_state.set_branch(branch.clone())?;

                // checkout the conflicts
                repo.checkout_index_builder(&mut merge_index)
                    .allow_conflicts()
                    .conflict_style_merge()
                    .force()
                    .checkout()
                    .context("failed to checkout index")?;

                // mark conflicts
                let conflicts = merge_index
                    .conflicts()
                    .context("failed to get merge index conflicts")?;
                let mut merge_conflicts = Vec::new();
                for path in conflicts.flatten() {
                    if let Some(ours) = path.our {
                        let path = std::str::from_utf8(&ours.path)
                            .context("failed to convert path to utf8")?
                            .to_string();
                        merge_conflicts.push(path);
                    }
                }
                conflicts::mark(
                    self.project_repository,
                    &merge_conflicts,
                    Some(default_target.sha),
                )?;

                return Ok(branch.name);
            }

            let head_commit = repo
                .find_commit(branch.head)
                .context("failed to find head commit")?;

            let merged_branch_tree_oid = merge_index
                .write_tree_to(self.project_repository.repo())
                .context("failed to write tree")?;

            let merged_branch_tree = repo
                .find_tree(merged_branch_tree_oid)
                .context("failed to find tree")?;

            let ok_with_force_push = branch.allow_rebasing;
            if branch.upstream.is_some() && !ok_with_force_push {
                // branch was pushed to upstream, and user doesn't like force pushing.
                // create a merge commit to avoid the need of force pushing then.

                let new_branch_head = self.project_repository.commit(
                    format!(
                        "Merged {}/{} into {}",
                        default_target.branch.remote(),
                        default_target.branch.branch(),
                        branch.name
                    )
                    .as_str(),
                    &merged_branch_tree,
                    &[&head_commit, &target_commit],
                    None,
                )?;

                // ok, update the virtual branch
                branch.head = new_branch_head;
            } else {
                let rebase = cherry_rebase(
                    self.project_repository,
                    target_commit.id(),
                    target_commit.id(),
                    branch.head,
                );
                let mut rebase_success = true;
                let mut last_rebase_head = branch.head;
                match rebase {
                    Ok(rebase_oid) => {
                        if let Some(oid) = rebase_oid {
                            last_rebase_head = oid;
                        }
                    }
                    Err(_) => {
                        rebase_success = false;
                    }
                }

                if rebase_success {
                    // rebase worked out, rewrite the branch head
                    branch.head = last_rebase_head;
                } else {
                    // rebase failed, do a merge commit

                    // get tree from merge_tree_oid
                    let merge_tree = repo
                        .find_tree(merged_branch_tree_oid)
                        .context("failed to find tree")?;

                    // commit the merge tree oid
                    let new_branch_head = self
                        .project_repository
                        .commit(
                            format!(
                                "Merged {}/{} into {}",
                                default_target.branch.remote(),
                                default_target.branch.branch(),
                                branch.name
                            )
                            .as_str(),
                            &merge_tree,
                            &[&head_commit, &target_commit],
                            None,
                        )
                        .context("failed to commit merge")?;

                    branch.head = new_branch_head;
                }
            }

            branch.tree = repo
                .find_commit(branch.head)?
                .tree()
                .map_err(anyhow::Error::from)?
                .id();
            vb_state.set_branch(branch.clone())?;
        }

        let wd_tree = self.project_repository.repo().get_wd_tree()?;

        let branch_tree = repo
            .find_tree(branch.tree)
            .context("failed to find branch tree")?;

        // check index for conflicts
        let mut merge_index = repo
            .merge_trees(&target_tree, &wd_tree, &branch_tree, None)
            .context("failed to merge trees")?;

        if merge_index.has_conflicts() {
            // mark conflicts
            let conflicts = merge_index
                .conflicts()
                .context("failed to get merge index conflicts")?;
            let mut merge_conflicts = Vec::new();
            for path in conflicts.flatten() {
                if let Some(ours) = path.our {
                    let path = std::str::from_utf8(&ours.path)
                        .context("failed to convert path to utf8")?
                        .to_string();
                    merge_conflicts.push(path);
                }
            }
            conflicts::mark(
                self.project_repository,
                &merge_conflicts,
                Some(default_target.sha),
            )?;
        }

        // apply the branch
        vb_state.set_branch(branch.clone())?;

        ensure_selected_for_changes(&vb_state).context("failed to ensure selected for changes")?;
        // checkout the merge index
        repo.checkout_index_builder(&mut merge_index)
            .force()
            .checkout()
            .context("failed to checkout index")?;

        // Look for and handle the vbranch indicator commit
        // TODO: This is not unapplying the WIP commit for some unholy reason.
        // If you can figgure it out I'll buy you a beer.
        {
            if let Some(wip_commit_to_unapply) = branch.not_in_workspace_wip_change_id {
                let potential_wip_commit = repo.find_commit(branch.head)?;

                if let Some(headers) = potential_wip_commit.gitbutler_headers() {
                    if headers.change_id == wip_commit_to_unapply {
                        undo_commit(self.project_repository, branch.id, branch.head)?;
                    }
                }

                branch.not_in_workspace_wip_change_id = None;
                vb_state.set_branch(branch.clone())?;
            }
        }

        update_gitbutler_integration(&vb_state, self.project_repository)?;

        Ok(branch.name)
    }
}
