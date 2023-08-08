use std::time;

use anyhow::{bail, Context, Result};
use uuid::Uuid;

use crate::{gb_repository, project_repository, reader, sessions};

use super::{branch, iterator, target};

pub fn get_base_branch_data(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<Option<super::BaseBranch>> {
    match gb_repository.default_target()? {
        None => Ok(None),
        Some(target) => {
            let base = target_to_base_branch(project_repository, &target)?;
            Ok(Some(base))
        }
    }
}

pub fn set_base_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    target_branch: &str,
) -> Result<super::BaseBranch> {
    let repo = &project_repository.git_repository;

    // lookup a branch by name
    let branch = repo.find_branch(target_branch, git2::BranchType::Remote)?;

    let remote_name = repo.branch_remote_name(branch.get().name().unwrap())?;
    let remote = repo.find_remote(remote_name.as_str().unwrap())?;
    let remote_url = remote.url().unwrap();

    // get a list of currently active virtual branches

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

    // if there are no applied virtual branches, calculate the sha as the merge-base between HEAD in project_repository and this target commit
    let commit = branch.get().peel_to_commit()?;
    let mut commit_oid = commit.id();

    let head_ref = repo.head().context("Failed to get HEAD reference")?;
    let head_branch: project_repository::branch::Name = head_ref
        .name()
        .context("Failed to get HEAD reference name")?
        .parse()
        .context("Failed to parse HEAD reference name")?;
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
        branch_name: branch.name()?.unwrap().to_string(),
        remote_name: remote.name().unwrap().to_string(),
        remote_url: remote_url.to_string(),
        sha: commit_oid,
    };

    let target_writer = target::Writer::new(gb_repository);
    target_writer.write_default(&target)?;

    if active_virtual_branches.is_empty() {
        create_virtual_branch_from_branch(
            gb_repository,
            project_repository,
            &head_branch,
            Some(true),
        )?;
    }

    super::update_gitbutler_integration(gb_repository, project_repository)?;

    let base = target_to_base_branch(project_repository, &target)?;
    Ok(base)
}

// try to update the target branch
// this means that we need to:
// determine if what the target branch is now pointing to is mergeable with our current working directory
// merge the target branch into our current working directory
// update the target sha
pub fn update_base_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<()> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    // look up the target and see if there is a new oid
    let target = gb_repository
        .default_target()
        .context("failed to get default target")?
        .context("no default target set")?;

    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(gb_repository);

    let repo = &project_repository.git_repository;
    let target_branch = repo
        .find_branch(&target.branch_name, git2::BranchType::Remote)
        .context(format!("failed to find branch {}", target.branch_name))?;

    let new_target_commit = target_branch.get().peel_to_commit().context(format!(
        "failed to peel branch {} to commit",
        target.branch_name
    ))?;
    let new_target_commit_oid = new_target_commit.id();

    // if the target has not changed, do nothing
    if new_target_commit_oid == target.sha {
        return Ok(());
    }

    // ok, target has changed, so now we need to merge it into our current work and update our branches

    // get all virtual branches, we need to try to update them all
    let mut virtual_branches = iterator::BranchIterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .collect::<Vec<_>>();

    // get tree from new target
    let new_target_tree = new_target_commit.tree()?;

    // get tree from target.sha
    let old_target_commit = repo.find_commit(target.sha)?;
    let old_target_tree = old_target_commit.tree()?;

    // ok, now we need to deal with a number of situations
    // 1. applied branch, uncommitted conflicts
    // 2. applied branch, committed conflicts but not uncommitted
    // 3. applied branch, no conflicts
    // 4. unapplied branch, uncommitted conflicts
    // 5. unapplied branch, committed conflicts but not uncommitted
    // 6. unapplied branch, no conflicts

    let mut vbranches = super::get_status_by_branch(gb_repository, project_repository)?;
    let mut vbranches_commits = super::list_virtual_branches(gb_repository, project_repository)?;
    // update the heads of all our virtual branches
    for virtual_branch in &mut virtual_branches {
        let mut virtual_branch = virtual_branch.clone();

        let all_files = vbranches
            .iter()
            .find(|(vbranch, _)| vbranch.id == virtual_branch.id)
            .map(|(_, files)| files);

        let non_commited_files = vbranches_commits
            .iter()
            .find(|vbranch| vbranch.id == virtual_branch.id)
            .map(|vbranch| vbranch.files.clone())
            .unwrap_or_default();

        let tree_oid = if virtual_branch.applied {
            super::write_tree(project_repository, &target, all_files.unwrap()).context(format!(
                "failed to write tree for branch {}",
                virtual_branch.id
            ))?
        } else {
            virtual_branch.tree
        };
        let branch_tree = repo.find_tree(tree_oid)?;

        // check for conflicts with this tree
        let merge_index = repo
            .merge_trees(
                &old_target_tree,
                &branch_tree,
                &new_target_tree,
                Some(&git2::MergeOptions::new()),
            )
            .context(format!(
                "failed to merge trees for branch {}",
                virtual_branch.id
            ))?;

        // check if the branch head has conflicts
        if merge_index.has_conflicts() {
            // unapply branch for now
            if virtual_branch.applied {
                // this changes the wd, and thus the hunks, so we need to re-run the active branch listing
                super::unapply_branch(gb_repository, project_repository, &virtual_branch.id)?;
                vbranches = super::get_status_by_branch(gb_repository, project_repository)?;
                vbranches_commits =
                    super::list_virtual_branches(gb_repository, project_repository)?;
            }
            virtual_branch = branch_reader.read(&virtual_branch.id)?;

            if target.sha != virtual_branch.head {
                // check if the head conflicts
                // there are commits on this branch, so create a merge commit with the new tree
                // get tree from virtual branch head
                let head_commit = repo.find_commit(virtual_branch.head)?;
                let head_tree = head_commit.tree()?;

                let mut merge_index = repo
                    .merge_trees(
                        &old_target_tree,
                        &head_tree,
                        &new_target_tree,
                        Some(&git2::MergeOptions::new()),
                    )
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

                    let (author, committer) = gb_repository.git_signatures()?;
                    // commit the merge tree oid
                    let new_branch_head = repo.commit(
                        None,
                        &author,
                        &committer,
                        "merged upstream (head only)",
                        &merge_tree,
                        &[&head_commit, &new_target_commit],
                    )?;
                    virtual_branch.head = new_branch_head;
                    virtual_branch.tree = merge_tree_oid;
                    branch_writer.write(&virtual_branch)?;
                }
            }
        } else {
            // branch head does not have conflicts, so don't unapply it, but still try to merge it's head if there are commits
            // but also remove/archive it if the branch is fully integrated
            if target.sha == virtual_branch.head {
                // there were no conflicts and no commits, so just update the head
                virtual_branch.head = new_target_commit_oid;
                virtual_branch.tree = tree_oid;
                branch_writer.write(&virtual_branch)?;
            } else {
                // no conflicts, but there have been commits, so update head with a merge
                // there are commits on this branch, so create a merge commit with the new tree
                // get tree from virtual branch head
                let head_commit = repo.find_commit(virtual_branch.head)?;
                let head_tree = repo.find_tree(virtual_branch.tree)?;

                let mut merge_index = repo
                    .merge_trees(
                        &old_target_tree,
                        &head_tree,
                        &new_target_tree,
                        Some(&git2::MergeOptions::new()),
                    )
                    .context("failed to merge trees")?;

                // check index for conflicts
                if merge_index.has_conflicts() {
                    // unapply branch for now
                    // this changes the wd, and thus the hunks, so we need to re-run the active branch listing
                    super::unapply_branch(gb_repository, project_repository, &virtual_branch.id)?;
                    vbranches = super::get_status_by_branch(gb_repository, project_repository)?;
                    vbranches_commits =
                        super::list_virtual_branches(gb_repository, project_repository)?;
                } else {
                    // get the merge tree oid from writing the index out
                    let merge_tree_oid = merge_index
                        .write_tree_to(repo)
                        .context("failed to write tree")?;
                    // get tree from merge_tree_oid
                    let merge_tree = repo
                        .find_tree(merge_tree_oid)
                        .context("failed to find tree")?;

                    // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
                    // then the vbranch is fully merged, so delete it
                    if merge_tree_oid == new_target_tree.id() && non_commited_files.is_empty() {
                        branch_writer.delete(&virtual_branch)?;
                    } else {
                        let (author, committer) = gb_repository.git_signatures()?;
                        // commit the merge tree oid
                        let new_branch_head = repo.commit(
                            None,
                            &author,
                            &committer,
                            "merged upstream",
                            &merge_tree,
                            &[&head_commit, &new_target_commit],
                        )?;
                        virtual_branch.head = new_branch_head;
                        virtual_branch.tree = merge_tree_oid;
                        branch_writer.write(&virtual_branch)?;
                    }
                }
            }
        }
    }

    // ok, now all the problematic branches have been unapplied, so we can try to merge the upstream branch into our current working directory
    // first, get a new wd tree
    let wd_tree = super::get_wd_tree(repo)?;

    // and try to merge it
    let mut merge_index = repo
        .merge_trees(
            &old_target_tree,
            &wd_tree,
            &new_target_tree,
            Some(&git2::MergeOptions::new()),
        )
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        bail!("this should not have happened, we should have already detected this");
    }

    // now we can try to merge the upstream branch into our current working directory
    let mut checkout_options = git2::build::CheckoutBuilder::new();
    checkout_options.force();
    repo.checkout_index(Some(&mut merge_index), Some(&mut checkout_options))?;

    // write new target oid
    let target_writer = target::Writer::new(gb_repository);
    target_writer.write_default(&target::Target {
        sha: new_target_commit_oid,
        ..target
    })?;

    super::update_gitbutler_integration(gb_repository, project_repository)?;

    Ok(())
}

pub fn target_to_base_branch(
    project_repository: &project_repository::Repository,
    target: &target::Target,
) -> Result<super::BaseBranch> {
    let repo = &project_repository.git_repository;
    let branch = repo.find_branch(&target.branch_name, git2::BranchType::Remote)?;
    let commit = branch.get().peel_to_commit()?;
    let oid = commit.id();

    // gather a list of commits between oid and target.sha
    let mut upstream_commits = vec![];
    for commit in project_repository
        .log(oid, target.sha)
        .context(format!("failed to get log for branch {:?}", branch.name()?))?
    {
        let commit = super::commit_to_vbranch_commit(project_repository, &commit, None)?;
        upstream_commits.push(commit);
    }

    // get some recent commits
    let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
    revwalk
        .push(target.sha)
        .context(format!("failed to push {}", target.sha))?;
    let mut recent_commits = vec![];
    for oid in revwalk.take(10) {
        let oid = oid.context("failed to get oid")?;
        let commit = repo.find_commit(oid).context("failed to find commit")?;
        let commit = super::commit_to_vbranch_commit(project_repository, &commit, None)?;
        recent_commits.push(commit);
    }

    let base = super::BaseBranch {
        branch_name: target.branch_name.clone(),
        remote_name: target.remote_name.clone(),
        remote_url: target.remote_url.clone(),
        base_sha: target.sha.to_string(),
        current_sha: oid.to_string(),
        behind: upstream_commits.len() as u32,
        upstream_commits,
        recent_commits,
    };
    Ok(base)
}

pub fn create_virtual_branch_from_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    upstream: &project_repository::branch::Name,
    applied: Option<bool>,
) -> Result<String> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let default_target = super::get_default_target(&current_session_reader)
        .context("failed to get default target")?
        .context("no default target found")?;

    let repo = &project_repository.git_repository;
    let head = repo.revparse_single(&upstream.to_string())?;
    let head_commit = head.peel_to_commit()?;
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

    let branch_id = Uuid::new_v4().to_string();
    let mut branch = branch::Branch {
        id: branch_id.clone(),
        name: upstream.branch().to_string(),
        applied: applied.unwrap_or(false),
        upstream: Some(match upstream {
            project_repository::branch::Name::Remote(remote) => remote.clone(),
            project_repository::branch::Name::Local(local) => format!(
                "refs/remotes/{}/{}",
                default_target.remote_name,
                local.branch()
            )
            .parse()
            .unwrap(),
        }),
        tree: tree.id(),
        head: head_commit.id(),
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        ownership: branch::Ownership::default(),
        order,
    };

    // add file ownership based off the diff
    let target_commit = repo.find_commit(default_target.sha)?;
    let merge_base = repo.merge_base(target_commit.id(), head_commit.id())?;
    let merge_tree = repo.find_commit(merge_base)?.tree()?;
    if merge_base != target_commit.id() {
        let target_tree = target_commit.tree()?;
        let head_tree = head_commit.tree()?;

        // merge target and head
        let merge_options = git2::MergeOptions::new();
        let mut merge_index = repo
            .merge_trees(&merge_tree, &head_tree, &target_tree, Some(&merge_options))
            .context("failed to merge trees")?;

        if merge_index.has_conflicts() {
            bail!("merge conflict");
        } else {
            let (author, committer) = gb_repository.git_signatures()?;
            let new_head_tree_oid = merge_index
                .write_tree_to(repo)
                .context("failed to write merge tree")?;
            let new_head_tree = repo
                .find_tree(new_head_tree_oid)
                .context("failed to find tree")?;

            let new_branch_head = repo.commit(
                None,
                &author,
                &committer,
                "merged upstream",
                &new_head_tree,
                &[&head_commit, &target_commit],
            )?;
            branch.head = new_branch_head;
            branch.tree = new_head_tree_oid
        }
    }

    // do a diff between the head of this branch and the target base
    let diff = project_repository::diff::trees(project_repository, &merge_tree, &tree)
        .context("failed to diff trees")?;
    let hunks_by_filepath = super::hunks_by_filepath(project_repository, &diff);

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
    Ok(branch_id)
}
