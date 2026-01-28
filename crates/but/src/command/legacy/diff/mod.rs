use but_ctx::Context;
use serde::Serialize;

use crate::{CliId, IdMap, command::legacy::diff::show::Filter, utils::OutputChannel};

mod display;
mod show;

// Note: To use the DiffDisplay trait in other modules,
// import it with: use crate::command::diff::display::DiffDisplay;

pub fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    target_str: Option<&str>,
) -> anyhow::Result<()> {
    let wt_changes = but_api::legacy::diff::changes_in_worktree(ctx)?;
    let id_map = IdMap::new_from_context(ctx, Some(wt_changes.assignments.clone()))?;

    if let Some(entity) = target_str {
        let id = id_map
            .parse_using_context(entity, ctx)? // TODO: look up plain names
            .first() // TODO: handle ambiguity
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No ID found for entity"))?;

        match id {
            CliId::Uncommitted(id) => show::worktree(id_map, out, Some(Filter::Uncommitted(id))),
            CliId::Unassigned { .. } => show::worktree(id_map, out, Some(Filter::Unassigned)),
            CliId::CommittedFile {
                commit_id, path, ..
            } => show::commit(ctx, out, commit_id, Some(path)),
            CliId::Branch { name, .. } => show::branch(ctx, out, name),
            CliId::Commit { commit_id: id, .. } => show::commit(ctx, out, id, None),
            CliId::Stack { id: _, stack_id } => {
                show::worktree(id_map, out, Some(Filter::Stack(stack_id)))
            }
        }
    } else {
        show::worktree(id_map, out, None)
    }
}

// JSON output structures

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDiffOutput {
    changes: Vec<JsonChange>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonChange {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    path: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_path: Option<String>,
    diff: JsonDiff,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum JsonDiff {
    Binary,
    TooLarge {
        size_in_bytes: u64,
    },
    Patch {
        hunks: Vec<JsonHunk>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        is_binary_to_text: bool,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonHunk {
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
    diff: String,
}
