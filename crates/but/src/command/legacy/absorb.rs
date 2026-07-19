use crate::theme::{self, Paint};
use but_core::{RepositoryExt, sync::RepoExclusive};
use but_ctx::Context;
use but_hunk_assignment::{
    AbsorptionTarget, CommitAbsorption, HunkAssignment, JsonAbsorbOutput, JsonCommitAbsorption,
    JsonFileAbsorption,
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
    utils::OutputChannel,
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
) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
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
                    || matches!(id, CliId::Branch { .. })
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
            CliId::UncommittedHunkOrFile(UncommittedHunkOrFile {
                hunk_assignments, ..
            }) => {
                // Absorb this particular file
                AbsorptionTarget::HunkAssignments {
                    assignments: hunk_assignments.into(),
                }
            }
            CliId::Branch { name, .. } => {
                // Absorb everything that is assigned to this lane
                AbsorptionTarget::Branch {
                    branch_name: name.clone(),
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

    // Display the plan (in JSON mode for non-dry-run, collect without writing — we'll
    // combine it with the result in absorb_assignments to avoid a double-write that
    // would overwrite the plan in the JSON buffer).
    let plan_json = {
        let repo = ctx.repo.get()?.clone().for_commit_shortening();
        display_absorption_plan(&absorption_plan, &id_map, &repo, out, dry_run)?
    };

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
fn get_hunk_ranges(assignment: &HunkAssignment) -> Vec<String> {
    if let Some(hunk_header) = &assignment.hunk_header {
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
    repo: &gix::Repository,
    out: &mut OutputChannel,
    write_json: bool,
) -> anyhow::Result<Option<JsonAbsorbOutput>> {
    // Count total files
    let total_files: usize = commit_absorptions
        .iter()
        .flat_map(|c| c.files.iter().map(|f| &f.path))
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
                .files
                .iter()
                .map(|file| {
                    let hunks = get_hunk_ranges(&file.assignment);

                    JsonFileAbsorption {
                        path: file.path.clone(),
                        hunks,
                    }
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
                theme::CommitRef(id_map, repo, absorption.commit_id),
                absorption.commit_summary
            )?;
            writeln!(out, "  ({})", t.hint.paint(absorption.reason.description()))?;

            for file in &absorption.files {
                let hunks = get_hunk_ranges(&file.assignment);
                let hunk_display = hunks.join(", ");

                writeln!(out, "    {} {}", file.path, t.hint.paint(&hunk_display))?;
            }
            writeln!(out)?;
        }
    }

    // When write_json is false (non-dry-run), return the plan so the caller can
    // combine it with the operation result in a single write_value call.
    Ok(if write_json { None } else { Some(plan_output) })
}
