use std::time;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    gb_repository,
    git::{self, diff},
    keys,
    project_repository::{self, LogUntil},
    reader, sessions, users,
};

use super::{
    branch, delete_branch,
    errors::{self, CreateVirtualBranchFromBranchError},
    integration::GITBUTLER_INTEGRATION_REFERENCE,
    iterator, target, BranchId, RemoteCommit,
};

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BaseBranch {
    pub branch_name: String,
    pub remote_name: String,
    pub remote_url: String,
    pub base_sha: git::Oid,
    pub current_sha: String,
    pub behind: usize,
    pub upstream_commits: Vec<RemoteCommit>,
    pub recent_commits: Vec<RemoteCommit>,
}

pub fn get_base_branch_data(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<Option<super::BaseBranch>, errors::GetBaseBranchDataError> {
    match gb_repository
        .default_target()
        .context("failed to get default target")?
    {
        None => Ok(None),
        Some(target) => {
            let base = target_to_base_branch(project_repository, &target)
                .context("failed to convert default target to base branch")?;
            Ok(Some(base))
        }
    }
}

pub fn set_base_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    user: Option<&users::User>,
    target_branch: &git::RemoteRefname,
) -> Result<super::BaseBranch, errors::SetBaseBranchError> {
    let repo = &project_repository.git_repository;

    // lookup a branch by name
    let branch = match repo.find_branch(&target_branch.clone().into()) {
        Ok(branch) => Ok(branch),
        Err(git::Error::NotFound(_)) => Err(errors::SetBaseBranchError::BranchNotFound(
            target_branch.clone(),
        )),
        Err(error) => Err(errors::SetBaseBranchError::Other(error.into())),
    }?;

    let remote_name = repo
        .branch_remote_name(branch.refname().unwrap())
        .context(format!(
            "failed to get remote name for branch {}",
            branch.name().unwrap()
        ))?;
    let remote = repo.find_remote(&remote_name).context(format!(
        "failed to find remote {} for branch {}",
        remote_name,
        branch.name().unwrap()
    ))?;
    let remote_url = remote
        .url()
        .context(format!(
            "failed to get remote url for remote {}",
            remote_name
        ))?
        .unwrap();

    // get a list of currently active virtual branches

    // if there are no applied virtual branches, calculate the sha as the merge-base between HEAD in project_repository and this target commit
    let commit = branch.peel_to_commit().context(format!(
        "failed to peel branch {} to commit",
        branch.name().unwrap()
    ))?;
    let mut commit_oid = commit.id();

    let head_ref = repo.head().context("Failed to get HEAD reference")?;
    let head_name: git::Refname = head_ref
        .name()
        .context("Failed to get HEAD reference name")?;
    let head_oid = head_ref
        .peel_to_commit()
        .context("Failed to peel HEAD reference to commit")?
        .id();

    if head_oid != commit_oid {
        // calculate the commit as the merge-base between HEAD in project_repository and this target commit
        commit_oid = repo.merge_base(head_oid, commit_oid).context(format!(
            "Failed to calculate merge base between {} and {}",
            head_oid, commit_oid
        ))?;
    }

    let target = target::Target {
        branch: target_branch.clone(),
        remote_url: remote_url.to_string(),
        sha: commit_oid,
    };

    let target_writer = target::Writer::new(gb_repository);
    target_writer.write_default(&target)?;

    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session for reading")?;

    let virtual_branches = iterator::BranchIterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?;

    let active_virtual_branches = virtual_branches
        .iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    if active_virtual_branches.is_empty()
        && !head_name
            .to_string()
            .eq(&GITBUTLER_INTEGRATION_REFERENCE.to_string())
    {
        let branch = create_virtual_branch_from_branch(
            gb_repository,
            project_repository,
            &head_name,
            Some(true),
            user,
        )
        .context("failed to create virtual branch")?;
        if branch.ownership.is_empty() && branch.head == target.sha {
            delete_branch(gb_repository, project_repository, &branch.id)
                .context("failed to delete branch")?;
        }
    }

    set_exclude_decoration(project_repository)?;

    super::integration::update_gitbutler_integration(gb_repository, project_repository)?;

    let base = target_to_base_branch(project_repository, &target)?;
    Ok(base)
}

fn set_exclude_decoration(project_repository: &project_repository::Repository) -> Result<()> {
    let repo = &project_repository.git_repository;
    let mut config = repo.config()?;
    config
        .set_multivar("log.excludeDecoration", "refs/gitbutler", "refs/gitbutler")
        .context("failed to set log.excludeDecoration")?;
    Ok(())
}

// try to update the target branch
// this means that we need to:
// determine if what the target branch is now pointing to is mergeable with our current working directory
// merge the target branch into our current working directory
// update the target sha
pub fn update_base_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    user: Option<&users::User>,
    signing_key: Option<&keys::PrivateKey>,
) -> Result<(), errors::UpdateBaseBranchError> {
    // look up the target and see if there is a new oid
    let target = gb_repository
        .default_target()
        .context("failed to get default target")?
        .ok_or_else(|| {
            errors::UpdateBaseBranchError::DefaultTargetNotSet(errors::DefaultTargetNotSetError {
                project_id: project_repository.project().id,
            })
        })?;

    let repo = &project_repository.git_repository;
    let target_branch = repo
        .find_branch(&target.branch.clone().into())
        .context(format!("failed to find branch {}", target.branch))?;

    let new_target_commit = target_branch
        .peel_to_commit()
        .context(format!("failed to peel branch {} to commit", target.branch))?;

    // if the target has not changed, do nothing
    if new_target_commit.id() == target.sha {
        return Ok(());
    }

    // ok, target has changed, so now we need to merge it into our current work and update our branches

    // get tree from new target
    let new_target_tree = new_target_commit
        .tree()
        .context("failed to get new target commit tree")?;

    let old_target_tree = repo
        .find_commit(target.sha)
        .and_then(|commit| commit.tree())
        .context(format!(
            "failed to get old target commit tree {}",
            target.sha
        ))?;

    // ok, now we need to deal with a number of situations
    // 1. applied branch, uncommitted conflicts
    // 2. applied branch, committed conflicts but not uncommitted
    // 3. applied branch, no conflicts
    // 4. unapplied branch, uncommitted conflicts
    // 5. unapplied branch, committed conflicts but not uncommitted
    // 6. unapplied branch, no conflicts

    let branch_writer = branch::Writer::new(gb_repository);
    let vbranches = super::get_status_by_branch(gb_repository, project_repository)?;
    for (virtual_branch, all_files) in &vbranches {
        let non_commited_files = super::calculate_non_commited_diffs(
            project_repository,
            virtual_branch,
            &target,
            all_files,
        )?;

        let branch_tree = if virtual_branch.applied {
            super::write_tree(project_repository, &target, all_files).and_then(|tree_id| {
                repo.find_tree(tree_id)
                    .context(format!("failed to find writen tree {}", tree_id))
            })?
        } else {
            repo.find_tree(virtual_branch.tree).context(format!(
                "failed to find tree for branch {}",
                virtual_branch.id
            ))?
        };

        // check for conflicts with this tree
        let mut merge_index = repo
            .merge_trees(&old_target_tree, &branch_tree, &new_target_tree)
            .context(format!(
                "failed to merge trees for branch {}",
                virtual_branch.id
            ))?;

        // check if the branch head has conflicts
        if merge_index.has_conflicts() {
            // unapply branch for now
            if virtual_branch.applied {
                // this changes the wd, and thus the hunks, so we need to re-run the active branch listing
                super::unapply_branch(gb_repository, project_repository, &virtual_branch.id)
                    .context("failed to unapply branch")?;
            }

            if target.sha != virtual_branch.head {
                // check if the head conflicts
                // there are commits on this branch, so create a merge commit with the new tree
                // get tree from virtual branch head
                let head_commit = repo.find_commit(virtual_branch.head).context(format!(
                    "failed to find commit {} for branch {}",
                    virtual_branch.head, virtual_branch.id
                ))?;
                let head_tree = head_commit.tree().context(format!(
                    "failed to find tree for commit {} for branch {}",
                    virtual_branch.head, virtual_branch.id
                ))?;

                let mut merge_index = repo
                    .merge_trees(&old_target_tree, &head_tree, &new_target_tree)
                    .context("failed to merge trees")?;

                // check index for conflicts
                // if it has conflicts, we just ignore it
                if !merge_index.has_conflicts() {
                    // does not conflict with head, so lets merge it and update the head
                    let merge_tree_oid = merge_index
                        .write_tree_to(repo)
                        .context("failed to write tree")?;
                    // get tree from merge_tree_oid
                    let merge_tree = repo
                        .find_tree(merge_tree_oid)
                        .context("failed to find tree")?;

                    // commit the merge tree oid
                    let new_branch_head = project_repository
                        .commit(
                            user,
                            "merged upstream (head only)",
                            &merge_tree,
                            &[&head_commit, &new_target_commit],
                            signing_key,
                        )
                        .context("failed to commit merge")?;

                    branch_writer.write(&branch::Branch {
                        head: new_branch_head,
                        tree: merge_tree_oid,
                        ..virtual_branch.clone()
                    })?;
                }
            }
        } else {
            // get the merge tree oid from writing the index out
            let merge_tree_oid = merge_index
                .write_tree_to(repo)
                .context("failed to write tree")?;

            // branch head does not have conflicts, so don't unapply it, but still try to merge it's head if there are commits
            // but also remove/archive it if the branch is fully integrated
            if target.sha == virtual_branch.head {
                // there were no conflicts and no commits, so write the merge index as the new tree and update the head to the new target
                branch_writer.write(&branch::Branch {
                    head: new_target_commit.id(),
                    tree: merge_tree_oid,
                    ..virtual_branch.clone()
                })?;
            } else {
                // no conflicts, but there have been commits, so update head with a merge
                // there are commits on this branch, so create a merge commit with the new tree
                // get tree from virtual branch head
                let head_commit = repo.find_commit(virtual_branch.head).context(format!(
                    "failed to find commit {} for branch {}",
                    virtual_branch.head, virtual_branch.id
                ))?;
                let head_tree = repo.find_tree(virtual_branch.tree).context(format!(
                    "failed to find tree {} for branch {}",
                    virtual_branch.tree, virtual_branch.id
                ))?;

                let mut merge_index = repo
                    .merge_trees(&old_target_tree, &head_tree, &new_target_tree)
                    .context("failed to merge trees")?;

                // check index for conflicts
                if merge_index.has_conflicts() {
                    // unapply branch for now. we'll handle it later, when user applied it back.
                    super::unapply_branch(gb_repository, project_repository, &virtual_branch.id)
                        .context("failed to unapply branch")?;
                } else {
                    let merge_tree_oid = merge_index
                        .write_tree_to(repo)
                        .context("failed to write tree")?;
                    // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
                    // then the vbranch is fully merged, so delete it
                    if merge_tree_oid == new_target_tree.id() && non_commited_files.is_empty() {
                        branch_writer.delete(virtual_branch)?;
                    } else {
                        // check to see if these commits have already been pushed
                        let mut last_rebase_head = virtual_branch.head;
                        let new_branch_head;

                        match &virtual_branch.upstream {
                            // if there are upstream pushes, just merge, otherwise try to rebase
                            None => {
                                let (author, committer) =
                                    project_repository.git_signatures(user)?;
                                // attempt to rebase, otherwise, fall back to the merge
                                let annotated_branch_head = repo
                                    .find_annotated_commit(virtual_branch.head)
                                    .context("failed to find annotated commit")?;
                                let annotated_upstream_base = repo
                                    .find_annotated_commit(new_target_commit.id())
                                    .context("failed to find annotated commit")?;
                                let mut rebase_options = git2::RebaseOptions::new();
                                rebase_options.quiet(true);
                                rebase_options.inmemory(true);
                                let mut rebase = repo
                                    .rebase(
                                        Some(&annotated_branch_head),
                                        Some(&annotated_upstream_base),
                                        None,
                                        Some(&mut rebase_options),
                                    )
                                    .context("failed to rebase")?;

                                let mut rebase_success = true;

                                while let Some(_rebase_operation) = rebase.next() {
                                    let index = rebase
                                        .inmemory_index()
                                        .context("failed to get inmemory index")?;
                                    if index.has_conflicts() {
                                        rebase_success = false;
                                        break;
                                    }
                                    // try to commit this stage
                                    let commit_result =
                                        rebase.commit(None, &committer.clone().into(), None);
                                    match commit_result {
                                        Ok(commit_id) => {
                                            last_rebase_head = commit_id.into();
                                        }
                                        Err(_e) => {
                                            rebase_success = false;
                                            break;
                                        }
                                    }
                                }

                                if rebase_success {
                                    // Finish the rebase.
                                    rebase.finish(None).context("failed to finish rebase")?;
                                    new_branch_head = last_rebase_head;
                                } else {
                                    // abort the rebase, just do a merge
                                    rebase.abort().context("failed to abort rebase")?;

                                    // get tree from merge_tree_oid
                                    let merge_tree = repo
                                        .find_tree(merge_tree_oid)
                                        .context("failed to find tree")?;

                                    // commit the merge tree oid
                                    new_branch_head = repo
                                        .commit(
                                            None,
                                            &author,
                                            &committer,
                                            "merged upstream",
                                            &merge_tree,
                                            &[&head_commit, &new_target_commit],
                                        )
                                        .context("failed to commit merge")?;
                                }
                            }
                            Some(_) => {
                                // get tree from merge_tree_oid
                                let merge_tree = repo
                                    .find_tree(merge_tree_oid)
                                    .context("failed to find tree")?;

                                // commit the merge tree oid
                                new_branch_head = project_repository
                                    .commit(
                                        user,
                                        "merged upstream",
                                        &merge_tree,
                                        &[&head_commit, &new_target_commit],
                                        signing_key,
                                    )
                                    .context("failed to commit merge")?;
                            }
                        }

                        branch_writer.write(&branch::Branch {
                            head: new_branch_head,
                            tree: merge_tree_oid,
                            ..virtual_branch.clone()
                        })?;
                    }
                }
            }
        }
    }

    // ok, now all the problematic branches have been unapplied, so we can try to merge the upstream branch into our current working directory
    // first, get a new wd tree
    let wd_tree = project_repository
        .get_wd_tree()
        .context("failed to get wd tree")?;

    // and try to merge it
    let mut merge_index = repo
        .merge_trees(&old_target_tree, &wd_tree, &new_target_tree)
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        return Err(errors::UpdateBaseBranchError::Other(anyhow::anyhow!(
            "this should not have happened, we should have already detected this"
        )));
    }

    // now we can try to merge the upstream branch into our current working directory
    repo.checkout_index(&mut merge_index).force().checkout().context(
        "failed to checkout index, this should not have happened, we should have already detected this",
    )?;

    // write new target oid
    let target_writer = target::Writer::new(gb_repository);
    target_writer.write_default(&target::Target {
        sha: new_target_commit.id(),
        ..target
    })?;

    super::integration::update_gitbutler_integration(gb_repository, project_repository)?;

    Ok(())
}

pub fn target_to_base_branch(
    project_repository: &project_repository::Repository,
    target: &target::Target,
) -> Result<super::BaseBranch> {
    let repo = &project_repository.git_repository;
    let branch = repo.find_branch(&target.branch.clone().into())?;
    let commit = branch.peel_to_commit()?;
    let oid = commit.id();

    // gather a list of commits between oid and target.sha
    let upstream_commits = project_repository
        .log(oid, project_repository::LogUntil::Commit(target.sha))
        .context("failed to get upstream commits")?
        .iter()
        .map(super::commit_to_remote_commit)
        .collect::<Result<Vec<_>>>()?;

    // get some recent commits
    let recent_commits = project_repository
        .log(target.sha, LogUntil::Take(20))
        .context("failed to get recent commits")?
        .iter()
        .map(super::commit_to_remote_commit)
        .collect::<Result<Vec<_>>>()?;

    let base = super::BaseBranch {
        branch_name: format!("{}/{}", target.branch.remote(), target.branch.branch()),
        remote_name: target.branch.remote().to_string(),
        remote_url: target.remote_url.clone(),
        base_sha: target.sha,
        current_sha: oid.to_string(),
        behind: upstream_commits.len(),
        upstream_commits,
        recent_commits,
    };
    Ok(base)
}

pub fn create_virtual_branch_from_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    upstream: &git::Refname,
    applied: Option<bool>,
    user: Option<&users::User>,
) -> Result<branch::Branch, CreateVirtualBranchFromBranchError> {
    if !matches!(upstream, git::Refname::Local(_) | git::Refname::Remote(_)) {
        return Err(errors::CreateVirtualBranchFromBranchError::BranchNotFound(
            upstream.clone(),
        ));
    }

    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let default_target = super::get_default_target(&current_session_reader)
        .context("failed to get default target")?
        .ok_or_else(|| {
            errors::CreateVirtualBranchFromBranchError::DefaultTargetNotSet(
                errors::DefaultTargetNotSetError {
                    project_id: project_repository.project().id,
                },
            )
        })?;

    let repo = &project_repository.git_repository;
    let head = match repo.find_reference(upstream) {
        Ok(head) => Ok(head),
        Err(git::Error::NotFound(_)) => Err(
            errors::CreateVirtualBranchFromBranchError::BranchNotFound(upstream.clone()),
        ),
        Err(error) => Err(errors::CreateVirtualBranchFromBranchError::Other(
            error.into(),
        )),
    }?;
    let head_commit = head.peel_to_commit().context("failed to peel to commit")?;
    let tree = head_commit.tree().context("failed to find tree")?;

    let virtual_branches = iterator::BranchIterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?;

    let order = virtual_branches.len();

    let now = time::UNIX_EPOCH
        .elapsed()
        .context("failed to get elapsed time")?
        .as_millis();

    // only set upstream if it's not the default target
    let upstream_branch = match upstream {
        git::Refname::Other(_) | git::Refname::Virtual(_) => {
            // we only support local or remote branches
            return Err(errors::CreateVirtualBranchFromBranchError::BranchNotFound(
                upstream.clone(),
            ));
        }
        git::Refname::Remote(remote) => Some(remote.clone()),
        git::Refname::Local(local) => {
            let remote_name = format!("{}/{}", default_target.branch.remote(), local.branch());
            (remote_name != default_target.branch.branch())
                .then(|| format!("refs/remotes/{}", remote_name).parse().unwrap())
        }
    };

    let mut branch = branch::Branch {
        id: BranchId::generate(),
        name: upstream
            .branch()
            .expect("always a branch reference")
            .to_string(),
        notes: String::new(),
        applied: applied.unwrap_or(false),
        upstream: upstream_branch,
        upstream_head: None,
        tree: tree.id(),
        head: head_commit.id(),
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        ownership: branch::Ownership::default(),
        order,
    };

    // add file ownership based off the diff
    let target_commit = repo
        .find_commit(default_target.sha)
        .map_err(|error| CreateVirtualBranchFromBranchError::Other(error.into()))?;
    let merge_base = repo
        .merge_base(target_commit.id(), head_commit.id())
        .map_err(|error| CreateVirtualBranchFromBranchError::Other(error.into()))?;
    let merge_tree = repo
        .find_commit(merge_base)
        .map_err(|error| CreateVirtualBranchFromBranchError::Other(error.into()))?
        .tree()
        .map_err(|error| CreateVirtualBranchFromBranchError::Other(error.into()))?;
    if merge_base != target_commit.id() {
        let target_tree = target_commit
            .tree()
            .map_err(|error| CreateVirtualBranchFromBranchError::Other(error.into()))?;
        let head_tree = head_commit
            .tree()
            .map_err(|error| CreateVirtualBranchFromBranchError::Other(error.into()))?;

        // merge target and head
        let mut merge_index = repo
            .merge_trees(&merge_tree, &head_tree, &target_tree)
            .context("failed to merge trees")?;

        if merge_index.has_conflicts() {
            return Err(CreateVirtualBranchFromBranchError::MergeConflict);
        }

        let (author, committer) = project_repository.git_signatures(user)?;
        let new_head_tree_oid = merge_index
            .write_tree_to(repo)
            .context("failed to write merge tree")?;
        let new_head_tree = repo
            .find_tree(new_head_tree_oid)
            .context("failed to find tree")?;

        let new_branch_head = repo
            .commit(
                None,
                &author,
                &committer,
                "merged upstream",
                &new_head_tree,
                &[&head_commit, &target_commit],
            )
            .map_err(|error| CreateVirtualBranchFromBranchError::Other(error.into()))?;
        branch.head = new_branch_head;
        branch.tree = new_head_tree_oid;
    }

    // do a diff between the head of this branch and the target base
    let diff = diff::trees(&project_repository.git_repository, &merge_tree, &tree)
        .context("failed to diff trees")?;
    let hunks_by_filepath =
        super::virtual_hunks_by_filepath(&project_repository.git_repository, &diff);

    // assign ownership to the branch
    for hunk in hunks_by_filepath.values().flatten() {
        branch.ownership.put(
            &format!("{}:{}", hunk.file_path.display(), hunk.id)
                .parse()
                .unwrap(),
        );
    }

    let writer = branch::Writer::new(gb_repository);
    writer.write(&branch).context("failed to write branch")?;

    project_repository.add_branch_reference(&branch)?;

    Ok(branch)
}
