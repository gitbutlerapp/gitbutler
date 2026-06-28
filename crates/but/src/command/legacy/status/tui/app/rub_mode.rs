use std::sync::Arc;

use anyhow::Context as _;
use bstr::BString;
use but_core::{DryRun, HunkHeader, ref_metadata::StackId};
use but_ctx::Context;
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use nonempty::NonEmpty;

use crate::{
    CliId,
    command::legacy::{
        rub::{CommitToUncommittedAreaOperation, RubOperation, SquashCommitsOperation},
        status::{
            FilesStatusFlag,
            tui::{
                App, Message, ReloadCause, SelectAfterReload, cursor,
                marking::{MarkClasses, Markable, Marks},
                message_on_drop::MessageOnDrop,
                mode::Mode,
                nonempty_from_refs, operations,
            },
        },
    },
    id::{ShortId, UNCOMMITTED},
    utils::diff_specs::DiffSpecBuilder,
};

#[derive(Debug, Clone)]
pub struct RubMode {
    pub source: RubSource,
    pub available_targets: Vec<Arc<CliId>>,
    pub how_to_combine_messages: MessageCombinationStrategy,
    pub _unlock_details: Option<MessageOnDrop>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CommittedHunk {
    pub commit_id: gix::ObjectId,
    pub header: HunkHeader,
    pub path: Arc<BString>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RubSource {
    Marks(Marks),
    CliId(Arc<CliId>),
    CommittedHunk(CommittedHunk),
}

impl RubSource {
    pub fn contains(&self, other: &CliId) -> bool {
        match self {
            RubSource::Marks(marks) => {
                Markable::try_from_cli_id(other).is_some_and(|markable| marks.contains(&markable))
            }
            RubSource::CliId(source) => &**source == other,
            RubSource::CommittedHunk { .. } => false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RubMessage {
    Start,
    StartWithSource {
        source: RubSource,
        unlock_details: Option<MessageOnDrop>,
    },
    StartReverse,
    UseTargetMessage,
    UseSourceMessage,
    Confirm,
}

impl App {
    pub fn handle_rub(
        &mut self,
        rub_message: RubMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match rub_message {
            RubMessage::Start => self.handle_rub_start(),
            RubMessage::StartWithSource {
                source,
                unlock_details,
            } => {
                self.handle_rub_start_with_source(source, unlock_details);
            }
            RubMessage::StartReverse => {
                self.handle_rub_start_reverse(ctx)?;
            }
            RubMessage::Confirm => self.handle_rub_confirm(ctx, messages)?,
            RubMessage::UseTargetMessage => {
                self.handle_rub_use_target_message();
            }
            RubMessage::UseSourceMessage => {
                self.handle_rub_use_source_message();
            }
        }

        Ok(())
    }

    fn handle_rub_start(&mut self) {
        let Mode::Normal(normal_mode) = &*self.mode else {
            return;
        };
        let Some(selected_line) = self.cursor.selected_line(&self.status_lines) else {
            return;
        };
        let Some(cli_id) = selected_line.data.cli_id() else {
            return;
        };
        if normal_mode.marks.is_empty() {
            self.handle_rub_start_with_source(RubSource::CliId(Arc::clone(cli_id)), None);
        } else {
            self.handle_rub_start_with_source(RubSource::Marks(normal_mode.marks.clone()), None);
        }
    }

    fn available_targets_for_rub_mode(&self, source: &RubSource) -> Vec<Arc<CliId>> {
        match &source {
            RubSource::CliId(source) => self
                .status_lines
                .iter()
                .filter_map(|line| line.data.cli_id())
                .filter(|target| {
                    source == *target
                        || route_operation(
                            NonEmpty::new(source),
                            target,
                            MessageCombinationStrategy::KeepBoth,
                        )
                        .is_some()
                })
                .cloned()
                .collect::<Vec<_>>(),
            RubSource::CommittedHunk(hunk) => self
                .status_lines
                .iter()
                .filter_map(|line| line.data.cli_id())
                .filter(|target| {
                    source.contains(target)
                        || route_operation_from_detail_view(hunk, target).is_some()
                })
                .cloned()
                .collect::<Vec<_>>(),
            RubSource::Marks(marks) => {
                let marks = marks
                    .iter()
                    .cloned()
                    .map(|mark| mark.into_cli_id())
                    .collect::<Vec<_>>();
                self.status_lines
                    .iter()
                    .filter_map(|line| line.data.cli_id())
                    .filter(|target| {
                        source.contains(target) || {
                            marks.iter().all(|mark| {
                                route_operation(
                                    NonEmpty::new(mark),
                                    target,
                                    MessageCombinationStrategy::KeepBoth,
                                )
                                .is_some()
                            })
                        }
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            }
        }
    }

    fn handle_rub_start_with_source(
        &mut self,
        source: RubSource,
        unlock_details: Option<MessageOnDrop>,
    ) {
        match &source {
            RubSource::CliId(cli_id) => {
                if !supports_rubbing(cli_id) {
                    return;
                }
            }
            RubSource::Marks(marks) => {
                let MarkClasses {
                    marked_commits,
                    marked_uncommitted,
                } = marks.classify();
                if marked_commits && marked_uncommitted {
                    return;
                }

                for mark in marks {
                    if !mark_supports_rubbing(mark) {
                        return;
                    }
                }
            }
            RubSource::CommittedHunk(..) => {}
        }

        let available_targets = self.available_targets_for_rub_mode(&source);

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Rub(RubMode {
                    source,
                    available_targets,
                    how_to_combine_messages: MessageCombinationStrategy::KeepBoth,
                    _unlock_details: unlock_details,
                });
            });

        if self
            .cursor
            .selected_line(&self.status_lines)
            .is_some_and(|line| {
                cursor::is_selectable_in_mode(line, &self.mode, self.flags.show_files)
            })
        {
            return;
        }

        if let Some(new_cursor) =
            self.cursor
                .move_down(&self.status_lines, &self.mode, self.flags.show_files)
        {
            self.cursor = new_cursor;
        } else if let Some(new_cursor) =
            self.cursor
                .move_up(&self.status_lines, &self.mode, self.flags.show_files)
        {
            self.cursor = new_cursor;
        }
    }

    fn handle_rub_start_reverse(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
        else {
            return Ok(());
        };

        let CliId::Commit { commit_id, .. } = &**selection else {
            return Ok(());
        };

        let stack_id = {
            let (_guard, _, ws, _) = ctx.workspace_and_db()?;
            ws.find_commit_and_containers(*commit_id)
                .and_then(|(stack, _, _)| stack.id)
        };

        let source = if let Some(stack_id) = stack_id
            && operations::stack_has_assigned_changes(ctx, stack_id)?
            && let Some(id) = self
                .status_lines
                .iter()
                .filter_map(|line| line.data.cli_id())
                .find_map(|id| {
                    if let CliId::Stack { id, stack_id: sid } = &**id
                        && *sid == stack_id
                    {
                        Some(id)
                    } else {
                        None
                    }
                }) {
            RubSource::CliId(Arc::new(CliId::Stack {
                id: id.to_owned(),
                stack_id,
            }))
        } else {
            RubSource::CliId(Arc::new(CliId::Uncommitted {
                id: UNCOMMITTED.to_owned(),
            }))
        };

        let available_targets = self.available_targets_for_rub_mode(&source);

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Rub(RubMode {
                    source,
                    available_targets,
                    how_to_combine_messages: MessageCombinationStrategy::KeepBoth,
                    _unlock_details: None,
                });
            });

        Ok(())
    }

    fn handle_rub_use_target_message(&mut self) {
        let Mode::Rub(RubMode {
            how_to_combine_messages,
            ..
        }) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };
        *how_to_combine_messages = match *how_to_combine_messages {
            MessageCombinationStrategy::KeepBoth | MessageCombinationStrategy::KeepSubject => {
                MessageCombinationStrategy::KeepTarget
            }
            MessageCombinationStrategy::KeepTarget => MessageCombinationStrategy::KeepBoth,
        };
    }

    fn handle_rub_use_source_message(&mut self) {
        let Mode::Rub(RubMode {
            how_to_combine_messages,
            ..
        }) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };
        *how_to_combine_messages = match *how_to_combine_messages {
            MessageCombinationStrategy::KeepBoth | MessageCombinationStrategy::KeepTarget => {
                MessageCombinationStrategy::KeepSubject
            }
            MessageCombinationStrategy::KeepSubject => MessageCombinationStrategy::KeepBoth,
        };
    }

    /// Handles confirming the currently selected rub operation.
    fn handle_rub_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Rub(RubMode {
            source,
            how_to_combine_messages,
            available_targets: _,
            _unlock_details: _,
        }) = &*self.mode
        else {
            return Ok(());
        };

        let Some(target) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
        else {
            return Ok(());
        };

        let reload_message = match source {
            RubSource::CliId(source) => {
                if let Some(operation) =
                    route_operation(NonEmpty::new(source), target, *how_to_combine_messages)
                {
                    let what_to_select = perform_operation(ctx, &operation)?;
                    Message::Reload(what_to_select, ReloadCause::Mutation)
                } else {
                    return Ok(());
                }
            }
            RubSource::CommittedHunk(hunk) => {
                if let Some(operation) = route_operation_from_detail_view(hunk, target) {
                    Message::Reload(Some(operation.execute(ctx)?), ReloadCause::Mutation)
                } else {
                    return Ok(());
                }
            }
            RubSource::Marks(marks) => {
                let sources = marks
                    .iter()
                    .cloned()
                    .map(|mark| mark.into_cli_id())
                    .filter(|source| source != &**target)
                    .collect::<Vec<_>>();
                let mut iter = sources.iter();
                if let Some(sources) = iter.next().map(|first| nonempty_from_refs(first, iter))
                    && let Some(operation) =
                        route_operation(sources, target, *how_to_combine_messages)
                {
                    let what_to_select = perform_operation(ctx, &operation)?;
                    Message::Reload(what_to_select, ReloadCause::Mutation)
                } else {
                    return Ok(());
                }
            }
        };

        match self.flags.show_files {
            FilesStatusFlag::Commit(..) => {
                self.backstack.remove_show_file_list();
                self.flags.show_files = FilesStatusFlag::None;
            }
            FilesStatusFlag::None | FilesStatusFlag::All => {}
        }

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            reload_message,
        ]);

        Ok(())
    }
}

pub fn route_operation<'a>(
    sources: NonEmpty<&'a CliId>,
    target: &'a CliId,
    how_to_combine_messages: MessageCombinationStrategy,
) -> Option<RubOperation<'a>> {
    Some(
        match crate::command::legacy::rub::route_operation(
            sources,
            target,
            how_to_combine_messages,
        )? {
            op @ RubOperation::UnassignUncommitted(..) => op,
            op @ RubOperation::UncommittedToCommit(..) => op,
            op @ RubOperation::UncommittedAreaToCommit(..) => op,
            op @ RubOperation::CommitToUncommittedArea(..) => op,
            op @ RubOperation::CommitToStack(..) => op,
            op @ RubOperation::SquashCommits(..) => op,
            op @ RubOperation::CommittedFileToCommit(..) => op,
            op @ RubOperation::CommittedFileToUncommittedArea(..) => op,
            op @ RubOperation::UncommittedToStack(..) => op,
            op @ RubOperation::StackToUncommittedArea(..) => op,
            op @ RubOperation::StackToStack(..) => op,
            op @ RubOperation::UncommittedAreaToStack(..) => op,
            op @ RubOperation::StackToCommit(..) => op,

            // dont allow rubbing with branches
            RubOperation::UncommittedToBranch(..)
            | RubOperation::StackToBranch(..)
            | RubOperation::UncommittedAreaToBranch(..)
            | RubOperation::MoveCommitToBranch(..)
            | RubOperation::BranchToUncommittedArea(..)
            | RubOperation::BranchToStack(..)
            | RubOperation::BranchToCommit(..)
            | RubOperation::BranchToBranch(..)
            | RubOperation::CommittedFileToBranch(..) => return None,
        },
    )
}

pub fn supports_rubbing(id: &CliId) -> bool {
    match id {
        CliId::Branch { .. } => false,
        CliId::UncommittedHunkOrFile(..)
        | CliId::PathPrefix { .. }
        | CliId::CommittedFile { .. }
        | CliId::Commit { .. }
        | CliId::Uncommitted { .. }
        | CliId::Stack { .. } => true,
    }
}

pub fn mark_supports_rubbing(mark: &Markable) -> bool {
    match mark {
        Markable::Commit { .. } | Markable::Uncommitted(..) => true,
    }
}

/// Returns a human-facing operation descriptor for the source/target pair.
pub fn rub_operation_display(
    sources: NonEmpty<&CliId>,
    target: &CliId,
    how_to_combine_messages: MessageCombinationStrategy,
) -> Option<&'static str> {
    if sources.len() == 1 && *sources.first() == target {
        return Some("noop");
    }

    let operation = route_operation(sources, target, how_to_combine_messages)?;
    Some(match operation {
        RubOperation::UnassignUncommitted(..) => "unassign hunks",
        RubOperation::UncommittedToCommit(..) => "amend",
        RubOperation::UncommittedToBranch(..) => "assign hunks",
        RubOperation::UncommittedToStack(..) => "assign hunks",
        RubOperation::StackToUncommittedArea(..) => "unassign hunks",
        RubOperation::StackToStack(..) => "reassign hunks",
        RubOperation::StackToBranch(..) => "reassign hunks",
        RubOperation::UncommittedAreaToCommit(..) => "amend",
        RubOperation::UncommittedAreaToBranch(..) => "assign hunks",
        RubOperation::UncommittedAreaToStack(..) => "assign hunks",
        RubOperation::CommitToUncommittedArea(CommitToUncommittedAreaOperation { commits }) => {
            if commits.len() == 1 {
                "undo commit"
            } else {
                "undo commits"
            }
        }
        RubOperation::CommitToStack(..) => "undo commit",
        RubOperation::SquashCommits(SquashCommitsOperation {
            sources: _,
            destination: _,
            how_to_combine_messages,
        }) => squash_operation_display(how_to_combine_messages),
        RubOperation::MoveCommitToBranch(..) => "move commit",
        RubOperation::BranchToUncommittedArea(..) => "unassign hunks",
        RubOperation::BranchToStack(..) => "reassign hunks",
        RubOperation::BranchToCommit(..) => "amend",
        RubOperation::BranchToBranch(..) => "reassign hunks",
        RubOperation::CommittedFileToBranch(..) => "uncommit file",
        RubOperation::CommittedFileToCommit(..) => "move file",
        RubOperation::CommittedFileToUncommittedArea(..) => "uncommit file",
        RubOperation::StackToCommit(..) => "amend",
    })
}

pub fn squash_operation_display(
    how_to_combine_messages: MessageCombinationStrategy,
) -> &'static str {
    match how_to_combine_messages {
        MessageCombinationStrategy::KeepBoth => "squash",
        MessageCombinationStrategy::KeepSubject => "squash (discard this message)",
        MessageCombinationStrategy::KeepTarget => "squash (use this message)",
    }
}

/// Executes a rub operation and returns which item should be selected after reloading.
pub fn perform_operation(
    ctx: &mut Context,
    operation: &RubOperation<'_>,
) -> anyhow::Result<Option<SelectAfterReload>> {
    let selection = match operation {
        RubOperation::UnassignUncommitted(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::UncommittedToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            result
                .new_commit
                .map(SelectAfterReload::Commit)
                .unwrap_or(SelectAfterReload::Uncommitted)
        }
        RubOperation::UncommittedToBranch(operation) => {
            let assignment = operation.hunk_assignments.first();
            let path = assignment.path_bytes.clone();
            let stack_id = assignment.stack_id;
            operation.execute_inner(ctx)?;
            SelectAfterReload::UncommittedFile { path, stack_id }
        }
        RubOperation::UncommittedToStack(operation) => {
            let path = operation.hunk_assignments.first().path_bytes.clone();
            operation.execute_inner(ctx)?;
            SelectAfterReload::UncommittedFile {
                path,
                stack_id: Some(operation.stack_id),
            }
        }
        RubOperation::StackToUncommittedArea(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::StackToStack(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Stack(operation.to)
        }
        RubOperation::StackToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.to.to_string())
        }
        RubOperation::UncommittedAreaToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            SelectAfterReload::Commit(result.new_commit.unwrap_or(operation.oid))
        }
        RubOperation::UncommittedAreaToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.to.to_string())
        }
        RubOperation::UncommittedAreaToStack(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Stack(operation.to)
        }
        RubOperation::CommitToUncommittedArea(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::CommitToStack(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Stack(operation.stack)
        }
        RubOperation::SquashCommits(operation) => {
            let result = operation.execute_inner(ctx)?;
            SelectAfterReload::Commit(result.new_commit)
        }
        RubOperation::MoveCommitToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.name.to_string())
        }
        RubOperation::BranchToUncommittedArea(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::BranchToStack(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Stack(operation.to)
        }
        RubOperation::BranchToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            result
                .new_commit
                .map(SelectAfterReload::Commit)
                .unwrap_or(SelectAfterReload::Branch(operation.name.to_string()))
        }
        RubOperation::BranchToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.to.to_string())
        }
        RubOperation::CommittedFileToBranch(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Branch(operation.name.to_string())
        }
        RubOperation::CommittedFileToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            let destination_to_select = result
                .workspace
                .replaced_commits
                .get(&operation.oid)
                .copied()
                .unwrap_or(operation.oid);
            SelectAfterReload::Commit(destination_to_select)
        }
        RubOperation::CommittedFileToUncommittedArea(operation) => {
            operation.execute_inner(ctx)?;
            SelectAfterReload::Uncommitted
        }
        RubOperation::StackToCommit(operation) => {
            let result = operation.execute_inner(ctx)?;
            SelectAfterReload::Commit(result.new_commit.unwrap_or(operation.to))
        }
    };

    Ok(Some(selection))
}

pub fn route_operation_from_detail_view<'a>(
    source: &'a CommittedHunk,
    target: &'a CliId,
) -> Option<RubFromDetailViewOperation<'a>> {
    match target {
        CliId::Commit { commit_id, .. } => {
            Some(RubFromDetailViewOperation::CommittedHunkToCommit {
                source,
                target: *commit_id,
            })
        }
        CliId::Uncommitted { id } => {
            Some(RubFromDetailViewOperation::CommittedHunkToUncommittedArea { source, id })
        }
        CliId::Stack { stack_id, .. } => {
            Some(RubFromDetailViewOperation::CommittedHunkToStack { source, stack_id })
        }
        CliId::Branch { stack_id, .. } => stack_id
            .as_ref()
            .map(|stack_id| RubFromDetailViewOperation::CommittedHunkToStack { source, stack_id }),
        CliId::UncommittedHunkOrFile(..)
        | CliId::PathPrefix { .. }
        | CliId::CommittedFile { .. } => None,
    }
}

#[derive(Debug, Copy, Clone)]
#[expect(clippy::enum_variant_names)]
pub enum RubFromDetailViewOperation<'a> {
    CommittedHunkToCommit {
        source: &'a CommittedHunk,
        target: gix::ObjectId,
    },
    CommittedHunkToUncommittedArea {
        source: &'a CommittedHunk,
        #[expect(dead_code)]
        id: &'a ShortId,
    },
    CommittedHunkToStack {
        source: &'a CommittedHunk,
        stack_id: &'a StackId,
    },
}

impl<'a> RubFromDetailViewOperation<'a> {
    pub fn execute(self, ctx: &mut Context) -> anyhow::Result<SelectAfterReload> {
        match self {
            RubFromDetailViewOperation::CommittedHunkToCommit { source, target } => {
                let CommittedHunk {
                    commit_id: source_commit_id,
                    header,
                    path,
                } = source;

                let changes =
                    single_hunk_changes(ctx, Arc::unwrap_or_clone(Arc::clone(path)), *header)?;

                let move_result = but_api::commit::move_changes::commit_move_changes_between(
                    ctx,
                    *source_commit_id,
                    target,
                    changes,
                    DryRun::No,
                )?;

                Ok(SelectAfterReload::Commit(
                    move_result
                        .workspace
                        .replaced_commits
                        .get(&target)
                        .with_context(|| {
                            format!("{target} not found in move_result.workspace.replaced_commits")
                        })
                        .copied()?,
                ))
            }
            RubFromDetailViewOperation::CommittedHunkToUncommittedArea { source, id: _ } => {
                let CommittedHunk {
                    commit_id: source_commit_id,
                    header,
                    path,
                } = source;

                let changes =
                    single_hunk_changes(ctx, Arc::unwrap_or_clone(Arc::clone(path)), *header)?;

                but_api::commit::uncommit::commit_uncommit_changes(
                    ctx,
                    *source_commit_id,
                    changes,
                    None,
                    DryRun::No,
                )?;

                Ok(SelectAfterReload::Uncommitted)
            }
            RubFromDetailViewOperation::CommittedHunkToStack { source, stack_id } => {
                let CommittedHunk {
                    commit_id: source_commit_id,
                    header,
                    path,
                } = source;

                let changes =
                    single_hunk_changes(ctx, Arc::unwrap_or_clone(Arc::clone(path)), *header)?;

                but_api::commit::uncommit::commit_uncommit_changes(
                    ctx,
                    *source_commit_id,
                    changes,
                    Some(*stack_id),
                    DryRun::No,
                )?;

                Ok(SelectAfterReload::Stack(*stack_id))
            }
        }
    }
}

fn single_hunk_changes(
    ctx: &Context,
    path: bstr::BString,
    header: but_core::HunkHeader,
) -> anyhow::Result<Vec<but_core::DiffSpec>> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
    builder.push_changes_from_single_hunk(path, header);
    Ok(builder.into_diff_specs())
}

pub fn rub_from_detail_view_operation_display(
    source: &CommittedHunk,
    target: &CliId,
) -> Option<&'static str> {
    Some(match route_operation_from_detail_view(source, target)? {
        RubFromDetailViewOperation::CommittedHunkToCommit { .. } => "amend commit",
        RubFromDetailViewOperation::CommittedHunkToUncommittedArea { .. }
        | RubFromDetailViewOperation::CommittedHunkToStack { .. } => "unassign hunk",
    })
}
