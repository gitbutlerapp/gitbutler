use std::{borrow::Cow, sync::Arc};

use but_ctx::Context;
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use nonempty::NonEmpty;
use ratatui::prelude::Span;

use crate::{
    CliId,
    command::legacy::{
        rub::{CommitToUncommittedAreaOperation, RubOperation, SquashCommitsOperation},
        status::{
            FilesStatusFlag,
            output::StatusOutputLineData,
            tui::{
                App, DetailsLayoutMessage, Message, NOOP, ReloadCause, SelectAfterReload,
                app::mark::{MarkedCommit, Marks, MarksRef},
                cursor,
                mode::Mode,
                nonempty_from_refs, operations,
                render::{ModeRender, RenderSingleLineSpans, SpanExt, source_span},
            },
        },
    },
    id::{UNCOMMITTED, UncommittedHunkOrFile},
};

#[derive(Debug, Clone)]
pub struct RubMode {
    pub source: RubSource,
    pub available_targets: Vec<Arc<CliId>>,
    pub how_to_combine_messages: MessageCombinationStrategy,
}

#[derive(Debug, Clone, PartialEq)]
#[expect(clippy::large_enum_variant)]
pub enum RubSource {
    Marks(RubMarks),
    CliId(Arc<CliId>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RubMarks {
    Hunks(NonEmpty<UncommittedHunkOrFile>),
    Commits(NonEmpty<MarkedCommit>),
}

impl RubSource {
    pub fn contains(&self, other: &CliId) -> bool {
        match self {
            RubSource::Marks(marks) => marks.contains_cli_id(other),
            RubSource::CliId(source) => &**source == other,
        }
    }
}

impl RubMarks {
    pub fn as_ref(&self) -> MarksRef<'_> {
        match self {
            Self::Hunks(hunks) => MarksRef::from_hunks(hunks),
            Self::Commits(commits) => MarksRef::from_commits(commits),
        }
    }

    fn contains_cli_id(&self, other: &CliId) -> bool {
        let marks = self.as_ref();
        marks.contains_cli_id(other) || marks.contains_child_of(other)
    }

    fn to_cli_ids(&self) -> Vec<CliId> {
        match self {
            Self::Hunks(hunks) => hunks
                .iter()
                .cloned()
                .map(CliId::UncommittedHunkOrFile)
                .collect(),
            Self::Commits(commits) => commits
                .iter()
                .map(
                    |MarkedCommit {
                         commit_id,
                         id,
                         change_id,
                     }| CliId::Commit {
                        commit_id: *commit_id,
                        id: id.clone(),
                        change_id: change_id.clone(),
                    },
                )
                .collect(),
        }
    }
}

impl ModeRender for RubMode {
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        let Some(target) = data.cli_id() else {
            return;
        };

        if self.source.contains(target) {
            line.extend([source_span(app.theme), Span::raw(" ")]);
        }

        let display = match &self.source {
            RubSource::CliId(source) => Cow::Borrowed(
                rub_operation_display(NonEmpty::new(source), target, self.how_to_combine_messages)
                    .unwrap_or("invalid"),
            ),
            RubSource::Marks(marks) => {
                let sources = marks.to_cli_ids();
                let mut sources = sources.iter();
                let Some(sources) = sources
                    .next()
                    .map(|first| nonempty_from_refs(first, sources))
                else {
                    return;
                };
                Cow::Borrowed(
                    rub_operation_display(sources, target, self.how_to_combine_messages).unwrap_or(
                        {
                            if self.source.contains(target) {
                                NOOP
                            } else {
                                "invalid"
                            }
                        },
                    ),
                )
            }
        };
        line.extend([
            Span::raw("<< ").mode_colors(&*app.mode, app.theme),
            Span::raw(display).mode_colors(&*app.mode, app.theme),
            Span::raw(" >>").mode_colors(&*app.mode, app.theme),
            Span::raw(" "),
        ]);
    }

    fn render_operation_source_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        if let Some(cli_id) = data.cli_id()
            && self.source.contains(cli_id)
        {
            line.extend([source_span(app.theme), Span::raw(" ")]);
        }
    }
}

#[derive(Debug)]
pub enum RubMessage {
    Start,
    StartWithSource(Arc<CliId>),
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
            RubMessage::Start => self.handle_rub_start(messages),
            RubMessage::StartWithSource(source) => {
                self.handle_rub_start_with_source(RubSource::CliId(source));
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

    fn handle_rub_start(&mut self, messages: &mut Vec<Message>) {
        match &*self.mode {
            Mode::Normal(normal_mode) => {
                let Some(selected_line) = self.cursor.selected_line(&self.status_lines) else {
                    return;
                };
                let Some(cli_id) = selected_line.data.cli_id() else {
                    return;
                };
                match &normal_mode.marks {
                    Marks::Empty => {
                        self.handle_rub_start_with_source(RubSource::CliId(Arc::clone(cli_id)));
                    }
                    Marks::Hunks(hunks) => {
                        self.handle_rub_start_with_source(RubSource::Marks(RubMarks::Hunks(
                            hunks.clone(),
                        )));
                    }
                    Marks::Commits(commits) => {
                        self.handle_rub_start_with_source(RubSource::Marks(RubMarks::Commits(
                            commits.clone(),
                        )));
                    }
                    Marks::CommittedFiles(..) => {}
                }
            }
            Mode::Details(details_mode) => match details_mode.return_mode.marks() {
                MarksRef::Empty => {
                    let Some(selection) = self.details.selected_section_cli_id() else {
                        return;
                    };
                    if details_mode.full_screen {
                        messages.push(Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit));
                    }
                    messages.extend([
                        Message::UnfocusDetails,
                        Message::Rub(RubMessage::StartWithSource(Arc::clone(selection))),
                    ]);
                }
                MarksRef::Hunks { .. } => {
                    if details_mode.full_screen {
                        messages.push(Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit));
                    }
                    messages.extend([Message::UnfocusDetails, Message::Rub(RubMessage::Start)]);
                }
                MarksRef::Commits { .. } | MarksRef::CommittedFiles { .. } => {}
            },
            _ => {}
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
            RubSource::Marks(marks) => {
                let marks = marks.to_cli_ids();
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

    fn handle_rub_start_with_source(&mut self, source: RubSource) {
        match &source {
            RubSource::CliId(cli_id) => {
                if !supports_rubbing(cli_id) {
                    return;
                }
            }
            RubSource::Marks(..) => {}
        }

        let available_targets = self.available_targets_for_rub_mode(&source);

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Rub(RubMode {
                    source,
                    available_targets,
                    how_to_combine_messages: MessageCombinationStrategy::KeepBoth,
                });
            });

        if self
            .cursor
            .selected_line(&self.status_lines)
            .is_some_and(|line| {
                cursor::is_selectable_in_mode(line, self.mode.as_ref(), self.flags.show_files)
            })
        {
            return;
        }

        self.ensure_cursor_is_on_selectable_line();
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
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
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
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
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
            RubSource::Marks(marks) => {
                let mut sources = marks.to_cli_ids();
                sources.retain(|source| source != &**target);
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
