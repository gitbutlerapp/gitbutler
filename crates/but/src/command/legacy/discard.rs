//! Implementation of the `but discard` command.
//!
//! This module provides functionality to discard uncommitted changes from the worktree.

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_api::diff;
use but_core::sync::RepoExclusive;
use but_ctx::Context;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};

use crate::{
    CliId, IdMap,
    id::parser::parse_uncommitted_sources,
    utils::{OutputChannel, diff_specs},
};

/// Handle the `but discard <id>` command.
///
/// Discards changes to files or hunks identified by the given ID.
/// The ID should be a file or hunk ID as shown in `but status`.
pub fn handle(ctx: &mut Context, out: &mut OutputChannel, id: &str) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    // Build ID map to resolve the user's ID
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    // Resolve the ID to get file information
    let resolved_ids = parse_uncommitted_sources(ctx, &id_map, id)
        .with_context(|| format!("Could not resolve ID '{id}'"))?;

    if resolved_ids.is_empty() {
        bail!("No entity found for the given ID");
    }

    // Get worktree changes once for the Uncommitted case.
    let worktree_changes = diff::changes_in_worktree_with_perm(ctx, true, guard.read_permission())?;

    // Extract DiffSpecs from all resolved entities.
    let diff_specs = {
        let context_lines = ctx.settings.context_lines;
        let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(guard.read_permission())?;
        let mut builder = diff_specs::DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);

        for resolved_id in resolved_ids {
            match resolved_id {
                CliId::UncommittedHunkOrFile(uncommitted) => {
                    builder.push_hunk_assignments(uncommitted.hunk_assignments)?;
                }
                CliId::PathPrefix {
                    id,
                    hunk_assignments,
                } => {
                    builder.push_changes_from_path_prefix(&id, &hunk_assignments)?;
                }
                CliId::Uncommitted { .. } => {
                    builder.push_hunk_assignments(worktree_changes.assignments.clone())?;
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
                CliId::Stack { .. } => {
                    bail!("Cannot discard a stack. Use a file or hunk ID instead.");
                }
            }
        }

        builder.into_diff_specs()
    };

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
    create_snapshot(
        ctx,
        OperationKind::Discard,
        &file_names,
        guard.write_permission(),
    );

    // Perform the discard operation
    let repo = ctx.repo.get()?;
    let dropped = but_workspace::discard_workspace_changes(
        &repo,
        diff_specs.clone(),
        ctx.settings.context_lines,
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
fn create_snapshot(
    ctx: &mut Context,
    operation: OperationKind,
    file_names: &[String],
    perm: &mut RepoExclusive,
) {
    use gitbutler_oplog::entry::Trailer;

    // Create trailers with file names
    let trailers = file_names.iter().cloned().map(Trailer::File);

    let details = SnapshotDetails::new(operation).with_trailers(trailers);
    let _snapshot = ctx.create_snapshot(details, perm).ok();
}
