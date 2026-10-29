use anyhow::Context as _;
use but_core::WORKSPACE_REF_NAME;
use but_ctx::Context;
use gix::refs::{Category, transaction::PreviousValue};
use serde::Serialize;

use crate::{
    CliError, CliResult, bad_input,
    theme::{self, Paint},
    utils::{OutputChannel, shorten_object_id},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TeardownResult {
    snapshot_id: String,
    checked_out_branch: String,
    /// Hook cleanup problems; when non-empty, some GitButler hooks are still
    /// installed and may keep affecting git operations.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    hook_warnings: Vec<String>,
}

pub(crate) fn teardown(
    ctx: &mut Context,
    checkout_to: Option<String>,
    out: &mut OutputChannel,
) -> CliResult<()> {
    let t = theme::get();

    // Check that we're on gitbutler/workspace
    let head_name = {
        let repo = ctx.repo.get()?;
        let head = repo.head()?;
        head.referent_name()
            .map(|n| n.shorten().to_owned())
            .unwrap_or_default()
    };

    if !head_name.starts_with(b"gitbutler/") {
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{}",
                t.error.paint(format!(
                    "Not currently on gitbutler/workspace branch (on: {head_name})."
                ))
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                t.hint
                    .paint("Teardown can only be run while on the gitbutler/workspace branch.")
            )?;
        }
        return Err(bad_input("Not on gitbutler/workspace branch").into());
    }

    // Note: Validate checkout_to before snapshot creation to prevent unnecessary snapshot
    let checkout_to = if let Some(checkout_to) = &checkout_to {
        let repo = ctx.repo.get()?;
        let ref_name: gix::refs::PartialName = checkout_to.clone().try_into().map_err(|_| {
            CliError::from(
                bad_input(format!("Invalid ref name: {checkout_to}")).arg_name("--checkout-to"),
            )
        })?;
        let resolved_ref = match repo.try_find_reference(ref_name.as_ref())? {
            Some(resolved_ref) => resolved_ref,
            None => {
                return Err(
                    bad_input(format!("The reference '{checkout_to}' did not exist"))
                        .arg_name("--checkout-to")
                        .into(),
                );
            }
        };
        if !matches!(resolved_ref.name().category(), Some(Category::LocalBranch)) {
            return Err(bad_input(format!(
                "Invalid ref for checkout: '{checkout_to}' is not a local branch"
            ))
            .arg_name("--checkout-to")
            .into());
        }
        Some(resolved_ref.name().shorten().to_string())
    } else {
        None
    };

    if let Some(out) = out.for_human() {
        writeln!(out, "{}", t.progress.paint("Exiting GitButler mode..."))?;
        writeln!(out)?;
    }

    // Create an oplog snapshot
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", t.hint.paint("→ Creating snapshot..."))?;
    }

    let snapshot = but_api::legacy::oplog::create_snapshot(
        ctx,
        Some("Teardown: exiting GitButler mode".to_string()),
    )?;

    let snapshot_id = snapshot.to_string();

    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        let snapshot_short = shorten_object_id(&repo, snapshot);
        writeln!(
            out,
            "  {}",
            t.success
                .paint(format!("✓ Snapshot created: {snapshot_short}"))
        )?;
        writeln!(out)?;
    }

    // Find the first active branch (leftmost/lowest order)
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            t.hint.paint("→ Finding active branch to check out...")
        )?;
    }

    let stacks = match crate::legacy::workspace::applied_stacks(ctx) {
        Ok(stacks) => stacks,
        Err(_e) => {
            try_stack_fixes(ctx, out)?;
            crate::legacy::workspace::applied_stacks(ctx)
                .context("Failed to retrieve stacks after attempting fixes")?
        }
    };

    let target_branch_name = if let Some(checkout_to) = checkout_to {
        checkout_to
    } else {
        if let Some(target_stack) = stacks.first() {
            target_stack
                .branches
                .first()
                .map(|branch| branch.name.clone())
                .ok_or_else(|| anyhow::anyhow!("Stack has no branches"))?
        } else {
            return Err(bad_input(
                "Failed to determine checkout target branch. Specify a target branch with `--checkout-to <branch>`.",
            ).into());
        }
    };

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            t.success
                .paint(format!("✓ Will check out: {target_branch_name}"))
        )?;
        writeln!(out)?;
    }

    // Uninstall managed hooks before checking out. `uninstall_managed_hooks`
    // reports removal failures as PartialSuccess warnings rather than Err, so
    // surface those too -- they mean GitButler hooks are still active.
    let mut hook_warnings = Vec::new();
    if let Ok(repo) = ctx.repo.get() {
        hook_warnings = match gitbutler_repo::managed_hooks::uninstall_managed_hooks(&repo) {
            Ok(gitbutler_repo::managed_hooks::HookInstallationResult::PartialSuccess {
                warnings,
            }) => warnings,
            Ok(_) => Vec::new(),
            Err(e) => vec![format!("Failed to uninstall Git hooks: {e}")],
        };
        if let Some(out) = out.for_human() {
            for problem in &hook_warnings {
                writeln!(
                    out,
                    "  {}",
                    t.attention.paint(format!("Warning: {problem}"))
                )?;
            }
        }
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            t.hint
                .paint(format!("→ Checking out {target_branch_name}..."))
        )?;
    }

    let mut guard = ctx.exclusive_worktree_access();
    let hook_result = {
        let repo = ctx.repo.get()?;
        let branch_name = Category::LocalBranch.to_full_name(target_branch_name.as_str())?;
        let target = repo
            .find_reference(branch_name.as_ref())?
            .peel_to_commit()?;
        let old_head = repo.head_id()?.detach();
        let baseline = repo.index_from_tree(&repo.head_tree_id()?)?;
        let mut target_index = repo.index_from_tree(&target.tree_id()?)?;
        let checked_out = {
            // Lock before worktree writes so another index writer cannot leave a partial checkout.
            let mut index_lock = gix::lock::File::acquire_to_update_resource(
                repo.index_path(),
                gix::lock::acquire::Fail::Immediately,
                None,
                0,
            )
            .context("Could not lock the index for checkout")?;
            let mut index = repo.index_or_empty()?.into_owned_or_cloned();
            match repo.checkout_index_with_baseline(
                &baseline,
                &mut target_index,
                &mut index,
                gix::progress::Discard,
                &Default::default(),
                gix::worktree::checkout::Options {
                    overwrite_ignored: true,
                    ..Default::default()
                },
            ) {
                Ok(outcome) => {
                    if outcome.index_changed {
                        let mut buffered = std::io::BufWriter::new(&mut index_lock);
                        index.write_to(&mut buffered, Default::default())?;
                        std::io::Write::flush(&mut buffered)?;
                        drop(buffered);
                        index_lock.commit()?;
                    }
                    true
                }
                Err(err)
                    if err
                        .downcast_any_ref::<gix::worktree::checkout::Conflict>()
                        .is_some() =>
                {
                    false
                }
                Err(err) => return Err(anyhow::Error::from(err).context("Checkout failed").into()),
            }
        };

        if !checked_out && let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                t.attention.paint("⚠ Checkout failed, trying soft reset...\n  ⚠ This will leave changes from multiple branches in your working directory.\n  ⚠ You will have to manually remove, stash or re-commit the changes.")
            )?;
        }
        let mut reflog_message = gix::bstr::BString::default();
        if checked_out {
            reflog_message.extend_from_slice(b"checkout: moving from ");
            reflog_message.extend_from_slice(&head_name);
            reflog_message.extend_from_slice(b" to ");
            reflog_message.extend_from_slice(target_branch_name.as_bytes());
        }
        repo.edit_reference(
            gix::refs::transaction::RefEdit::update(
                "HEAD".try_into()?,
                gix::refs::Target::Symbolic(branch_name),
                PreviousValue::Any,
                reflog_message,
            )
            .with_deref(false),
        )?;
        if checked_out {
            but_core::hooks::post_checkout(&repo, old_head, target.id)
        } else {
            Ok(())
        }
    };
    ctx.reload_repo_and_invalidate_workspace(guard.write_permission())?;
    hook_result?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "  {}",
            t.success
                .paint(format!("✓ Checked out: {target_branch_name}"))
        )?;
        writeln!(out)?;
    }

    // Final success message
    if let Some(out) = out.for_human() {
        if hook_warnings.is_empty() {
            writeln!(
                out,
                "{}",
                t.success.paint("✓ Successfully exited GitButler mode!")
            )?;
        } else {
            writeln!(
                out,
                "{}",
                t.attention.paint(
                    "⚠ Exited GitButler mode, but some GitButler hooks could not be removed (see warnings above)."
                )
            )?;
        }
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            t.hint
                .paint(format!("You are now on branch: {target_branch_name}"))
        )?;
        writeln!(out)?;
    }

    // Output JSON if requested
    if let Some(out) = out.for_json() {
        out.write_value(&TeardownResult {
            snapshot_id,
            checked_out_branch: target_branch_name,
            hook_warnings,
        })?;
    }

    Ok(())
}

// a call to get stacks failed, which could be because someone committed on top
// of gitbutler/workspace. Try to fix that.
fn try_stack_fixes(ctx: &Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    let t = theme::get();
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "\n{}",
            t.attention.paint("Attempting to fix workspace stacks...")
        )?;
    }

    // check if gitbutler/workspace is pointing at a commit that does not start with "GitButler Workspace Commit"
    let repo = ctx.repo.get()?;

    // Look for dangling commits on gitbutler/workspace
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            t.hint.paint("→ Checking for dangling commits...")
        )?;
    }

    let mut workspace_ref = repo.find_reference(WORKSPACE_REF_NAME)?;
    let workspace_commit = workspace_ref.peel_to_commit()?;

    // Get all commits on workspace that are not workspace commits
    let mut dangling_commits = Vec::new();
    let mut commit = workspace_commit;

    loop {
        let message = commit.message_raw()?;
        let message_str = String::from_utf8_lossy(message);
        let first_line = message_str.lines().next().unwrap_or("");

        // Stop when we hit a workspace commit or reach the base
        if first_line.starts_with("GitButler Workspace Commit") {
            break;
        } // This is a dangling commit
        dangling_commits.push(commit.clone());

        // Move to parent
        let parent_ids: Vec<_> = commit.parent_ids().collect();
        if parent_ids.is_empty() {
            break;
        }

        commit = repo.find_commit(parent_ids[0])?;
    }

    if dangling_commits.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "  {}", t.success.paint("✓ No dangling commits found"))?;
            writeln!(out)?;
        }
    } else {
        // soft reset to the first workspace commit
        let target_commit = commit.id();
        let target_short = shorten_object_id(&repo, target_commit);
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{}",
                t.hint
                    .paint(format!("→ Resetting gitbutler/workspace to {target_short}"))
            )?;
        }
        repo.reference(
            WORKSPACE_REF_NAME,
            target_commit,
            PreviousValue::Any,
            "soft resetting to GitButler workspace",
        )?;
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "  {}",
                t.success
                    .paint(format!("✓ gitbutler/workspace reset to {target_short}"))
            )?;
        }

        // Reverse to get chronological order (oldest first)
        dangling_commits.reverse();

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "\n  {}",
                t.attention.paint(format!(
                    "⚠ Non-GitButler created commits found.
  ⚠ Undoing these commits but keeping the changes in your working directory.
  ⚠ Uncommitted {} dangling commit(s):",
                    dangling_commits.len()
                ))
            )?;
            for commit in &dangling_commits {
                let message = String::from_utf8_lossy(commit.message_raw().unwrap_or((&[]).into()));
                let first_line = message.lines().next().unwrap_or("");
                writeln!(
                    out,
                    "    {}: {}",
                    shorten_object_id(&repo, commit.id()),
                    first_line
                )?;
            }
            writeln!(out)?;
        }
    }

    Ok(())
}
