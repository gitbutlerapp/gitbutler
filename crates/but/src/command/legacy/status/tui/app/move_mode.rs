use std::sync::Arc;

use but_core::DiffSpec;
use but_ctx::Context;
use gix::refs::Category;
use nonempty::NonEmpty;

use crate::{
    CliId,
    command::legacy::{
        r#move::{
            self, MoveChangesRelativeToOperation, MoveChangesToNewBranchOperation,
            MoveCommitsRelativeToOperation, MoveCommitsToNewBranchOperation, MoveOperation,
            MoveOutcome as MoveOperationOutcome, StackBranchOnOperation, UnstackBranchOperation,
        },
        reword2::CommitMessageSource,
        status::tui::{
            App, DetailsLayoutMessage, Message, Mode, ReloadCause, SelectAfterReload,
            render::ModeRender,
            stop::{SelectedStop, Stop, StopMode},
        },
    },
    id::{BranchId, CommitId},
    utils::{diff_specs::DiffSpecBuilder, targeting::Side},
};

use super::{
    MoveCursorDiration, SquashMarks, SquashSource,
    mark::{CommittedMark, MarksRef, committed_changes_to_act_on},
};

#[derive(Debug, Clone)]
pub struct MoveMode {
    pub source: MoveSource,
    /// Which of the stops on the cursor line the source moves to, as
    /// [`StopMode::selected_stop`] says.
    pub stop: Option<SelectedStop>,
}

/// A subset of [`CliId`] that supports being moved
#[derive(Debug, Clone)]
pub enum MoveSource {
    Marks(MoveMarks),
    Commit(CommitId),
    CommittedChange(CommittedMark),
    Branch(BranchId),
}

#[derive(Debug, Clone)]
pub enum MoveMarks {
    Commits(NonEmpty<CommitId>),
    Committed(NonEmpty<CommittedMark>),
}

impl MoveMarks {
    pub fn as_ref(&self) -> MarksRef<'_> {
        match self {
            MoveMarks::Commits(commits) => MarksRef::from_commits(commits),
            MoveMarks::Committed(marks) => MarksRef::from_committed(marks),
        }
    }
}

impl ModeRender for MoveMode {}

impl StopMode for MoveMode {
    fn stop_label(&self, stop: &Stop) -> Option<String> {
        let what = match &self.source {
            MoveSource::Branch(_) => {
                // Branches only stack on top of another branch or go to a stack of their own.
                return match stop {
                    Stop::NewBranchAbove(_) => Some("stack branch".to_owned()),
                    Stop::NewStack => Some("unstack branch".to_owned()),
                    Stop::BranchTip(_) | Stop::BelowCommit(_) | Stop::NewBranchBelow(_) => None,
                };
            }
            MoveSource::Commit(_) => "commit",
            MoveSource::Marks(MoveMarks::Commits(commits)) => {
                if commits.len() == 1 {
                    "commit"
                } else {
                    "commits"
                }
            }
            MoveSource::Marks(MoveMarks::Committed(marks)) => {
                committed_changes_noun(&committed_changes_to_act_on(marks))
            }
            MoveSource::CommittedChange(change) => committed_changes_noun(&NonEmpty::new(change)),
        };
        Some(match stop {
            Stop::BranchTip(_) => format!("move {what} to branch"),
            Stop::BelowCommit(_) => format!("move {what} below"),
            Stop::NewBranchAbove(_) => format!("move {what} to new branch above"),
            Stop::NewBranchBelow(_) => format!("move {what} to new branch below"),
            Stop::NewStack => format!("move {what} to new stack"),
        })
    }

    fn is_source(&self, id: &CliId) -> bool {
        self.source.contains(id)
    }

    fn selected_stop(&self) -> Option<&SelectedStop> {
        self.stop.as_ref()
    }
}

/// What committed `changes` are called, after whether they're files, hunks or both.
fn committed_changes_noun(changes: &NonEmpty<&CommittedMark>) -> &'static str {
    let file_count = changes
        .iter()
        .filter(|change| matches!(change, CommittedMark::File(..)))
        .count();
    match (file_count, changes.len()) {
        (1, 1) => "file",
        (0, 1) => "hunk",
        (file_count, change_count) if file_count == change_count => "files",
        (0, _) => "hunks",
        _ => "changes",
    }
}

impl MoveSource {
    pub fn contains(&self, other: &CliId) -> bool {
        match self {
            MoveSource::Marks(marks) => marks.as_ref().contains_cli_id(other),
            MoveSource::Commit(lhs) => {
                matches!(other, CliId::Commit{ commit: rhs, .. } if lhs == rhs)
            }
            MoveSource::CommittedChange(change) => {
                MarksRef::from_committed_ref(change).contains_cli_id(other)
            }
            MoveSource::Branch(lhs) => {
                matches!(other, CliId::Branch(rhs) if lhs == rhs)
            }
        }
    }

    fn try_from_cli_id(id: &CliId) -> Option<Self> {
        match id {
            CliId::Branch(branch) => Some(Self::Branch(branch.clone())),
            CliId::Commit { commit, .. } => Some(Self::Commit(commit.clone())),
            CliId::CommittedFile { committed_file, .. } => Some(Self::CommittedChange(
                CommittedMark::File(committed_file.clone()),
            )),
            CliId::CommittedHunk(hunk) => {
                Some(Self::CommittedChange(CommittedMark::Hunk(hunk.clone())))
            }
            CliId::AnonymousSegment(..)
            | CliId::UncommittedHunkOrFile(..)
            | CliId::PathPrefix { .. }
            | CliId::UncommittedArea { .. }
            | CliId::Stack { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum MoveMessage {
    Start,
    StartWith(Arc<CliId>),
    Confirm,
}

impl App {
    pub fn handle_move(
        &mut self,
        move_message: MoveMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match move_message {
            MoveMessage::Start => self.handle_move_start(messages),
            MoveMessage::StartWith(id) => self.handle_move_start_with(&id),
            MoveMessage::Confirm => self.handle_move_confirm(ctx, messages)?,
        }

        Ok(())
    }

    fn handle_move_start(&mut self, messages: &mut Vec<Message>) {
        match self.mode.marks_ref() {
            MarksRef::Branches { .. } => return,
            MarksRef::Empty
            | MarksRef::Hunks { .. }
            | MarksRef::Commits { .. }
            | MarksRef::Committed { .. } => {}
        }

        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|selection| selection.data.cli_id())
        else {
            return;
        };

        let move_mode = match &*self.mode {
            Mode::Normal(normal_mode) => match normal_mode.marks.as_ref() {
                MarksRef::Empty => {
                    let Some(source) = MoveSource::try_from_cli_id(selection) else {
                        return;
                    };
                    MoveMode { source, stop: None }
                }
                MarksRef::Commits { head, tail } => MoveMode {
                    source: MoveSource::Marks(MoveMarks::Commits(NonEmpty {
                        head: head.clone(),
                        tail: tail.to_vec(),
                    })),
                    stop: None,
                },
                MarksRef::Committed { head, tail } => MoveMode {
                    source: MoveSource::Marks(MoveMarks::Committed(NonEmpty {
                        head: head.clone(),
                        tail: tail.to_vec(),
                    })),
                    stop: None,
                },
                MarksRef::Branches { .. } | MarksRef::Hunks { .. } => return,
            },
            Mode::Details(details_mode) => {
                // The targets are in the status, so leave the details to move what's selected.
                let message = match details_mode.return_mode.marks() {
                    MarksRef::Empty => {
                        let Some(selection) = self.details.selected_section_cli_id() else {
                            return;
                        };
                        MoveMessage::StartWith(Arc::clone(selection))
                    }
                    MarksRef::Commits { .. } | MarksRef::Committed { .. } => MoveMessage::Start,
                    MarksRef::Branches { .. } | MarksRef::Hunks { .. } => return,
                };
                if details_mode.full_screen {
                    messages.push(Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit));
                }
                messages.extend([Message::UnfocusDetails, Message::Move(message)]);
                return;
            }
            Mode::Squash(squash_mode) => match &squash_mode.source {
                SquashSource::Marks(squash_marks) => match squash_marks {
                    SquashMarks::Commits(commits) => MoveMode {
                        source: MoveSource::Marks(MoveMarks::Commits(commits.clone())),
                        stop: None,
                    },
                    SquashMarks::Committed(marks) => MoveMode {
                        source: MoveSource::Marks(MoveMarks::Committed(marks.clone())),
                        stop: None,
                    },
                    SquashMarks::Hunks(..) | SquashMarks::Branches(..) => return,
                },
                SquashSource::Commit(commit) => MoveMode {
                    source: MoveSource::Commit(commit.clone()),
                    stop: None,
                },
                SquashSource::Branch(branch) => MoveMode {
                    source: MoveSource::Branch(branch.clone()),
                    stop: None,
                },

                SquashSource::CommittedChange(change) => MoveMode {
                    source: MoveSource::CommittedChange(change.clone()),
                    stop: None,
                },
                SquashSource::UncommittedHunk(..) | SquashSource::Uncommitted(..) => return,
            },
            _ => return,
        };

        self.move_start_with_mode(move_mode);
    }

    fn handle_move_start_with(&mut self, source: &CliId) {
        let Some(source) = MoveSource::try_from_cli_id(source) else {
            return;
        };
        self.move_start_with_mode(MoveMode { source, stop: None });
    }

    fn move_start_with_mode(&mut self, move_mode: MoveMode) {
        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Move(move_mode);
            });

        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Down);
    }

    fn handle_move_confirm(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Move(mode) = &*self.mode else {
            return Ok(());
        };

        let Some(stop) = self.stop_to_confirm(mode, messages) else {
            return Ok(());
        };

        let move_op = match &mode.source {
            MoveSource::Commit(commit) => {
                move_commits_operation(NonEmpty::new(commit.clone()), &stop)?
            }
            MoveSource::Marks(MoveMarks::Commits(commits)) => {
                move_commits_operation(commits.clone(), &stop)?
            }
            MoveSource::Marks(MoveMarks::Committed(marks)) => {
                move_committed_changes_operation(ctx, committed_changes_to_act_on(marks), &stop)?
            }
            MoveSource::CommittedChange(change) => {
                move_committed_changes_operation(ctx, NonEmpty::new(change), &stop)?
            }
            MoveSource::Branch(source) => {
                let source_branch = Category::LocalBranch.to_full_name(source.name.as_str())?;
                match &stop {
                    Stop::NewBranchAbove(target) => {
                        MoveOperation::StackBranch(StackBranchOnOperation {
                            source_branch,
                            target_branch: Category::LocalBranch
                                .to_full_name(target.name.as_str())?,
                        })
                    }
                    Stop::NewStack => {
                        MoveOperation::UnstackBranch(UnstackBranchOperation { source_branch })
                    }
                    Stop::BranchTip(_) | Stop::BelowCommit(_) | Stop::NewBranchBelow(_) => {
                        return Ok(());
                    }
                }
            }
        };

        let selection_after_reload = move_with(ctx, move_op)?;

        messages.extend([
            Message::CloseCommitFileListAfterConfirmingOperation,
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(selection_after_reload, ReloadCause::Mutation),
        ]);

        Ok(())
    }
}

/// Where moving commits or changes to `stop` puts them, relative to what's there, or `None` for
/// a new stack.
fn move_target_at(stop: &Stop) -> anyhow::Result<Option<r#move::MoveTarget>> {
    Ok(Some(match stop {
        Stop::BranchTip(branch) => r#move::MoveTarget::BranchTip {
            name: Category::LocalBranch.to_full_name(branch.name.as_str())?,
        },
        Stop::BelowCommit(commit) => r#move::MoveTarget::Commit {
            commit: commit.clone(),
            side: Side::Below,
        },
        Stop::NewBranchAbove(branch) => new_branch_beside(branch, Side::Above)?,
        Stop::NewBranchBelow(branch) => new_branch_beside(branch, Side::Below)?,
        Stop::NewStack => return Ok(None),
    }))
}

fn new_branch_beside(branch: &BranchId, side: Side) -> anyhow::Result<r#move::MoveTarget> {
    Ok(r#move::MoveTarget::BranchBucket {
        name: Category::LocalBranch.to_full_name(branch.name.as_str())?,
        side,
        new_branch_name: None,
    })
}

fn move_commits_operation(
    sources: NonEmpty<CommitId>,
    stop: &Stop,
) -> anyhow::Result<MoveOperation> {
    Ok(match move_target_at(stop)? {
        Some(target) => {
            MoveOperation::CommitsRelativeTo(MoveCommitsRelativeToOperation { sources, target })
        }
        None => MoveOperation::CommitsToNewBranch(MoveCommitsToNewBranchOperation {
            sources,
            branch_name: None,
        }),
    })
}

/// Moves `changes`, which all come from one commit.
fn move_committed_changes_operation(
    ctx: &Context,
    changes: NonEmpty<&CommittedMark>,
    stop: &Stop,
) -> anyhow::Result<MoveOperation> {
    let source_commit = changes.head.committed_file().to_commit_id();
    let changes = changes_in_committed(ctx, changes)?;
    Ok(match move_target_at(stop)? {
        Some(target) => MoveOperation::ChangesRelativeTo(MoveChangesRelativeToOperation {
            source_commit,
            changes,
            target,
            reword: CommitMessageSource::Keep,
        }),
        None => MoveOperation::ChangesToNewBranch(MoveChangesToNewBranchOperation {
            source_commit,
            changes,
            branch_name: None,
            reword: CommitMessageSource::Keep,
        }),
    })
}

fn changes_in_committed<'a>(
    ctx: &Context,
    changes: impl IntoIterator<Item = &'a CommittedMark>,
) -> anyhow::Result<NonEmpty<DiffSpec>> {
    let repo = ctx.repo.get()?;
    let mut builder = DiffSpecBuilder::new(&repo, ctx.settings.context_lines);
    for change in changes {
        match change {
            CommittedMark::File(file) => {
                builder.push_changes_from_committed_file(file.commit_id, file.path.as_ref())?
            }
            CommittedMark::Hunk(hunk) => builder.push_changes_from_committed_hunk(hunk)?,
        }
    }
    let changes = NonEmpty::from_vec(builder.into_diff_specs())
        .expect("BUG: Cannot possibly not have any changes here");
    Ok(changes)
}

fn move_with(
    ctx: &mut Context,
    move_op: MoveOperation,
) -> anyhow::Result<Option<SelectAfterReload>> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let (outcome, _ws) = r#move::run(ctx, &mut meta, guard.write_permission(), move_op, false)?;

    Ok(match outcome {
        MoveOperationOutcome::Commits { moved_commits, .. } => {
            Some(SelectAfterReload::Commit(moved_commits.head.commit_id))
        }
        MoveOperationOutcome::Changes { new_commit, .. } => {
            Some(SelectAfterReload::Commit(new_commit.commit_id))
        }
        MoveOperationOutcome::StackBranch { source_branch, .. }
        | MoveOperationOutcome::UnstackBranch { source_branch } => Some(SelectAfterReload::Branch(
            source_branch.shorten().to_string(),
        )),
    })
}
