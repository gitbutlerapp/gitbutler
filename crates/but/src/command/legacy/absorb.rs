use crate::{
    id::{CommitId, CommitIdRef},
    theme::{self, Paint},
};
use bstr::ByteSlice as _;
use but_core::sync::RepoExclusive;
use but_ctx::Context;
use but_hunk_assignment::{
    AbsorptionTarget, CommitAbsorption, JsonAbsorbOutput, JsonCommitAbsorption, JsonFileAbsorption,
};
use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use itertools::Itertools;

use crate::{
    CliId, IdMap,
    id::{UncommittedHunkOrFile, parser::parse_sources},
    utils::{OutputChannel, merged_upstream::MergedUpstream},
};

/// Amends changes into the appropriate commits where they belong.
///
/// The semantic for finding "the appropriate commit" is as follows
/// - Changes are amended into the topmost commit of the leftmost (first) lane (branch)
/// - If a change is assigned to a particular lane (branch), it will be amended into a commit there
///     - If there are no commits in this branch, a new commit is created
/// - If a change has a dependency to a particular commit, it will be amended into that particular commit
///
/// Optionally an identifier to an Uncommitted File or a Branch (stack) may be provided.
///
/// If an Uncommitted File id is provided, absorb will be performed for just that file
/// If a Branch (stack) id is provided, absorb will be performed for all changes assigned to that stack
/// If no source is provided, absorb is performed for all uncommitted changes
pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source: Option<&str>,
    dry_run: bool,
    allow_merged: crate::args::atoms::AllowMergedArg,
) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let source: Option<CliId> = source
        .map(|s| -> anyhow::Result<CliId> {
            // Uncommitted selectors resolve in the uncommitted namespace first
            // so later commits cannot shadow them; branch selectors resolve in
            // the full namespace. A selector that names neither is an error —
            // silently falling back to absorbing everything is never intended.
            let resolved =
                match crate::id::parser::resolve_uncommitted_part(ctx, &id_map, s) {
                    Ok(ids) if !ids.is_empty() => ids,
                    _ => parse_sources(ctx, &id_map, s)?,
                };
            let mut acceptable = resolved.into_iter().filter(|id| {
                matches!(id, CliId::UncommittedHunkOrFile { .. })
                    || matches!(id, CliId::Branch(..))
            });
            let first = acceptable.next().ok_or_else(|| {
                anyhow::anyhow!(
                    "'{s}' does not name an uncommitted change or branch; refusing to absorb everything"
                )
            })?;
            if acceptable.next().is_some() {
                anyhow::bail!(
                    "'{s}' is ambiguous - it matches more than one uncommitted change. Use more characters to disambiguate."
                );
            }
            Ok(first)
        })
        .transpose()?;

    let target = if let Some(source) = source {
        match source {
            CliId::UncommittedHunkOrFile(UncommittedHunkOrFile { hunks, .. }) => {
                // Absorb this particular file
                AbsorptionTarget::Hunks {
                    hunks: hunks.map(|id_and_hunk| id_and_hunk.hunk).into(),
                }
            }
            CliId::Branch(branch) => {
                // Absorb everything that is assigned to this lane
                AbsorptionTarget::Branch {
                    branch_name: branch.name,
                }
            }
            _ => {
                anyhow::bail!("Invalid source: expected an uncommitted file or branch");
            }
        }
    } else {
        // Try to absorb everything uncommitted
        Default::default()
    };

    // TODO: Ideally, there's a simpler way of getting the worktree changes without passing the context to it.
    // At this time, the context is passed pretty deep into the function.
    let absorption_plan =
        but_api::legacy::absorb::absorption_plan_with_perm(ctx, target, guard.write_permission())?;

    // Absorbing into a landed commit entangles the new change with an entire
    // already-merged commit, which conflicts on the next `but pull`. Drop such
    // plan entries and tell the user, instead of silently amending them.
    let merged = MergedUpstream::from_ctx(ctx, allow_merged)?;
    let Some((absorption_plan, skipped_merged)) =
        drop_landed_absorptions(absorption_plan, &merged, out)?
    else {
        return Ok(());
    };

    // Display the plan (in JSON mode for non-dry-run, collect without writing — we'll
    // combine it with the result in absorb_assignments to avoid a double-write that
    // would overwrite the plan in the JSON buffer).
    let plan_json = display_absorption_plan(&absorption_plan, &id_map, out, dry_run)?;

    if dry_run {
        // Nothing more to do
        if let Some(out) = out.for_human() {
            let t = theme::get();
            let message = t.success.paint("Dry run complete. No changes were made.");
            writeln!(out, "{message}")?;
        }
        return Ok(());
    }

    // Create a snapshot before performing absorb or auto-commit operations
    // This allows the user to undo if needed
    let operation = OperationKind::Absorb;
    let _snapshot = ctx
        .create_snapshot(SnapshotDetails::new(operation), guard.write_permission())
        .ok(); // Ignore errors for snapshot creation
    absorb_assignments(
        ctx,
        absorption_plan,
        guard.write_permission(),
        out,
        plan_json,
        skipped_merged,
    )?;

    Ok(())
}

/// Absorb a single file into the appropriate commit
fn absorb_assignments(
    ctx: &mut Context,
    absorption_plan: Vec<CommitAbsorption>,
    perm: &mut RepoExclusive,
    out: &mut OutputChannel,
    plan_json: Option<JsonAbsorbOutput>,
    skipped_merged: Vec<String>,
) -> anyhow::Result<()> {
    let total_rejected = but_api::legacy::absorb::absorb_with_perm(ctx, absorption_plan, perm)?;
    // Refresh the workspace commit so `gitbutler/workspace` HEAD stays in sync
    // with the rewritten branch commits. Without this, tools that inspect HEAD
    // (e.g. pre-push hooks that stash against it) see a stale synthetic commit.
    update_workspace_commit(ctx, false)?;

    // Display completion message
    let t = theme::get();
    if let Some(out) = out.for_human() {
        writeln!(out)?;
        if total_rejected > 0 {
            writeln!(
                out,
                "{}: Failed to absorb {} file{}",
                t.attention.paint("Warning"),
                total_rejected,
                if total_rejected == 1 { "" } else { "s" }
            )?;
        }
        writeln!(
            out,
            "{}: you can run `but undo` to undo these changes",
            t.info.paint("Hint")
        )?;
    } else if let Some(out) = out.for_json() {
        // Combine plan and result into a single JSON write to avoid overwriting
        // the plan in the JSON buffer (which would lose absorption plan data).
        let mut combined = serde_json::json!({
            "ok": total_rejected == 0,
            "rejected": total_rejected,
        });
        if let Some(plan) = plan_json {
            combined["plan"] = serde_json::to_value(plan).unwrap_or(serde_json::Value::Null);
        }
        if !skipped_merged.is_empty() {
            combined["skippedMergedUpstream"] =
                serde_json::to_value(skipped_merged).unwrap_or(serde_json::Value::Null);
        }
        out.write_value(combined)?;
    }

    Ok(())
}

/// Format a hunk range for display
fn format_hunk_range(hunk_header: &but_core::HunkHeader) -> String {
    if hunk_header.old_lines == 0 {
        // New file or added lines only
        format!("+{},{}", hunk_header.new_start, hunk_header.new_lines)
    } else if hunk_header.new_lines == 0 {
        // Deleted lines only
        format!("-{},{}", hunk_header.old_start, hunk_header.old_lines)
    } else {
        // Modified lines
        format!(
            "@{},{} +{},{}",
            hunk_header.old_start,
            hunk_header.old_lines,
            hunk_header.new_start,
            hunk_header.new_lines
        )
    }
}

/// Get all hunk ranges for a file
fn get_hunk_ranges(hunk: &but_core::SingleHunk) -> Vec<String> {
    if let Some(hunk_header) = &hunk.hunk_header {
        vec![format_hunk_range(hunk_header)]
    } else {
        // Binary file or file too large - no hunk information
        vec!["(binary or large file)".to_string()]
    }
}

/// Display the absorption plan to the user.
///
/// When `write_json` is true (dry-run), writes JSON directly. When false (non-dry-run),
/// returns the plan data so the caller can combine it with the operation result
/// in a single JSON write — avoiding a double-write that would overwrite the buffer.
fn display_absorption_plan(
    commit_absorptions: &[CommitAbsorption],
    id_map: &IdMap,
    out: &mut OutputChannel,
    write_json: bool,
) -> anyhow::Result<Option<JsonAbsorbOutput>> {
    // Count total files
    let total_files: usize = commit_absorptions
        .iter()
        .flat_map(|c| c.hunks.iter().map(|h| &h.path))
        .unique()
        .count();

    // Handle empty case
    if commit_absorptions.is_empty() || total_files == 0 {
        let output = JsonAbsorbOutput {
            total_files: 0,
            commits: vec![],
        };
        if write_json && let Some(json_out) = out.for_json() {
            json_out.write_value(&output)?;
        }
        if let Some(out) = out.for_human() {
            writeln!(out, "No files to absorb")?;
        }
        return Ok(if write_json { None } else { Some(output) });
    }

    let json_commits: Vec<JsonCommitAbsorption> = commit_absorptions
        .iter()
        .map(|absorption| {
            let files: Vec<JsonFileAbsorption> = absorption
                .hunks
                .iter()
                .map(|hunk| JsonFileAbsorption {
                    path: hunk.path.to_str_lossy().into_owned(),
                    hunks: get_hunk_ranges(hunk),
                })
                .collect();

            JsonCommitAbsorption {
                commit_id: absorption.commit_id.to_hex().to_string(),
                commit_summary: absorption.commit_summary.clone(),
                reason: absorption.reason.clone(),
                reason_description: absorption.reason.description().to_string(),
                files,
            }
        })
        .collect();

    let plan_output = JsonAbsorbOutput {
        total_files,
        commits: json_commits,
    };

    if write_json && let Some(json_out) = out.for_json() {
        json_out.write_value(&plan_output)?;
    }

    let t = theme::get();
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Found {} changed file{} to absorb:",
            total_files,
            if total_files == 1 { "" } else { "s" }
        )?;
        writeln!(out)?;

        for absorption in commit_absorptions {
            writeln!(
                out,
                "Absorbed to commit: {} {}",
                theme::Commit(CommitIdRef {
                    commit_id: absorption.commit_id,
                    change_id: id_map
                        .change_id_ref(absorption.commit_id)
                        .map(|change_id| &change_id.change_id),
                }),
                absorption.commit_summary
            )?;
            writeln!(out, "  ({})", t.hint.paint(absorption.reason.description()))?;

            for hunk in &absorption.hunks {
                let ranges = get_hunk_ranges(hunk);
                let hunk_display = ranges.join(", ");

                writeln!(out, "    {} {}", hunk.path, t.hint.paint(&hunk_display))?;
            }
            writeln!(out)?;
        }
    }

    // When write_json is false (non-dry-run), return the plan so the caller can
    // combine it with the operation result in a single write_value call.
    Ok(if write_json { None } else { Some(plan_output) })
}

/// Drop plan entries that target commits already merged upstream, reporting
/// each skip. Returns the remaining plan and the skipped commit ids for the
/// final JSON output, or `None` when nothing is left to absorb — the outcome
/// has then been fully reported and the caller should stop before
/// snapshotting.
#[allow(clippy::type_complexity)]
fn drop_landed_absorptions(
    plan: Vec<CommitAbsorption>,
    merged: &MergedUpstream,
    out: &mut OutputChannel,
) -> anyhow::Result<Option<(Vec<CommitAbsorption>, Vec<String>)>> {
    let (skipped, plan): (Vec<_>, Vec<_>) = plan
        .into_iter()
        .partition(|absorption| merged.contains_commit(absorption.commit_id));
    let skipped_ids = || {
        skipped
            .iter()
            .map(|absorption| absorption.commit_id.to_string())
            .collect::<Vec<_>>()
    };
    if skipped.is_empty() {
        return Ok(Some((plan, Vec::new())));
    }

    if let Some(out) = out.for_human() {
        let t = theme::get();
        for absorption in &skipped {
            writeln!(
                out,
                "{}: not absorbing into {} {}: commit is merged upstream",
                t.attention.paint("Skipped"),
                theme::Commit(CommitId {
                    commit_id: absorption.commit_id,
                    change_id: None
                }),
                absorption.commit_summary,
            )?;
        }
        writeln!(
            out,
            "{}: most likely you want `but pull`, which removes landed work; in rare cases pass --allow-merged to absorb anyway",
            t.info.paint("Hint")
        )?;
    } else {
        // JSON output is parsed, so the warning goes to stderr.
        let ids = skipped
            .iter()
            .map(|absorption| absorption.commit_id.to_hex_with_len(7).to_string())
            .join(", ");
        eprintln!(
            "warning: skipped absorbing into {} merged-upstream commit(s): {ids}. Run `but pull` to update the workspace, or pass --allow-merged to absorb anyway.",
            skipped.len()
        );
    }

    if !plan.is_empty() {
        return Ok(Some((plan, skipped_ids())));
    }

    if let Some(out) = out.for_human() {
        writeln!(out, "Nothing left to absorb")?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "ok": false,
            "skippedMergedUpstream": skipped_ids(),
        }))?;
    }
    Ok(None)
}
