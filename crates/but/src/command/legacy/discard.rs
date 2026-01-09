//! Implementation of the `but discard` command.
//!
//! This module provides functionality to discard uncommitted changes from the worktree.

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_api::legacy::diff;
use but_core::DiffSpec;
use but_ctx::Context;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};

use crate::{CliId, IdMap, command::legacy::rub::parse_sources, utils::OutputChannel};

/// Handle the `but discard <id>` command.
///
/// Discards changes to files or hunks identified by the given ID.
/// The ID should be a file or hunk ID as shown in `but status`.
pub fn handle(ctx: &mut Context, out: &mut OutputChannel, id: &str) -> Result<()> {
    // Build ID map to resolve the user's ID
    let mut id_map = IdMap::new_from_context(ctx, None)?;
    id_map.add_committed_file_info_from_context(ctx)?;

    // Resolve the ID to get file information
    let resolved_ids = parse_sources(ctx, &id_map, id)
        .with_context(|| format!("Could not resolve ID '{}'", id))?;

    if resolved_ids.is_empty() {
        bail!("No entity found for the given ID");
    }

    // Get worktree changes once for the Unassigned case
    // Also used to determine file status for additions/deletions
    let worktree_changes = diff::changes_in_worktree(ctx)?;
    let path_status: std::collections::HashMap<_, _> = worktree_changes
        .worktree_changes
        .changes
        .iter()
        .map(|change| (change.path.as_bstr(), &change.status))
        .collect();

    // Extract DiffSpecs from all resolved entities
    let mut diff_specs: Vec<DiffSpec> = Vec::new();

    for resolved_id in resolved_ids {
        match resolved_id {
            CliId::Uncommitted(uncommitted) => {
                // Convert hunk assignments to DiffSpecs
                for assignment in uncommitted.hunk_assignments {
                    let is_addition_or_deletion = path_status
                        .get(assignment.path_bytes.as_bstr())
                        .map(|status| {
                            matches!(
                                status,
                                but_core::ui::TreeStatus::Addition { .. }
                                    | but_core::ui::TreeStatus::Deletion { .. }
                            )
                        })
                        .unwrap_or(false);

                    diff_specs.push(DiffSpec {
                        previous_path: None, // HunkAssignment doesn't track previous path (renames)
                        path: assignment.path_bytes,
                        // For additions/deletions, use empty hunk_headers to signal whole-file mode
                        hunk_headers: if is_addition_or_deletion {
                            Vec::new()
                        } else {
                            assignment.hunk_header.into_iter().collect()
                        },
                    });
                }
            }
            CliId::Unassigned { .. } => {
                // Discard all uncommitted changes
                let assignments = worktree_changes.assignments.clone();

                // Convert all assignments to DiffSpecs
                // For file additions and deletions, we must use whole-file mode (empty hunk_headers)
                for assignment in assignments {
                    let is_addition_or_deletion = path_status
                        .get(assignment.path_bytes.as_bstr())
                        .map(|status| {
                            matches!(
                                status,
                                but_core::ui::TreeStatus::Addition { .. }
                                    | but_core::ui::TreeStatus::Deletion { .. }
                            )
                        })
                        .unwrap_or(false);

                    diff_specs.push(DiffSpec {
                        previous_path: None,
                        path: assignment.path_bytes,
                        // For additions/deletions, use empty hunk_headers to signal whole-file mode
                        hunk_headers: if is_addition_or_deletion {
                            Vec::new()
                        } else {
                            assignment.hunk_header.into_iter().collect()
                        },
                    });
                }
            }
            CliId::Branch { .. } => {
                bail!("Cannot discard a branch. Use a file or hunk ID instead.");
            }
            CliId::Commit { .. } => {
                bail!("Cannot discard a commit. Use a file or hunk ID instead.");
            }
            CliId::CommittedFile { .. } => {
                bail!(
                    "Cannot discard a committed file. Use an uncommitted file or hunk ID instead."
                );
            }
        }
    }

    if diff_specs.is_empty() {
        bail!("No changes found for the given ID");
    }

    // Collect unique file names for the snapshot message
    let file_names: Vec<String> = {
        let mut names: std::collections::HashSet<String> = diff_specs
            .iter()
            .map(|spec| spec.path.to_str_lossy().to_string())
            .collect();
        let mut names_vec: Vec<_> = names.drain().collect();
        names_vec.sort();
        names_vec
    };

    // Create a snapshot before performing discard operation
    // This allows the user to undo with `but undo` if needed
    create_snapshot(ctx, OperationKind::Discard, &file_names);

    // Perform the discard operation
    let repo = ctx.repo.get()?;
    let dropped = but_workspace::discard_workspace_changes(
        &repo,
        diff_specs.clone(),
        3, // context_lines - default value used by GUI
    )?;

    // Report results
    if !dropped.is_empty()
        && let Some(out) = out.for_human()
    {
        writeln!(
            out,
            "Warning: Some changes could not be discarded (possibly already discarded or modified):"
        )?;
        for spec in &dropped {
            writeln!(out, "  {}", spec.path.as_bstr())?;
        }
    }

    let discarded_count = diff_specs.len() - dropped.len();
    if discarded_count > 0 {
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "Successfully discarded changes to {} {}",
                discarded_count,
                if discarded_count == 1 {
                    "item"
                } else {
                    "items"
                }
            )?;
        }
        if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "discarded": discarded_count,
                "failed": dropped.len(),
            }))?;
        }
    } else {
        if let Some(out) = out.for_human() {
            writeln!(out, "No changes were discarded.")?;
        }
        if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "discarded": 0,
                "failed": dropped.len(),
            }))?;
        }
    }

    Ok(())
}

/// Create a snapshot in the oplog before performing an operation
fn create_snapshot(ctx: &mut Context, operation: OperationKind, file_names: &[String]) {
    use gitbutler_oplog::entry::Trailer;

    let mut guard = ctx.exclusive_worktree_access();

    // Create trailers with file names
    let trailers: Vec<Trailer> = file_names
        .iter()
        .map(|name| Trailer {
            key: "file".to_string(),
            value: name.clone(),
        })
        .collect();

    let details = SnapshotDetails::new(operation).with_trailers(trailers);
    let _snapshot = ctx.create_snapshot(details, guard.write_permission()).ok();
}
