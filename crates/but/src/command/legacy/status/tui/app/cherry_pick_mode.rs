use but_ctx::Context;
use but_rebase::graph_rebase::mutate::InsertSide;
use gix::{ObjectId, refs::Category};
use nonempty::NonEmpty;
use ratatui::text::Span;

use crate::{
    CliId,
    command::legacy::{
        commit::{CommitAtOperation, CommitOperation, CommitRelativeToTarget},
        pick::{self, PickOperation, PickOutcome},
        status::{
            output::StatusOutputLineData,
            tui::{
                Message, ReloadCause, SelectAfterReload,
                app::{App, mark::MarksRef},
                graph_extension::ExtensionDirection,
                mode::Mode,
                render::{
                    ModeRender, OperationExtension, RenderSingleLineSpans,
                    render_cherry_pick_operation_target_marker, source_span,
                },
            },
        },
    },
    id::CommitId,
};

use super::mark::Marks;

#[derive(Debug, Clone)]
pub struct CherryPickMode {
    pub source: CherryPickSource,
    pub insert_side: InsertSide,
}

impl CherryPickMode {}

impl ModeRender for CherryPickMode {
    fn operation_extension(&self, data: &StatusOutputLineData) -> Option<OperationExtension<'_>> {
        let direction = if let StatusOutputLineData::Commit { cli_id: target, .. } = data
            && !self.source.contains(target)
        {
            self.insert_side.into()
        } else if matches!(data, StatusOutputLineData::Branch { .. }) {
            ExtensionDirection::Below
        } else {
            return None;
        };

        Some(OperationExtension::CherryPick {
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
            render_cherry_pick_operation_target_marker(app, data, self, line);
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

#[derive(Debug, Clone)]
pub enum CherryPickSource {
    Marks(CherryPickMarks),
    Commit(CommitId),
}

impl CherryPickSource {
    pub fn contains(&self, other: &CliId) -> bool {
        let marks = match self {
            CherryPickSource::Marks(marks) => marks.as_ref(),
            CherryPickSource::Commit(commit) => MarksRef::from_commit_ref(commit),
        };
        marks.contains_cli_id(other) || marks.contains_child_of(other)
    }
}

#[derive(Debug, Clone)]
pub enum CherryPickMarks {
    Commits(NonEmpty<CommitId>),
}

impl CherryPickMarks {
    pub fn as_ref(&self) -> MarksRef<'_> {
        match self {
            Self::Commits(commits) => MarksRef::from_commits(commits),
        }
    }
}

#[derive(Debug)]
pub enum CherryPickMessage {
    Start,
    ToggleInsertSide,
    CherryPickToNewBranch,
    Confirm,
}

impl App {
    pub fn handle_cherry_pick(
        &mut self,
        cherry_pick_message: CherryPickMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match cherry_pick_message {
            CherryPickMessage::Start => self.handle_cherry_pick_start(),
            CherryPickMessage::ToggleInsertSide => self.handle_cherry_pick_toggle_insert_side(),
            CherryPickMessage::CherryPickToNewBranch => {
                self.handle_cherry_pick_to_new_branch(ctx, messages)?
            }
            CherryPickMessage::Confirm => self.handle_cherry_pick_confirm(ctx, messages)?,
        }

        Ok(())
    }

    #[expect(clippy::single_match)]
    fn handle_cherry_pick_start(&mut self) {
        match &*self.mode {
            Mode::Normal(normal_mode) => match &normal_mode.marks {
                Marks::Empty => {
                    let Some(selection) = self
                        .cursor
                        .selected_line(&self.status_lines)
                        .and_then(|line| line.data.cli_id())
                    else {
                        return;
                    };

                    let source = match &**selection {
                        CliId::Commit { commit, .. } => CherryPickSource::Commit(commit.clone()),
                        CliId::UncommittedHunkOrFile(..)
                        | CliId::PathPrefix { .. }
                        | CliId::CommittedFile { .. }
                        | CliId::Branch(..)
                        | CliId::Uncommitted { .. }
                        | CliId::Stack { .. } => return,
                    };

                    self.cherry_pick_start_with_source(source);
                }
                Marks::Commits(commits) => {
                    let source = CherryPickSource::Marks(CherryPickMarks::Commits(commits.clone()));

                    self.cherry_pick_start_with_source(source);
                }
                Marks::Hunks(..) | Marks::CommittedFiles(..) | Marks::Branches(..) => {}
            },
            _ => {}
        }
    }

    fn cherry_pick_start_with_source(&mut self, source: CherryPickSource) {
        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::CherryPick(CherryPickMode {
                    source,
                    insert_side: InsertSide::Below,
                });
            });
    }

    fn handle_cherry_pick_toggle_insert_side(&mut self) {
        let Mode::CherryPick(cherry_pick_mode) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        else {
            return;
        };
        cherry_pick_mode.insert_side = match cherry_pick_mode.insert_side {
            InsertSide::Above => InsertSide::Below,
            InsertSide::Below => InsertSide::Above,
        };
    }

    fn handle_cherry_pick_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        self.cherry_pick_confirm_with(ctx, messages, |commits, target, insert_side| match target {
            CliId::Branch(branch_id) => {
                let name = Category::LocalBranch.to_full_name(&*branch_id.name)?;
                Ok(Some(PickOperation {
                    sources: commits,
                    commit_op: CommitOperation::CommitAt(CommitAtOperation {
                        target: CommitRelativeToTarget::BranchTip { name },
                    }),
                    order_commits_by_parentage: true,
                }))
            }
            CliId::Commit { commit: target, .. } => Ok(Some(PickOperation {
                sources: commits,
                commit_op: CommitOperation::CommitAt(CommitAtOperation {
                    target: CommitRelativeToTarget::Commit {
                        commit: target.clone(),
                        side: insert_side.into(),
                    },
                }),
                order_commits_by_parentage: true,
            })),

            CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Uncommitted { .. }
            | CliId::Stack { .. } => Ok(None),
        })
    }

    fn handle_cherry_pick_to_new_branch(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        self.cherry_pick_confirm_with(ctx, messages, |commits, target, _| match target {
            CliId::Branch(branch_id) => {
                let name = Category::LocalBranch.to_full_name(&*branch_id.name)?;
                Ok(Some(PickOperation {
                    sources: commits,
                    commit_op: CommitOperation::CommitAt(CommitAtOperation {
                        target: CommitRelativeToTarget::BranchBucket {
                            name,
                            side: InsertSide::Above.into(),
                        },
                    }),
                    order_commits_by_parentage: true,
                }))
            }
            CliId::Commit { .. }
            | CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Uncommitted { .. }
            | CliId::Stack { .. } => Ok(None),
        })
    }

    fn cherry_pick_confirm_with<F>(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
        make_pick_operation: F,
    ) -> anyhow::Result<()>
    where
        F: FnOnce(Vec<ObjectId>, &CliId, InsertSide) -> anyhow::Result<Option<PickOperation>>,
    {
        let Mode::CherryPick(CherryPickMode {
            source,
            insert_side,
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

        if source.contains(target) {
            messages.push(Message::EnterNormalModeAfterConfirmingOperation);
            return Ok(());
        }

        let commits = match source {
            CherryPickSource::Marks(CherryPickMarks::Commits(commits)) => {
                commits.iter().map(|c| c.commit_id).collect()
            }
            CherryPickSource::Commit(commit) => Vec::from([commit.commit_id]),
        };

        let Some(pick_operation) = make_pick_operation(commits, target, *insert_side)? else {
            return Ok(());
        };

        let mut guard = ctx.exclusive_worktree_access();
        let mut meta = ctx.meta()?;

        let (PickOutcome { new_commits, .. }, _ws) =
            pick::run(ctx, &mut meta, guard.write_permission(), pick_operation)?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                new_commits
                    .first()
                    .map(|commit| SelectAfterReload::Commit(commit.commit_id)),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }
}
