use bstr::{BString, ByteSlice as _};
use but_api::diff::ComputeLineStats;
use but_core::{UnifiedPatch, unified_diff::DiffHunk};
use but_ctx::Context;

use super::{
    JsonChange, JsonDiff, JsonDiffOutput, JsonHunk,
    display::{DiffDisplay, TreeChangeWithPatch},
};
use crate::{
    IdMap,
    id::{IdAndHunk, UncommittedHunkOrFile},
    utils::OutputChannel,
};

pub(crate) enum Filter {
    UncommittedArea,
    Uncommitted(UncommittedHunkOrFile),
}

pub(crate) fn worktree(
    id_map: IdMap,
    out: &mut OutputChannel,
    filter: Option<Filter>,
) -> anyhow::Result<()> {
    let short_id_hunk_pairs: Vec<(&str, &but_core::SingleHunk)> = id_map
        .uncommitted_hunks
        .iter()
        .filter(|(_, uncommitted_hunk)| {
            let a = &uncommitted_hunk.hunk;
            match &filter {
                None => true,
                Some(Filter::UncommittedArea) => true,
                Some(Filter::Uncommitted(id)) => {
                    if id.is_entire_file {
                        a.path == id.hunks.first().hunk.path
                    } else {
                        a.identifies_same_hunk(&id.hunks.first().hunk)
                    }
                }
            }
        })
        .map(|(short_id, uncommitted_hunk)| (short_id.as_str(), &uncommitted_hunk.hunk))
        .collect();
    print_short_id_hunk_pairs(short_id_hunk_pairs, out)
}

pub(crate) fn hunks<'a>(
    hunks: impl IntoIterator<Item = &'a IdAndHunk>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let short_id_hunk_pairs: Vec<(&str, &but_core::SingleHunk)> = hunks
        .into_iter()
        .map(|id_and_hunk| (id_and_hunk.id.as_str(), &id_and_hunk.hunk))
        .collect();
    print_short_id_hunk_pairs(short_id_hunk_pairs, out)
}

fn print_short_id_hunk_pairs<'a>(
    mut short_id_hunk_pairs: Vec<(&'a str, &'a but_core::SingleHunk)>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    short_id_hunk_pairs.sort_by(|(_, a_hunk), (_, b_hunk)| {
        a_hunk
            .path
            .cmp(&b_hunk.path)
            .then_with(|| a_hunk.hunk_header.cmp(&b_hunk.hunk_header))
    });

    if short_id_hunk_pairs.is_empty() {
        if let Some(json_out) = out.for_json() {
            let output = JsonDiffOutput { changes: vec![] };
            json_out.write_value(output)?;
        } else if let Some(out) = out.for_human_or_shell() {
            writeln!(out, "No diffs to show.")?;
        }
    } else if let Some(json_out) = out.for_json() {
        let changes: Vec<JsonChange> = short_id_hunk_pairs
            .into_iter()
            .map(|(short_id, hunk)| hunk_to_json(Some(short_id), hunk))
            .collect();

        let output = JsonDiffOutput { changes };
        json_out.write_value(output)?;
    } else if let Some(out) = out.for_human_or_shell() {
        for (short_id, hunk) in short_id_hunk_pairs {
            write!(out, "{}", hunk.print_diff(Some(short_id)))?;
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

fn hunk_to_json(id: Option<&str>, hunk: &but_core::SingleHunk) -> JsonChange {
    let diff = if let (Some(diff_bytes), Some(header)) = (&hunk.diff, &hunk.hunk_header) {
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
        path: hunk.path.to_str_lossy().into_owned(),
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
