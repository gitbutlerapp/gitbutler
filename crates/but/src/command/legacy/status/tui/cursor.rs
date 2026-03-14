use std::sync::Arc;

use crate::{
    CliId,
    command::legacy::status::{StatusOutputLine, output::StatusOutputLineData, tui::Mode},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct Cursor(usize);

impl Cursor {
    pub(super) fn new(lines: &[StatusOutputLine]) -> Self {
        Self(
            lines
                .iter()
                .position(|line| line.is_selectable())
                .unwrap_or(0),
        )
    }

    pub(super) fn restore(selected_cli_id: &CliId, lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| {
                let cli_id = line.data.cli_id()?;
                Some((idx, cli_id))
            })
            .find_map(|(idx, cli_id)| {
                if &**cli_id == selected_cli_id {
                    Some(idx)
                } else {
                    None
                }
            })?;
        Some(Self(idx))
    }

    pub(super) fn select(object_id: gix::ObjectId, lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::Commit { commit_id, .. }) = line.data.cli_id().map(|id| &**id)
                && *commit_id == object_id
            {
                true
            } else {
                false
            }
        })?;
        Some(Self(idx))
    }

    pub(super) fn iter_lines(
        self,
        lines: &[StatusOutputLine],
    ) -> impl Iterator<Item = (&StatusOutputLine, bool)> {
        lines
            .iter()
            .enumerate()
            .map(move |(idx, line)| (line, self.0 == idx))
    }

    pub(super) fn selected_line(self, lines: &[StatusOutputLine]) -> Option<&StatusOutputLine> {
        lines.get(self.0)
    }

    pub(super) fn selection_cli_id_for_reload(
        self,
        lines: &[StatusOutputLine],
        show_files: bool,
    ) -> Option<&Arc<CliId>> {
        let selected_line = self.selected_line(lines)?;

        if matches!(&selected_line.data, StatusOutputLineData::File { .. }) && !show_files {
            return self.parent_cli_id_of_selected_file(lines);
        }

        selected_line.data.cli_id()
    }

    fn parent_cli_id_of_selected_file(self, lines: &[StatusOutputLine]) -> Option<&Arc<CliId>> {
        lines
            .iter()
            .take(self.0)
            .rev()
            .find_map(|line| match line.data {
                StatusOutputLineData::Commit { .. }
                | StatusOutputLineData::Branch { .. }
                | StatusOutputLineData::StagedChanges { .. }
                | StatusOutputLineData::UnstagedChanges { .. } => line.data.cli_id(),
                StatusOutputLineData::UpdateNotice
                | StatusOutputLineData::Connector
                | StatusOutputLineData::StagedFile { .. }
                | StatusOutputLineData::UnstagedFile { .. }
                | StatusOutputLineData::CommitMessage
                | StatusOutputLineData::EmptyCommitMessage
                | StatusOutputLineData::File { .. }
                | StatusOutputLineData::MergeBase
                | StatusOutputLineData::UpstreamChanges
                | StatusOutputLineData::Warning
                | StatusOutputLineData::Hint
                | StatusOutputLineData::NoAssignmentsUnstaged => None,
            })
    }

    pub(super) fn move_up(&mut self, lines: &[StatusOutputLine], mode: &Mode) {
        if let Some((idx, _)) = lines
            .iter()
            .enumerate()
            .rev()
            .skip(lines.len() - self.0)
            .find(|(_, line)| is_selectable_in_mode(line, mode))
        {
            self.0 = idx;
        }
    }

    pub(super) fn move_down(&mut self, lines: &[StatusOutputLine], mode: &Mode) {
        if let Some((idx, _)) = lines
            .iter()
            .enumerate()
            .skip(self.0 + 1)
            .find(|(_, line)| is_selectable_in_mode(line, mode))
        {
            self.0 = idx;
        }
    }
}

pub(super) fn is_selectable_in_mode(line: &StatusOutputLine, mode: &Mode) -> bool {
    match mode {
        Mode::Normal => line.is_selectable(),
        Mode::Rub {
            source: _,
            available_targets,
        } => {
            line.is_selectable()
                && line
                    .data
                    .cli_id()
                    .is_some_and(|cli_id| available_targets.contains(cli_id))
        }
    }
}
