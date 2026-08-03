use but_ctx::Context;
use serde::Serialize;

use crate::{
    CliId, IdMap,
    command::legacy::diff::show::Filter,
    id::{CommitId, CommittedFileId},
    utils::OutputChannel,
};

mod display;
mod show;

pub fn handle_tui(ctx: &mut Context, target_str: Option<&str>) -> anyhow::Result<()> {
    use crate::tui::diff_viewer::{DiffFileEntry, WorktreeFilter};

    let id_map = IdMap::legacy_new_from_context(ctx)?;

    let files = if let Some(entity) = target_str {
        let id = id_map
            .parse_using_context(entity, ctx)?
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No ID found for entity"))?;

        match id {
            CliId::UncommittedHunkOrFile(ref uncommitted_id) => {
                let filter = WorktreeFilter::Uncommitted(Box::new(uncommitted_id.clone()));
                DiffFileEntry::from_worktree(&id_map, Some(&filter))
            }
            CliId::PathPrefix { hunks, .. } => DiffFileEntry::from_hunks(&hunks),
            CliId::Uncommitted { .. } => {
                DiffFileEntry::from_worktree(&id_map, Some(&WorktreeFilter::UncommittedArea))
            }
            CliId::Stack { .. } => {
                DiffFileEntry::from_worktree(&id_map, Some(&WorktreeFilter::UncommittedArea))
            }
            CliId::CommittedFile {
                committed_file:
                    CommittedFileId {
                        commit_id, path, ..
                    },
                id: _,
            } => DiffFileEntry::from_commit(ctx, commit_id, Some(path))?,
            CliId::Commit {
                commit: CommitId { commit_id, .. },
                id: _,
            } => DiffFileEntry::from_commit(ctx, commit_id, None)?,
            CliId::Branch(branch) => DiffFileEntry::from_branch(ctx, branch.name)?,
        }
    } else {
        DiffFileEntry::from_worktree(&id_map, None)
    };

    if files.is_empty() {
        anyhow::bail!("No diffs to show.");
    }

    crate::tui::diff_viewer::run_diff_viewer(files)
}

pub fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    target_str: Option<&str>,
) -> anyhow::Result<()> {
    let id_map = IdMap::legacy_new_from_context(ctx)?;

    if let Some(entity) = target_str {
        let id = id_map
            .parse_using_context(entity, ctx)? // TODO: look up plain names
            .first() // TODO: handle ambiguity
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No ID found for entity"))?;

        match id {
            CliId::UncommittedHunkOrFile(id) => {
                show::worktree(id_map, out, Some(Filter::Uncommitted(id)))
            }
            CliId::PathPrefix { hunks, .. } => show::hunks(&hunks, out),
            CliId::Uncommitted { .. } => show::worktree(id_map, out, Some(Filter::UncommittedArea)),
            CliId::CommittedFile {
                committed_file:
                    CommittedFileId {
                        commit_id, path, ..
                    },
                id: _,
            } => show::commit(ctx, out, commit_id, Some(path)),
            CliId::Branch(branch) => show::branch(ctx, out, branch.name),
            CliId::Commit {
                commit: CommitId { commit_id: id, .. },
                id: _,
            } => show::commit(ctx, out, id, None),
            CliId::Stack { .. } => show::worktree(id_map, out, Some(Filter::UncommittedArea)),
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
