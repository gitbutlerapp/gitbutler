use std::sync::Arc;

use bstr::BStr;
use but_core::ref_metadata::StackId;

use crate::{
    CliId,
    command::legacy::status::{
        FilesStatusFlag, StatusOutputLine,
        output::StatusOutputLineData,
        tui::{
            CommitSource, Mode, NormalMode, PickUncommittedMode, SelectAfterReload,
            marking::{MarkClasses, Markable, Marks},
            render::{
                commit_operation_display, move_operation_display, reorder_operation_display,
                stack_operation_display,
            },
        },
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

    pub(super) fn select_closest_commit_source(
        self,
        lines: &[StatusOutputLine],
        source: &CommitSource,
    ) -> Option<Self> {
        lines
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                line.data
                    .cli_id()
                    .is_some_and(|cli_id| source.contains(cli_id))
            })
            .min_by_key(|(idx, _)| idx.abs_diff(self.0))
            .map(|(idx, _)| Self(idx))
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

    /// Selects what should be focused after discarding the currently selected commit.
    pub(super) fn select_after_discarded_commit(
        self,
        lines: &[StatusOutputLine],
    ) -> Option<SelectAfterReload> {
        if let Some(CliId::Commit { commit_id, .. }) = lines
            .get(self.0)
            .and_then(|line| line.data.cli_id())
            .map(|id| &**id)
        {
            self.select_after_discarded_commits(lines, &[*commit_id])
        } else {
            self.select_after_discarded_commits(lines, &[])
        }
    }

    /// Selects what should be focused after discarding marked items.
    pub(super) fn select_after_discarded_marks(
        self,
        lines: &[StatusOutputLine],
        discarded_marks: &Marks,
    ) -> Option<SelectAfterReload> {
        if self.0 >= lines.len() {
            return None;
        }

        if let Some(cli_id) = lines[self.0].data.cli_id() {
            let selected_is_discarded = Markable::try_from_cli_id(cli_id)
                .as_ref()
                .is_some_and(|markable| discarded_marks.contains(markable));

            if !selected_is_discarded {
                return Some(select_after_reload_for_cli_id(cli_id));
            }
        }

        for line in lines.iter().skip(self.0 + 1) {
            if is_discard_commit_boundary(line) {
                break;
            }

            let Some(cli_id) = line.data.cli_id() else {
                continue;
            };
            if !line.is_selectable() {
                continue;
            }
            if Markable::try_from_cli_id(cli_id)
                .as_ref()
                .is_some_and(|markable| discarded_marks.contains(markable))
            {
                continue;
            }

            return Some(select_after_reload_for_cli_id(cli_id));
        }

        for line in lines.iter().take(self.0).rev() {
            if is_discard_commit_boundary(line) {
                break;
            }

            let Some(cli_id) = line.data.cli_id() else {
                continue;
            };
            if !line.is_selectable() {
                continue;
            }
            if Markable::try_from_cli_id(cli_id)
                .as_ref()
                .is_some_and(|markable| discarded_marks.contains(markable))
            {
                continue;
            }

            return Some(select_after_reload_for_cli_id(cli_id));
        }

        for line in lines.iter().take(self.0 + 1).rev() {
            if let Some(cli_id) = line.data.cli_id()
                && is_discard_commit_boundary(line)
            {
                return Some(select_after_reload_for_cli_id(cli_id));
            }
        }

        if Self::select_uncommitted(lines).is_some() {
            return Some(SelectAfterReload::Uncommitted);
        }

        None
    }

    /// Selects what should be focused after discarding marked commits.
    pub(super) fn select_after_discarded_commits(
        self,
        lines: &[StatusOutputLine],
        discarded_commits: &[gix::ObjectId],
    ) -> Option<SelectAfterReload> {
        if self.0 >= lines.len() {
            return None;
        }

        if let Some(CliId::Commit { commit_id, .. }) = lines[self.0].data.cli_id().map(|id| &**id)
            && !discarded_commits.contains(commit_id)
        {
            return Some(SelectAfterReload::Commit(*commit_id));
        }

        for line in lines.iter().skip(self.0 + 1) {
            if is_discard_commit_boundary(line) {
                break;
            }

            if let Some(CliId::Commit { commit_id, .. }) = line.data.cli_id().map(|id| &**id)
                && !discarded_commits.contains(commit_id)
            {
                return Some(SelectAfterReload::Commit(*commit_id));
            }
        }

        for line in lines.iter().take(self.0).rev() {
            if is_discard_commit_boundary(line) {
                break;
            }

            if let Some(CliId::Commit { commit_id, .. }) = line.data.cli_id().map(|id| &**id)
                && !discarded_commits.contains(commit_id)
            {
                return Some(SelectAfterReload::Commit(*commit_id));
            }
        }

        for line in lines.iter().take(self.0 + 1).rev() {
            if let StatusOutputLineData::Branch { cli_id } = &line.data {
                return Some(SelectAfterReload::CliId(Arc::clone(cli_id)));
            }

            if is_discard_commit_boundary(line) {
                break;
            }
        }

        None
    }

    /// Selects what should be focused after discarding the currently selected branch.
    pub(super) fn select_after_discarded_branch(
        self,
        lines: &[StatusOutputLine],
    ) -> Option<SelectAfterReload> {
        if self.0 >= lines.len() {
            return None;
        }

        let Some(StatusOutputLineData::Branch { .. }) = lines.get(self.0).map(|line| &line.data)
        else {
            return None;
        };

        for line in lines.iter().skip(self.0 + 1) {
            if let Some(CliId::Branch { name, .. }) = line.data.cli_id().map(|id| &**id) {
                return Some(SelectAfterReload::Branch(name.clone()));
            }
        }

        for line in lines.iter().take(self.0).rev() {
            if let Some(CliId::Branch { name, .. }) = line.data.cli_id().map(|id| &**id) {
                return Some(SelectAfterReload::Branch(name.clone()));
            }
        }

        if Self::select_uncommitted(lines).is_some() {
            return Some(SelectAfterReload::Uncommitted);
        }

        None
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
    pub(super) fn select_branch(branch_name: &str, lines: &[StatusOutputLine]) -> Option<Self> {
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

    /// Select the first line that points to the given stack.
    pub(super) fn select_stack(stack_id: StackId, lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::Stack { stack_id: id, .. }) = line.data.cli_id().map(|id| &**id)
                && stack_id == *id
            {
                true
            } else {
                false
            }
        })?;
        Some(Self(idx))
    }

    /// Select the first uncommitted file line that points to the given path in the given stack.
    pub(super) fn select_uncommitted_file(
        path: &BStr,
        stack_id: Option<StackId>,
        lines: &[StatusOutputLine],
    ) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::UncommittedHunkOrFile(uncommitted)) =
                line.data.cli_id().map(|id| &**id)
            {
                let assignment = uncommitted.hunk_assignments.first();
                &**assignment.path_bytes == path && assignment.stack_id == stack_id
            } else {
                false
            }
        })?;
        Some(Self(idx))
    }

    /// Select the first line that points to the uncommitted section.
    pub(super) fn select_uncommitted(lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            matches!(
                line.data.cli_id().map(|id| &**id),
                Some(CliId::Uncommitted { .. })
            )
        })?;
        Some(Self(idx))
    }

    /// Select the merge-base line.
    pub(super) fn select_merge_base(lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines
            .iter()
            .position(|line| matches!(line.data, StatusOutputLineData::MergeBase))?;
        Some(Self(idx))
    }

    pub(super) fn selected_line(self, lines: &[StatusOutputLine]) -> Option<&StatusOutputLine> {
        lines.get(self.0)
    }

    /// Selects the previous selectable row and returns it as a reload target.
    ///
    /// Falls back to selecting the uncommitted section if there is no previous
    /// selectable row.
    pub(super) fn select_previous_cli_id_or_uncommitted(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> SelectAfterReload {
        self.move_up(lines, mode, show_files)
            .and_then(|cursor| cursor.selected_line(lines))
            .and_then(|line| line.data.cli_id().cloned())
            .map(SelectAfterReload::CliId)
            .unwrap_or(SelectAfterReload::Uncommitted)
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
                Some(CliId::UncommittedHunkOrFile(..))
                | Some(CliId::PathPrefix { .. })
                | Some(CliId::Branch { .. })
                | Some(CliId::Commit { .. })
                | Some(CliId::Uncommitted { .. })
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
                | StatusOutputLineData::UncommittedChanges { .. } => line.data.cli_id(),
                StatusOutputLineData::UpdateNotice
                | StatusOutputLineData::Connector
                | StatusOutputLineData::BetweenStacks
                | StatusOutputLineData::StagedFile { .. }
                | StatusOutputLineData::UncommittedFile { .. }
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

    #[must_use]
    pub(super) fn move_up(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> Option<Self> {
        if self.0 >= lines.len() {
            return None;
        }

        let (idx, _) = lines
            .iter()
            .enumerate()
            .rev()
            .skip(lines.len() - self.0)
            .find(|(idx, _)| is_cursor_selectable_at_index(*idx, lines, mode, show_files))?;
        Some(Self(idx))
    }

    #[must_use]
    pub(super) fn move_down(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> Option<Self> {
        if self.0 >= lines.len() {
            return None;
        }

        let (idx, _) = lines
            .iter()
            .enumerate()
            .skip(self.0 + 1)
            .find(|(idx, _)| is_cursor_selectable_at_index(*idx, lines, mode, show_files))?;
        Some(Self(idx))
    }

    #[must_use]
    pub(super) fn move_down_within_section(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> Option<Self> {
        if self.0 >= lines.len() {
            return None;
        }

        find_section_start_at_or_before(lines, mode, self.0)?;
        let next_section_start =
            find_next_section_start(lines, mode, self.0).unwrap_or(lines.len());

        let (idx, _) = lines
            .iter()
            .enumerate()
            .skip(self.0 + 1)
            .take(next_section_start.saturating_sub(self.0 + 1))
            .find(|(idx, _)| is_cursor_selectable_at_index(*idx, lines, mode, show_files))?;
        Some(Self(idx))
    }

    /// Moves the cursor to the first selectable row in the next section.
    #[must_use]
    pub(super) fn move_next_section(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> Option<Self> {
        if self.0 >= lines.len() {
            return None;
        }

        let mut next_section_start = find_next_section_start(lines, mode, self.0)?;
        loop {
            if let Some(idx) =
                first_selectable_in_section(lines, mode, show_files, next_section_start)
            {
                return Some(Self(idx));
            }

            next_section_start = find_next_section_start(lines, mode, next_section_start)?;
        }
    }

    /// Moves the cursor to the first selectable row in the previous section.
    ///
    /// If the cursor is inside a section, this jumps to that section's first selectable row first.
    /// If the cursor is already on that row, this jumps to the previous section's first selectable
    /// row.
    #[must_use]
    pub(super) fn move_previous_section(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> Option<Self> {
        if self.0 >= lines.len() {
            return None;
        }

        let current_section_start = find_section_start_at_or_before(lines, mode, self.0)?;

        if let Some(current_section_first_selectable) =
            first_selectable_in_section(lines, mode, show_files, current_section_start)
            && self.0 != current_section_first_selectable
        {
            return Some(Self(current_section_first_selectable));
        }

        let mut search_end = current_section_start;
        while let Some(previous_section_start) =
            find_previous_section_start(lines, mode, search_end)
        {
            if let Some(idx) =
                first_selectable_in_section(lines, mode, show_files, previous_section_start)
            {
                return Some(Self(idx));
            }

            search_end = previous_section_start;
        }

        None
    }
}

/// Finds the start index of the nearest section at or before `idx`.
fn find_section_start_at_or_before(
    lines: &[StatusOutputLine],
    mode: &Mode,
    idx: usize,
) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .take(idx + 1)
        .rev()
        .find(|(_, line)| is_section_header(line, mode))
        .map(|(idx, _)| idx)
}

/// Finds the next section start after `idx`.
fn find_next_section_start(lines: &[StatusOutputLine], mode: &Mode, idx: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(idx + 1)
        .find(|(_, line)| is_section_header(line, mode))
        .map(|(idx, _)| idx)
}

/// Finds the previous section start before `search_end`.
fn find_previous_section_start(
    lines: &[StatusOutputLine],
    mode: &Mode,
    search_end: usize,
) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .take(search_end)
        .rev()
        .find(|(_, line)| is_section_header(line, mode))
        .map(|(idx, _)| idx)
}

/// Finds the first selectable line in the section starting at `section_start`.
fn first_selectable_in_section(
    lines: &[StatusOutputLine],
    mode: &Mode,
    show_files: FilesStatusFlag,
    section_start: usize,
) -> Option<usize> {
    let next_section_start =
        find_next_section_start(lines, mode, section_start).unwrap_or(lines.len());

    lines
        .iter()
        .enumerate()
        .skip(section_start)
        .take(next_section_start.saturating_sub(section_start))
        .find(|(idx, _)| is_cursor_selectable_at_index(*idx, lines, mode, show_files))
        .map(|(idx, _)| idx)
}

fn select_after_reload_for_cli_id(cli_id: &Arc<CliId>) -> SelectAfterReload {
    match &**cli_id {
        CliId::Commit { commit_id, .. } => SelectAfterReload::Commit(*commit_id),
        CliId::Uncommitted { .. }
        | CliId::UncommittedHunkOrFile(..)
        | CliId::PathPrefix { .. }
        | CliId::CommittedFile { .. }
        | CliId::Branch { .. }
        | CliId::Stack { .. } => SelectAfterReload::CliId(Arc::clone(cli_id)),
    }
}

/// Returns true if a line marks the boundary of a commit list within a branch section.
fn is_discard_commit_boundary(line: &StatusOutputLine) -> bool {
    match &line.data {
        StatusOutputLineData::Branch { .. }
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::MergeBase => true,
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedFile { .. }
        | StatusOutputLineData::Commit { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => false,
    }
}

/// Returns true if a line is a section header row.
fn is_section_header(line: &StatusOutputLine, mode: &Mode) -> bool {
    match mode {
        Mode::Normal(..)
        | Mode::PickChanges(..)
        | Mode::InlineReword(..)
        | Mode::Command(..)
        | Mode::Commit(..)
        | Mode::Move(..)
        | Mode::Stack(..)
        | Mode::MoveStack(..)
        | Mode::Details(..) => {
            matches!(
                line.data,
                StatusOutputLineData::Branch { .. }
                    | StatusOutputLineData::UncommittedChanges { .. }
                    | StatusOutputLineData::MergeBase
            )
        }

        Mode::Rub(..) => {
            matches!(
                line.data,
                StatusOutputLineData::Branch { .. }
                    | StatusOutputLineData::StagedChanges { .. }
                    | StatusOutputLineData::UncommittedChanges { .. }
                    | StatusOutputLineData::MergeBase
            )
        }
    }
}

fn is_cursor_selectable_at_index(
    idx: usize,
    lines: &[StatusOutputLine],
    mode: &Mode,
    show_files_flag: FilesStatusFlag,
) -> bool {
    let Some(line) = lines.get(idx) else {
        return false;
    };

    is_selectable_in_mode(line, mode, show_files_flag)
        && !is_noop_move_stack_target(idx, lines, mode)
}

fn is_noop_move_stack_target(idx: usize, lines: &[StatusOutputLine], mode: &Mode) -> bool {
    let Mode::MoveStack(move_mode) = mode else {
        return false;
    };

    let Some(line) = lines.get(idx) else {
        return false;
    };
    if !matches!(line.data, StatusOutputLineData::BetweenStacks) {
        return false;
    }

    let current_stack_order = super::stack_ids_in_display_order(lines);
    let Some(source_index) = current_stack_order
        .iter()
        .position(|stack| *stack == move_mode.source.stack)
    else {
        return false;
    };

    let target_index = super::stack_ids_in_display_order(&lines[..idx]).len();
    target_index == source_index || target_index == source_index + 1
}

pub(super) fn is_selectable_in_mode(
    line: &StatusOutputLine,
    mode: &Mode,
    show_files_flag: FilesStatusFlag,
) -> bool {
    if !line.is_selectable() {
        if let Mode::MoveStack(..) = mode
            && let StatusOutputLineData::BetweenStacks = line.data
        {
            // `BetweenStacks` lines are selectable in reorder mode
        } else {
            return false;
        }
    }

    // selecting the source line should always be possible
    match mode {
        Mode::Rub(rub_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && rub_mode.source.contains(cli_id)
            {
                return true;
            }
        }
        Mode::Commit(commit_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && commit_mode.source.contains(cli_id)
            {
                return true;
            }
        }
        Mode::Move(move_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && move_mode.source.contains(cli_id)
            {
                return true;
            }
        }
        Mode::MoveStack(move_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && move_mode.source.matches(cli_id)
            {
                return true;
            }
        }
        Mode::Command(..)
        | Mode::InlineReword(..)
        | Mode::Normal(..)
        | Mode::PickChanges(..)
        | Mode::Details(..)
        | Mode::Stack(..) => {}
    }

    // don't allow mixing marks
    match mode {
        Mode::Normal(NormalMode { marks }) | Mode::PickChanges(PickUncommittedMode { marks }) => {
            if !marks.is_empty() {
                let MarkClasses {
                    marked_commits,
                    marked_uncommitted,
                } = marks.classify();
                if marked_commits
                    && !matches!(
                        &line.data,
                        StatusOutputLineData::Branch { .. } | StatusOutputLineData::Commit { .. }
                    )
                {
                    return false;
                }
                if marked_uncommitted
                    && !matches!(
                        &line.data,
                        StatusOutputLineData::UncommittedChanges { .. }
                            | StatusOutputLineData::UncommittedFile { .. },
                    )
                {
                    return false;
                }
            }
        }
        Mode::Rub(..)
        | Mode::InlineReword(..)
        | Mode::Command(..)
        | Mode::Commit(..)
        | Mode::Move(..)
        | Mode::Details(..)
        | Mode::MoveStack(..)
        | Mode::Stack(..) => {}
    }

    match mode {
        Mode::Normal(..) | Mode::Details(..) => match show_files_flag {
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
        Mode::Rub(rub_mode) => line
            .data
            .cli_id()
            .is_some_and(|cli_id| rub_mode.available_targets.contains(cli_id)),
        Mode::Commit(commit_mode) => commit_operation_display(&line.data, commit_mode).is_some(),
        Mode::Move(move_mode) => move_operation_display(&line.data, move_mode).is_some(),
        Mode::MoveStack(move_mode) => reorder_operation_display(&line.data, move_mode).is_some(),
        Mode::Stack(stack_mode) => stack_operation_display(&line.data, stack_mode).is_some(),
        Mode::PickChanges(..) => {
            if let Some(cli_id) = line.data.cli_id() {
                match &**cli_id {
                    CliId::UncommittedHunkOrFile(..) | CliId::Uncommitted { .. } => true,
                    CliId::PathPrefix { .. }
                    | CliId::CommittedFile { .. }
                    | CliId::Branch { .. }
                    | CliId::Commit { .. }
                    | CliId::Stack { .. } => false,
                }
            } else {
                false
            }
        }
        Mode::InlineReword(..) | Mode::Command(..) => {
            // you can't actually move the selection in these modes
            // but returning `false` would dim every line which hurts UX
            true
        }
    }
}
