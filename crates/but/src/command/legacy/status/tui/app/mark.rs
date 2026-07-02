use std::sync::Arc;

use anyhow::Context as _;
use but_core::ref_metadata::StackId;
use but_ctx::Context;

use crate::{
    CliId, IdMap,
    command::legacy::status::{
        StatusOutputLine,
        output::StatusOutputLineData,
        tui::{
            app::{
                App, normal_mode::NormalMode, pick_changes_mode::PickChangesMode,
                rub_mode::RubSource,
            },
            marking::{Markable, Marks},
            mode::Mode,
        },
    },
};

impl App {
    pub fn handle_mark(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|selection| selection.data.cli_id())
        else {
            return Ok(());
        };

        match &**selection {
            CliId::Commit { .. } | CliId::UncommittedHunkOrFile(..) => {
                if handle_mark_cli_id(
                    selection,
                    self.mode
                        .get_mut_without_updating_backstack_and_i_promise_not_to_change_state(),
                ) && let Some(new_cursor) = self.cursor.move_down_within_section(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                }
            }
            CliId::Branch {
                name,
                id: _,
                stack_id,
            } => {
                // you cannot select branches in rub mode so we don't need to care about that
                if let Some(stack_id) = *stack_id {
                    match self
                        .mode
                        .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
                    {
                        Mode::Normal(NormalMode { marks })
                        | Mode::PickChanges(PickChangesMode { marks }) => {
                            handle_mark_branch(marks, ctx, stack_id, name)?;
                        }
                        Mode::Rub(..)
                        | Mode::InlineReword(..)
                        | Mode::Command(..)
                        | Mode::Commit(..)
                        | Mode::Move(..)
                        | Mode::Details(..)
                        | Mode::MoveStack(..)
                        | Mode::Jump(..)
                        | Mode::Stack(..) => {}
                    }
                }
            }
            CliId::Uncommitted { .. } => {
                // you cannot select uncommitted changes in rub mode so we don't need to care about that
                match self
                    .mode
                    .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
                {
                    Mode::Normal(NormalMode { marks })
                    | Mode::PickChanges(PickChangesMode { marks }) => {
                        handle_mark_uncommitted(marks, &self.status_lines);
                    }
                    Mode::Rub(..)
                    | Mode::InlineReword(..)
                    | Mode::Command(..)
                    | Mode::Commit(..)
                    | Mode::Move(..)
                    | Mode::Details(..)
                    | Mode::MoveStack(..)
                    | Mode::Jump(..)
                    | Mode::Stack(..) => {}
                }
            }
            CliId::PathPrefix { .. } | CliId::CommittedFile { .. } | CliId::Stack { .. } => {}
        }

        if let Some(marks) = self.marks() {
            if marks.is_empty() {
                self.backstack.remove_mark();
            } else {
                self.backstack.push_mark();
            }
        }

        Ok(())
    }

    pub fn handle_clear_normal_mode_marks(&mut self) {
        let Mode::Normal(normal_mode) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };

        normal_mode.marks.clear();
        self.backstack.remove_mark();
    }

    pub fn marks(&self) -> Option<&Marks> {
        self.mode.marks()
    }
}

fn handle_mark_cli_id(commit: &CliId, mode: &mut Mode) -> bool {
    let Some(markable) = Markable::try_from_cli_id(commit) else {
        return false;
    };

    match mode {
        Mode::Normal(normal_mode) => {
            normal_mode.marks.toggle(markable);
        }
        Mode::PickChanges(pick_uncommitted_mode) => {
            pick_uncommitted_mode.marks.toggle(markable);
        }
        Mode::Rub(rub_mode) => {
            match &mut rub_mode.source {
                RubSource::CliId(cli_id) => {
                    match &**cli_id {
                        CliId::Commit { .. } => {
                            // we only support rubbing commits, meaning the source
                            // also most be a commit
                            let mut marks = Marks::default();
                            if let Some(previous_source) = Markable::try_from_cli_id(cli_id)
                                && markable != previous_source
                            {
                                marks.toggle(previous_source);
                            }
                            marks.toggle(markable);
                            rub_mode.source = RubSource::Marks(marks);
                        }
                        CliId::UncommittedHunkOrFile(..)
                        | CliId::PathPrefix { .. }
                        | CliId::CommittedFile { .. }
                        | CliId::Branch { .. }
                        | CliId::Uncommitted { .. }
                        | CliId::Stack { .. } => return false,
                    }
                }
                RubSource::Marks(marks) => {
                    marks.toggle(markable.clone());

                    match marks.len() {
                        0 => {
                            rub_mode.source = RubSource::CliId(Arc::new(markable.into_cli_id()));
                        }
                        1 => {
                            let only_remaining_mark = marks.iter().next().cloned();
                            if let Some(mark) = only_remaining_mark {
                                rub_mode.source = RubSource::CliId(Arc::new(mark.into_cli_id()));
                            }
                        }
                        _ => {
                            //
                        }
                    }
                }
            }
        }
        Mode::InlineReword(..)
        | Mode::Command(..)
        | Mode::Commit(..)
        | Mode::Move(..)
        | Mode::Stack(..)
        | Mode::MoveStack(..)
        | Mode::Jump(..)
        | Mode::Details(..) => {
            return false;
        }
    }

    true
}

fn handle_mark_branch(
    marks: &mut Marks,
    ctx: &Context,
    stack_id: StackId,
    name: &str,
) -> anyhow::Result<()> {
    let Some(commits) = commits_on_branch(ctx, stack_id, name)?
        .into_iter()
        .map(|(commit_id, short_id)| {
            Markable::try_from_cli_id(&CliId::Commit {
                commit_id,
                id: short_id,
            })
        })
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(());
    };

    toggle_markables(marks, commits);

    Ok(())
}

fn handle_mark_uncommitted(marks: &mut Marks, status_lines: &[StatusOutputLine]) {
    let uncommitted_files = status_lines.iter().filter_map(|line| match &line.data {
        StatusOutputLineData::UncommittedFile { cli_id } => Markable::try_from_cli_id(cli_id),
        StatusOutputLineData::UpdateNotice
        | StatusOutputLineData::Connector
        | StatusOutputLineData::BetweenStacks
        | StatusOutputLineData::StagedChanges { .. }
        | StatusOutputLineData::StagedFile { .. }
        | StatusOutputLineData::UncommittedChanges { .. }
        | StatusOutputLineData::Branch { .. }
        | StatusOutputLineData::Commit { .. }
        | StatusOutputLineData::CommitMessage
        | StatusOutputLineData::EmptyCommitMessage
        | StatusOutputLineData::File { .. }
        | StatusOutputLineData::MergeBase
        | StatusOutputLineData::UpstreamChanges
        | StatusOutputLineData::Warning
        | StatusOutputLineData::Hint
        | StatusOutputLineData::NoAssignmentsUnstaged => None,
    });

    toggle_markables(marks, uncommitted_files);
}

fn toggle_markables(marks: &mut Marks, markables: impl IntoIterator<Item = Markable>) {
    let (marked, unmarked) = markables
        .into_iter()
        .partition::<Vec<_>, _>(|markable| marks.contains(markable));

    match (marked.is_empty(), unmarked.is_empty()) {
        (true, false) => {
            for markable in unmarked {
                marks.insert(markable);
            }
        }
        (false, true) => {
            for markable in marked {
                marks.remove(&markable);
            }
        }
        _ => {
            for markable in unmarked {
                marks.insert(markable);
            }
        }
    }
}

pub fn commits_on_branch(
    ctx: &Context,
    stack_id: StackId,
    name: &str,
) -> anyhow::Result<Vec<(gix::ObjectId, String)>> {
    let guard = ctx.shared_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    let segment = id_map
        .stacks()
        .iter()
        .filter(|stack| stack.id.is_some_and(|id| id == stack_id))
        .flat_map(|stack| &stack.segments)
        .find(|segment| {
            segment
                .branch_name()
                .is_some_and(|branch_name| branch_name == name)
        })
        .context("segment not found")?;

    let commits = segment
        .workspace_commits
        .iter()
        .map(|commit| (commit.commit_id(), commit.short_id.clone()))
        .collect::<Vec<_>>();

    Ok(commits)
}
