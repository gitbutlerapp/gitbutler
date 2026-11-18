use but_ctx::Context;
use nonempty::NonEmpty;

use crate::{
    CliId,
    command::legacy::{
        pick::{self, PickOperation, PickOutcome},
        status::tui::{
            Message, ReloadCause, SelectAfterReload,
            app::{App, commit_mode::commit_operation_at, mark::MarksRef},
            mode::Mode,
            render::ModeRender,
            stop::{SelectedStop, Stop, StopMode},
        },
    },
    id::CommitId,
};

use super::mark::Marks;

#[derive(Debug, Clone)]
pub struct CherryPickMode {
    pub source: CherryPickSource,
    /// Which of the stops on the cursor line the picked commits go to, as
    /// [`StopMode::selected_stop`] says.
    pub stop: Option<SelectedStop>,
}

impl ModeRender for CherryPickMode {}

impl StopMode for CherryPickMode {
    fn stop_label(&self, stop: &Stop) -> Option<String> {
        let label = match stop {
            Stop::BranchTip(_) => "pick to branch",
            Stop::BelowCommit(_) => "pick below",
            Stop::NewBranchAbove(_) => "pick to new branch above",
            Stop::NewBranchBelow(_) => "pick to new branch below",
            Stop::NewStack => "pick to new stack",
        };
        Some(label.to_owned())
    }

    fn is_source(&self, id: &CliId) -> bool {
        self.source.contains(id)
    }

    fn selected_stop(&self) -> Option<&SelectedStop> {
        self.stop.as_ref()
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
                        CliId::AnonymousSegment(..)
                        | CliId::UncommittedHunkOrFile(..)
                        | CliId::PathPrefix { .. }
                        | CliId::CommittedFile { .. }
                        | CliId::CommittedHunk { .. }
                        | CliId::Branch(..)
                        | CliId::UncommittedArea { .. }
                        | CliId::Stack { .. } => return,
                    };

                    self.cherry_pick_start_with_source(source);
                }
                Marks::Commits(commits) => {
                    let source = CherryPickSource::Marks(CherryPickMarks::Commits(commits.clone()));

                    self.cherry_pick_start_with_source(source);
                }
                Marks::Hunks(..) | Marks::Committed(..) | Marks::Branches(..) => {}
            },
            _ => {}
        }
    }

    fn cherry_pick_start_with_source(&mut self, source: CherryPickSource) {
        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::CherryPick(CherryPickMode { source, stop: None });
            });
    }

    fn handle_cherry_pick_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::CherryPick(mode) = &*self.mode else {
            return Ok(());
        };

        let Some(stop) = self.stop_to_confirm(mode, messages) else {
            return Ok(());
        };

        let sources = match &mode.source {
            CherryPickSource::Marks(CherryPickMarks::Commits(commits)) => {
                commits.iter().map(|c| c.commit_id).collect()
            }
            CherryPickSource::Commit(commit) => Vec::from([commit.commit_id]),
        };
        let pick_operation = PickOperation {
            sources,
            commit_op: commit_operation_at(&stop)?,
            order_commits_by_parentage: true,
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
