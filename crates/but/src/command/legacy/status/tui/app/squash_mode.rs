use std::sync::Arc;

use but_ctx::Context;
use but_graph::Workspace;
use itertools::Either;
use nonempty::NonEmpty;
use ratatui::{prelude::Backend, text::Span};

use crate::{
    CliId, CliResultExt,
    args::atoms::{AllowMergedArg, BranchArg, ResolvedCliIdArgRef},
    command::legacy::{
        reword2::CommitMessageSource,
        squash::{
            self, HowToRewordTarget, ResolvedSquashArgsRef, SquashOperation, SquashOutcome,
            SquashTarget, resolve_target,
        },
        status::{
            FilesStatusFlag,
            output::StatusOutputLineData,
            tui::{
                DetailsLayoutMessage, Message, ReloadCause, SelectAfterReload,
                app::{App, MoveCursorDiration},
                mode::Mode,
                render::{ModeRender, RenderSingleLineSpans, SpanExt as _, source_span},
            },
        },
    },
    id::{BranchId, CommitId, CommittedFileId, UncommittedHunkOrFile},
    tui::TerminalGuard,
    utils::merged_upstream::MergedUpstream,
};

use super::{
    CommitSource, MoveSource,
    mark::{Marks, MarksRef},
};

#[derive(Debug, Clone)]
pub enum SquashSource {
    Marks(SquashMarks),
    Uncommitted,
    Commit(CommitId),
    UncommittedHunk(UncommittedHunkOrFile),
    Branch(BranchId),
    CommittedFile(CommittedFileId),
}

#[derive(Debug, Clone)]
pub enum SquashMarks {
    Hunks(NonEmpty<UncommittedHunkOrFile>),
    Commits(NonEmpty<CommitId>),
    CommittedFiles(NonEmpty<CommittedFileId>),
    Branches(NonEmpty<BranchId>),
}

impl SquashMarks {
    pub fn as_ref(&self) -> MarksRef<'_> {
        match self {
            Self::Hunks(hunks) => MarksRef::from_hunks(hunks),
            Self::Commits(commits) => MarksRef::from_commits(commits),
            Self::CommittedFiles(files) => MarksRef::from_committed_files(files),
            Self::Branches(branches) => MarksRef::from_branches(branches),
        }
    }
}

impl SquashSource {
    pub fn contains(&self, other: &CliId) -> bool {
        let marks = match self {
            SquashSource::Uncommitted => {
                return matches!(other, CliId::Uncommitted { .. });
            }
            SquashSource::Marks(marks) => marks.as_ref(),
            SquashSource::Branch(branch) => MarksRef::from_branch_ref(branch),
            SquashSource::Commit(commit) => MarksRef::from_commit_ref(commit),
            SquashSource::UncommittedHunk(hunk) => MarksRef::from_hunk_ref(hunk),
            SquashSource::CommittedFile(committed_file) => {
                MarksRef::from_committed_file_ref(committed_file)
            }
        };
        marks.contains_cli_id(other) || marks.contains_child_of(other)
    }

    pub fn can_target(&self, target: &CliId) -> bool {
        self.operation_for_target(target).is_some()
    }

    pub fn operation_for_target(&self, target: &CliId) -> Option<&'static str> {
        Some(match self.route(target)? {
            SquashRoute::UncommittedHunkToCommit { .. }
            | SquashRoute::UncommittedToBranch { .. }
            | SquashRoute::UncommittedHunkToBranch { .. }
            | SquashRoute::UncommittedToCommit { .. } => "amend",
            SquashRoute::CommitToCommit { .. }
            | SquashRoute::CommitToBranch { .. }
            | SquashRoute::BranchToCommit { .. }
            | SquashRoute::BranchToBranch { .. }
            | SquashRoute::CommittedFileToCommit { .. }
            | SquashRoute::CommittedFileToBranch { .. }
            | SquashRoute::BranchToSelf { .. } => "squash",
            SquashRoute::CommittedFileToUncommitted { .. }
            | SquashRoute::CommitToUncommitted { .. }
            | SquashRoute::BranchToUncommitted { .. } => "uncommit",
        })
    }

    fn route<'a>(&'a self, target: &'a CliId) -> Option<SquashRoute<'a>> {
        match self {
            SquashSource::Uncommitted => match target {
                CliId::Commit {
                    commit: target,
                    id: _,
                } => Some(SquashRoute::UncommittedToCommit {
                    target: target.clone(),
                }),
                CliId::Branch(branch) => Some(SquashRoute::UncommittedToBranch {
                    target: &branch.name,
                }),
                _ => None,
            },
            SquashSource::Commit(source_commit) => {
                squash_route_from_commit(source_commit.into(), target)
            }
            SquashSource::Marks(SquashMarks::Commits(source_commits)) => {
                squash_route_from_commit(source_commits.into(), target)
            }
            SquashSource::Branch(source_branch) => {
                squash_route_from_branch(source_branch.into(), target)
            }
            SquashSource::Marks(SquashMarks::Branches(source_branches)) => {
                squash_route_from_branch(source_branches.into(), target)
            }
            SquashSource::UncommittedHunk(source_hunk) => {
                squash_route_from_uncommitted_hunk(source_hunk.into(), target)
            }
            SquashSource::Marks(SquashMarks::Hunks(source_hunks)) => {
                squash_route_from_uncommitted_hunk(source_hunks.into(), target)
            }
            SquashSource::CommittedFile(source_file) => {
                squash_route_from_committed_file(source_file.into(), target)
            }
            SquashSource::Marks(SquashMarks::CommittedFiles(source_files)) => {
                squash_route_from_committed_file(source_files.into(), target)
            }
        }
    }
}

enum SquashRoute<'a> {
    UncommittedToCommit {
        target: CommitId,
    },
    UncommittedToBranch {
        target: &'a str,
    },
    UncommittedHunkToCommit {
        sources: NonEmptyRef<'a, UncommittedHunkOrFile>,
        target: CommitId,
    },
    UncommittedHunkToBranch {
        sources: NonEmptyRef<'a, UncommittedHunkOrFile>,
        target: &'a str,
    },
    CommitToUncommitted {
        sources: NonEmptyRef<'a, CommitId>,
    },
    CommitToCommit {
        sources: NonEmptyRef<'a, CommitId>,
        target: CommitId,
    },
    CommitToBranch {
        sources: NonEmptyRef<'a, CommitId>,
        target: &'a str,
    },
    BranchToCommit {
        sources: NonEmptyRef<'a, BranchId>,
        target: CommitId,
    },
    BranchToBranch {
        sources: NonEmptyRef<'a, BranchId>,
        target: &'a str,
    },
    BranchToUncommitted {
        sources: NonEmptyRef<'a, BranchId>,
    },
    BranchToSelf {
        source: &'a BranchId,
    },
    CommittedFileToCommit {
        sources: NonEmptyRef<'a, CommittedFileId>,
        target: CommitId,
    },
    CommittedFileToBranch {
        sources: NonEmptyRef<'a, CommittedFileId>,
        target: &'a str,
    },
    CommittedFileToUncommitted {
        sources: NonEmptyRef<'a, CommittedFileId>,
    },
}

#[derive(Debug, Clone)]
pub struct SquashMode {
    pub source: SquashSource,
    pub reword: SquashReword,
}

impl ModeRender for SquashMode {
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        let Some(target) = data.cli_id() else {
            return;
        };

        if let Some(display) = self.source.operation_for_target(target) {
            if self.source.contains(target) {
                line.extend([source_span(app.theme), Span::raw(" ")]);
            }

            line.render(Span::raw("<< ").mode_colors(&*app.mode, app.theme));
            line.render(Span::raw(display).mode_colors(&*app.mode, app.theme));
            match self.reword {
                SquashReword::Infer => {}
                SquashReword::UseTarget => {
                    line.render(
                        Span::raw(" (use this message)").mode_colors(&*app.mode, app.theme),
                    );
                }
            }
            line.render(Span::raw(" >>").mode_colors(&*app.mode, app.theme));
            line.render(Span::raw(" "));
        } else {
            if self.source.contains(target) {
                line.extend([source_span(app.theme), Span::raw(" ")]);
            }
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

#[derive(Debug, Copy, Clone)]
pub enum SquashReword {
    Infer,
    UseTarget,
}

#[derive(Debug)]
pub enum SquashMessage {
    Start,
    StartWith(Arc<CliId>),
    StartReverse,
    Confirm,
    UseTargetMessage,
}

impl App {
    pub fn handle_squash<T>(
        &mut self,
        squash_message: SquashMessage,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        match squash_message {
            SquashMessage::Start => self.handle_squash_start(messages),
            SquashMessage::StartWith(id) => self.handle_squash_start_with(id),
            SquashMessage::StartReverse => self.handle_squash_reverse(),
            SquashMessage::Confirm => self.handle_squash_confirm(ctx, terminal_guard, messages)?,
            SquashMessage::UseTargetMessage => self.handle_use_target_message(),
        }

        Ok(())
    }

    fn handle_squash_start(&mut self, messages: &mut Vec<Message>) {
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

                    messages.push(Message::Squash(SquashMessage::StartWith(Arc::clone(
                        selection,
                    ))));
                }
                Marks::Hunks(hunks) => self.squash_start_with_source(SquashSource::Marks(
                    SquashMarks::Hunks(hunks.clone()),
                )),
                Marks::Commits(commits) => self.squash_start_with_source(SquashSource::Marks(
                    SquashMarks::Commits(commits.clone()),
                )),
                Marks::CommittedFiles(files) => self.squash_start_with_source(SquashSource::Marks(
                    SquashMarks::CommittedFiles(files.clone()),
                )),
                Marks::Branches(branches) => self.squash_start_with_source(SquashSource::Marks(
                    SquashMarks::Branches(branches.clone()),
                )),
            },
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
                        Message::Squash(SquashMessage::StartWith(Arc::clone(selection))),
                    ]);
                }
                MarksRef::Hunks { .. } => {
                    if details_mode.full_screen {
                        messages.push(Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit));
                    }
                    messages.extend([
                        Message::UnfocusDetails,
                        Message::Squash(SquashMessage::Start),
                    ]);
                }
                MarksRef::Branches { .. }
                | MarksRef::Commits { .. }
                | MarksRef::CommittedFiles { .. } => {}
            },
            Mode::Commit(commit_mode) => match &*commit_mode.source {
                CommitSource::Uncommitted => {
                    self.squash_start_with_source(SquashSource::Uncommitted);
                }
                CommitSource::UncommittedHunk(hunk) => {
                    self.squash_start_with_source(SquashSource::UncommittedHunk(hunk.clone()));
                }
                CommitSource::Marks(hunks) => {
                    self.squash_start_with_source(SquashSource::Marks(SquashMarks::Hunks(
                        hunks.clone(),
                    )));
                }
            },
            Mode::Move(move_mode) => match &*move_mode.source {
                MoveSource::Marks(commits) => {
                    self.squash_start_with_source(SquashSource::Marks(SquashMarks::Commits(
                        commits.clone(),
                    )));
                }
                MoveSource::Commit(commit) => {
                    self.squash_start_with_source(SquashSource::Commit(commit.clone()));
                }
                MoveSource::Branch(branch) => {
                    self.squash_start_with_source(SquashSource::Branch(branch.clone()));
                }
            },
            _ => {}
        }
    }

    fn handle_squash_start_with(&mut self, source: Arc<CliId>) {
        match &*source {
            CliId::Uncommitted { .. } => {
                self.squash_start_with_source(SquashSource::Uncommitted);
            }
            CliId::Branch(branch) => {
                self.squash_start_with_source(SquashSource::Branch(branch.clone()));
            }
            CliId::Commit { commit, id: _ } => {
                self.squash_start_with_source(SquashSource::Commit(commit.clone()));
            }
            CliId::UncommittedHunkOrFile(hunk) => {
                self.squash_start_with_source(SquashSource::UncommittedHunk(hunk.clone()));
            }
            CliId::CommittedFile {
                committed_file,
                id: _,
            } => {
                self.squash_start_with_source(SquashSource::CommittedFile(committed_file.clone()));
            }
            CliId::PathPrefix { .. } | CliId::Stack { .. } => {}
        }
    }

    fn handle_squash_reverse(&mut self) {
        if !matches!(&*self.mode, Mode::Normal(..)) {
            return;
        }

        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
        else {
            return;
        };

        if matches!(&**selection, CliId::UncommittedHunkOrFile(..)) {
            return;
        }

        self.squash_start_with_source(SquashSource::Uncommitted);
    }

    fn squash_start_with_source(&mut self, source: SquashSource) {
        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Squash(SquashMode {
                    source,
                    reword: SquashReword::Infer,
                });
            });

        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Up);
    }

    fn handle_use_target_message(&mut self) {
        let Mode::Squash(SquashMode { source, reword, .. }) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        else {
            return;
        };
        if let Some(line) = self.cursor.selected_line(&self.status_lines)
            && let Some(target) = line.data.cli_id()
            && !source.can_target(target)
        {
            return;
        }
        *reword = match reword {
            SquashReword::Infer => SquashReword::UseTarget,
            SquashReword::UseTarget => SquashReword::Infer,
        };
    }

    fn handle_squash_confirm<T>(
        &mut self,
        ctx: &mut Context,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        let Mode::Squash(SquashMode { source, reword }) = &*self.mode else {
            return Ok(());
        };

        let Some(target) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
        else {
            return Ok(());
        };

        let mut guard = ctx.exclusive_worktree_access();
        let head_info = but_api::legacy::workspace::head_info(ctx)?;
        let merged = MergedUpstream::new(&*ctx.repo.get()?, &head_info, AllowMergedArg::default());
        let (repo, ws, _) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        let mut meta = ctx.meta()?;

        let Some(squash_op) =
            resolve_squash_operation(source, target, *reword, &repo, &ws, &head_info, &merged)?
        else {
            return Ok(());
        };

        drop(repo);
        drop(ws);

        let _suspend_guard = squash_op
            .will_open_editor()
            .then(|| terminal_guard.suspend())
            .transpose()?;

        let (outcome, _ws) = squash::run(ctx, &mut meta, guard.write_permission(), squash_op)?;

        let what_to_select = match outcome {
            SquashOutcome::Branch { new_commit, .. }
            | SquashOutcome::Commits { new_commit, .. }
            | SquashOutcome::Hunks { new_commit, .. } => {
                SelectAfterReload::Commit(new_commit.commit_id)
            }
            SquashOutcome::UncommitCommit { .. }
            | SquashOutcome::UncommitHunk { .. }
            | SquashOutcome::UncommitBranch { .. } => SelectAfterReload::Uncommitted,
        };

        drop(_suspend_guard);

        match self.flags.show_files {
            FilesStatusFlag::Commit(..) => {
                self.backstack.remove_show_file_list();
                self.flags.show_files = FilesStatusFlag::None;
            }
            FilesStatusFlag::None | FilesStatusFlag::All => {}
        }

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(Some(what_to_select), ReloadCause::Mutation),
        ]);

        Ok(())
    }
}

fn resolve_squash_operation<'a>(
    source: &'a SquashSource,
    target: &'a CliId,
    reword: SquashReword,
    repo: &gix::Repository,
    ws: &Workspace,
    head_info: &but_workspace::RefInfo,
    merged: &MergedUpstream,
) -> anyhow::Result<Option<SquashOperation<'a>>> {
    let Some(op) = source.route(target) else {
        return Ok(None);
    };

    let reword = match reword {
        SquashReword::Infer => {
            HowToRewordTarget::Reword(CommitMessageSource::Editor { initial: None })
        }
        SquashReword::UseTarget => HowToRewordTarget::UseTargetMessage,
    };

    let resolved_args = match op {
        SquashRoute::UncommittedToCommit { target } => ResolvedSquashArgsRef::Normal {
            sources: Vec::from([ResolvedCliIdArgRef::Uncommitted]),
            target: SquashTarget::Commit {
                commit: target,
                reword: HowToRewordTarget::UseTargetMessage,
            },
        },
        SquashRoute::UncommittedToBranch { target } => {
            let source = Vec::from([ResolvedCliIdArgRef::Uncommitted]);
            let target = ResolvedCliIdArgRef::Branch(target);
            resolve_squash_operation_with_branch(source, target, reword, head_info, repo)?
        }
        SquashRoute::UncommittedHunkToCommit { sources, target } => ResolvedSquashArgsRef::Normal {
            sources: sources
                .iter()
                .map(ResolvedCliIdArgRef::UncommittedHunkOrFile)
                .collect(),
            target: SquashTarget::Commit {
                commit: target,
                reword: HowToRewordTarget::UseTargetMessage,
            },
        },
        SquashRoute::CommittedFileToCommit { sources, target } => ResolvedSquashArgsRef::Normal {
            sources: sources
                .iter()
                .map(ResolvedCliIdArgRef::CommittedFile)
                .collect(),
            target: SquashTarget::Commit {
                commit: target,
                reword,
            },
        },
        SquashRoute::UncommittedHunkToBranch { sources, target } => {
            let source = sources
                .iter()
                .map(ResolvedCliIdArgRef::UncommittedHunkOrFile)
                .collect();
            let target = ResolvedCliIdArgRef::Branch(target);
            resolve_squash_operation_with_branch(source, target, reword, head_info, repo)?
        }
        SquashRoute::CommitToCommit { sources, target } => ResolvedSquashArgsRef::Normal {
            sources: sources
                .iter()
                .map(|source| ResolvedCliIdArgRef::Commit(source.as_ref()))
                .collect(),
            target: SquashTarget::Commit {
                commit: target,
                reword,
            },
        },
        SquashRoute::BranchToCommit { sources, target } => {
            let sources = sources
                .iter()
                .map(|branch| ResolvedCliIdArgRef::Branch(&branch.name))
                .collect();
            let target = ResolvedCliIdArgRef::Commit(target.as_ref());
            resolve_squash_operation_with_branch(sources, target, reword, head_info, repo)?
        }
        SquashRoute::BranchToBranch { sources, target } => {
            let sources = sources
                .iter()
                .map(|branch| ResolvedCliIdArgRef::Branch(&branch.name))
                .collect();
            let target = ResolvedCliIdArgRef::Branch(target);
            resolve_squash_operation_with_branch(sources, target, reword, head_info, repo)?
        }
        SquashRoute::CommitToBranch { sources, target } => {
            let sources = sources
                .iter()
                .map(|source| ResolvedCliIdArgRef::Commit(source.as_ref()))
                .collect();
            let target = ResolvedCliIdArgRef::Branch(target);
            resolve_squash_operation_with_branch(sources, target, reword, head_info, repo)?
        }
        SquashRoute::CommittedFileToBranch { sources, target } => {
            let sources = sources
                .iter()
                .map(ResolvedCliIdArgRef::CommittedFile)
                .collect();
            let target = ResolvedCliIdArgRef::Branch(target);
            resolve_squash_operation_with_branch(sources, target, reword, head_info, repo)?
        }
        SquashRoute::BranchToSelf { source } => {
            ResolvedSquashArgsRef::SingleBranchSourceAndTarget {
                branch: BranchArg(source.name.clone()),
                reword,
            }
        }
        SquashRoute::CommitToUncommitted { sources } => ResolvedSquashArgsRef::Normal {
            sources: sources
                .iter()
                .map(|source| ResolvedCliIdArgRef::Commit(source.as_ref()))
                .collect(),
            target: SquashTarget::Uncommitted,
        },
        SquashRoute::CommittedFileToUncommitted { sources } => ResolvedSquashArgsRef::Normal {
            sources: sources
                .iter()
                .map(ResolvedCliIdArgRef::CommittedFile)
                .collect(),
            target: SquashTarget::Uncommitted,
        },
        SquashRoute::BranchToUncommitted { sources } => ResolvedSquashArgsRef::Normal {
            sources: sources
                .iter()
                .map(|branch| ResolvedCliIdArgRef::Branch(&branch.name))
                .collect(),
            target: SquashTarget::Uncommitted,
        },
    };

    let op = squash::resolve(resolved_args, ws, repo, merged).into_internal_error()?;

    Ok(Some(op))
}

fn resolve_squash_operation_with_branch<'a>(
    sources: Vec<ResolvedCliIdArgRef<'a>>,
    target: ResolvedCliIdArgRef<'_>,
    reword: HowToRewordTarget,
    head_info: &but_workspace::RefInfo,
    repo: &gix::Repository,
) -> anyhow::Result<ResolvedSquashArgsRef<'a>> {
    let target = resolve_target(target, reword, head_info, repo).map_err(|err| match err {
        squash::ResolveTargetError::Other(err) => err,
        other => {
            anyhow::anyhow!("BUG: failed to compute squash target: {other:?}")
        }
    })?;

    Ok(ResolvedSquashArgsRef::Normal { sources, target })
}

#[derive(Debug)]
enum NonEmptyRef<'a, T> {
    Single(&'a T),
    List(&'a NonEmpty<T>),
}

impl<'a, T> From<&'a T> for NonEmptyRef<'a, T> {
    fn from(item: &'a T) -> Self {
        Self::Single(item)
    }
}

impl<'a, T> From<&'a NonEmpty<T>> for NonEmptyRef<'a, T> {
    fn from(value: &'a NonEmpty<T>) -> Self {
        Self::List(value)
    }
}

impl<'a, T> Clone for NonEmptyRef<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Copy for NonEmptyRef<'a, T> {}

impl<'a, T> NonEmptyRef<'a, T> {
    fn iter(self) -> impl Iterator<Item = &'a T> {
        match self {
            NonEmptyRef::Single(item) => Either::Left(std::iter::once(item)),
            NonEmptyRef::List(list) => Either::Right(list.iter()),
        }
    }

    fn len(self) -> usize {
        match self {
            NonEmptyRef::Single(_) => 1,
            NonEmptyRef::List(list) => list.len(),
        }
    }

    fn first(self) -> &'a T {
        match self {
            NonEmptyRef::Single(item) => item,
            NonEmptyRef::List(list) => &list.head,
        }
    }
}

fn squash_route_from_commit<'a>(
    source_commits: NonEmptyRef<'a, CommitId>,
    target: &'a CliId,
) -> Option<SquashRoute<'a>> {
    match target {
        CliId::Commit {
            commit: target,
            id: _,
        } => {
            if source_commits.len() == 1 {
                if source_commits.first().commit_id == target.commit_id {
                    None
                } else {
                    Some(SquashRoute::CommitToCommit {
                        sources: source_commits,
                        target: target.clone(),
                    })
                }
            } else {
                Some(SquashRoute::CommitToCommit {
                    sources: source_commits,
                    target: target.clone(),
                })
            }
        }
        CliId::Branch(branch) => Some(SquashRoute::CommitToBranch {
            sources: source_commits,
            target: &branch.name,
        }),
        CliId::Uncommitted { .. } => Some(SquashRoute::CommitToUncommitted {
            sources: source_commits,
        }),
        _ => None,
    }
}

fn squash_route_from_branch<'a>(
    source_branches: NonEmptyRef<'a, BranchId>,
    target: &'a CliId,
) -> Option<SquashRoute<'a>> {
    if source_branches.len() == 1
        && let CliId::Branch(target_branch) = target
        && source_branches.first() == target_branch
    {
        Some(SquashRoute::BranchToSelf {
            source: source_branches.first(),
        })
    } else {
        match target {
            CliId::Commit {
                commit: target,
                id: _,
            } => Some(SquashRoute::BranchToCommit {
                sources: source_branches,
                target: target.clone(),
            }),
            CliId::Branch(branch) => Some(SquashRoute::BranchToBranch {
                sources: source_branches,
                target: &branch.name,
            }),
            CliId::Uncommitted { .. } => Some(SquashRoute::BranchToUncommitted {
                sources: source_branches,
            }),
            _ => None,
        }
    }
}

fn squash_route_from_uncommitted_hunk<'a>(
    source_hunks: NonEmptyRef<'a, UncommittedHunkOrFile>,
    target: &'a CliId,
) -> Option<SquashRoute<'a>> {
    match target {
        CliId::Commit {
            commit: target,
            id: _,
        } => Some(SquashRoute::UncommittedHunkToCommit {
            sources: source_hunks,
            target: target.clone(),
        }),
        CliId::Branch(branch) => Some(SquashRoute::UncommittedHunkToBranch {
            sources: source_hunks,
            target: &branch.name,
        }),
        _ => None,
    }
}

fn squash_route_from_committed_file<'a>(
    source_files: NonEmptyRef<'a, CommittedFileId>,
    target: &'a CliId,
) -> Option<SquashRoute<'a>> {
    match target {
        CliId::Commit {
            commit: target,
            id: _,
        } => Some(SquashRoute::CommittedFileToCommit {
            sources: source_files,
            target: target.clone(),
        }),
        CliId::Branch(branch) => Some(SquashRoute::CommittedFileToBranch {
            sources: source_files,
            target: &branch.name,
        }),
        CliId::Uncommitted { .. } => Some(SquashRoute::CommittedFileToUncommitted {
            sources: source_files,
        }),
        _ => None,
    }
}
