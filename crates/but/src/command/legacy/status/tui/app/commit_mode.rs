use std::sync::Arc;

use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use gix::refs::Category;
use nonempty::NonEmpty;
use ratatui::backend::Backend;

use crate::{
    CliId,
    command::legacy::{
        commit2, reword2,
        status::{
            output::StatusOutputLineData,
            tui::{
                App, Markable, Marks, Message, Mode, ReloadCause, RewordMessage, SelectAfterReload,
                stack_has_assigned_changes,
            },
        },
    },
    id::{ShortId, UNCOMMITTED, UncommittedHunkOrFile},
    tui::TerminalGuard,
};

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
#[expect(clippy::large_enum_variant)]
pub enum CommitSource {
    Marks(Marks),
    UncommittedArea(UncommittedAreaCommitSource),
    Uncommitted(UncommittedHunkOrFile),
    Stack(StackCommitSource),
}

#[derive(Debug)]
pub struct UncommittedAreaCommitSource {
    pub id: ShortId,
}

#[derive(Debug)]
pub struct StackCommitSource {
    pub stack_id: StackId,
}

impl CommitSource {
    pub fn try_new(id: CliId) -> Option<Self> {
        match id {
            CliId::Uncommitted { id } => {
                Some(Self::UncommittedArea(UncommittedAreaCommitSource { id }))
            }
            CliId::UncommittedHunkOrFile(uncommitted_cli_id) => {
                Some(Self::Uncommitted(uncommitted_cli_id))
            }
            CliId::Stack { stack_id, .. } => Some(Self::Stack(StackCommitSource { stack_id })),
            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Commit { .. } => None,
        }
    }

    pub fn contains(&self, other: &CliId) -> bool {
        match self {
            CommitSource::Marks(marks) => {
                Markable::try_from_cli_id(other).is_some_and(|markable| marks.contains(&markable))
            }
            CommitSource::UncommittedArea(UncommittedAreaCommitSource { id: lhs_id }) => {
                if let CliId::Uncommitted { id: rhs_id } = other {
                    lhs_id == rhs_id
                } else {
                    false
                }
            }
            CommitSource::Uncommitted(lhs) => {
                if let CliId::UncommittedHunkOrFile(rhs) = other {
                    lhs == rhs
                } else {
                    false
                }
            }
            CommitSource::Stack(StackCommitSource {
                stack_id: stack_id_lhs,
            }) => {
                if let CliId::Stack {
                    stack_id: stack_id_rhs,
                    ..
                } = other
                {
                    stack_id_lhs == stack_id_rhs
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum CommitMessage {
    CreateEmpty,
    Start,
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
            CommitMessage::Start => self.handle_commit_start(ctx)?,
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

    fn handle_commit_start(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        if self.marks().is_none_or(|marks| marks.is_empty()) {
            self.handle_commit_start_selection(ctx)?;
        } else {
            self.handle_commit_start_marks();
        }
        Ok(())
    }

    fn handle_commit_start_selection(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let commit_mode = match &selection.data {
            StatusOutputLineData::UncommittedChanges { cli_id } => {
                let Some(source) = CommitSource::try_new(Arc::unwrap_or_clone(Arc::clone(cli_id)))
                else {
                    return Ok(());
                };
                CommitMode {
                    source: Arc::new(source),
                    insert_side: InsertSide::Below,
                    scope_to_stack: None,
                    message_composer: CommitMessageComposer::default(),
                }
            }
            StatusOutputLineData::UncommittedFile { cli_id }
            | StatusOutputLineData::StagedChanges { cli_id }
            | StatusOutputLineData::StagedFile { cli_id } => {
                let Some(source) = CommitSource::try_new(Arc::unwrap_or_clone(Arc::clone(cli_id)))
                else {
                    return Ok(());
                };
                CommitMode {
                    source: Arc::new(source),
                    insert_side: InsertSide::Below,
                    scope_to_stack: cli_id.stack_id(),
                    message_composer: CommitMessageComposer::default(),
                }
            }
            StatusOutputLineData::Commit { stack_id, .. } => {
                let (source, scope_to_stack) = if let Some(stack_id) = *stack_id
                    && stack_has_assigned_changes(ctx, stack_id)?
                {
                    (
                        CommitSource::Stack(StackCommitSource { stack_id }),
                        Some(stack_id),
                    )
                } else {
                    (
                        CommitSource::UncommittedArea(UncommittedAreaCommitSource {
                            id: UNCOMMITTED.to_string(),
                        }),
                        None,
                    )
                };
                CommitMode {
                    scope_to_stack,
                    insert_side: InsertSide::Below,
                    message_composer: CommitMessageComposer::default(),
                    source: Arc::new(source),
                }
            }
            StatusOutputLineData::Branch { cli_id } => {
                let CliId::Branch { stack_id, .. } = &**cli_id else {
                    return Ok(());
                };
                let (source, scope_to_stack) = if let Some(stack_id) = *stack_id
                    && stack_has_assigned_changes(ctx, stack_id)?
                {
                    (
                        CommitSource::Stack(StackCommitSource { stack_id }),
                        Some(stack_id),
                    )
                } else {
                    (
                        CommitSource::UncommittedArea(UncommittedAreaCommitSource {
                            id: UNCOMMITTED.to_string(),
                        }),
                        None,
                    )
                };
                CommitMode {
                    source: Arc::new(source),
                    insert_side: InsertSide::Below,
                    scope_to_stack,
                    message_composer: CommitMessageComposer::default(),
                }
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Commit(commit_mode);
            });

        Ok(())
    }

    fn handle_commit_start_marks(&mut self) {
        let Mode::Normal(normal_mode) = &*self.mode else {
            return;
        };

        if normal_mode.marks.is_empty() {
            return;
        }

        let uncommitted = normal_mode
            .marks
            .iter()
            .cloned()
            .map(|mark| match mark {
                Markable::Uncommitted(uncommitted_cli_id) => Some(uncommitted_cli_id),
                Markable::Commit { .. } => None,
            })
            .collect::<Option<Vec<_>>>();
        let Some(uncommitted) = uncommitted else {
            return;
        };

        if uncommitted.is_empty() {
            return;
        }

        let source = Arc::new(CommitSource::Marks(normal_mode.marks.clone()));

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
            CliId::Branch { name, .. } => commit2::CommitRelativeToTarget::BranchTip {
                name: Category::LocalBranch.to_full_name(&**name)?,
            },
            CliId::Commit { commit_id, .. } => commit2::CommitRelativeToTarget::Commit {
                commit_id: *commit_id,
                position: commit2::CommitRelativeToTargetPosition::from(*insert_side),
            },
            CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Uncommitted { .. }
            | CliId::Stack { .. } => return Ok(()),
        };
        let commit_op = commit2::CommitOperation::CommitAt(commit2::CommitAtOperation { target });

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
                commit2::CommitOperation::CommitToNewBranch(commit2::CommitToNewBranchOperation {
                    branch_name: None,
                })
            }
            CliId::Branch { name, .. } => {
                commit2::CommitOperation::CommitAt(commit2::CommitAtOperation {
                    target: commit2::CommitRelativeToTarget::BranchBucket {
                        name: Category::LocalBranch.to_full_name(&**name)?,
                        position: commit2::CommitRelativeToTargetPosition::Above,
                    },
                })
            }

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
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
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
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
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
    commit_op: commit2::CommitOperation,
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
        CommitSource::Marks(marks) => {
            let mut hunks = Vec::new();
            for mark in marks {
                match mark {
                    Markable::Uncommitted(hunk) => {
                        hunks.push(hunk.clone());
                    }
                    Markable::Commit { .. } => {
                        anyhow::bail!("Error: Cannot commit a commit");
                    }
                }
            }
            let Some(hunks) = NonEmpty::from_vec(hunks) else {
                return Ok(());
            };
            commit2::CommitSelection::Changes(Box::new(hunks))
        }
        CommitSource::UncommittedArea(..) => commit2::CommitSelection::AllChanges,
        CommitSource::Uncommitted(hunk) => {
            commit2::CommitSelection::Changes(Box::new(NonEmpty::new(hunk.clone())))
        }
        CommitSource::Stack(..) => {
            anyhow::bail!("committing stack assignments is not supported. Use `but commit`")
        }
    };

    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;

    let (reword_op, reword_msg) = match message_composer {
        CommitMessageComposer::Editor => (reword2::RewordCommitOperation::UseEditor, None),
        CommitMessageComposer::Inline => (
            reword2::RewordCommitOperation::NoMessage,
            Some(Message::Reword(RewordMessage::InlineStart)),
        ),
        CommitMessageComposer::Empty => (reword2::RewordCommitOperation::NoMessage, None),
    };

    let _suspend_guard = reword_op
        .will_open_editor()
        .then(|| terminal_guard.suspend())
        .transpose()?;

    let commit2::CommitOutcome {
        new_commit,
        branch_name: _,
    } = commit2::run(
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
                Some(SelectAfterReload::Commit(new_commit)),
                ReloadCause::Mutation,
            ),
        ]
        .into_iter()
        .chain(reword_msg),
    );

    Ok(())
}
