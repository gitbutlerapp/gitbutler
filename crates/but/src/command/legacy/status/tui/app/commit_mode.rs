use std::sync::Arc;

use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use gix::refs::Category;
use nonempty::NonEmpty;
use ratatui::{backend::Backend, prelude::Span};

use crate::{
    CliId,
    command::legacy::{
        commit,
        reword2::CommitMessageSource,
        status::{
            output::StatusOutputLineData,
            tui::{
                App, DetailsLayoutMessage, Message, Mode, ReloadCause, RewordMessage,
                SelectAfterReload,
                app::{MoveCursorDiration, mark::hunk_is_child_of},
                graph_extension::ExtensionDirection,
                render::{
                    ModeRender, OperationExtension, RenderSingleLineSpans,
                    render_commit_operation_target_marker, source_span,
                },
            },
        },
    },
    id::UncommittedHunkOrFile,
    tui::TerminalGuard,
    utils::targeting,
};

use super::{SquashMarks, SquashSource, mark::MarksRef};

#[derive(Debug, Clone)]
pub struct CommitMode {
    pub source: Arc<CommitSource>,
    pub insert_side: InsertSide,
    /// If set, then the commit must be made on this stack
    ///
    /// Used when committing changes staged to a specific stack
    // TODO: remove this when we no dont support assignments
    pub scope_to_stack: Option<StackId>,
    /// How to compose the commit message.
    pub message_composer: CommitMessageComposer,
}

#[derive(Debug, Copy, Clone, Default)]
pub enum CommitMessageComposer {
    /// Open an editor to compose the commit message.
    #[default]
    Editor,
    /// Use an inline editor to compose the commit message.
    Inline,
    /// Create the commit with an empty message.
    Empty,
}

/// A subset of [`CliId`] that supports being committed
#[derive(Debug)]
pub enum CommitSource {
    Marks(NonEmpty<UncommittedHunkOrFile>),
    Uncommitted,
    UncommittedHunk(UncommittedHunkOrFile),
}

impl ModeRender for CommitMode {
    fn operation_extension(&self, data: &StatusOutputLineData) -> Option<OperationExtension<'_>> {
        let direction = if matches!(data, StatusOutputLineData::Commit { .. }) {
            self.insert_side.into()
        } else if matches!(data, StatusOutputLineData::Branch { .. }) {
            ExtensionDirection::Below
        } else {
            return None;
        };

        Some(OperationExtension::Commit {
            mode: self,
            direction,
        })
    }

    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        if data
            .cli_id()
            .is_some_and(|target| self.source.contains(target))
        {
            render_commit_operation_target_marker(app, data, self, line);
        }
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

impl CommitSource {
    pub fn contains(&self, other: &CliId) -> bool {
        match self {
            CommitSource::Marks(hunks) => {
                if let CliId::UncommittedHunkOrFile(rhs) = other {
                    hunks
                        .iter()
                        .any(|lhs| lhs == rhs || hunk_is_child_of(rhs, lhs))
                } else {
                    false
                }
            }
            CommitSource::Uncommitted => {
                matches!(other, CliId::Uncommitted { .. })
            }
            CommitSource::UncommittedHunk(lhs) => {
                if let CliId::UncommittedHunkOrFile(rhs) = other {
                    lhs == rhs || hunk_is_child_of(rhs, lhs)
                } else {
                    false
                }
            }
        }
    }

    fn try_from_cli_id(id: &CliId) -> Option<Self> {
        match id {
            CliId::Branch(..) | CliId::Commit { .. } | CliId::Uncommitted { .. } => {
                Some(CommitSource::Uncommitted)
            }
            CliId::UncommittedHunkOrFile(hunk) => Some(CommitSource::UncommittedHunk(hunk.clone())),
            CliId::PathPrefix { .. } | CliId::CommittedFile { .. } | CliId::Stack { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CommitMessage {
    CreateEmpty,
    Start,
    StartWithSource(Arc<CliId>),
    ToggleMessageComposer(CommitMessageComposer),
    Confirm,
    CommitToNewBranch,
    ToggleInsertSide,
}

impl App {
    pub fn handle_commit<T>(
        &mut self,
        message: CommitMessage,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        match message {
            CommitMessage::CreateEmpty => self.handle_commit_create_empty(ctx, messages)?,
            CommitMessage::Start => self.handle_commit_start(messages),
            CommitMessage::StartWithSource(source) => {
                if let Some(source) = CommitSource::try_from_cli_id(&source) {
                    self.handle_commit_start_source(source);
                }
            }
            CommitMessage::Confirm => self.handle_commit_confirm(ctx, terminal_guard, messages)?,
            CommitMessage::ToggleMessageComposer(composer) => {
                self.handle_commit_toggle_message_composer(composer);
            }
            CommitMessage::CommitToNewBranch => {
                self.handle_commit_to_new_branch(ctx, terminal_guard, messages)?;
            }
            CommitMessage::ToggleInsertSide => {
                self.handle_commit_toggle_insert_side();
            }
        }

        Ok(())
    }

    fn handle_commit_start(&mut self, messages: &mut Vec<Message>) {
        match &*self.mode {
            Mode::Normal(..) => {
                if self.marks_ref().is_empty() {
                    let Some(source) = self
                        .cursor
                        .selected_line(&self.status_lines)
                        .and_then(|selection| selection.data.cli_id())
                        .and_then(|id| CommitSource::try_from_cli_id(id))
                    else {
                        return;
                    };
                    self.handle_commit_start_source(source);
                } else {
                    self.handle_commit_start_marks();
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
                        Message::Commit(CommitMessage::StartWithSource(Arc::clone(selection))),
                    ]);
                }
                MarksRef::Hunks { .. } => {
                    if details_mode.full_screen {
                        messages.push(Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit));
                    }
                    messages.extend([
                        Message::UnfocusDetails,
                        Message::Commit(CommitMessage::Start),
                    ]);
                }
                MarksRef::Commits { .. }
                | MarksRef::CommittedFiles { .. }
                | MarksRef::Branches { .. } => {}
            },
            Mode::Squash(squash_mode) => match &squash_mode.source {
                SquashSource::Uncommitted => {
                    self.handle_commit_start_source(CommitSource::Uncommitted);
                }
                SquashSource::UncommittedHunk(hunk) => {
                    self.handle_commit_start_source(CommitSource::UncommittedHunk(hunk.clone()));
                }
                SquashSource::Marks(squash_marks) => match squash_marks {
                    SquashMarks::Hunks(hunks) => {
                        self.handle_commit_start_source(CommitSource::Marks(hunks.clone()));
                    }
                    SquashMarks::Commits(..)
                    | SquashMarks::CommittedFiles(..)
                    | SquashMarks::Branches(..) => {}
                },
                SquashSource::Branch(..)
                | SquashSource::CommittedFile(..)
                | SquashSource::Commit(..) => {}
            },
            _ => {}
        }
    }

    fn handle_commit_start_source(&mut self, source: CommitSource) {
        let commit_mode = CommitMode {
            source: Arc::new(source),
            insert_side: InsertSide::Below,
            scope_to_stack: None,
            message_composer: CommitMessageComposer::default(),
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Commit(commit_mode);
            });

        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Down);
    }

    fn handle_commit_start_marks(&mut self) {
        let Mode::Normal(normal_mode) = &*self.mode else {
            return;
        };

        let Some(hunks) = normal_mode.marks.as_hunks().cloned() else {
            return;
        };

        let source = Arc::new(CommitSource::Marks(hunks));

        if let Some(cursor) = self
            .cursor
            .select_closest_commit_source(&self.status_lines, &source)
        {
            self.cursor = cursor;
        }

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Commit(CommitMode {
                    source,
                    insert_side: InsertSide::Below,
                    scope_to_stack: None,
                    message_composer: CommitMessageComposer::default(),
                });
            });

        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Down);
    }

    fn handle_commit_confirm<T>(
        &mut self,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Mode::Commit(
            mode @ CommitMode {
                source,
                insert_side,
                scope_to_stack: _,
                message_composer: _,
            },
        ) = &*self.mode
        else {
            return Ok(());
        };

        let Some(data) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|s| s.data.cli_id())
        else {
            return Ok(());
        };

        if source.contains(data) {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        let target = match &**data {
            CliId::Branch(branch) => commit::CommitRelativeToTarget::BranchTip {
                name: Category::LocalBranch.to_full_name(&*branch.name)?,
            },
            CliId::Commit { commit, id: _ } => commit::CommitRelativeToTarget::Commit {
                commit: commit.clone(),
                side: targeting::Side::from(*insert_side),
            },
            CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Uncommitted { .. }
            | CliId::Stack { .. } => return Ok(()),
        };
        let commit_op = commit::CommitOperation::CommitAt(commit::CommitAtOperation { target });

        commit_with(ctx, terminal_guard, messages, mode, commit_op)?;

        Ok(())
    }

    fn handle_commit_to_new_branch<T>(
        &mut self,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Mode::Commit(mode) = &*self.mode else {
            return Ok(());
        };

        let Some(data) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|s| s.data.cli_id())
        else {
            return Ok(());
        };

        let commit_op = match &**data {
            CliId::UncommittedHunkOrFile(..) | CliId::Uncommitted { .. } => {
                commit::CommitOperation::CommitToNewBranch(commit::CommitToNewBranchOperation {
                    branch_name: None,
                })
            }
            CliId::Branch(branch) => commit::CommitOperation::CommitAt(commit::CommitAtOperation {
                target: commit::CommitRelativeToTarget::BranchBucket {
                    name: Category::LocalBranch.to_full_name(&*branch.name)?,
                    side: targeting::Side::Above,
                },
            }),

            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Commit { .. }
            | CliId::Stack { .. } => return Ok(()),
        };

        commit_with(ctx, terminal_guard, messages, mode, commit_op)?;

        Ok(())
    }

    fn handle_commit_toggle_insert_side(&mut self) {
        let Mode::Commit(commit_mode) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        else {
            return;
        };
        commit_mode.insert_side = match commit_mode.insert_side {
            InsertSide::Above => InsertSide::Below,
            InsertSide::Below => InsertSide::Above,
        };
    }

    fn handle_commit_toggle_message_composer(&mut self, composer: CommitMessageComposer) {
        if let Mode::Commit(mode) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        {
            match composer {
                CommitMessageComposer::Editor => {
                    // you can't toggle the editor composer, that is always the default
                }
                CommitMessageComposer::Empty => {
                    mode.message_composer = match mode.message_composer {
                        CommitMessageComposer::Editor | CommitMessageComposer::Inline => {
                            CommitMessageComposer::Empty
                        }
                        CommitMessageComposer::Empty => CommitMessageComposer::Editor,
                    };
                }
                CommitMessageComposer::Inline => {
                    mode.message_composer = match mode.message_composer {
                        CommitMessageComposer::Editor | CommitMessageComposer::Empty => {
                            CommitMessageComposer::Inline
                        }
                        CommitMessageComposer::Inline => CommitMessageComposer::Editor,
                    };
                }
            }
        }
    }
}

fn commit_with<T>(
    ctx: &mut Context,
    terminal_guard: &mut T,
    messages: &mut Vec<Message>,
    mode: &CommitMode,
    commit_op: commit::CommitOperation,
) -> anyhow::Result<()>
where
    T: TerminalGuard,
    anyhow::Error: From<<T::Backend as Backend>::Error>,
{
    let CommitMode {
        source,
        message_composer,
        insert_side: _,
        scope_to_stack,
    } = mode;

    anyhow::ensure!(
        scope_to_stack.is_none(),
        "committing stack assignments is not supported. Use `but commit`"
    );

    let commit_selection = match &**source {
        CommitSource::Marks(hunks) => commit::CommitSelection::Changes(Box::new(hunks.clone())),
        CommitSource::Uncommitted => commit::CommitSelection::AllChanges,
        CommitSource::UncommittedHunk(hunk) => {
            commit::CommitSelection::Changes(Box::new(NonEmpty::new(hunk.clone())))
        }
    };

    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;

    let (reword_op, reword_msg) = match message_composer {
        CommitMessageComposer::Editor => (CommitMessageSource::Editor { initial: None }, None),
        CommitMessageComposer::Inline => (
            CommitMessageSource::Empty,
            Some(Message::Reword(RewordMessage::InlineStart)),
        ),
        CommitMessageComposer::Empty => (CommitMessageSource::Empty, None),
    };

    let _suspend_guard = reword_op
        .will_open_editor()
        .then(|| terminal_guard.suspend())
        .transpose()?;

    let (
        commit::CommitOutcome {
            new_commit,
            branch_name: _,
        },
        _ws,
    ) = commit::run(
        ctx,
        &mut meta,
        guard.write_permission(),
        commit_op,
        commit_selection,
        reword_op,
    )?;

    drop(_suspend_guard);

    messages.extend(
        [
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Commit(new_commit.commit_id)),
                ReloadCause::Mutation,
            ),
        ]
        .into_iter()
        .chain(reword_msg),
    );

    Ok(())
}
