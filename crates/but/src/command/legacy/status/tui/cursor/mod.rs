use std::sync::Arc;

use crate::{
    CliId,
    command::legacy::status::{
        FilesStatusFlag, StatusOutputLine,
        output::StatusOutputLineData,
        tui::{Mode, branch_operation_display, commit_operation_display, move_operation_display},
    },
};

#[cfg(test)]
mod tests;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[must_use]
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

    pub(super) fn select_first_file_in_commit(
        object_id: gix::ObjectId,
        lines: &[StatusOutputLine],
    ) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::CommittedFile { commit_id, .. }) = line.data.cli_id().map(|id| &**id)
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

    /// Selects the merge-base line.
    pub(super) fn select_merge_base(lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines
            .iter()
            .position(|line| matches!(line.data, StatusOutputLineData::MergeBase))?;
        Some(Self(idx))
    }

    pub(super) fn selected_line(self, lines: &[StatusOutputLine]) -> Option<&StatusOutputLine> {
        lines.get(self.0)
    }

    pub(super) fn selection_cli_id_for_reload(
        self,
        lines: &[StatusOutputLine],
        show_files: FilesStatusFlag,
    ) -> Option<&Arc<CliId>> {
        let selected_line = self.selected_line(lines)?;

        if matches!(selected_line.data, StatusOutputLineData::File { .. }) {
            let file_is_visible = match selected_line.data.cli_id().map(|id| &**id) {
                Some(CliId::CommittedFile { commit_id, .. }) => {
                    show_files.show_files_for(*commit_id)
                }
                Some(CliId::Uncommitted(..))
                | Some(CliId::PathPrefix { .. })
                | Some(CliId::Branch { .. })
                | Some(CliId::Commit { .. })
                | Some(CliId::Unassigned { .. })
                | Some(CliId::Stack { .. }) => matches!(show_files, FilesStatusFlag::All),
                None => false,
            };

            if !file_is_visible {
                return self.parent_cli_id_of_selected_file(lines);
            }
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

    pub(super) fn move_up(
        &mut self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) {
        if self.0 >= lines.len() {
            return;
        }

        if let Some((idx, _)) = lines
            .iter()
            .enumerate()
            .rev()
            .skip(lines.len() - self.0)
            .find(|(_, line)| is_selectable_in_mode(line, mode, show_files))
        {
            self.0 = idx;
        }
    }

    pub(super) fn move_down(
        &mut self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) {
        if self.0 >= lines.len() {
            return;
        }

        if let Some((idx, _)) = lines
            .iter()
            .enumerate()
            .skip(self.0 + 1)
            .find(|(_, line)| is_selectable_in_mode(line, mode, show_files))
        {
            self.0 = idx;
        }
    }

    /// Moves the cursor to the next selectable jump-target line after the current cursor position.
    pub(super) fn move_next_section(
        &mut self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) {
        if self.0 >= lines.len() {
            return;
        }

        if let Some((idx, _)) = lines
            .iter()
            .enumerate()
            .skip(self.0 + 1)
            .find(|(_, line)| is_jump_target_in_mode(line, mode, show_files))
        {
            self.0 = idx;
        }
    }

    /// Moves the cursor to the previous selectable jump-target line.
    ///
    /// If the cursor is inside a section (for example, on a file or commit row), this jumps to the
    /// current section header first. If the cursor is already on a section header, this jumps to the
    /// previous section header.
    pub(super) fn move_previous_section(
        &mut self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) {
        if self.0 >= lines.len() {
            return;
        }

        let current_line_is_section_header = lines.get(self.0).is_some_and(is_section_header);
        let search_end = if current_line_is_section_header {
            self.0
        } else {
            self.0 + 1
        };

        if let Some((target_idx, _)) = lines
            .iter()
            .enumerate()
            .take(search_end)
            .rev()
            .find(|(_, line)| is_jump_target_in_mode(line, mode, show_files))
        {
            self.0 = target_idx;
        }
    }

    /// Returns the cursor position for the closest branch based on the currently selected row.
    #[must_use]
    pub(super) fn move_to_closest_branch(self, lines: &[StatusOutputLine]) -> Option<Self> {
        if self.0 >= lines.len() {
            return None;
        }

        let selected_line = lines.get(self.0)?;

        if matches!(selected_line.data, StatusOutputLineData::MergeBase) {
            return Some(self);
        }

        let selected_cli_id = selected_line.data.cli_id().map(|id| &**id)?;

        let target_idx = match (&selected_line.data, selected_cli_id) {
            (StatusOutputLineData::Branch { .. }, CliId::Branch { .. }) => Some(self.0),
            (StatusOutputLineData::Commit { stack_id, .. }, CliId::Commit { .. }) => stack_id
                .and_then(|stack_id| branch_index_for_stack(lines, stack_id))
                .or_else(|| previous_branch_index(lines, self.0)),
            (_, CliId::CommittedFile { .. }) | (_, CliId::Commit { .. }) => {
                previous_branch_index(lines, self.0)
            }
            (StatusOutputLineData::StagedChanges { .. }, _)
            | (StatusOutputLineData::StagedFile { .. }, _) => selected_cli_id
                .stack_id()
                .and_then(|stack_id| branch_index_for_stack(lines, stack_id))
                .or_else(|| first_branch_index(lines)),
            _ => first_branch_index(lines),
        };

        target_idx.map(Self)
    }
}

/// Returns the index of the first branch line, if any.
fn first_branch_index(lines: &[StatusOutputLine]) -> Option<usize> {
    lines.iter().position(|line| {
        matches!(
            line.data.cli_id().map(|id| &**id),
            Some(CliId::Branch { .. })
        )
    })
}

/// Returns the index of the nearest preceding branch line before or at `from_idx`.
fn previous_branch_index(lines: &[StatusOutputLine], from_idx: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .take(from_idx + 1)
        .rev()
        .find(|(_, line)| {
            matches!(
                line.data.cli_id().map(|id| &**id),
                Some(CliId::Branch { .. })
            )
        })
        .map(|(idx, _)| idx)
}

/// Returns the index of the first branch line that belongs to `stack_id`.
fn branch_index_for_stack(
    lines: &[StatusOutputLine],
    stack_id: gitbutler_stack::StackId,
) -> Option<usize> {
    lines.iter().position(|line| {
        if let Some(CliId::Branch {
            stack_id: Some(branch_stack_id),
            ..
        }) = line.data.cli_id().map(|id| &**id)
        {
            *branch_stack_id == stack_id
        } else {
            false
        }
    })
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
fn is_jump_target_in_mode(
    line: &StatusOutputLine,
    mode: &Mode,
    show_files: FilesStatusFlag,
) -> bool {
    is_selectable_in_mode(line, mode, show_files) && is_section_header(line)
}

pub(super) fn is_selectable_in_mode(
    line: &StatusOutputLine,
    mode: &Mode,
    show_files: FilesStatusFlag,
) -> bool {
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
        Mode::Move(move_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && *move_mode.source == **cli_id
            {
                return true;
            }
        }
        Mode::Command(..) | Mode::InlineReword(..) | Mode::Normal | Mode::Branch => {}
    }

    match mode {
        Mode::Normal => match show_files {
            FilesStatusFlag::None | FilesStatusFlag::All => true,
            FilesStatusFlag::Commit(object_id) => {
                if let Some(cli_id) = line.data.cli_id()
                    && let CliId::CommittedFile { commit_id, .. } = &**cli_id
                {
                    object_id == *commit_id
                } else {
                    false
                }
            }
        },
        Mode::Rub(rub_mode) | Mode::RubButApi(rub_mode) => line
            .data
            .cli_id()
            .is_some_and(|cli_id| rub_mode.available_targets.contains(cli_id)),
        Mode::Commit(commit_mode) => commit_operation_display(&line.data, commit_mode).is_some(),
        Mode::Move(move_mode) => move_operation_display(&line.data, move_mode).is_some(),
        Mode::Branch => branch_operation_display(&line.data).is_some(),
        Mode::InlineReword(..) | Mode::Command(..) => {
            // you can't actually move the selection in these modes
            // but returning `false` would dim every line which hurts UX
            true
        }
    }
}
