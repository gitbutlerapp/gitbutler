use crate::{
    CliId,
    command::legacy::status::{
        FilesStatusFlag, StatusOutputLine,
        tui::{Mode, cursor},
    },
};

pub(super) fn find_line_by_short_id<'a>(
    query: &str,
    lines: &'a [StatusOutputLine],
    return_mode: &Mode,
    show_files_flag: FilesStatusFlag,
) -> Option<&'a StatusOutputLine> {
    if query.is_empty() {
        return None;
    }

    let mut matches = lines
        .iter()
        .filter(|line| prefix_match(query, line, return_mode, show_files_flag));

    let needle = matches.next()?;

    if matches.next().is_none()
        && let Some(id) = needle.data.cli_id()
        && short_id(id) == query
    {
        Some(needle)
    } else {
        None
    }
}

pub(super) fn prefix_match(
    query: &str,
    line: &StatusOutputLine,
    return_mode: &Mode,
    show_files_flag: FilesStatusFlag,
) -> bool {
    let Some(id) = line.data.cli_id() else {
        return false;
    };
    if !cursor::is_selectable_in_mode(line, return_mode, show_files_flag) {
        return false;
    }
    if query.is_empty() {
        true
    } else {
        short_id(id).starts_with(query)
    }
}

fn short_id(id: &CliId) -> &str {
    match id {
        CliId::UncommittedHunkOrFile(hunk) => &hunk.id,
        CliId::PathPrefix { id, .. }
        | CliId::CommittedFile { id, .. }
        | CliId::Branch { id, .. }
        | CliId::Commit { id, .. }
        | CliId::Uncommitted { id }
        | CliId::Stack { id, .. } => id,
    }
}
