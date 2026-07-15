use bstr::BString;
use but_api::diff::ComputeLineStats;
use but_core::{UnifiedPatch, ref_metadata::StackId, unified_diff::DiffHunk};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;

use super::{
    JsonChange, JsonDiff, JsonDiffOutput, JsonHunk,
    display::{DiffDisplay, TreeChangeWithPatch},
};
use crate::{
    IdMap,
    id::{UncommittedHunkOrFile, WorktreeChange},
    utils::OutputChannel,
};

#[expect(clippy::large_enum_variant)]
pub(crate) enum Filter {
    UncommittedArea,
    Uncommitted(UncommittedHunkOrFile),
    Stack(StackId),
}

pub(crate) fn worktree(
    id_map: IdMap,
    out: &mut OutputChannel,
    filter: Option<Filter>,
) -> anyhow::Result<()> {
    let short_id_assignment_pairs: Vec<(&str, &HunkAssignment)> = id_map
        .uncommitted_hunks
        .iter()
        .filter(|(_, uncommitted_hunk)| {
            let a = &uncommitted_hunk.hunk_assignment;
            match &filter {
                None => true,
                Some(Filter::UncommittedArea) => a.stack_id.is_none(),
                Some(Filter::Uncommitted(id)) => {
                    if id.is_entire_file {
                        a.path_bytes == id.hunk_assignments.first().path_bytes
                    } else {
                        a.eq(id.hunk_assignments.first())
                    }
                }
                Some(Filter::Stack(stack_id)) => a.stack_id.as_ref() == Some(stack_id),
            }
        })
        .map(|(short_id, uncommitted_hunk)| (short_id.as_str(), &uncommitted_hunk.hunk_assignment))
        .collect();
    print_short_id_assignment_pairs(short_id_assignment_pairs, out)
}

pub(crate) fn hunk_assignments<'a>(
    hunk_assignments: impl IntoIterator<Item = &'a (String, HunkAssignment)>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let short_id_assignment_pairs: Vec<(&str, &HunkAssignment)> = hunk_assignments
        .into_iter()
        .map(|(short_id, hunk_assignment)| (short_id.as_str(), hunk_assignment))
        .collect();
    print_short_id_assignment_pairs(short_id_assignment_pairs, out)
}

pub(crate) fn worktree_change(
    ctx: &Context,
    out: &mut OutputChannel,
    change: WorktreeChange,
) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, change.name.as_ref())?;
    let tree_change = but_core::diff::worktree_changes(&wt_repo)?
        .changes
        .into_iter()
        .find(|candidate| candidate.path == change.path)
        .ok_or_else(|| anyhow::anyhow!("The linked-worktree change no longer exists"))?;
    let patch = tree_change.unified_patch(&wt_repo, ctx.settings.context_lines)?;
    let base_id = change.id.split(":#").next().unwrap_or(&change.id);
    let assignments = HunkAssignment::from_tree_change(&tree_change, patch)
        .into_iter()
        .enumerate()
        .filter(|(_, assignment)| {
            change
                .hunk_header
                .is_none_or(|header| assignment.hunk_header == Some(header))
        })
        .map(|(index, assignment)| {
            let id = if assignment.hunk_header.is_some() {
                format!("{base_id}:#{index}")
            } else {
                base_id.to_owned()
            };
            (id, assignment)
        })
        .collect::<Vec<_>>();
    print_short_id_assignment_pairs(
        assignments
            .iter()
            .map(|(id, assignment)| (id.as_str(), assignment))
            .collect(),
        out,
    )
}

pub(crate) fn committed_file(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit_id: gix::ObjectId,
    path: BString,
    id: String,
    selected_hunk: Option<but_core::HunkHeader>,
) -> anyhow::Result<()> {
    let result = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    let mut indexed_assignments = Vec::new();
    for change in result
        .diff_with_first_parent
        .into_iter()
        .filter(|change| change.path == path)
    {
        let core_change = but_core::TreeChange::from(change.clone());
        let patch = but_api::diff::tree_change_diffs(ctx, core_change.clone().into())?;
        indexed_assignments.extend(HunkAssignment::from_tree_change(&core_change, patch));
    }
    let base_id = id.split(":#").next().unwrap_or(&id);
    let assignments = indexed_assignments
        .into_iter()
        .enumerate()
        .filter(|(_, assignment)| {
            selected_hunk.is_none_or(|header| assignment.hunk_header == Some(header))
        })
        .map(|(index, assignment)| {
            let id = if assignment.hunk_header.is_some() {
                format!("{base_id}:#{index}")
            } else {
                base_id.to_owned()
            };
            (id, assignment)
        })
        .collect::<Vec<_>>();
    print_short_id_assignment_pairs(
        assignments
            .iter()
            .map(|(id, assignment)| (id.as_str(), assignment))
            .collect(),
        out,
    )
}

fn print_short_id_assignment_pairs<'a>(
    mut short_id_assignment_pairs: Vec<(&'a str, &'a HunkAssignment)>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    short_id_assignment_pairs.sort_by(|(_, a_assignment), (_, b_assignment)| {
        a_assignment
            .stack_id
            .cmp(&b_assignment.stack_id)
            .then_with(|| {
                a_assignment
                    .path_bytes
                    .cmp(&b_assignment.path_bytes)
                    .then_with(|| a_assignment.hunk_header.cmp(&b_assignment.hunk_header))
            })
    });

    if short_id_assignment_pairs.is_empty() {
        if let Some(json_out) = out.for_json() {
            let output = JsonDiffOutput { changes: vec![] };
            json_out.write_value(output)?;
        } else if let Some(out) = out.for_human_or_shell() {
            writeln!(out, "No diffs to show.")?;
        }
    } else if let Some(json_out) = out.for_json() {
        let changes: Vec<JsonChange> = short_id_assignment_pairs
            .into_iter()
            .map(|(short_id, assignment)| hunk_assignment_to_json(Some(short_id), assignment))
            .collect();

        let output = JsonDiffOutput { changes };
        json_out.write_value(output)?;
    } else if let Some(out) = out.for_human_or_shell() {
        for (short_id, assignment) in short_id_assignment_pairs {
            write!(out, "{}", assignment.print_diff(Some(short_id)))?;
        }
    }
    Ok(())
}

pub(crate) fn commit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    id: gix::ObjectId,
    path: Option<BString>,
) -> anyhow::Result<()> {
    let result = but_api::diff::commit_details(ctx, id, ComputeLineStats::No)?;

    if let Some(json_out) = out.for_json() {
        let changes: Vec<JsonChange> = result
            .diff_with_first_parent
            .into_iter()
            .filter(|change| path.as_ref().is_none_or(|p| p == &change.path))
            .map(|change| {
                let patch = but_api::diff::tree_change_diffs(ctx, change.clone().into())
                    .ok()
                    .flatten();
                tree_change_to_json(None, change.into(), patch)
            })
            .collect();

        let output = JsonDiffOutput { changes };
        json_out.write_value(output)?;
    } else if let Some(out) = out.for_human_or_shell() {
        for change in result.diff_with_first_parent {
            if path.as_ref().is_none_or(|p| p == &change.path) {
                let patch = but_api::diff::tree_change_diffs(ctx, change.clone().into())
                    .ok()
                    .flatten();
                let diff = TreeChangeWithPatch::new(change.into(), patch);
                write!(out, "{}", diff.print_diff(None))?;
            }
        }
    }
    Ok(())
}

pub(crate) fn branch(
    ctx: &Context,
    out: &mut OutputChannel,
    short_name: String,
) -> anyhow::Result<()> {
    let result = but_api::branch::branch_diff(ctx, short_name)?;

    if let Some(json_out) = out.for_json() {
        let changes: Vec<JsonChange> = result
            .changes
            .into_iter()
            .map(|change| {
                let patch = but_api::diff::tree_change_diffs(ctx, change.clone())
                    .ok()
                    .flatten();
                tree_change_to_json(None, change, patch)
            })
            .collect();

        let output = JsonDiffOutput { changes };
        json_out.write_value(output)?;
    } else if let Some(out) = out.for_human_or_shell() {
        for change in result.changes {
            let patch = but_api::diff::tree_change_diffs(ctx, change.clone())
                .ok()
                .flatten();

            let diff = TreeChangeWithPatch::new(change, patch);
            write!(out, "{}", diff.print_diff(None))?;
        }
    }
    Ok(())
}

// Helper functions for JSON conversion

fn hunk_assignment_to_json(id: Option<&str>, assignment: &HunkAssignment) -> JsonChange {
    let diff = if let (Some(diff_bytes), Some(header)) = (&assignment.diff, &assignment.hunk_header)
    {
        JsonDiff::Patch {
            hunks: vec![hunk_to_json_hunk(&DiffHunk {
                old_start: header.old_start,
                old_lines: header.old_lines,
                new_start: header.new_start,
                new_lines: header.new_lines,
                diff: diff_bytes.clone(),
            })],
            is_binary_to_text: false,
        }
    } else {
        // No detailed diff available
        JsonDiff::Patch {
            hunks: vec![],
            is_binary_to_text: false,
        }
    };

    JsonChange {
        id: id.map(str::to_string),
        path: assignment.path.clone(),
        status: "modified".to_owned(),
        old_path: None,
        diff,
    }
}

fn tree_change_to_json(
    id: Option<&str>,
    change: but_core::ui::TreeChange,
    patch: Option<UnifiedPatch>,
) -> JsonChange {
    use but_core::ui::TreeStatus;

    let (status, old_path) = match &change.status {
        TreeStatus::Addition { .. } => ("added", None),
        TreeStatus::Deletion { .. } => ("deleted", None),
        TreeStatus::Modification { .. } => ("modified", None),
        TreeStatus::Rename { previous_path, .. } => ("renamed", Some(previous_path.to_string())),
    };

    let diff = match patch {
        Some(UnifiedPatch::Binary) => JsonDiff::Binary,
        Some(UnifiedPatch::TooLarge { size_in_bytes }) => JsonDiff::TooLarge { size_in_bytes },
        Some(UnifiedPatch::Patch {
            hunks,
            is_result_of_binary_to_text_conversion,
            ..
        }) => JsonDiff::Patch {
            hunks: hunks.iter().map(hunk_to_json_hunk).collect(),
            is_binary_to_text: is_result_of_binary_to_text_conversion,
        },
        None => JsonDiff::Patch {
            hunks: vec![],
            is_binary_to_text: false,
        },
    };

    JsonChange {
        id: id.map(str::to_string),
        path: change.path_bytes.to_string(),
        status: status.to_owned(),
        old_path,
        diff,
    }
}

fn hunk_to_json_hunk(hunk: &DiffHunk) -> JsonHunk {
    use bstr::ByteSlice;

    JsonHunk {
        old_start: hunk.old_start,
        old_lines: hunk.old_lines,
        new_start: hunk.new_start,
        new_lines: hunk.new_lines,
        diff: hunk.diff.to_str_lossy().into_owned(),
    }
}
