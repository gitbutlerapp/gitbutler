//! Confirmation prompt and end-of-command reporting, kept honest about what actually happened.

use anyhow::bail;
use but_api::land::{BranchLandKind, BranchLandResult};
use but_ctx::Context;

use crate::{
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel},
};

/// Render the outcome of a land: the headline, a note when the remaining branches were left
/// un-reconciled, stale pull-request warnings for rebased siblings, and the undo caveat.
pub(super) fn report_land_result(
    out: &mut OutputChannel,
    ctx: &Context,
    result: &BranchLandResult,
    branch_names: &[String],
    target_display: &str,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let t = theme::get();

    if let Some(out) = out.for_human() {
        let already_integrated = |name: &String| result.already_integrated.contains(name);
        let headline = match result.landed {
            BranchLandKind::AlreadyIntegrated => {
                let verb = if branch_names.len() == 1 {
                    "was"
                } else {
                    "were"
                };
                format!(
                    "{} {verb} already on {target_display}.",
                    branch_names.join(", ")
                )
            }
            BranchLandKind::Updated { .. } => {
                let landed: Vec<&str> = branch_names
                    .iter()
                    .filter(|name| !already_integrated(name))
                    .map(String::as_str)
                    .collect();
                format!("Landed {} onto {target_display}.", landed.join(", "))
            }
        };
        let style = if result.reconcile_skipped {
            t.attention
        } else {
            t.success
        };
        writeln!(out, "\n{}", style.paint(headline))?;
        if matches!(result.landed, BranchLandKind::Updated { .. }) {
            for name in branch_names.iter().filter(|name| already_integrated(name)) {
                writeln!(
                    out,
                    "{}",
                    t.hint
                        .paint(format!("{name} was already on {target_display}."))
                )?;
            }
        }
        for name in &result.deleted_remote_branches {
            writeln!(
                out,
                "{}",
                t.hint
                    .paint(format!("Deleted {push_remote_name}/{name} (landed)"))
            )?;
        }
    }

    if result.reconcile_skipped {
        if let Some(out) = out.for_human() {
            // One message for both skip causes (the fetch tracking ref hadn't caught up, or
            // uncommitted changes blocked the rebase). Both are resolved by a later `but pull`, so
            // we don't assert a cause the user may not have.
            writeln!(
                out,
                "{}",
                t.attention.paint(
                    "The remaining branches were not updated onto the new target. Run `but pull` \
                     to finish."
                )
            )?;
        }
    } else {
        warn_stale_sibling_prs(ctx, out, result, branch_names)?;
    }

    if let BranchLandKind::Updated {
        prev_target_oid, ..
    } = &result.landed
    {
        print_undo_caveat(
            out,
            result.local_delivery,
            *prev_target_oid,
            push_remote_name,
            target_branch_name,
        )?;
    }

    Ok(())
}

/// Warn for each sibling branch that the reconcile actually rewrote and that still has an open pull
/// request: its pushed branch and PR are now stale. We never auto-force-push a sibling's PR branch —
/// we only tell the user it needs re-pushing.
///
/// "Actually rewrote" is the key: only branches whose head is a replacement commit are warned, so a
/// branch that wasn't rebased (or isn't applied in the workspace) is not falsely flagged.
fn warn_stale_sibling_prs(
    ctx: &Context,
    out: &mut OutputChannel,
    result: &BranchLandResult,
    landed_branches: &[String],
) -> anyhow::Result<()> {
    let rewritten: std::collections::HashSet<gix::ObjectId> = result
        .workspace
        .replaced_commits
        .values()
        .copied()
        .collect();
    if rewritten.is_empty() {
        return Ok(());
    }
    let Some(out) = out.for_human() else {
        return Ok(());
    };
    let branches = but_api::legacy::virtual_branches::list_branches(ctx, None)?;
    let t = theme::get();
    for branch in &branches {
        let name = branch.name.to_string();
        if landed_branches.contains(&name) || !rewritten.contains(&branch.head) {
            continue;
        }
        let pr = branch
            .stack
            .as_ref()
            .and_then(|stack| stack.pull_requests.get(&name).copied());
        if let Some(pr) = pr {
            writeln!(
                out,
                "{}",
                t.attention.paint(format!(
                    "Branch {name} (PR #{pr}) was rebased onto the new target — its pushed branch \
                     and pull request are now stale. Re-push it to update the PR."
                ))
            )?;
        }
    }
    Ok(())
}

/// Print an honest note about reversibility. `but undo` cannot un-push a real remote, so on that
/// path we point at the manual revert recipe instead of promising a clean undo.
fn print_undo_caveat(
    out: &mut OutputChannel,
    local_delivery: bool,
    prev_target_oid: gix::ObjectId,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let Some(out) = out.for_human() else {
        return Ok(());
    };
    let t = theme::get();
    if local_delivery {
        // The local-ref move is not captured by the oplog snapshot yet, so `but undo` rolls back
        // the branch reconcile but not the target move. Say so rather than over-promising.
        writeln!(
            out,
            "{}",
            t.hint.paint(format!(
                "`but undo` reverts the branch reconcile; to also move the local target back: \
                 git update-ref refs/heads/{target_branch_name} {prev_target_oid} && \
                 git update-ref refs/remotes/{push_remote_name}/{target_branch_name} {prev_target_oid}"
            ))
        )?;
    } else {
        writeln!(
            out,
            "{}",
            t.hint.paint(format!(
                "Pushed to {push_remote_name}/{target_branch_name}; `but undo` cannot un-push it. \
                 To revert the remote: git push --force-with-lease {push_remote_name} \
                 {prev_target_oid}:refs/heads/{target_branch_name} — or, if the branch is protected \
                 against force-pushes, revert with a new commit or via your forge instead."
            ))
        )?;
    }
    Ok(())
}

/// A branch about to land, with everything its tip publishes beyond its own segment.
pub(crate) struct Landing {
    /// The short name of the branch being landed.
    pub branch: String,
    /// What a `--whole-stack` land publishes below it; empty otherwise.
    pub lower: but_api::land::LowerStack,
}

/// The [`Landing`]s for `branch_names`, in landing order. The API re-derives and enforces what
/// lands; this is for display.
pub(crate) fn landings(
    ctx: &mut Context,
    branch_names: &[String],
    whole_stack: bool,
) -> anyhow::Result<Vec<Landing>> {
    branch_names
        .iter()
        .map(|branch| {
            // With --whole-stack the confirmation must disclose everything that will be published,
            // not just the branch the user named — including commits on segments that no longer
            // have a name.
            let lower = if whole_stack {
                but_api::land::lower_stack(ctx, branch)?
            } else {
                but_api::land::LowerStack::default()
            };
            Ok(Landing {
                branch: branch.clone(),
                lower,
            })
        })
        .collect()
}

/// What `landing` publishes beyond its own segment, e.g. "the 1 segment(s) below it (bottom)", or
/// `None` when it publishes only itself.
fn published_below(landing: &Landing) -> Option<String> {
    let lower = &landing.lower;
    let mut extras = Vec::new();
    if !lower.segments.is_empty() {
        extras.push(format!(
            "the {} segment(s) below it ({})",
            lower.segments.len(),
            lower.segments.join(", "),
        ));
    }
    if lower.unnamed_commits > 0 {
        extras.push(format!(
            "{} commit(s) on unnamed segments below it",
            lower.unnamed_commits,
        ));
    }
    (!extras.is_empty()).then(|| extras.join(" and "))
}

/// The warning shown before a direct target update: everything that will be published, and the
/// open pull requests that landing closes. Shared by the CLI prompt and the TUI confirmation.
pub(crate) fn direct_target_update_warning(
    ctx: &Context,
    landings: &[Landing],
    target_display: &str,
) -> anyhow::Result<String> {
    let subject = match landings {
        [landing] => match published_below(landing) {
            None => landing.branch.clone(),
            Some(below) => format!("{} — together with {below} —", landing.branch),
        },
        _ => {
            let branches = landings
                .iter()
                .map(|landing| match published_below(landing) {
                    None => landing.branch.clone(),
                    Some(below) => format!("{} (together with {below})", landing.branch),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} branches — {branches} —", landings.len())
        }
    };
    let action = format!(
        "This lands {subject} directly onto {target_display} without a pull request — \
         skipping any code review, CI checks, or branch protections your team may rely on."
    );

    let attached_prs = attached_pr_numbers(ctx, landings)?;
    if attached_prs.is_empty() {
        return Ok(action);
    }
    let prs = attached_prs
        .iter()
        .map(|(name, pr)| format!("{name} (PR #{pr})"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "{action} This closes open pull request(s) {prs}: their remote branches are deleted after \
         landing."
    ))
}

/// Confirm a direct target update. For a whole-stack land, each landing's lower stack describes
/// everything else that will be published — named segments and commits on unnamed segments alike —
/// so the user confirms the full set. The warning is always printed; `--yes` only skips the
/// interactive prompt, never the warning.
pub(super) fn confirm_direct_target_update(
    out: &mut OutputChannel,
    ctx: &Context,
    landings: &[Landing],
    target_display: &str,
    yes: bool,
) -> anyhow::Result<()> {
    let warning = direct_target_update_warning(ctx, landings, target_display)?;
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", theme::get().attention.paint(&warning))?;
    }

    if yes {
        return Ok(());
    }

    let Some(mut inout) = out.prepare_for_terminal_input() else {
        bail!(
            "Refusing to directly update {target_display} without confirmation. Re-run with --yes to confirm."
        );
    };

    let question = match landings {
        [landing] if published_below(landing).is_none() => {
            format!("Land {} directly onto {target_display}?", landing.branch)
        }
        [landing] => format!(
            "Land {} and everything below it directly onto {target_display}?",
            landing.branch
        ),
        _ => format!(
            "Land these {} branches directly onto {target_display}?",
            landings.len()
        ),
    };
    if inout.confirm(question, ConfirmDefault::Yes)? == Confirm::No {
        bail!("Land cancelled");
    }

    Ok(())
}

/// The open pull-request numbers attached to the landed branches or any of the segments landing
/// with them. Landing deletes each landed branch's remote copy, which closes the attached reviews
/// on the forge, so all are surfaced.
///
/// Only applied workspace segments can land, so this reads the forge review associations that
/// `head_info` projects onto the workspace segments instead of computing the repository-wide
/// branch listing. Only open reviews are surfaced: a segment's number can also be settled
/// display identity (a landed review), which landing cannot close.
fn attached_pr_numbers(
    ctx: &Context,
    landings: &[Landing],
) -> anyhow::Result<Vec<(String, usize)>> {
    let landed = |name: &str| {
        landings.iter().any(|landing| {
            landing.branch == name || landing.lower.segments.iter().any(|lower| lower == name)
        })
    };
    let info = but_api::legacy::workspace::head_info(ctx)?;
    let open_reviews = but_api::legacy::forge::open_review_numbers(ctx)?;
    // A segment shared between stacks is listed once per stack; dedup so the warning doesn't
    // repeat it.
    let mut seen = std::collections::HashSet::new();
    Ok(info
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|segment| {
            let ref_name = &segment.ref_info.as_ref()?.ref_name;
            if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
                return None;
            }
            let name = ref_name.shorten();
            let name = std::str::from_utf8(name.as_ref()).ok()?;
            if !landed(name) {
                return None;
            }
            let pr = segment.metadata.as_ref()?.review.pull_request?;
            if !open_reviews.contains(&(pr as i64)) {
                return None;
            }
            Some((name.to_string(), pr))
        })
        .filter(|(name, _)| seen.insert(name.clone()))
        .collect())
}
