use std::{path::Path, time};

use anyhow::{anyhow, Context, Result};
use git2::Index;
use serde::Serialize;

use super::{
    branch, convert_to_real_branch,
    integration::{
        get_workspace_head, update_gitbutler_integration, GITBUTLER_INTEGRATION_REFERENCE,
    },
    target, BranchId, RemoteCommit, VirtualBranchHunk, VirtualBranchesHandle,
};
use crate::{git::RepositoryExt, virtual_branches::errors::Marker};
use crate::{
    git::{self, diff},
    project_repository::{self, LogUntil},
    projects::FetchResult,
    users,
    virtual_branches::{branch::BranchOwnershipClaims, cherry_rebase},
};

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BaseBranch {
    pub branch_name: String,
    pub remote_name: String,
    pub remote_url: String,
    pub push_remote_name: Option<String>,
    pub push_remote_url: String,
    #[serde(with = "crate::serde::oid")]
    pub base_sha: git2::Oid,
    #[serde(with = "crate::serde::oid")]
    pub current_sha: git2::Oid,
    pub behind: usize,
    pub upstream_commits: Vec<RemoteCommit>,
    pub recent_commits: Vec<RemoteCommit>,
    pub last_fetched_ms: Option<u128>,
}

pub fn get_base_branch_data(
    project_repository: &project_repository::Repository,
) -> Result<BaseBranch> {
    let target = default_target(&project_repository.project().gb_dir())?;
    let base = target_to_base_branch(project_repository, &target)?;
    Ok(base)
}

fn go_back_to_integration(
    project_repository: &project_repository::Repository,
    default_target: &target::Target,
) -> Result<BaseBranch> {
    let statuses = project_repository
        .repo()
        .statuses(Some(
            git2::StatusOptions::new()
                .show(git2::StatusShow::IndexAndWorkdir)
                .include_untracked(true),
        ))
        .context("failed to get status")?;
    if !statuses.is_empty() {
        return Err(anyhow!("current HEAD is dirty")).context(Marker::ProjectConflict);
    }

    let vb_state = project_repository.project().virtual_branches();
    let all_virtual_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?;

    let target_commit = project_repository
        .repo()
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let base_tree = target_commit
        .tree()
        .context("failed to get base tree from commit")?;
    let mut final_tree = target_commit
        .tree()
        .context("failed to get base tree from commit")?;
    for branch in &all_virtual_branches {
        // merge this branches tree with our tree
        let branch_head = project_repository
            .repo()
            .find_commit(branch.head)
            .context("failed to find branch head")?;
        let branch_tree = branch_head
            .tree()
            .context("failed to get branch head tree")?;
        let mut result = project_repository
            .repo()
            .merge_trees(&base_tree, &final_tree, &branch_tree, None)
            .context("failed to merge")?;
        let final_tree_oid = result
            .write_tree_to(project_repository.repo())
            .context("failed to write tree")?;
        final_tree = project_repository
            .repo()
            .find_tree(final_tree_oid)
            .context("failed to find written tree")?;
    }

    project_repository
        .repo()
        .checkout_tree_builder(&final_tree)
        .force()
        .checkout()
        .context("failed to checkout tree")?;

    let base = target_to_base_branch(project_repository, default_target)?;
    update_gitbutler_integration(&vb_state, project_repository)?;
    Ok(base)
}

pub fn set_base_branch(
    project_repository: &project_repository::Repository,
    target_branch_ref: &git::RemoteRefname,
) -> Result<BaseBranch> {
    let repo = project_repository.repo();

    // if target exists, and it is the same as the requested branch, we should go back
    if let Ok(target) = default_target(&project_repository.project().gb_dir()) {
        if target.branch.eq(target_branch_ref) {
            return go_back_to_integration(project_repository, &target);
        }
    }

    // lookup a branch by name
    let target_branch = match repo.find_branch_by_refname(&target_branch_ref.clone().into()) {
        Ok(branch) => branch,
        Err(err) => return Err(err),
    }
    .ok_or(anyhow!("remote branch '{}' not found", target_branch_ref))?;

    let remote = repo
        .find_remote(target_branch_ref.remote())
        .context(format!(
            "failed to find remote for branch {}",
            target_branch.get().name().unwrap()
        ))?;
    let remote_url = remote.url().context(format!(
        "failed to get remote url for {}",
        target_branch_ref.remote()
    ))?;

    let target_branch_head = target_branch.get().peel_to_commit().context(format!(
        "failed to peel branch {} to commit",
        target_branch.get().name().unwrap()
    ))?;

    let current_head = repo.head().context("Failed to get HEAD reference")?;
    let current_head_commit = current_head
        .peel_to_commit()
        .context("Failed to peel HEAD reference to commit")?;

    // calculate the commit as the merge-base between HEAD in project_repository and this target commit
    let target_commit_oid = repo
        .merge_base(current_head_commit.id(), target_branch_head.id())
        .context(format!(
            "Failed to calculate merge base between {} and {}",
            current_head_commit.id(),
            target_branch_head.id()
        ))?;

    let target = target::Target {
        branch: target_branch_ref.clone(),
        remote_url: remote_url.to_string(),
        sha: target_commit_oid,
        push_remote_name: None,
    };

    let vb_state = project_repository.project().virtual_branches();
    vb_state.set_default_target(target.clone())?;

    // TODO: make sure this is a real branch
    let head_name: git::Refname = current_head
        .name()
        .map(|name| name.parse().expect("libgit2 provides valid refnames"))
        .context("Failed to get HEAD reference name")?;
    if !head_name
        .to_string()
        .eq(&GITBUTLER_INTEGRATION_REFERENCE.to_string())
    {
        // if there are any commits on the head branch or uncommitted changes in the working directory, we need to
        // put them into a virtual branch

        let wd_diff = diff::workdir(repo, &current_head_commit.id())?;
        if !wd_diff.is_empty() || current_head_commit.id() != target.sha {
            // assign ownership to the branch
            let ownership = wd_diff.iter().fold(
                BranchOwnershipClaims::default(),
                |mut ownership, (file_path, diff)| {
                    for hunk in &diff.hunks {
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

            let now_ms = crate::time::now_ms();

            let (upstream, upstream_head) = if let git::Refname::Local(head_name) = &head_name {
                let upstream_name = target_branch_ref.with_branch(head_name.branch());
                if upstream_name.eq(target_branch_ref) {
                    (None, None)
                } else {
                    match repo.find_reference(&git::Refname::from(&upstream_name).to_string()) {
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
                        Err(err) if err.code() == git2::ErrorCode::NotFound => Ok((None, None)),
                        Err(error) => Err(error),
                    }
                    .context(format!("failed to find upstream for {}", head_name))?
                }
            } else {
                (None, None)
            };

            let branch = branch::Branch {
                id: BranchId::generate(),
                name: head_name.to_string().replace("refs/heads/", ""),
                notes: String::new(),
                old_applied: true,
                upstream,
                upstream_head,
                created_timestamp_ms: now_ms,
                updated_timestamp_ms: now_ms,
                head: current_head_commit.id(),
                tree: super::write_tree_onto_commit(
                    project_repository,
                    current_head_commit.id(),
                    diff::diff_files_into_hunks(wd_diff),
                )?,
                ownership,
                order: 0,
                selected_for_changes: None,
            };

            vb_state.set_branch(branch)?;
        }
    }

    set_exclude_decoration(project_repository)?;

    update_gitbutler_integration(&vb_state, project_repository)?;

    let base = target_to_base_branch(project_repository, &target)?;
    Ok(base)
}

pub fn set_target_push_remote(
    project_repository: &project_repository::Repository,
    push_remote_name: &str,
) -> Result<()> {
    let remote = project_repository
        .repo()
        .find_remote(push_remote_name)
        .context(format!("failed to find remote {}", push_remote_name))?;

    // if target exists, and it is the same as the requested branch, we should go back
    let mut target = default_target(&project_repository.project().gb_dir())?;
    target.push_remote_name = remote
        .name()
        .context("failed to get remote name")?
        .to_string()
        .into();
    let vb_state = project_repository.project().virtual_branches();
    vb_state.set_default_target(target)?;

    Ok(())
}

fn set_exclude_decoration(project_repository: &project_repository::Repository) -> Result<()> {
    let repo = project_repository.repo();
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
pub fn update_base_branch<'repo>(
    project_repository: &'repo project_repository::Repository,
    user: Option<&users::User>,
) -> anyhow::Result<Vec<git2::Branch<'repo>>> {
    project_repository.assure_resolved()?;

    // look up the target and see if there is a new oid
    let target = default_target(&project_repository.project().gb_dir())?;
    let repo = project_repository.repo();
    let target_branch = repo
        .find_branch_by_refname(&target.branch.clone().into())
        .context(format!("failed to find branch {}", target.branch))?;

    let new_target_commit = target_branch
        .ok_or(anyhow!("failed to get branch"))?
        .get()
        .peel_to_commit()
        .context(format!("failed to peel branch {} to commit", target.branch))?;

    let mut unapplied_branch_names: Vec<git2::Branch> = Vec::new();

    if new_target_commit.id() == target.sha {
        return Ok(unapplied_branch_names);
    }

    let new_target_tree = new_target_commit
        .tree()
        .context("failed to get new target commit tree")?;

    let old_target_tree = repo.find_commit(target.sha)?.tree().context(format!(
        "failed to get old target commit tree {}",
        target.sha
    ))?;

    let vb_state = project_repository.project().virtual_branches();
    let integration_commit = get_workspace_head(&vb_state, project_repository)?;

    // try to update every branch
    let updated_vbranches =
        super::get_status_by_branch(project_repository, Some(&integration_commit))?
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
                                project_repository.repo(),
                                &branch_head_tree,
                                &branch_tree,
                            )?;
                            if non_commited_files.is_empty() {
                                // if there are no commited files, then the branch is fully merged
                                // and we can delete it.
                                vb_state.remove_branch(branch.id)?;
                                project_repository.delete_branch_reference(&branch)?;
                                Ok(None)
                            } else {
                                vb_state.set_branch(branch.clone())?;
                                Ok(Some(branch))
                            }
                        };

                    if branch_head_tree.id() == new_target_tree.id() {
                        return result_integrated_detected(branch);
                    }

                    // try to merge branch head with new target
                    let mut branch_tree_merge_index = repo
                        .merge_trees(&old_target_tree, &branch_tree, &new_target_tree, None)
                        .context(format!("failed to merge trees for branch {}", branch.id))?;

                    if branch_tree_merge_index.has_conflicts() {
                        // branch tree conflicts with new target, unapply branch for now. we'll handle it later, when user applies it back.
                        let unapplied_real_branch = convert_to_real_branch(
                            project_repository,
                            branch.id,
                            Default::default(),
                        )?;
                        unapplied_branch_names.push(unapplied_real_branch);

                        return Ok(Some(branch));
                    }

                    let branch_merge_index_tree_oid =
                        branch_tree_merge_index.write_tree_to(project_repository.repo())?;

                    if branch_merge_index_tree_oid == new_target_tree.id() {
                        return result_integrated_detected(branch);
                    }

                    if branch.head == target.sha {
                        // there are no commits on the branch, so we can just update the head to the new target and calculate the new tree
                        branch.head = new_target_commit.id();
                        branch.tree = branch_merge_index_tree_oid;
                        vb_state.set_branch(branch.clone())?;
                        return Ok(Some(branch));
                    }

                    let mut branch_head_merge_index = repo
                        .merge_trees(&old_target_tree, &branch_head_tree, &new_target_tree, None)
                        .context(format!(
                            "failed to merge head tree for branch {}",
                            branch.id
                        ))?;

                    if branch_head_merge_index.has_conflicts() {
                        // branch commits conflict with new target, make sure the branch is
                        // unapplied. conflicts witll be dealt with when applying it back.
                        let unapplied_real_branch = convert_to_real_branch(
                            project_repository,
                            branch.id,
                            Default::default(),
                        )?;
                        unapplied_branch_names.push(unapplied_real_branch);

                        return Ok(Some(branch));
                    }

                    // branch commits do not conflict with new target, so lets merge them
                    let branch_head_merge_tree_oid = branch_head_merge_index
                        .write_tree_to(project_repository.repo())
                        .context(format!(
                            "failed to write head merge index for {}",
                            branch.id
                        ))?;

                    let ok_with_force_push = project_repository.project().ok_with_force_push;

                    let result_merge =
                        |mut branch: branch::Branch| -> Result<Option<branch::Branch>> {
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
                                        branch.name,
                                    )
                                    .as_str(),
                                    &branch_head_merge_tree,
                                    &[&branch_head_commit, &new_target_commit],
                                    None,
                                )
                                .context("failed to commit merge")?;

                            branch.head = new_target_head;
                            branch.tree = branch_merge_index_tree_oid;
                            vb_state.set_branch(branch.clone())?;
                            Ok(Some(branch))
                        };

                    if branch.upstream.is_some() && !ok_with_force_push {
                        return result_merge(branch);
                    }

                    // branch was not pushed to upstream yet. attempt a rebase,
                    let rebased_head_oid = cherry_rebase(
                        project_repository,
                        new_target_commit.id(),
                        new_target_commit.id(),
                        branch.head,
                    );

                    // rebase failed, just do the merge
                    if rebased_head_oid.is_err() {
                        return result_merge(branch);
                    }

                    if let Some(rebased_head_oid) = rebased_head_oid? {
                        // rebase worked out, rewrite the branch head
                        branch.head = rebased_head_oid;
                        branch.tree = branch_merge_index_tree_oid;
                        vb_state.set_branch(branch.clone())?;
                        return Ok(Some(branch));
                    }

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
        .fold(new_target_commit.tree(), |final_tree, branch| {
            let repo: &git2::Repository = repo;
            let final_tree = final_tree?;
            let branch_tree = repo.find_tree(branch.tree)?;
            let mut merge_result: Index =
                repo.merge_trees(&new_target_tree, &final_tree, &branch_tree, None)?;
            let final_tree_oid = merge_result.write_tree_to(repo)?;
            repo.find_tree(final_tree_oid)
        })
        .context("failed to calculate final tree")?;

    repo.checkout_tree_builder(&final_tree)
        .force()
        .checkout()
        .context("failed to checkout index, this should not have happened, we should have already detected this")?;

    // write new target oid
    vb_state.set_default_target(target::Target {
        sha: new_target_commit.id(),
        ..target
    })?;

    // Rewriting the integration commit is necessary after changing target sha.
    super::integration::update_gitbutler_integration(&vb_state, project_repository)?;
    Ok(unapplied_branch_names)
}

pub fn target_to_base_branch(
    project_repository: &project_repository::Repository,
    target: &target::Target,
) -> Result<super::BaseBranch> {
    let repo = project_repository.repo();
    let branch = repo
        .find_branch_by_refname(&target.branch.clone().into())?
        .ok_or(anyhow!("failed to get branch"))?;
    let commit = branch.get().peel_to_commit()?;
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

    // there has got to be a better way to do this.
    let push_remote_url = match target.push_remote_name {
        Some(ref name) => match repo.find_remote(name) {
            Ok(remote) => match remote.url() {
                Some(url) => url.to_string(),
                None => target.remote_url.clone(),
            },
            Err(_err) => target.remote_url.clone(),
        },
        None => target.remote_url.clone(),
    };

    let base = super::BaseBranch {
        branch_name: format!("{}/{}", target.branch.remote(), target.branch.branch()),
        remote_name: target.branch.remote().to_string(),
        remote_url: target.remote_url.clone(),
        push_remote_name: target.push_remote_name.clone(),
        push_remote_url,
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

fn default_target(base_path: &Path) -> Result<target::Target> {
    VirtualBranchesHandle::new(base_path).get_default_target()
}
