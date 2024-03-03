use std::time;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    gb_repository,
    git::{
        self,
        diff::{self},
    },
    keys,
    project_repository::{self, LogUntil},
    projects::FetchResult,
    users,
    virtual_branches::branch::Ownership,
};

use super::{
    branch, errors, integration::GITBUTLER_INTEGRATION_REFERENCE, target, BranchId, RemoteCommit,
};

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BaseBranch {
    pub branch_name: String,
    pub remote_name: String,
    pub remote_url: String,
    pub base_sha: git::Oid,
    pub current_sha: git::Oid,
    pub behind: usize,
    pub upstream_commits: Vec<RemoteCommit>,
    pub recent_commits: Vec<RemoteCommit>,
    pub last_fetched_ms: Option<u128>,
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
    target_branch_ref: &git::RemoteRefname,
) -> Result<super::BaseBranch, errors::SetBaseBranchError> {
    let repo = &project_repository.git_repository;

    // lookup a branch by name
    let target_branch = match repo.find_branch(&target_branch_ref.clone().into()) {
        Ok(branch) => Ok(branch),
        Err(git::Error::NotFound(_)) => Err(errors::SetBaseBranchError::BranchNotFound(
            target_branch_ref.clone(),
        )),
        Err(error) => Err(errors::SetBaseBranchError::Other(error.into())),
    }?;

    let remote = repo
        .find_remote(target_branch_ref.remote())
        .context(format!(
            "failed to find remote for branch {}",
            target_branch.name().unwrap()
        ))?;
    let remote_url = remote
        .url()
        .context(format!(
            "failed to get remote url for {}",
            target_branch_ref.remote()
        ))?
        .unwrap();

    let target_branch_head = target_branch.peel_to_commit().context(format!(
        "failed to peel branch {} to commit",
        target_branch.name().unwrap()
    ))?;

    let current_head = repo.head().context("Failed to get HEAD reference")?;
    let current_head_commit = current_head
        .peel_to_commit()
        .context("Failed to peel HEAD reference to commit")?;

    // calculate the commit as the merge-base between HEAD in project_repository and this target commit
    let commit_oid = repo
        .merge_base(current_head_commit.id(), target_branch_head.id())
        .context(format!(
            "Failed to calculate merge base between {} and {}",
            current_head_commit.id(),
            target_branch_head.id()
        ))?;

    // if default target was already set, and the new target is a descendant of the current head, then we want to
    // keep the current target to avoid unnecessary rebases
    let commit_oid = if let Some(current_target) = gb_repository.default_target()? {
        if repo
            .is_descendant_of(current_target.sha, commit_oid)
            .context("failed to check if target branch is descendant of current head")?
        {
            current_target.sha
        } else {
            commit_oid
        }
    } else {
        commit_oid
    };

    let target = target::Target {
        branch: target_branch_ref.clone(),
        remote_url: remote_url.to_string(),
        sha: commit_oid,
    };

    let target_writer =
        target::Writer::new(gb_repository).context("failed to create target writer")?;
    target_writer.write_default(&target)?;

    let head_name: git::Refname = current_head
        .name()
        .context("Failed to get HEAD reference name")?;
    if !head_name
        .to_string()
        .eq(&GITBUTLER_INTEGRATION_REFERENCE.to_string())
    {
        // if there are any commits on the head branch or uncommitted changes in the working directory, we need to
        // put them into a virtual branch

        let use_context = project_repository
            .project()
            .use_diff_context
            .unwrap_or(false);
        let context_lines = if use_context { 3_u32 } else { 0_u32 };
        let wd_diff = diff::workdir(repo, &current_head_commit.id(), context_lines)?.0;
        if !wd_diff.is_empty() || current_head_commit.id() != target.sha {
            let hunks_by_filepath =
                super::virtual_hunks_by_filepath(&project_repository.project().path, &wd_diff);

            // assign ownership to the branch
            let ownership = hunks_by_filepath.values().flatten().fold(
                Ownership::default(),
                |mut ownership, hunk| {
                    ownership.put(
                        &format!("{}:{}", hunk.file_path.display(), hunk.id)
                            .parse()
                            .unwrap(),
                    );
                    ownership
                },
            );

            let now_ms = time::UNIX_EPOCH
                .elapsed()
                .context("failed to get elapsed time")?
                .as_millis();

            let (upstream, upstream_head) = if let git::Refname::Local(head_name) = &head_name {
                let upstream_name = target_branch_ref.with_branch(head_name.branch());
                if upstream_name.eq(target_branch_ref) {
                    (None, None)
                } else {
                    match repo.find_reference(&git::Refname::from(&upstream_name)) {
                        Ok(upstream) => {
                            let head = upstream
                                .peel_to_commit()
                                .map(|commit| commit.id())
                                .context(format!(
                                    "failed to peel upstream {} to commit",
                                    upstream.name().unwrap()
                                ))?;
                            Ok((Some(upstream_name), Some(head)))
                        }
                        Err(git::Error::NotFound(_)) => Ok((None, None)),
                        Err(error) => Err(error),
                    }
                    .context(format!("failed to find upstream for {}", head_name))?
                }
            } else {
                (None, None)
            };

            let mut branch = branch::Branch {
                id: BranchId::generate(),
                name: head_name.to_string().replace("refs/heads/", ""),
                notes: String::new(),
                applied: true,
                upstream,
                upstream_head,
                created_timestamp_ms: now_ms,
                updated_timestamp_ms: now_ms,
                head: current_head_commit.id(),
                tree: super::write_tree_onto_commit(
                    project_repository,
                    current_head_commit.id(),
                    &wd_diff,
                )?,
                ownership,
                order: 0,
                selected_for_changes: None,
            };

            let branch_writer =
                branch::Writer::new(gb_repository).context("failed to create branch writer")?;
            branch_writer.write(&mut branch)?;
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

fn _print_tree(repo: &git2::Repository, tree: &git2::Tree) -> Result<()> {
    println!("tree id: {}", tree.id());
    for entry in tree {
        println!(
            "  entry: {} {}",
            entry.name().unwrap_or_default(),
            entry.id()
        );
        // get entry contents
        let object = entry.to_object(repo).context("failed to get object")?;
        let blob = object.as_blob().context("failed to get blob")?;
        // convert content to string
        if let Ok(content) = std::str::from_utf8(blob.content()) {
            println!("    blob: {}", content);
        } else {
            println!("    blob: BINARY");
        }
    }
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
    if project_repository.is_resolving() {
        return Err(errors::UpdateBaseBranchError::Conflict(
            errors::ProjectConflictError {
                project_id: project_repository.project().id,
            },
        ));
    }

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

    let branch_writer =
        branch::Writer::new(gb_repository).context("failed to create branch writer")?;

    let use_context = project_repository
        .project()
        .use_diff_context
        .unwrap_or(false);
    let context_lines = if use_context { 3_u32 } else { 0_u32 };

    // try to update every branch
    let updated_vbranches = super::get_status_by_branch(gb_repository, project_repository)?
        .0
        .into_iter()
        .map(|(branch, _)| branch)
        .map(
            |mut branch: branch::Branch| -> Result<Option<branch::Branch>> {
                let branch_tree = repo.find_tree(branch.tree)?;

                let branch_head_commit = repo.find_commit(branch.head).context(format!(
                    "failed to find commit {} for branch {}",
                    branch.head, branch.id
                ))?;
                let branch_head_tree = branch_head_commit.tree().context(format!(
                    "failed to find tree for commit {} for branch {}",
                    branch.head, branch.id
                ))?;

                let result_integrated_detected =
                    |mut branch: branch::Branch| -> Result<Option<branch::Branch>> {
                        // branch head tree is the same as the new target tree.
                        // meaning we can safely use the new target commit as the branch head.

                        branch.head = new_target_commit.id();

                        // it also means that the branch is fully integrated into the target.
                        // disconnect it from the upstream
                        branch.upstream = None;
                        branch.upstream_head = None;

                        let non_commited_files = diff::trees(
                            &project_repository.git_repository,
                            &branch_head_tree,
                            &branch_tree,
                            context_lines,
                        )?;
                        if non_commited_files.is_empty() {
                            // if there are no commited files, then the branch is fully merged
                            // and we can delete it.
                            branch_writer.delete(&branch)?;
                            project_repository.delete_branch_reference(&branch)?;
                            Ok(None)
                        } else {
                            branch_writer.write(&mut branch)?;
                            Ok(Some(branch))
                        }
                    };

                if branch_head_tree.id() == new_target_tree.id() {
                    return result_integrated_detected(branch);
                }

                // try to merge branch head with new target
                let mut branch_tree_merge_index = repo
                    .merge_trees(&old_target_tree, &branch_tree, &new_target_tree)
                    .context(format!("failed to merge trees for branch {}", branch.id))?;

                if branch_tree_merge_index.has_conflicts() {
                    // branch tree conflicts with new target, unapply branch for now. we'll handle it later, when user applies it back.
                    branch.applied = false;
                    branch_writer.write(&mut branch)?;
                    return Ok(Some(branch));
                }

                let branch_merge_index_tree_oid = branch_tree_merge_index.write_tree_to(repo)?;

                if branch_merge_index_tree_oid == new_target_tree.id() {
                    return result_integrated_detected(branch);
                }

                if branch.head == target.sha {
                    // there are no commits on the branch, so we can just update the head to the new target and calculate the new tree
                    branch.head = new_target_commit.id();
                    branch.tree = branch_merge_index_tree_oid;
                    branch_writer.write(&mut branch)?;
                    return Ok(Some(branch));
                }

                let mut branch_head_merge_index = repo
                    .merge_trees(&old_target_tree, &branch_head_tree, &new_target_tree)
                    .context(format!(
                        "failed to merge head tree for branch {}",
                        branch.id
                    ))?;

                if branch_head_merge_index.has_conflicts() {
                    // branch commits conflict with new target, make sure the branch is
                    // unapplied. conflicts witll be dealt with when applying it back.
                    branch.applied = false;
                    branch_writer.write(&mut branch)?;
                    return Ok(Some(branch));
                }

                // branch commits do not conflict with new target, so lets merge them
                let branch_head_merge_tree_oid = branch_head_merge_index
                    .write_tree_to(repo)
                    .context(format!(
                        "failed to write head merge index for {}",
                        branch.id
                    ))?;

                let ok_with_force_push = project_repository.project().ok_with_force_push;

                let result_merge = |mut branch: branch::Branch| -> Result<Option<branch::Branch>> {
                    // branch was pushed to upstream, and user doesn't like force pushing.
                    // create a merge commit to avoid the need of force pushing then.
                    let branch_head_merge_tree = repo
                        .find_tree(branch_head_merge_tree_oid)
                        .context("failed to find tree")?;

                    let new_target_head = project_repository
                        .commit(
                            user,
                            format!(
                                "Merged {}/{} into {}",
                                target.branch.remote(),
                                target.branch.branch(),
                                branch.name
                            )
                            .as_str(),
                            &branch_head_merge_tree,
                            &[&branch_head_commit, &new_target_commit],
                            signing_key,
                        )
                        .context("failed to commit merge")?;

                    branch.head = new_target_head;
                    branch.tree = branch_merge_index_tree_oid;
                    branch_writer.write(&mut branch)?;
                    Ok(Some(branch))
                };

                if branch.upstream.is_some() && !ok_with_force_push {
                    return result_merge(branch);
                }

                // branch was not pushed to upstream yet. attempt a rebase,
                let (_, committer) = project_repository.git_signatures(user)?;
                let mut rebase_options = git2::RebaseOptions::new();
                rebase_options.quiet(true);
                rebase_options.inmemory(true);
                let mut rebase = repo
                    .rebase(
                        Some(branch.head),
                        Some(new_target_commit.id()),
                        None,
                        Some(&mut rebase_options),
                    )
                    .context("failed to rebase")?;

                let mut rebase_success = true;
                // check to see if these commits have already been pushed
                let mut last_rebase_head = branch.head;
                while rebase.next().is_some() {
                    let index = rebase
                        .inmemory_index()
                        .context("failed to get inmemory index")?;
                    if index.has_conflicts() {
                        rebase_success = false;
                        break;
                    }

                    if let Ok(commit_id) = rebase.commit(None, &committer.clone().into(), None) {
                        last_rebase_head = commit_id.into();
                    } else {
                        rebase_success = false;
                        break;
                    }
                }

                if rebase_success {
                    // rebase worked out, rewrite the branch head
                    rebase.finish(None).context("failed to finish rebase")?;
                    branch.head = last_rebase_head;
                    branch.tree = branch_merge_index_tree_oid;
                    branch_writer.write(&mut branch)?;
                    return Ok(Some(branch));
                }

                // rebase failed, do a merge commit
                rebase.abort().context("failed to abort rebase")?;

                result_merge(branch)
            },
        )
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    // ok, now all the problematic branches have been unapplied
    // now we calculate and checkout new tree for the working directory

    let final_tree = updated_vbranches
        .iter()
        .filter(|branch| branch.applied)
        .fold(new_target_commit.tree(), |final_tree, branch| {
            let final_tree = final_tree?;
            let branch_tree = repo.find_tree(branch.tree)?;
            let mut merge_result = repo.merge_trees(&new_target_tree, &final_tree, &branch_tree)?;
            let final_tree_oid = merge_result.write_tree_to(repo)?;
            repo.find_tree(final_tree_oid)
        })
        .context("failed to calculate final tree")?;

    repo.checkout_tree(&final_tree).force().checkout().context(
        "failed to checkout index, this should not have happened, we should have already detected this",
    )?;

    // write new target oid
    let target_writer =
        target::Writer::new(gb_repository).context("failed to create target writer")?;
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
        .collect::<Vec<_>>();

    // get some recent commits
    let recent_commits = project_repository
        .log(target.sha, LogUntil::Take(20))
        .context("failed to get recent commits")?
        .iter()
        .map(super::commit_to_remote_commit)
        .collect::<Vec<_>>();

    let base = super::BaseBranch {
        branch_name: format!("{}/{}", target.branch.remote(), target.branch.branch()),
        remote_name: target.branch.remote().to_string(),
        remote_url: target.remote_url.clone(),
        base_sha: target.sha,
        current_sha: oid,
        behind: upstream_commits.len(),
        upstream_commits,
        recent_commits,
        last_fetched_ms: project_repository
            .project()
            .project_data_last_fetch
            .as_ref()
            .map(FetchResult::timestamp)
            .copied()
            .map(|t| t.duration_since(time::UNIX_EPOCH).unwrap().as_millis()),
    };
    Ok(base)
}
