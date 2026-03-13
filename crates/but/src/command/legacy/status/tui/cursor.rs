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

    /// Moves the cursor to the next selectable jump-target line after the current cursor position.
    pub(super) fn move_next_section(&mut self, lines: &[StatusOutputLine], mode: &Mode) {
        if let Some((idx, _)) = lines
            .iter()
            .enumerate()
            .skip(self.0 + 1)
            .find(|(_, line)| is_jump_target_in_mode(line, mode))
        {
            self.0 = idx;
        }
    }

    /// Moves the cursor to the previous selectable jump-target line before the current cursor position.
    ///
    /// If the current line is inside a section (for example, a file or commit row), moving to the
    /// previous section skips the current section header and jumps to the section before it.
    pub(super) fn move_previous_section(&mut self, lines: &[StatusOutputLine], mode: &Mode) {
        let current_line_is_jump_target = lines
            .get(self.0)
            .is_some_and(|line| is_jump_target_in_mode(line, mode));

        let previous_jump_targets: Vec<usize> = lines
            .iter()
            .enumerate()
            .rev()
            .skip(lines.len() - self.0)
            .filter_map(|(idx, line)| is_jump_target_in_mode(line, mode).then_some(idx))
            .collect();

        let target_idx = if current_line_is_jump_target {
            previous_jump_targets.first().copied()
        } else {
            previous_jump_targets.get(1).copied()
        };

        if let Some(target_idx) = target_idx {
            self.0 = target_idx;
        }
    }
}

/// Returns true if a line is selectable and is a jump target in the given mode.
fn is_jump_target_in_mode(line: &StatusOutputLine, mode: &Mode) -> bool {
    is_selectable_in_mode(line, mode)
        && matches!(
            line.data,
            StatusOutputLineData::Branch { .. }
                | StatusOutputLineData::StagedChanges { .. }
                | StatusOutputLineData::UnstagedChanges { .. }
        )
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
        // its not possible to move the cursor in these modes
        Mode::InlineReword { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Cursor;
    use crate::{
        CliId,
        command::legacy::status::{
            output::{StatusOutputContent, StatusOutputLine, StatusOutputLineData},
            tui::Mode,
        },
    };

    fn line(data: StatusOutputLineData) -> StatusOutputLine {
        StatusOutputLine {
            connector: None,
            content: StatusOutputContent::Plain(Vec::new()),
            data,
        }
    }

    #[test]
    fn move_previous_section_skips_current_section_when_cursor_is_inside_it() {
        let lines = vec![
            line(StatusOutputLineData::UnstagedChanges {
                cli_id: Arc::new(CliId::Unassigned { id: "u0".into() }),
            }),
            line(StatusOutputLineData::UnstagedFile {
                cli_id: Arc::new(CliId::Unassigned { id: "u1".into() }),
            }),
            line(StatusOutputLineData::StagedChanges {
                cli_id: Arc::new(CliId::Unassigned { id: "s0".into() }),
            }),
            line(StatusOutputLineData::StagedFile {
                cli_id: Arc::new(CliId::Unassigned { id: "s1".into() }),
            }),
        ];

        let mut cursor = Cursor(3);
        cursor.move_previous_section(&lines, &Mode::Normal);

        assert_eq!(cursor, Cursor(0));
    }

    #[test]
    fn move_previous_section_moves_to_immediate_previous_when_already_on_section_header() {
        let lines = vec![
            line(StatusOutputLineData::UnstagedChanges {
                cli_id: Arc::new(CliId::Unassigned { id: "u0".into() }),
            }),
            line(StatusOutputLineData::UnstagedFile {
                cli_id: Arc::new(CliId::Unassigned { id: "u1".into() }),
            }),
            line(StatusOutputLineData::StagedChanges {
                cli_id: Arc::new(CliId::Unassigned { id: "s0".into() }),
            }),
        ];

        let mut cursor = Cursor(2);
        cursor.move_previous_section(&lines, &Mode::Normal);

        assert_eq!(cursor, Cursor(0));
    }

    #[test]
    fn move_previous_section_does_not_move_when_only_current_section_exists_above_cursor() {
        let lines = vec![
            line(StatusOutputLineData::UnstagedChanges {
                cli_id: Arc::new(CliId::Unassigned { id: "u0".into() }),
            }),
            line(StatusOutputLineData::UnstagedFile {
                cli_id: Arc::new(CliId::Unassigned { id: "u1".into() }),
            }),
        ];

        let mut cursor = Cursor(1);
        cursor.move_previous_section(&lines, &Mode::Normal);

        assert_eq!(cursor, Cursor(1));
    }
}
