use but_ctx::Context;
use gix::{ObjectId, refs::Category};
use nonempty::NonEmpty;

use crate::{
    CliId,
    command::legacy::{
        discard::{self, DiscardOperation, DiscardOutcome, UncommittedSelection},
        status::{
            output::StatusOutputLineData,
            tui::{
                Message, ReloadCause, SelectAfterReload, Selectable,
                app::{App, Modal},
                confirm::Confirm,
                message_on_drop,
                mode::Mode,
                operations,
            },
        },
    },
    id::CommitId,
    theme,
};

use super::mark::Marks;

impl App {
    pub fn handle_discard(&mut self, messages: &mut Vec<Message>) -> anyhow::Result<()> {
        if self.marks_ref().is_empty() {
            self.handle_discard_selection(messages)
        } else {
            self.handle_discard_marks(messages)
        }
    }

    pub fn handle_discard_selection(&mut self, messages: &mut Vec<Message>) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };
        let Some(cli_id) = selection.data.cli_id() else {
            return Ok(());
        };

        self.modal = Some(Modal::Confirm {
            confirm: match &**cli_id {
                CliId::Uncommitted { .. } => {
                    self.to_be_discarded = Vec::from([Selectable::Uncommitted]);
                    // The uncommitted header remains selectable when staged assignments consume
                    // all changes. Preserve confirming discard as a no-op in that state.
                    let has_uncommitted_changes = self.status_lines.iter().any(|line| {
                        matches!(line.data, StatusOutputLineData::UncommittedFile { .. })
                    });
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new("Discard uncommitted changes?".into()),
                        self.theme,
                        move |ctx, messages| {
                            if has_uncommitted_changes {
                                let DiscardOutcome::Uncommitted { paths: _ } = run_discard(
                                    ctx,
                                    DiscardOperation::Uncommitted(UncommittedSelection::All),
                                )?
                                else {
                                    anyhow::bail!(
                                        "BUG: uncommitted discard returned an unexpected outcome"
                                    )
                                };
                            }
                            messages.push(Message::Reload(
                                Some(SelectAfterReload::Uncommitted),
                                ReloadCause::Mutation,
                            ));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::UncommittedHunkOrFile(uncommitted) => {
                    self.to_be_discarded =
                        Vec::from([Selectable::UncommittedHunkOrFile(uncommitted.clone())]);
                    let operation = DiscardOperation::Uncommitted(UncommittedSelection::changes(
                        NonEmpty::new(uncommitted.clone()),
                    ));

                    // Discarding only part of a file: select the previous selectable line.
                    let select_after_reload = self.cursor.select_previous_cli_id_or_uncommitted(
                        &self.status_lines,
                        &self.mode,
                        self.flags.show_files,
                    );

                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new("Discard uncommitted file?".into()),
                        self.theme,
                        move |ctx, messages| {
                            let DiscardOutcome::Uncommitted { paths: _ } =
                                run_discard(ctx, operation)?
                            else {
                                anyhow::bail!(
                                    "BUG: uncommitted discard returned an unexpected outcome"
                                )
                            };
                            messages.push(Message::Reload(
                                Some(select_after_reload),
                                ReloadCause::Mutation,
                            ));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Commit { commit, id: _ } => {
                    self.to_be_discarded = Vec::from([Selectable::Commit(commit.clone())]);
                    let commit = commit.clone();
                    let select_after_reload = self
                        .cursor
                        .select_after_discarded_commit(&self.status_lines);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new(format!("Discard commit {}?", theme::Commit(&commit)).into()),
                        self.theme,
                        move |ctx, messages| {
                            let DiscardOutcome::Commits {
                                commits: _,
                                replaced_commits,
                            } = run_discard(ctx, DiscardOperation::Commits(NonEmpty::new(commit)))?
                            else {
                                anyhow::bail!("BUG: commit discard returned an unexpected outcome")
                            };
                            let select_after_reload =
                                map_selected_commits(select_after_reload, |commit_id| {
                                    replaced_commits
                                        .get(&commit_id)
                                        .copied()
                                        .unwrap_or(commit_id)
                                });
                            messages
                                .push(Message::Reload(select_after_reload, ReloadCause::Mutation));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Branch(branch) => {
                    let name = branch.name.to_owned();
                    let ref_name = Category::LocalBranch.to_full_name(&*name)?;

                    self.to_be_discarded = Vec::from([Selectable::Branch(branch.clone())]);

                    let select_after_reload = self
                        .cursor
                        .select_after_discarded_branch(&self.status_lines);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);

                    Confirm::new(
                        NonEmpty::new(format!("Discard branch {name}?").into()),
                        self.theme,
                        move |ctx, messages| {
                            let DiscardOutcome::Branches(_) = run_discard(
                                ctx,
                                DiscardOperation::Branches(NonEmpty::new(ref_name)),
                            )?
                            else {
                                anyhow::bail!("BUG: branch discard returned an unexpected outcome")
                            };

                            messages
                                .push(Message::Reload(select_after_reload, ReloadCause::Mutation));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::CommittedFile {
                    committed_file,
                    id: _,
                } => {
                    let commit = CommitId {
                        commit_id: committed_file.commit_id,
                        change_id: committed_file.change_id.clone(),
                    };
                    let path = committed_file.path.to_owned();

                    self.to_be_discarded =
                        Vec::from([Selectable::CommittedFile(committed_file.clone())]);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);

                    Confirm::new(
                        NonEmpty::new(format!("Discard changes to {path}?").into()),
                        self.theme,
                        move |ctx, messages| {
                            let DiscardOutcome::CommittedFiles {
                                source: _,
                                paths: _,
                                new_commit,
                            } = run_discard(
                                ctx,
                                DiscardOperation::CommittedFiles {
                                    source: commit,
                                    paths: NonEmpty::new(path),
                                },
                            )?
                            else {
                                anyhow::bail!(
                                    "BUG: committed file discard returned an unexpected outcome"
                                )
                            };

                            let select_after_reload =
                                if operations::commit_is_empty(ctx, new_commit.commit_id)? {
                                    SelectAfterReload::Commit(new_commit.commit_id)
                                } else {
                                    SelectAfterReload::FirstFileInCommit(new_commit.commit_id)
                                };
                            messages.push(Message::Reload(
                                Some(select_after_reload),
                                ReloadCause::Mutation,
                            ));

                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Stack { .. } | CliId::PathPrefix { .. } => return Ok(()),
            },
        });

        Ok(())
    }

    pub fn handle_discard_marks(&mut self, messages: &mut Vec<Message>) -> anyhow::Result<()> {
        let Mode::Normal(normal_mode) = &*self.mode else {
            return Ok(());
        };

        let operation = match &normal_mode.marks {
            Marks::Empty => return Ok(()),
            Marks::Commits(commits) => DiscardOperation::Commits(commits.clone()),
            Marks::Branches(branches) => {
                let branches = branches
                    .iter()
                    .map(|branch| Category::LocalBranch.to_full_name(&*branch.name))
                    .collect::<Result<Vec<_>, _>>()?;
                let Some(branches) = NonEmpty::from_vec(branches) else {
                    anyhow::bail!("BUG: marked branches must not be empty")
                };
                DiscardOperation::Branches(branches)
            }
            Marks::Hunks(hunks) => {
                DiscardOperation::Uncommitted(UncommittedSelection::changes(hunks.clone()))
            }
            Marks::CommittedFiles(files) => {
                let source = CommitId {
                    commit_id: files.head.commit_id,
                    change_id: files.head.change_id.clone(),
                };
                anyhow::ensure!(
                    files
                        .tail
                        .iter()
                        .all(|file| file.commit_id == source.commit_id),
                    "BUG: marked committed files must come from one commit"
                );
                let paths = files.clone().map(|file| file.path);
                DiscardOperation::CommittedFiles { source, paths }
            }
        };

        self.to_be_discarded = normal_mode
            .marks
            .iter()
            .map(|mark| mark.to_owned().into_selectable())
            .collect::<Vec<_>>();

        let select_after_reload = self
            .cursor
            .select_after_discarded_marks(&self.status_lines, &normal_mode.marks);

        let drop_to_be_discarded =
            message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);

        let confirm = Confirm::new(
            NonEmpty::new("Discard?".into()),
            self.theme,
            move |ctx, messages| {
                let select_after_reload = match run_discard(ctx, operation)? {
                    DiscardOutcome::Commits {
                        commits: _,
                        replaced_commits,
                    } => map_selected_commits(select_after_reload, |commit_id| {
                        replaced_commits
                            .get(&commit_id)
                            .copied()
                            .unwrap_or(commit_id)
                    }),
                    DiscardOutcome::CommittedFiles {
                        source,
                        paths: _,
                        new_commit,
                    } => map_selected_commits(select_after_reload, |commit_id| {
                        if commit_id == source.commit_id {
                            new_commit.commit_id
                        } else {
                            commit_id
                        }
                    }),
                    DiscardOutcome::Branches(_) | DiscardOutcome::Uncommitted { .. } => {
                        select_after_reload
                    }
                };

                drop(drop_to_be_discarded);

                messages.extend([
                    Message::ClearMarks,
                    Message::Reload(select_after_reload, ReloadCause::Mutation),
                ]);

                Ok(())
            },
        );

        self.modal = Some(Modal::Confirm { confirm });

        Ok(())
    }
}

fn map_selected_commits(
    selection: Option<SelectAfterReload>,
    mut map: impl FnMut(ObjectId) -> ObjectId,
) -> Option<SelectAfterReload> {
    selection.map(|selection| match selection {
        SelectAfterReload::Commit(commit_id) => SelectAfterReload::Commit(map(commit_id)),
        SelectAfterReload::FirstFileInCommit(commit_id) => {
            SelectAfterReload::FirstFileInCommit(map(commit_id))
        }
        SelectAfterReload::Branch(name) => SelectAfterReload::Branch(name),
        SelectAfterReload::Uncommitted => SelectAfterReload::Uncommitted,
        SelectAfterReload::UncommittedFile { path } => SelectAfterReload::UncommittedFile { path },
        SelectAfterReload::UncommittedDetailsSection { index, direction } => {
            SelectAfterReload::UncommittedDetailsSection { index, direction }
        }
        SelectAfterReload::CliId(cli_id) => SelectAfterReload::CliId(cli_id),
    })
}

pub(in crate::command::legacy::status::tui) fn run_discard(
    ctx: &mut Context,
    operation: DiscardOperation,
) -> anyhow::Result<DiscardOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let (outcome, _ws) = discard::run(ctx, &mut meta, guard.write_permission(), operation)?;
    Ok(outcome)
}
