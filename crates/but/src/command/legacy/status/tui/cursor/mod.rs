use std::sync::Arc;

use bstr::BStr;

use crate::{
    CliId, CliResult,
    args::atoms::ResolvedCliIdArg,
    bad_input,
    command::legacy::status::{
        FilesStatusFlag, StatusOutputLine,
        output::StatusOutputLineData,
        tui::{
            Mode, NormalMode, PickChangesMode, SelectAfterReload,
            app::{
                CommitSource, SquashMode,
                mark::{MarkableRef, Marks, hunk_is_child_of},
                prefix_match,
            },
            mode::ModeRef,
            render::{
                cherry_pick_operation_display, commit_operation_display, move_operation_display,
                reorder_operation_display, stack_operation_display,
            },
        },
    },
    id::{CommitId, CommittedFileId},
};

#[cfg(test)]
mod tests;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[must_use]
pub struct Cursor(usize);

impl Cursor {
    pub fn new(lines: &[StatusOutputLine]) -> Self {
        Self(
            lines
                .iter()
                .position(|line| line.is_selectable())
                .unwrap_or(0),
        )
    }

    pub fn index(self) -> usize {
        self.0
    }

    pub fn scroll_top_for_viewport(
        self,
        current_top: usize,
        total_rows: usize,
        viewport_height: usize,
        context_rows: usize,
    ) -> usize {
        let max_scroll_top = total_rows.saturating_sub(viewport_height);
        let current_top = current_top.min(max_scroll_top);
        if viewport_height == 0 {
            return current_top;
        }

        let context_rows = context_rows.min(viewport_height.saturating_sub(1) / 2);
        let visible_start_with_context = current_top.saturating_add(context_rows);
        if self.0 < visible_start_with_context {
            return self.0.saturating_sub(context_rows).min(max_scroll_top);
        }

        let visible_end = current_top.saturating_add(viewport_height);
        let visible_end_with_context = visible_end.saturating_sub(context_rows);
        if self.0 >= visible_end_with_context {
            return self
                .0
                .saturating_add(context_rows)
                .saturating_add(1)
                .saturating_sub(viewport_height)
                .min(max_scroll_top);
        }

        current_top
    }

    pub fn select_resolved_target(
        target: ResolvedCliIdArg,
        lines: &[StatusOutputLine],
    ) -> CliResult<Option<Self>> {
        let hint = "`TARGET` can be a commit, branch, committed file, uncommitted file, or uncommitted hunk";
        match &target {
            ResolvedCliIdArg::Commit(..)
            | ResolvedCliIdArg::Branch(..)
            | ResolvedCliIdArg::Uncommitted
            | ResolvedCliIdArg::UncommittedHunkOrFile(..)
            | ResolvedCliIdArg::CommittedFile(..) => {}
            ResolvedCliIdArg::PathPrefix { .. } => {
                return Err(bad_input("Selecting path prefixes is not supported")
                    .hint(hint)
                    .into());
            }
            ResolvedCliIdArg::Stack { .. } => {
                return Err(bad_input("Selecting stacks is not supported")
                    .hint(hint)
                    .into());
            }
        }

        let Some(idx) = lines.iter().position(|line| {
            line.data.cli_id().is_some_and(|cli_id| match &target {
                ResolvedCliIdArg::UncommittedHunkOrFile(hunk) if !hunk.is_entire_file => {
                    match &**cli_id {
                        CliId::UncommittedHunkOrFile(file) => hunk_is_child_of(file, hunk),
                        CliId::PathPrefix { .. }
                        | CliId::CommittedFile { .. }
                        | CliId::Branch(..)
                        | CliId::Commit { .. }
                        | CliId::Uncommitted { .. }
                        | CliId::Stack { .. } => false,
                    }
                }
                ResolvedCliIdArg::Commit(..)
                | ResolvedCliIdArg::Branch(..)
                | ResolvedCliIdArg::UncommittedHunkOrFile(..)
                | ResolvedCliIdArg::CommittedFile(..)
                | ResolvedCliIdArg::Uncommitted
                | ResolvedCliIdArg::PathPrefix { .. }
                | ResolvedCliIdArg::Stack { .. } => target == **cli_id,
            })
        }) else {
            return Ok(None);
        };
        if !lines[idx].is_selectable() {
            return Err(bad_input("`TARGET` is not selectable").hint(hint).into());
        }
        Ok(Some(Self(idx)))
    }

    pub fn restore(selected_cli_id: &CliId, lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            line.data
                .cli_id()
                .is_some_and(|cli_id| same_entity_for_reload(selected_cli_id, cli_id))
        })?;
        Some(Self(idx))
    }

    pub fn select_closest_commit_source(
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

    pub fn select_commit(object_id: gix::ObjectId, lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::Commit {
                commit: CommitId { commit_id, .. },
                id: _,
            }) = line.data.cli_id().map(|id| &**id)
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
    pub fn select_after_discarded_commit(
        self,
        lines: &[StatusOutputLine],
    ) -> Option<SelectAfterReload> {
        if let Some(CliId::Commit {
            commit: CommitId { commit_id, .. },
            id: _,
        }) = lines
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
    pub fn select_after_discarded_marks(
        self,
        lines: &[StatusOutputLine],
        discarded_marks: &Marks,
    ) -> Option<SelectAfterReload> {
        if self.0 >= lines.len() {
            return None;
        }

        if let Some(cli_id) = lines[self.0].data.cli_id() {
            let selected_is_discarded = discarded_marks.contains_cli_id(cli_id);

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
            if discarded_marks.contains_cli_id(cli_id) {
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
            if discarded_marks.contains_cli_id(cli_id) {
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
    pub fn select_after_discarded_commits(
        self,
        lines: &[StatusOutputLine],
        discarded_commits: &[gix::ObjectId],
    ) -> Option<SelectAfterReload> {
        if self.0 >= lines.len() {
            return None;
        }

        if let Some(CliId::Commit {
            commit: CommitId { commit_id, .. },
            id: _,
        }) = lines[self.0].data.cli_id().map(|id| &**id)
            && !discarded_commits.contains(commit_id)
        {
            return Some(SelectAfterReload::Commit(*commit_id));
        }

        for line in lines.iter().skip(self.0 + 1) {
            if is_discard_commit_boundary(line) {
                break;
            }

            if let Some(CliId::Commit {
                commit: CommitId { commit_id, .. },
                id: _,
            }) = line.data.cli_id().map(|id| &**id)
                && !discarded_commits.contains(commit_id)
            {
                return Some(SelectAfterReload::Commit(*commit_id));
            }
        }

        for line in lines.iter().take(self.0).rev() {
            if is_discard_commit_boundary(line) {
                break;
            }

            if let Some(CliId::Commit {
                commit: CommitId { commit_id, .. },
                id: _,
            }) = line.data.cli_id().map(|id| &**id)
                && !discarded_commits.contains(commit_id)
            {
                return Some(SelectAfterReload::Commit(*commit_id));
            }
        }

        for line in lines.iter().take(self.0 + 1).rev() {
            if let StatusOutputLineData::Branch { cli_id, .. } = &line.data {
                return Some(SelectAfterReload::CliId(Box::new((**cli_id).clone())));
            }

            if is_discard_commit_boundary(line) {
                break;
            }
        }

        None
    }

    /// Selects what should be focused after discarding the currently selected branch.
    pub fn select_after_discarded_branch(
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
            if let Some(CliId::Branch(branch)) = line.data.cli_id().map(|id| &**id) {
                return Some(SelectAfterReload::Branch(branch.name.clone()));
            }
        }

        for line in lines.iter().take(self.0).rev() {
            if let Some(CliId::Branch(branch)) = line.data.cli_id().map(|id| &**id) {
                return Some(SelectAfterReload::Branch(branch.name.clone()));
            }
        }

        if Self::select_uncommitted(lines).is_some() {
            return Some(SelectAfterReload::Uncommitted);
        }

        None
    }

    pub fn select_first_file_in_commit(
        object_id: gix::ObjectId,
        lines: &[StatusOutputLine],
    ) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::CommittedFile {
                committed_file: CommittedFileId { commit_id, .. },
                id: _,
            }) = line.data.cli_id().map(|id| &**id)
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
    pub fn select_branch(branch_name: &str, lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::Branch(branch)) = line.data.cli_id().map(|id| &**id)
                && branch.name == branch_name
            {
                true
            } else {
                false
            }
        })?;
        Some(Self(idx))
    }

    /// Select the first uncommitted file line that points to the given path in the given stack.
    pub fn select_uncommitted_file(path: &BStr, lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            if let Some(CliId::UncommittedHunkOrFile(uncommitted)) =
                line.data.cli_id().map(|id| &**id)
            {
                let hunk = uncommitted.hunks.first();
                hunk.hunk.path == path
            } else {
                false
            }
        })?;
        Some(Self(idx))
    }

    /// Select the first line that points to the uncommitted section.
    pub fn select_uncommitted(lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines.iter().position(|line| {
            matches!(
                line.data.cli_id().map(|id| &**id),
                Some(CliId::Uncommitted { .. })
            )
        })?;
        Some(Self(idx))
    }

    /// Select the merge-base line.
    pub fn select_merge_base(lines: &[StatusOutputLine]) -> Option<Self> {
        let idx = lines
            .iter()
            .position(|line| matches!(line.data, StatusOutputLineData::MergeBase))?;
        Some(Self(idx))
    }

    pub fn selected_line(self, lines: &[StatusOutputLine]) -> Option<&StatusOutputLine> {
        lines.get(self.0)
    }

    /// Selects the previous selectable row and returns it as a reload target.
    ///
    /// Falls back to selecting the uncommitted section if there is no previous
    /// selectable row.
    pub fn select_previous_cli_id_or_uncommitted(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> SelectAfterReload {
        self.move_up(lines, mode, show_files)
            .and_then(|cursor| cursor.selected_line(lines))
            .and_then(|line| line.data.cli_id().cloned())
            .map(|cli_id| SelectAfterReload::CliId(Box::new((*cli_id).clone())))
            .unwrap_or(SelectAfterReload::Uncommitted)
    }

    pub fn selection_cli_id_for_reload(
        self,
        lines: &[StatusOutputLine],
        show_files: FilesStatusFlag,
    ) -> Option<&Arc<CliId>> {
        let selected_line = self.selected_line(lines)?;

        if matches!(selected_line.data, StatusOutputLineData::File { .. }) {
            let file_is_visible = match selected_line.data.cli_id().map(|id| &**id) {
                Some(CliId::CommittedFile {
                    committed_file: CommittedFileId { commit_id, .. },
                    id: _,
                }) => show_files.show_files_for(*commit_id),
                Some(CliId::UncommittedHunkOrFile(..))
                | Some(CliId::PathPrefix { .. })
                | Some(CliId::Branch(..))
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
    pub fn move_up(
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
    pub fn move_down(
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
    pub fn move_up_within_section(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> Option<Self> {
        if self.0 >= lines.len() {
            return None;
        }

        let section_start = find_section_start_at_or_before(lines, mode, self.0)?;
        let (idx, _) = lines
            .iter()
            .enumerate()
            .take(self.0)
            .skip(section_start)
            .rev()
            .find(|(idx, _)| is_cursor_selectable_at_index(*idx, lines, mode, show_files))?;
        Some(Self(idx))
    }

    #[must_use]
    pub fn move_down_within_section(
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

    #[must_use]
    pub fn move_after_mark(
        self,
        lines: &[StatusOutputLine],
        mode: &Mode,
        show_files: FilesStatusFlag,
    ) -> Option<Self> {
        let mark_state = |cursor: Self| {
            cursor
                .selected_line(lines)
                .and_then(|line| line.data.cli_id())
                .and_then(|id| {
                    MarkableRef::try_from_cli_id(id).map(|_| mode.marks_ref().contains_cli_id(id))
                })
        };
        let current_markable = self
            .selected_line(lines)
            .and_then(|line| line.data.cli_id())
            .and_then(|id| MarkableRef::try_from_cli_id(id))?;
        let current_is_marked = mark_state(self)?;
        let is_opposite = |cursor| mark_state(cursor) == Some(!current_is_marked);

        let (next, previous) = match current_markable {
            MarkableRef::Branch(..) => (
                self.move_down(lines, mode, show_files),
                self.move_up(lines, mode, show_files),
            ),
            MarkableRef::Uncommitted(..)
            | MarkableRef::Commit(..)
            | MarkableRef::CommittedFile(..) => (
                self.move_down_within_section(lines, mode, show_files),
                self.move_up_within_section(lines, mode, show_files),
            ),
        };

        next.filter(|next| is_opposite(*next))
            .or_else(|| previous.filter(|previous| is_opposite(*previous)))
    }

    /// Moves the cursor to the first selectable row in the next section.
    #[must_use]
    pub fn move_next_section(
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
    pub fn move_previous_section(
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

/// Short IDs are recomputed on reload, so compare the underlying entity instead.
pub(super) fn same_entity_for_reload(previous: &CliId, current: &CliId) -> bool {
    match (previous, current) {
        (CliId::UncommittedHunkOrFile(previous), CliId::UncommittedHunkOrFile(current)) => {
            if previous.is_entire_file != current.is_entire_file {
                return false;
            }
            if previous.is_entire_file {
                let previous = previous.hunks.first();
                let current = current.hunks.first();
                previous.hunk.path == current.hunk.path
            } else {
                previous == current
            }
        }
        (
            CliId::PathPrefix {
                hunks: previous, ..
            },
            CliId::PathPrefix { hunks: current, .. },
        ) => previous == current,
        (
            CliId::CommittedFile {
                committed_file:
                    CommittedFileId {
                        commit_id: previous_commit_id,
                        path: previous_path,
                        change_id: previous_change_id,
                    },
                id: _,
            },
            CliId::CommittedFile {
                committed_file:
                    CommittedFileId {
                        commit_id: current_commit_id,
                        path: current_path,
                        change_id: current_change_id,
                    },
                id: _,
            },
        ) => {
            previous_path == current_path
                && match (previous_change_id, current_change_id) {
                    (Some(previous), Some(current)) => previous == current,
                    (Some(_), None) | (None, Some(_)) | (None, None) => {
                        previous_commit_id == current_commit_id
                    }
                }
        }
        (CliId::Branch(previous), CliId::Branch(current)) => previous == current,
        (
            CliId::Commit {
                commit:
                    CommitId {
                        commit_id: previous_commit_id,
                        change_id: previous_change_id,
                        ..
                    },
                id: _,
            },
            CliId::Commit {
                commit:
                    CommitId {
                        commit_id: current_commit_id,
                        change_id: current_change_id,
                        ..
                    },
                id: _,
            },
        ) => match (previous_change_id, current_change_id) {
            (Some(previous), Some(current)) => previous == current,
            (Some(_), None) | (None, Some(_)) | (None, None) => {
                previous_commit_id == current_commit_id
            }
        },
        (CliId::Uncommitted { .. }, CliId::Uncommitted { .. }) => true,
        (
            CliId::Stack {
                stack_id: previous, ..
            },
            CliId::Stack {
                stack_id: current, ..
            },
        ) => previous == current,
        _ => false,
    }
}

fn select_after_reload_for_cli_id(cli_id: &Arc<CliId>) -> SelectAfterReload {
    match &**cli_id {
        CliId::Commit {
            commit: CommitId { commit_id, .. },
            id: _,
        } => SelectAfterReload::Commit(*commit_id),
        CliId::CommittedFile {
            committed_file: CommittedFileId { commit_id, .. },
            id: _,
        } => SelectAfterReload::FirstFileInCommit(*commit_id),
        CliId::Uncommitted { .. }
        | CliId::UncommittedHunkOrFile(..)
        | CliId::PathPrefix { .. }
        | CliId::Branch(..)
        | CliId::Stack { .. } => SelectAfterReload::CliId(Box::new((**cli_id).clone())),
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
        | Mode::Jump(..)
        | Mode::Squash(..)
        | Mode::CherryPick(..)
        | Mode::Details(..) => {
            matches!(
                line.data,
                StatusOutputLineData::Branch { .. }
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

    is_selectable_in_mode(line, mode.as_ref(), show_files_flag)
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

    let Some(source_stack_id) = move_mode.source.branch.stack_id else {
        return false;
    };
    let current_stack_order = super::app::stack_ids_in_display_order(lines);
    let Some(source_index) = current_stack_order
        .iter()
        .position(|stack| *stack == source_stack_id)
    else {
        return false;
    };

    let target_index = super::app::stack_ids_in_display_order(&lines[..idx]).len();
    target_index == source_index || target_index == source_index + 1
}

pub fn is_selectable_in_mode(
    line: &StatusOutputLine,
    mode: ModeRef<'_>,
    show_files_flag: FilesStatusFlag,
) -> bool {
    if !line.is_selectable() {
        if let ModeRef::MoveStack(..) = mode
            && let StatusOutputLineData::BetweenStacks = line.data
        {
            // `BetweenStacks` lines are selectable in reorder mode
        } else {
            return false;
        }
    }

    // selecting the source line should always be possible
    match mode {
        ModeRef::Squash(squash_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && squash_mode.source.contains(cli_id)
            {
                return true;
            }
        }
        ModeRef::Commit(commit_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && commit_mode.source.contains(cli_id)
            {
                return true;
            }
        }
        ModeRef::Move(move_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && move_mode.source.contains(cli_id)
            {
                return true;
            }
        }
        ModeRef::MoveStack(move_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && move_mode.source.matches(cli_id)
            {
                return true;
            }
        }
        ModeRef::CherryPick(cherry_pick_mode) => {
            if let Some(cli_id) = line.data.cli_id()
                && cherry_pick_mode.source.contains(cli_id)
            {
                return true;
            }
        }
        ModeRef::Command(..)
        | ModeRef::InlineReword(..)
        | ModeRef::Normal(..)
        | ModeRef::PickChanges(..)
        | ModeRef::Details(..)
        | ModeRef::Jump(..)
        | ModeRef::Stack(..) => {}
    }

    // don't allow mixing marks
    match mode {
        ModeRef::Normal(NormalMode { marks }) => match marks {
            Marks::Empty => {}
            Marks::Hunks(..) => {
                if !matches!(
                    &line.data,
                    StatusOutputLineData::UncommittedChanges { .. }
                        | StatusOutputLineData::UncommittedFile { .. },
                ) {
                    return false;
                }
            }
            Marks::Commits(..) => {
                if !matches!(&line.data, StatusOutputLineData::Commit { .. }) {
                    return false;
                }
            }
            Marks::CommittedFiles(files) => {
                if !matches!(&line.data, StatusOutputLineData::File { .. }) {
                    return false;
                }
                if let FilesStatusFlag::All = show_files_flag {
                    let Some(id) = line.data.cli_id() else {
                        return false;
                    };
                    let CliId::CommittedFile {
                        committed_file: CommittedFileId { commit_id, .. },
                        id: _,
                    } = &**id
                    else {
                        return false;
                    };
                    if *commit_id != files.head.commit_id {
                        return false;
                    }
                }
            }
            Marks::Branches(..) => {
                if !matches!(&line.data, StatusOutputLineData::Branch { .. }) {
                    return false;
                }
            }
        },
        ModeRef::PickChanges(PickChangesMode { marks }) => {
            if !marks.is_empty()
                && !matches!(
                    &line.data,
                    StatusOutputLineData::UncommittedChanges { .. }
                        | StatusOutputLineData::UncommittedFile { .. },
                )
            {
                return false;
            }
        }
        ModeRef::Squash(..)
        | ModeRef::InlineReword(..)
        | ModeRef::Command(..)
        | ModeRef::Commit(..)
        | ModeRef::Move(..)
        | ModeRef::Details(..)
        | ModeRef::MoveStack(..)
        | ModeRef::Jump(..)
        | ModeRef::CherryPick(..)
        | ModeRef::Stack(..) => {}
    }

    match mode {
        ModeRef::Normal(..) => match show_files_flag {
            FilesStatusFlag::None | FilesStatusFlag::All => true,
            FilesStatusFlag::Commit(object_id) => {
                if let Some(cli_id) = line.data.cli_id()
                    && let CliId::CommittedFile {
                        committed_file: CommittedFileId { commit_id, .. },
                        id: _,
                    } = &**cli_id
                {
                    object_id == *commit_id
                } else {
                    false
                }
            }
        },
        ModeRef::Details(details_mode) => {
            is_selectable_in_mode(line, details_mode.return_mode.as_ref(), show_files_flag)
        }
        ModeRef::Squash(SquashMode { source, reword: _ }) => line
            .data
            .cli_id()
            .is_some_and(|target| source.can_target(target)),
        ModeRef::Commit(commit_mode) => commit_operation_display(&line.data, commit_mode).is_some(),
        ModeRef::Move(move_mode) => move_operation_display(&line.data, move_mode).is_some(),
        ModeRef::MoveStack(move_mode) => reorder_operation_display(&line.data, move_mode).is_some(),
        ModeRef::Stack(stack_mode) => stack_operation_display(&line.data, stack_mode).is_some(),
        ModeRef::CherryPick(cherry_pick_mode) => {
            cherry_pick_operation_display(&line.data, cherry_pick_mode).is_some()
        }
        ModeRef::PickChanges(..) => {
            if let Some(cli_id) = line.data.cli_id() {
                match &**cli_id {
                    CliId::UncommittedHunkOrFile(..) | CliId::Uncommitted { .. } => true,
                    CliId::PathPrefix { .. }
                    | CliId::CommittedFile { .. }
                    | CliId::Branch(..)
                    | CliId::Commit { .. }
                    | CliId::Stack { .. } => false,
                }
            } else {
                false
            }
        }
        ModeRef::Command(command_mode) => {
            is_selectable_in_mode(line, command_mode.return_mode.as_ref(), show_files_flag)
        }
        ModeRef::InlineReword(..) => true,
        ModeRef::Jump(jump_mode) => prefix_match(
            jump_mode.query(),
            line,
            &jump_mode.return_mode,
            show_files_flag,
        ),
    }
}
