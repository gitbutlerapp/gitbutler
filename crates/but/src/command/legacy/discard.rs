//! Implementation of the `but discard` command.
//!
//! This module provides functionality to discard uncommitted changes from the worktree.

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_core::DiffSpec;
use but_ctx::Context;

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

    // We only support discarding uncommitted files or hunks
    let first_id = resolved_ids
        .into_iter()
        .next()
        .context("No entity found for the given ID")?;

    // Extract DiffSpec from resolved entity
    let diff_specs: Vec<DiffSpec> = match first_id {
        CliId::Uncommitted(uncommitted) => {
            // Convert hunk assignments to DiffSpecs
            uncommitted
                .hunk_assignments
                .into_iter()
                .map(|assignment| {
                    DiffSpec {
                        previous_path: None, // HunkAssignment doesn't track previous path (renames)
                        path: assignment.path_bytes,
                        hunk_headers: assignment.hunk_header.into_iter().collect(),
                    }
                })
                .collect()
        }
        CliId::Unassigned { .. } => {
            bail!("Cannot discard the unassigned area. Use a specific file or hunk ID instead.");
        }
        CliId::Branch { .. } => {
            bail!("Cannot discard a branch. Use a file or hunk ID instead.");
        }
        CliId::Commit { .. } => {
            bail!("Cannot discard a commit. Use a file or hunk ID instead.");
        }
        CliId::CommittedFile { .. } => {
            bail!("Cannot discard a committed file. Use an uncommitted file or hunk ID instead.");
        }
    };

    if diff_specs.is_empty() {
        bail!("No changes found for the given ID");
    }

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
                if discarded_count == 1 { "item" } else { "items" }
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
