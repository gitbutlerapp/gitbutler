use std::{fmt::Write, path::Path};

use anyhow::bail;
use but_ctx::Context;
use gitbutler_git::GitContextExt;

use crate::{
    CliId, IdMap,
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel, shorten_object_id},
};

const MAX_PUSH_ATTEMPTS: usize = 5;

pub async fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_id: &str,
    yes: bool,
) -> anyhow::Result<()> {
    let t = theme::get();
    let (branch_name, base_branch) = {
        let mut guard = ctx.exclusive_worktree_access();
        let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
        let resolved_ids = id_map.parse_using_context(branch_id, ctx)?;
        if resolved_ids.is_empty() {
            bail!("Could not find branch: {branch_id}");
        }
        if resolved_ids.len() > 1 {
            bail!("Ambiguous branch '{branch_id}', matches multiple items");
        }

        let cli_id = &resolved_ids[0];
        let branch_name = match cli_id {
            CliId::Branch { name, .. } => name.clone(),
            _ => bail!("Expected a branch ID, got {}", cli_id.kind_for_humans()),
        };

        let base_branch =
            but_api::legacy::virtual_branches::get_base_branch_data(ctx, guard.write_permission())?
                .ok_or_else(|| anyhow::anyhow!("No base branch configured"))?;
        (branch_name, base_branch)
    };

    let target_branch_name = base_branch.short_name.clone();
    if target_branch_name.is_empty() {
        bail!("Configured target branch has no branch name");
    }
    let push_remote_name = if base_branch.push_remote_name.is_empty() {
        base_branch.remote_name.clone()
    } else {
        base_branch.push_remote_name.clone()
    };
    if push_remote_name.is_empty() {
        bail!("Configured target branch has no push remote");
    }

    let target_display = format!("{push_remote_name}/{target_branch_name}");
    let push_remote_url = if base_branch.push_remote_url.is_empty() {
        &base_branch.remote_url
    } else {
        &base_branch.push_remote_url
    };
    if remote_points_at_current_repo(ctx, push_remote_url)? {
        bail!(
            "Refusing to directly push to {target_display}: the configured push remote points at this working repository"
        );
    }

    let pr_number = pr_number_for_branch(ctx, &branch_name)?;
    confirm_direct_target_update(out, &branch_name, pr_number, &target_display, yes)?;

    let mut progress = out.progress_channel();
    writeln!(
        progress,
        "Fetching newest data for target {}...",
        t.remote_branch.paint(&target_display)
    )?;
    but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("integrate".to_string()))?;

    let mut pushed_merge_commit = None;
    for attempt in 1..=MAX_PUSH_ATTEMPTS {
        let merge_outcome = {
            let _guard = ctx.exclusive_worktree_access();
            merge_branch_into_target(
                ctx,
                &branch_name,
                &base_branch.remote_name,
                &target_branch_name,
            )
        }?;
        let merge_commit = match merge_outcome {
            MergeOutcome::AlreadyIntegrated { target_oid } => {
                writeln!(
                    progress,
                    "{} is already reachable from {} ({})",
                    t.local_branch.paint(&branch_name),
                    t.remote_branch.paint(&target_display),
                    t.hint.paint(short_id(ctx, target_oid)?)
                )?;
                break;
            }
            MergeOutcome::MergeCommit { oid } => oid,
        };

        writeln!(
            progress,
            "Pushing merge commit {} to {}...",
            t.hint.paint(short_id(ctx, merge_commit)?),
            t.remote_branch.paint(&target_display)
        )?;

        let push_result =
            push_merge_commit(ctx, merge_commit, &push_remote_name, &target_branch_name);
        match push_result {
            Ok(()) => {
                pushed_merge_commit = Some(merge_commit);
                break;
            }
            Err(err) if is_non_fast_forward_push_error(&err) && attempt < MAX_PUSH_ATTEMPTS => {
                writeln!(
                    progress,
                    "Target moved while pushing; fetching and retrying ({}/{})...",
                    attempt + 1,
                    MAX_PUSH_ATTEMPTS
                )?;
                but_api::legacy::virtual_branches::fetch_from_remotes(
                    ctx,
                    Some("integrate".to_string()),
                )?;
            }
            Err(err) if is_non_fast_forward_push_error(&err) => {
                bail!("Target branch kept moving; fetched and retried {MAX_PUSH_ATTEMPTS} times");
            }
            Err(err) => return Err(err.context("Failed to push merge commit to target branch")),
        }
    }

    if let Some(merge_commit) = pushed_merge_commit {
        writeln!(
            progress,
            "Pushed {} to {}. Fetching updated target...",
            t.hint.paint(short_id(ctx, merge_commit)?),
            t.remote_branch.paint(&target_display)
        )?;
        but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("integrate".to_string()))?;
    }

    ctx.invalidate_workspace_cache()?;

    crate::command::legacy::pull::integrate_active_branches_after_target_update(ctx, out).await?;

    if let Some(out) = out.for_human() {
        writeln!(out, "\n{}", t.success.paint("Integration complete!"))?;
    }

    Ok(())
}

enum MergeOutcome {
    AlreadyIntegrated { target_oid: gix::ObjectId },
    MergeCommit { oid: gix::ObjectId },
}

fn merge_branch_into_target(
    ctx: &Context,
    branch_name: &str,
    target_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<MergeOutcome> {
    let repo = ctx.repo.get()?;
    let feature_ref_name = format!("refs/heads/{branch_name}");
    let feature_oid = repo
        .try_find_reference(&feature_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Branch {branch_name} not found"))?
        .into_fully_peeled_id()?
        .detach();

    let target_ref_name = format!("refs/remotes/{target_remote_name}/{target_branch_name}");
    let target_oid = repo
        .try_find_reference(&target_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Target branch {target_ref_name} not found"))?
        .into_fully_peeled_id()?
        .detach();

    if repo
        .merge_base(feature_oid, target_oid)
        .map(|id| id.detach() == feature_oid)
        .unwrap_or(false)
    {
        return Ok(MergeOutcome::AlreadyIntegrated { target_oid });
    }

    let mut merge_result = repo.merge_commits(
        target_oid,
        feature_oid,
        gix::merge::blob::builtin_driver::text::Labels {
            ancestor: Some("base".into()),
            current: Some("target".into()),
            other: Some("branch".into()),
        },
        gix::merge::commit::Options::default(),
    )?;

    if merge_result
        .tree_merge
        .has_unresolved_conflicts(Default::default())
    {
        bail!(
            "Cannot integrate {branch_name}: merging into {target_remote_name}/{target_branch_name} resulted in conflicts"
        );
    }

    let commit_message = format!("Merge branch '{branch_name}'");
    let merge_commit = repo.new_commit(
        commit_message,
        merge_result.tree_merge.tree.write()?,
        vec![target_oid, feature_oid],
    )?;

    Ok(MergeOutcome::MergeCommit {
        oid: merge_commit.id().detach(),
    })
}

fn push_merge_commit(
    ctx: &Context,
    merge_commit: gix::ObjectId,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let push_remote_tracking_ref = format!("refs/remotes/{push_remote_name}/{target_branch_name}");
    let refspec = format!("{merge_commit}:refs/heads/{target_branch_name}");
    ctx.push(
        merge_commit,
        push_remote_tracking_ref,
        false,
        ctx.legacy_project.force_push_protection,
        Some(refspec),
        None,
        vec![],
    )?;
    Ok(())
}

fn confirm_direct_target_update(
    out: &mut OutputChannel,
    branch_name: &str,
    pr_number: Option<usize>,
    target_display: &str,
    yes: bool,
) -> anyhow::Result<()> {
    let prompt = if let Some(pr_number) = pr_number {
        format!(
            "Branch {branch_name} has PR #{pr_number} attached. This will skip merging on GitHub and directly push to {target_display}. Are you sure?"
        )
    } else {
        format!("This will directly push to {target_display}. Are you sure?")
    };

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", theme::get().attention.paint(&prompt))?;
    }

    if yes {
        return Ok(());
    }

    let Some(mut inout) = out.prepare_for_terminal_input() else {
        bail!(
            "Refusing to directly update {target_display} without confirmation. Re-run with --yes to confirm."
        );
    };

    if inout.confirm(&prompt, ConfirmDefault::No)? == Confirm::No {
        bail!("Integration cancelled");
    }

    Ok(())
}

fn pr_number_for_branch(ctx: &Context, branch_name: &str) -> anyhow::Result<Option<usize>> {
    Ok(but_api::legacy::virtual_branches::list_branches(ctx, None)?
        .into_iter()
        .find(|branch| branch.name.to_string() == branch_name)
        .and_then(|branch| branch.stack)
        .and_then(|stack| stack.pull_requests.get(branch_name).copied()))
}

fn is_non_fast_forward_push_error(err: &anyhow::Error) -> bool {
    let error = format!("{err:?}").to_lowercase();
    error.contains("non-fast-forward")
        || error.contains("fetch first")
        || error.contains("stale info")
        || error.contains("needs force")
        || error.contains("failed to push some refs")
}

fn short_id(ctx: &Context, oid: gix::ObjectId) -> anyhow::Result<String> {
    let repo = ctx.repo.get()?;
    Ok(shorten_object_id(&repo, oid))
}

fn remote_points_at_current_repo(ctx: &Context, remote_url: &str) -> anyhow::Result<bool> {
    if remote_url.contains("://") || remote_url.starts_with("git@") {
        return Ok(false);
    }

    let remote_path = Path::new(remote_url);
    let workdir = ctx.workdir_or_gitdir()?;
    let remote_path = if remote_path.is_absolute() {
        remote_path.to_path_buf()
    } else {
        workdir.join(remote_path)
    };

    let Ok(remote_path) = remote_path.canonicalize() else {
        return Ok(false);
    };
    let workdir = workdir.canonicalize()?;
    let git_dir = ctx.repo.get()?.git_dir().canonicalize()?;

    Ok(remote_path == workdir || remote_path == git_dir)
}
