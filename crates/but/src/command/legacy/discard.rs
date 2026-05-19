//! Implementation of the `but discard` command.
//!
//! This module provides functionality to discard uncommitted changes from the worktree.

use anyhow::{Context as _, Result, bail};
use bstr::{BStr, ByteSlice};
use but_api::diff;
use but_core::{DiffSpec, sync::RepoExclusive};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};

use crate::{CliId, IdMap, id::parser::parse_sources, utils::OutputChannel};

/// Handle the `but discard <id>` command.
///
/// Discards changes to files or hunks identified by the given ID.
/// The ID should be a file or hunk ID as shown in `but status`.
pub fn handle(ctx: &mut Context, out: &mut OutputChannel, id: &str) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    // Build ID map to resolve the user's ID
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    // Resolve the ID to get file information
    let resolved_ids =
        parse_sources(ctx, &id_map, id).with_context(|| format!("Could not resolve ID '{id}'"))?;

    if resolved_ids.is_empty() {
        bail!("No entity found for the given ID");
    }

    // Get worktree changes once for the Unassigned case
    // Also used to determine file status for additions/deletions
    let worktree_changes = diff::changes_in_worktree_with_perm(ctx, guard.read_permission())?;
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
                collect_diff_specs_from_assignments(
                    uncommitted.hunk_assignments,
                    &path_status,
                    &mut diff_specs,
                );
            }
            CliId::PathPrefix { .. } => todo!(),
            CliId::Unassigned { .. } => {
                // Discard all uncommitted changes
                let assignments = worktree_changes.assignments.clone();

                collect_diff_specs_from_assignments(assignments, &path_status, &mut diff_specs);
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

fn collect_diff_specs_from_assignments(
    assignments: impl IntoIterator<Item = HunkAssignment>,
    path_status: &std::collections::HashMap<&BStr, &but_core::ui::TreeStatus>,
    diff_specs: &mut Vec<DiffSpec>,
) {
    for assignment in assignments {
        let spec = assignment_to_diff_spec(assignment, path_status);
        diff_specs.push(spec);
    }
}

fn assignment_to_diff_spec(
    assignment: HunkAssignment,
    path_status: &std::collections::HashMap<&BStr, &but_core::ui::TreeStatus>,
) -> DiffSpec {
    let (is_addition_or_deletion, previous_path) = path_status
        .get(assignment.path_bytes.as_bstr())
        .map(|status| match status {
            but_core::ui::TreeStatus::Addition { .. } => (true, None),
            but_core::ui::TreeStatus::Deletion { .. } => (true, None),
            but_core::ui::TreeStatus::Modification { .. } => (false, None),
            but_core::ui::TreeStatus::Rename {
                previous_path_bytes,
                ..
            } => (false, Some(previous_path_bytes.to_owned())),
        })
        .unwrap_or((false, None));

    DiffSpec {
        previous_path,
        path: assignment.path_bytes,
        // For additions/deletions, use empty hunk_headers to signal whole-file mode.
        hunk_headers: if is_addition_or_deletion {
            Vec::new()
        } else {
            assignment.hunk_header.into_iter().collect()
        },
    }
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bstr::{BStr, BString};
    use but_core::HunkHeader;
    use but_hunk_assignment::HunkAssignment;

    use super::{assignment_to_diff_spec, collect_diff_specs_from_assignments};

    #[test]
    fn assignment_to_diff_spec_keeps_hunk_for_non_filewide_case() {
        let assignment = make_assignment("a.txt", Some(hunk(1, 2, 3, 4)));
        let path_status: HashMap<&BStr, &but_core::ui::TreeStatus> = HashMap::new();

        let spec = assignment_to_diff_spec(assignment, &path_status);

        assert_eq!(spec.path, BString::from("a.txt"));
        assert_eq!(spec.hunk_headers, vec![hunk(1, 2, 3, 4)]);
        assert!(spec.previous_path.is_none());
    }

    #[test]
    fn collect_diff_specs_from_assignments_preserves_input_granularity() {
        let assignments = vec![
            make_assignment("a.txt", Some(hunk(1, 2, 3, 4))),
            make_assignment("a.txt", Some(hunk(5, 6, 7, 8))),
            make_assignment("b.txt", Some(hunk(9, 1, 10, 1))),
        ];
        let path_status: HashMap<&BStr, &but_core::ui::TreeStatus> = HashMap::new();
        let mut diff_specs = Vec::new();

        collect_diff_specs_from_assignments(assignments, &path_status, &mut diff_specs);

        assert_eq!(diff_specs.len(), 3);

        assert_eq!(diff_specs[0].path, BString::from("a.txt"));
        assert_eq!(diff_specs[0].hunk_headers, vec![hunk(1, 2, 3, 4)]);

        assert_eq!(diff_specs[1].path, BString::from("a.txt"));
        assert_eq!(diff_specs[1].hunk_headers, vec![hunk(5, 6, 7, 8)]);

        assert_eq!(diff_specs[2].path, BString::from("b.txt"));
        assert_eq!(diff_specs[2].hunk_headers, vec![hunk(9, 1, 10, 1)]);
    }

    #[test]
    fn collect_diff_specs_from_assignments_keeps_filewide_and_hunk_entries() {
        let assignments = vec![
            make_assignment("a.txt", Some(hunk(1, 2, 3, 4))),
            make_assignment("a.txt", None),
        ];
        let path_status: HashMap<&BStr, &but_core::ui::TreeStatus> = HashMap::new();
        let mut diff_specs = Vec::new();

        collect_diff_specs_from_assignments(assignments, &path_status, &mut diff_specs);

        assert_eq!(diff_specs.len(), 2);
        assert_eq!(diff_specs[0].hunk_headers, vec![hunk(1, 2, 3, 4)]);
        assert!(diff_specs[1].hunk_headers.is_empty());
    }

    fn make_assignment(path: &str, hunk_header: Option<HunkHeader>) -> HunkAssignment {
        HunkAssignment {
            id: None,
            hunk_header,
            path: path.to_string(),
            path_bytes: BString::from(path),
            stack_id: None,
            branch_ref_bytes: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        }
    }

    fn hunk(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> HunkHeader {
        HunkHeader {
            old_start,
            old_lines,
            new_start,
            new_lines,
        }
    }
}
