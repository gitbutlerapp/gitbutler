use std::{path::Path, time};

use anyhow::{anyhow, Context, Result};
use gitbutler_branch::{
    self, Branch, BranchOwnershipClaims, Target, VirtualBranchesHandle,
    GITBUTLER_WORKSPACE_REFERENCE,
};
use gitbutler_command_context::CommandContext;
use gitbutler_error::error::Marker;
use gitbutler_project::FetchResult;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{LogUntil, RepoActionsExt, RepositoryExt};
use serde::Serialize;

use crate::{
    conflicts::RepoConflictsExt,
    hunk::VirtualBranchHunk,
    integration::update_workspace_commit,
    remote::{commit_to_remote_commit, RemoteCommit},
    VirtualBranchesExt,
};

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BaseBranch {
    pub branch_name: String,
    pub remote_name: String,
    pub remote_url: String,
    pub push_remote_name: Option<String>,
    pub push_remote_url: String,
    #[serde(with = "gitbutler_serde::oid")]
    pub base_sha: git2::Oid,
    #[serde(with = "gitbutler_serde::oid")]
    pub current_sha: git2::Oid,
    pub behind: usize,
    pub upstream_commits: Vec<RemoteCommit>,
    pub recent_commits: Vec<RemoteCommit>,
    pub last_fetched_ms: Option<u128>,
    pub conflicted: bool,
    pub diverged: bool,
    #[serde(with = "gitbutler_serde::oid_vec")]
    pub diverged_ahead: Vec<git2::Oid>,
    #[serde(with = "gitbutler_serde::oid_vec")]
    pub diverged_behind: Vec<git2::Oid>,
}

pub(crate) fn get_base_branch_data(ctx: &CommandContext) -> Result<BaseBranch> {
    let target = default_target(&ctx.project().gb_dir())?;
    let base = target_to_base_branch(ctx, &target)?;
    Ok(base)
}

fn go_back_to_integration(ctx: &CommandContext, default_target: &Target) -> Result<BaseBranch> {
    let statuses = ctx
        .repository()
        .statuses(Some(
            git2::StatusOptions::new()
                .show(git2::StatusShow::IndexAndWorkdir)
                .include_untracked(true),
        ))
        .context("failed to get status")?;
    if !statuses.is_empty() {
        return Err(anyhow!("current HEAD is dirty")).context(Marker::ProjectConflict);
    }

    let vb_state = ctx.project().virtual_branches();
    let virtual_branches = vb_state
        .list_branches_in_workspace()
        .context("failed to read virtual branches")?;

    let target_commit = ctx
        .repository()
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let base_tree = target_commit
        .tree()
        .context("failed to get base tree from commit")?;
    let mut final_tree = target_commit
        .tree()
        .context("failed to get base tree from commit")?;
    for branch in &virtual_branches {
        // merge this branches tree with our tree
        let branch_head = ctx
            .repository()
            .find_commit(branch.head())
            .context("failed to find branch head")?;
        let branch_tree = branch_head
            .tree()
            .context("failed to get branch head tree")?;
        let mut result = ctx
            .repository()
            .merge_trees(&base_tree, &final_tree, &branch_tree, None)
            .context("failed to merge")?;
        let final_tree_oid = result
            .write_tree_to(ctx.repository())
            .context("failed to write tree")?;
        final_tree = ctx
            .repository()
            .find_tree(final_tree_oid)
            .context("failed to find written tree")?;
    }

    ctx.repository()
        .checkout_tree_builder(&final_tree)
        .force()
        .checkout()
        .context("failed to checkout tree")?;

    let base = target_to_base_branch(ctx, default_target)?;
    update_workspace_commit(&vb_state, ctx)?;
    Ok(base)
}

pub(crate) fn set_base_branch(
    ctx: &CommandContext,
    target_branch_ref: &RemoteRefname,
) -> Result<BaseBranch> {
    let repo = ctx.repository();

    // if target exists, and it is the same as the requested branch, we should go back
    if let Ok(target) = default_target(&ctx.project().gb_dir()) {
        if target.branch.eq(target_branch_ref) {
            return go_back_to_integration(ctx, &target);
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

    // calculate the commit as the merge-base between HEAD in ctx and this target commit
    let target_commit_oid = repo
        .merge_base(current_head_commit.id(), target_branch_head.id())
        .context(format!(
            "Failed to calculate merge base between {} and {}",
            current_head_commit.id(),
            target_branch_head.id()
        ))?;

    let target = Target {
        branch: target_branch_ref.clone(),
        remote_url: remote_url.to_string(),
        sha: target_commit_oid,
        push_remote_name: None,
    };

    let vb_state = ctx.project().virtual_branches();
    vb_state.set_default_target(target.clone())?;

    // TODO: make sure this is a real branch
    let head_name: Refname = current_head
        .name()
        .map(|name| name.parse().expect("libgit2 provides valid refnames"))
        .context("Failed to get HEAD reference name")?;
    if !head_name
        .to_string()
        .eq(&GITBUTLER_WORKSPACE_REFERENCE.to_string())
    {
        // if there are any commits on the head branch or uncommitted changes in the working directory, we need to
        // put them into a virtual branch

        let wd_diff = gitbutler_diff::workdir(repo, current_head_commit.id())?;
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

            let (upstream, upstream_head) = if let Refname::Local(head_name) = &head_name {
                let upstream_name = target_branch_ref.with_branch(head_name.branch());
                if upstream_name.eq(target_branch_ref) {
                    (None, None)
                } else {
                    match repo.find_reference(&Refname::from(&upstream_name).to_string()) {
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

            let mut branch = Branch::new(
                head_name.to_string().replace("refs/heads/", ""),
                Some(head_name),
                upstream,
                upstream_head,
                gitbutler_diff::write::hunks_onto_commit(
                    ctx,
                    current_head_commit.id(),
                    gitbutler_diff::diff_files_into_hunks(wd_diff),
                )?,
                current_head_commit.id(),
                0,
                None,
                ctx.project().ok_with_force_push.into(),
            );
            branch.ownership = ownership;

            vb_state.set_branch(branch)?;
        }
    }

    set_exclude_decoration(ctx)?;

    update_workspace_commit(&vb_state, ctx)?;

    let base = target_to_base_branch(ctx, &target)?;
    Ok(base)
}

pub(crate) fn set_target_push_remote(ctx: &CommandContext, push_remote_name: &str) -> Result<()> {
    let remote = ctx
        .repository()
        .find_remote(push_remote_name)
        .context(format!("failed to find remote {}", push_remote_name))?;

    // if target exists, and it is the same as the requested branch, we should go back
    let mut target = default_target(&ctx.project().gb_dir())?;
    target.push_remote_name = remote
        .name()
        .context("failed to get remote name")?
        .to_string()
        .into();
    let vb_state = ctx.project().virtual_branches();
    vb_state.set_default_target(target)?;

    Ok(())
}

fn set_exclude_decoration(ctx: &CommandContext) -> Result<()> {
    let repo = ctx.repository();
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

pub(crate) fn target_to_base_branch(ctx: &CommandContext, target: &Target) -> Result<BaseBranch> {
    let repo = ctx.repository();
    let branch = repo
        .find_branch_by_refname(&target.branch.clone().into())?
        .ok_or(anyhow!("failed to get branch"))?;
    let commit = branch.get().peel_to_commit()?;
    let oid = commit.id();

    // determine if the base branch is behind it's upstream
    let (number_commits_ahead, number_commits_behind) = repo.graph_ahead_behind(target.sha, oid)?;

    let diverged_ahead = repo
        .log(target.sha, LogUntil::Take(number_commits_ahead))
        .context("failed to get fork point")?
        .iter()
        .map(|commit| commit.id())
        .collect::<Vec<_>>();

    let diverged_behind = repo
        .log(oid, LogUntil::Take(number_commits_behind))
        .context("failed to get fork point")?
        .iter()
        .map(|commit| commit.id())
        .collect::<Vec<_>>();

    // if there are commits ahead of the base branch consider it diverged
    let diverged = !diverged_ahead.is_empty();

    // gather a list of commits between oid and target.sha
    let upstream_commits = repo
        .log(oid, LogUntil::Commit(target.sha))
        .context("failed to get upstream commits")?
        .iter()
        .map(commit_to_remote_commit)
        .collect::<Vec<_>>();

    // get some recent commits
    let recent_commits = repo
        .log(target.sha, LogUntil::Take(20))
        .context("failed to get recent commits")?
        .iter()
        .map(commit_to_remote_commit)
        .collect::<Vec<_>>();

    // we assume that only local commits can be conflicted
    let conflicted = recent_commits.iter().any(|commit| commit.conflicted);

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

    let base = BaseBranch {
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
        last_fetched_ms: ctx
            .project()
            .project_data_last_fetch
            .as_ref()
            .map(FetchResult::timestamp)
            .copied()
            .map(|t| t.duration_since(time::UNIX_EPOCH).unwrap().as_millis()),
        conflicted,
        diverged,
        diverged_ahead,
        diverged_behind,
    };
    Ok(base)
}

fn default_target(base_path: &Path) -> Result<Target> {
    VirtualBranchesHandle::new(base_path).get_default_target()
}

pub(crate) fn push(ctx: &CommandContext, with_force: bool) -> Result<()> {
    ctx.assure_resolved()?;
    let target = default_target(&ctx.project().gb_dir())?;
    let _ = ctx.push(target.sha, &target.branch, with_force, None, None);
    Ok(())
}
