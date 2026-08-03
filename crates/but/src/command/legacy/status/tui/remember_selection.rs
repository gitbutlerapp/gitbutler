use std::path::PathBuf;

use but_ctx::Context;

use crate::{
    CliId,
    command::legacy::status::{
        StatusOutputLine,
        tui::{app::App, cursor::Cursor},
    },
    id::{CommitId, CommittedFileId},
};

pub fn restore_selection(ctx: &Context, lines: &[StatusOutputLine]) -> Option<Cursor> {
    let cursor = (|| {
        let Ok(selection) = load_saved_selection_from_disk(ctx) else {
            return None;
        };
        let previous_selection = find_matching_line(&selection, lines)?;
        let id = previous_selection.data.cli_id()?;
        Cursor::restore(id, lines)
    })();

    if cursor.is_none() {
        _ = remove_saved_selection_from_disk(ctx);
    }

    cursor
}

fn load_saved_selection_from_disk(ctx: &Context) -> std::io::Result<String> {
    Ok(std::fs::read_to_string(path(ctx))?.to_owned())
}

fn remove_saved_selection_from_disk(ctx: &Context) -> std::io::Result<()> {
    std::fs::remove_file(path(ctx))?;
    Ok(())
}

pub fn save_selection_to_disk(ctx: &Context, app: &App) -> std::io::Result<()> {
    let Some(selection) = app
        .cursor
        .selected_line(&app.status_lines)
        .and_then(|s| s.data.cli_id())
    else {
        return Ok(());
    };
    let Some(id) = line_id(selection) else {
        return Ok(());
    };
    std::fs::write(path(ctx), &*id)?;
    Ok(())
}

fn find_matching_line<'a>(
    selection: &str,
    lines: &'a [StatusOutputLine],
) -> Option<&'a StatusOutputLine> {
    lines.iter().find(|line| {
        line.data
            .cli_id()
            .and_then(|id| line_id(id))
            .is_some_and(|id| id == selection)
    })
}

fn path(ctx: &Context) -> PathBuf {
    ctx.project_data_dir.join("tui-selection")
}

fn line_id(id: &CliId) -> Option<String> {
    Some(match id {
        CliId::UncommittedHunkOrFile(hunk) => {
            format!("hunk:{}", hunk.hunks.head.hunk.path)
        }
        CliId::Branch(branch) => format!("branch:{}", branch.name),
        CliId::CommittedFile {
            committed_file:
                CommittedFileId {
                    commit_id,
                    change_id,
                    ..
                },
            id: _,
        }
        | CliId::Commit {
            commit:
                CommitId {
                    commit_id,
                    change_id,
                    ..
                },
            id: _,
        } => {
            if let Some(change_id) = change_id {
                format!("commit:{change_id}")
            } else {
                format!("commit:{commit_id}")
            }
        }
        CliId::Uncommitted { id } => format!("uncommitted:{id}"),
        CliId::PathPrefix { .. } | CliId::Stack { .. } => return None,
    })
}
