use std::{path::Path, time};

use anyhow::{Context as _, Result, anyhow};
use but_core::worktree::checkout::UncommitedWorktreeChanges;
use but_ctx::Context;
use but_error::Marker;
use but_forge::ForgeRepoInfo;
use but_oxidize::{ObjectIdExt, OidExt};
use gitbutler_branch::GITBUTLER_WORKSPACE_REFERENCE;
use gitbutler_project::FetchResult;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{
    RepositoryExt,
    logging::{LogUntil, RepositoryExt as _},
};
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{
    BranchOwnershipClaims, Stack, Target, VirtualBranchesHandle, canned_branch_name,
};
use serde::Serialize;
use tracing::instrument;

use crate::{
    VirtualBranchesExt,
    hunk::VirtualBranchHunk,
    integration::update_workspace_commit,
    remote::{RemoteCommit, commit_to_remote_commit},
};

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BaseBranch {
    pub branch_name: String,
    pub remote_name: String,
    pub remote_url: String,
    pub push_remote_name: Option<String>,
    pub push_remote_url: String,
    #[serde(with = "but_serde::oid")]
    pub base_sha: git2::Oid,
    #[serde(with = "but_serde::oid")]
    pub current_sha: git2::Oid,
    pub behind: usize,
    pub upstream_commits: Vec<RemoteCommit>,
    pub recent_commits: Vec<RemoteCommit>,
    pub last_fetched_ms: Option<u128>,
    pub conflicted: bool,
    pub diverged: bool,
    #[serde(with = "but_serde::oid_vec")]
    pub diverged_ahead: Vec<git2::Oid>,
    #[serde(with = "but_serde::oid_vec")]
    pub diverged_behind: Vec<git2::Oid>,
    pub forge_repo_info: Option<ForgeRepoInfo>,
}

impl BaseBranch {
    pub fn short_name(&self) -> &str {
        let remote_prefix = format!("{}/", self.remote_name);
        self.branch_name
            .strip_prefix(&remote_prefix)
            .unwrap_or(&self.branch_name)
    }
}

#[instrument(skip(ctx), err(Debug))]
pub fn get_base_branch_data(ctx: &Context) -> Result<BaseBranch> {
    let target = default_target(&ctx.project_data_dir())?;
    let base = target_to_base_branch(ctx, &target)?;
    Ok(base)
}

#[instrument(skip(ctx), err(Debug))]
fn go_back_to_integration(ctx: &Context, default_target: &Target) -> Result<BaseBranch> {
    let gix_repo = ctx.clone_repo_for_merging()?;
    if ctx.settings().feature_flags.cv3 {
        let workspace_commit_to_checkout =
            but_workspace::legacy::remerged_workspace_commit_v2(ctx)?;
        let tree_to_checkout_to_avoid_ref_update = gix_repo
            .find_commit(workspace_commit_to_checkout.to_gix())?
            .tree_id()?;
        but_core::worktree::safe_checkout(
            gix_repo.head_id()?.detach(),
            tree_to_checkout_to_avoid_ref_update.detach(),
            &gix_repo,
            but_core::worktree::checkout::Options {
                uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                skip_head_update: false,
            },
        )?;
    } else {
        let (mut outcome, conflict_kind) =
            but_workspace::legacy::merge_worktree_with_workspace(ctx, &gix_repo)?;

        if outcome.has_unresolved_conflicts(conflict_kind) {
            return Err(anyhow!("Conflicts while going back to gitbutler/workspace"))
                .context(Marker::ProjectConflict);
        }

        let final_tree_id = outcome.tree.write()?.detach();

        let repo = &*ctx.git2_repo.get()?;
        let final_tree = repo.find_tree(final_tree_id.to_git2())?;
        repo.checkout_tree_builder(&final_tree)
            .force()
            .checkout()
            .context("failed to checkout tree")?;
    }

    let base = target_to_base_branch(ctx, default_target)?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    update_workspace_commit(&vb_state, ctx, false)?;
    Ok(base)
}

pub(crate) fn set_base_branch(
    ctx: &Context,
    target_branch_ref: &RemoteRefname,
) -> Result<BaseBranch> {
    let repo = &*ctx.git2_repo.get()?;

    // if target exists, and it is the same as the requested branch, we should go back
    if let Ok(target) = default_target(&ctx.project_data_dir())
        && target.branch.eq(target_branch_ref)
    {
        return go_back_to_integration(ctx, &target);
    }

    // lookup a branch by name
    let target_branch = repo
        .maybe_find_branch_by_refname(&target_branch_ref.clone().into())?
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

    let vb_state = ctx.legacy_project.virtual_branches();
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

            let (upstream, upstream_head, branch_matches_target) =
                if let Refname::Local(head_name) = &head_name {
                    let upstream_name = target_branch_ref.with_branch(head_name.branch());
                    if upstream_name.eq(target_branch_ref) {
                        (None, None, true)
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
                                Ok((Some(upstream_name), Some(head), false))
                            }
                            Err(err) if err.code() == git2::ErrorCode::NotFound => {
                                Ok((None, None, false))
                            }
                            Err(error) => Err(error),
                        }
                        .context(format!("failed to find upstream for {head_name}"))?
                    }
                } else {
                    (None, None, false)
                };

            let branch_name = if branch_matches_target {
                canned_branch_name(repo)?
            } else {
                head_name.to_string().replace("refs/heads/", "")
            };

            let mut branch = Stack::create(
                ctx,
                branch_name,
                Some(head_name),
                upstream,
                upstream_head,
                current_head_commit.tree_id(),
                current_head_commit.id(),
                0,
                None,
                ctx.legacy_project.ok_with_force_push.into(),
                !branch_matches_target, // allow duplicate name since here we are creating a lane from an existing branch
            )?;
            branch.ownership = ownership;

            vb_state.set_stack(branch)?;
        }
    }

    set_exclude_decoration(ctx)?;

    update_workspace_commit(&vb_state, ctx, true)?;

    let base = target_to_base_branch(ctx, &target)?;
    Ok(base)
}

pub(crate) fn set_target_push_remote(ctx: &Context, push_remote_name: &str) -> Result<()> {
    let git2_repo = &*ctx.git2_repo.get()?;
    let remote = git2_repo
        .find_remote(push_remote_name)
        .context(format!("failed to find remote {push_remote_name}"))?;

    // if target exists, and it is the same as the requested branch, we should go back
    let mut target = default_target(&ctx.project_data_dir())?;
    target.push_remote_name = remote
        .name()
        .context("failed to get remote name")?
        .to_string()
        .into();
    let vb_state = ctx.legacy_project.virtual_branches();
    vb_state.set_default_target(target)?;

    Ok(())
}

fn set_exclude_decoration(ctx: &Context) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;
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
            println!("    blob: {content}");
        } else {
            println!("    blob: BINARY");
        }
    }
    Ok(())
}

pub(crate) fn target_to_base_branch(ctx: &Context, target: &Target) -> Result<BaseBranch> {
    let repo = &*ctx.git2_repo.get()?;
    let target_branch = repo
        .maybe_find_branch_by_refname(&target.branch.clone().into())?
        .ok_or(anyhow!("failed to get branch"))?;
    let target_commit_id = target_branch.get().peel_to_commit()?.id();

    // determine if the base branch is behind its upstream
    let (number_commits_ahead, number_commits_behind) =
        repo.graph_ahead_behind(target.sha, target_commit_id)?;

    let diverged_ahead = repo
        .log(target.sha, LogUntil::Take(number_commits_ahead), false)
        .context("failed to get fork point")?
        .iter()
        .map(|commit| commit.id())
        .collect::<Vec<_>>();

    let diverged_behind = repo
        .log(
            target_commit_id,
            LogUntil::Take(number_commits_behind),
            false,
        )
        .context("failed to get fork point")?
        .iter()
        .map(|commit| commit.id())
        .collect::<Vec<_>>();

    // if there are commits ahead of the base branch consider it diverged
    let diverged = !diverged_ahead.is_empty();

    // gather a list of commits between oid and target.sha
    let upstream_commits = repo
        .log(target_commit_id, LogUntil::Commit(target.sha), false)
        .context("failed to get upstream commits")?
        .iter()
        .map(commit_to_remote_commit)
        .collect::<Vec<_>>();

    // get some recent commits
    let recent_commits = repo
        .log(target.sha, LogUntil::Take(20), false)
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

    // Fallback to the remote URL of the branch if the target remote URL is empty
    let remote_url = if target.remote_url.is_empty() {
        let remote = repo.find_remote(target.branch.remote()).context(format!(
            "failed to find remote for branch {}",
            target.branch.fullname()
        ))?;
        let remote_url = remote.url().context(format!(
            "failed to get remote url for {}",
            target.branch.fullname()
        ))?;
        remote_url.to_string()
    } else {
        target.remote_url.clone()
    };

    let forge_repo_info = but_forge::derive_forge_repo_info(&remote_url);

    let base = BaseBranch {
        branch_name: target.branch.fullname(),
        remote_name: target.branch.remote().to_string(),
        remote_url,
        push_remote_name: target.push_remote_name.clone(),
        push_remote_url,
        base_sha: target.sha,
        current_sha: target_commit_id,
        behind: upstream_commits.len(),
        upstream_commits,
        recent_commits,
        last_fetched_ms: ctx
            .legacy_project
            .project_data_last_fetch
            .as_ref()
            .map(FetchResult::timestamp)
            .map(|t| t.duration_since(time::UNIX_EPOCH).unwrap().as_millis()),
        conflicted,
        diverged,
        diverged_ahead,
        diverged_behind,
        forge_repo_info,
    };
    Ok(base)
}

fn default_target(base_path: &Path) -> Result<Target> {
    VirtualBranchesHandle::new(base_path).get_default_target()
}

pub(crate) fn push(ctx: &Context, with_force: bool) -> Result<()> {
    let target = default_target(&ctx.project_data_dir())?;
    let _ = ctx.push(
        target.sha,
        &target.branch,
        with_force,
        ctx.legacy_project.force_push_protection,
        None,
        None,
        vec![],
    );
    Ok(())
}
