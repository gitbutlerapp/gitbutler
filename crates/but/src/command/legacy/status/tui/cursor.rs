use std::sync::Arc;

use crate::{
    CliId,
    command::legacy::status::{
        StatusOutputLine,
        output::StatusOutputLineData,
        tui::{Mode, commit_operation_display},
    },
};

#[cfg(test)]
mod tests;

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

    #[cfg(test)]
    pub(super) fn index(self) -> usize {
        self.0
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

    pub(super) fn select_commit(
        object_id: gix::ObjectId,
        lines: &[StatusOutputLine],
    ) -> Option<Self> {
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

    /// Select the first line that points to the given branch name.
    pub(super) fn select_branch(branch_name: String, lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::Branch { name, .. }) = line.data.cli_id().map(|id| &**id)
                && *name == branch_name
            {
                true
            } else {
                false
            }
        })?;
        Some(Self(idx))
    }

    /// Select the first line that points to the unassigned section.
    pub(super) fn select_unassigned(lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            matches!(
                line.data.cli_id().map(|id| &**id),
                Some(CliId::Unassigned { .. })
            )
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
        if self.0 >= lines.len() {
            return;
        }

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
        if self.0 >= lines.len() {
            return;
        }

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
        if self.0 >= lines.len() {
            return;
        }

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
        if self.0 >= lines.len() {
            return;
        }

        let current_line_is_section_header = lines.get(self.0).is_some_and(is_section_header);

        let previous_jump_targets: Vec<usize> = lines
            .iter()
            .enumerate()
            .rev()
            .skip(lines.len() - self.0)
            .filter_map(|(idx, line)| is_jump_target_in_mode(line, mode).then_some(idx))
            .collect();

        let target_idx = if current_line_is_section_header {
            previous_jump_targets.first().copied()
        } else {
            previous_jump_targets.get(1).copied()
        };

        if let Some(target_idx) = target_idx {
            self.0 = target_idx;
        }
    }
}

/// Returns true if a line is a section header row.
fn is_section_header(line: &StatusOutputLine) -> bool {
    matches!(
        line.data,
        StatusOutputLineData::Branch { .. }
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::UnstagedChanges { .. }
            | StatusOutputLineData::MergeBase
    )
}

/// Returns true if a line is selectable and is a jump target in the given mode.
fn is_jump_target_in_mode(line: &StatusOutputLine, mode: &Mode) -> bool {
    is_selectable_in_mode(line, mode) && is_section_header(line)
}

pub(super) fn is_selectable_in_mode(line: &StatusOutputLine, mode: &Mode) -> bool {
    if !line.is_selectable() {
        return false;
    }

    // selecting the source line should always be possible
    match mode {
        Mode::Rub(rub_mode) | Mode::RubButApi(rub_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && &rub_mode.source == cli_id
            {
                return true;
            }
        }
        Mode::Commit(commit_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && *commit_mode.source == **cli_id
            {
                return true;
            }
        }
        Mode::Command(..) | Mode::InlineReword(..) | Mode::Normal => {}
    }

    match mode {
        Mode::Normal => true,
        Mode::Rub(rub_mode) | Mode::RubButApi(rub_mode) => line
            .data
            .cli_id()
            .is_some_and(|cli_id| rub_mode.available_targets.contains(cli_id)),
        Mode::Commit(commit_mode) => commit_operation_display(&line.data, commit_mode).is_some(),
        Mode::InlineReword(..) | Mode::Command(..) => {
            // you can't actually move the selection in these modes
            // but returning `false` would dim every line which hurts UX
            true
        }
    }
}
