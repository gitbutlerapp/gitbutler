use std::sync::Arc;

use but_core::{DiffSpec, DryRun, ref_metadata::StackId};
use but_ctx::Context;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{ObjectId, refs::Category};
use nonempty::NonEmpty;

use crate::{
    CliId,
    command::legacy::status::tui::{
        Message, ReloadCause, SelectAfterReload,
        app::{
            App, Modal,
            mark::{MarkableRef, commits_on_branch},
        },
        confirm::Confirm,
        message_on_drop,
        mode::Mode,
        operations,
    },
    utils::diff_specs::DiffSpecBuilder,
};

use super::mark::Marks;

impl App {
    pub fn select_top_branch_for_stack_after_reload(
        &self,
        stack_id: StackId,
    ) -> Option<SelectAfterReload> {
        self.status_lines.iter().find_map(|line| {
            let cli_id = line.data.cli_id()?;
            if let CliId::Branch {
                stack_id: Some(branch_stack_id),
                ..
            } = &**cli_id
                && *branch_stack_id == stack_id
            {
                Some(SelectAfterReload::CliId(Box::new((**cli_id).clone())))
            } else {
                None
            }
        })
    }

    pub fn handle_discard(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if self.marks_ref().is_empty() {
            self.handle_discard_selection(ctx, messages)
        } else {
            self.handle_discard_marks(ctx, messages)
        }
    }

    pub fn handle_discard_selection(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };
        let Some(cli_id) = selection.data.cli_id() else {
            return Ok(());
        };

        self.modal = Some(Modal::Confirm {
            confirm: match &**cli_id {
                CliId::Uncommitted { .. } => {
                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new("Discard uncommitted changes?".into()),
                        self.theme,
                        move |ctx, messages| {
                            operations::discard_uncommitted_legacy(ctx)?;
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
                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let uncommitted = uncommitted.clone();

                    let select_after_reload = if uncommitted.is_entire_file
                    // Discarding a whole file: handle stack-specific cursor fallback.
                    && let Some(stack_id) = uncommitted.hunk_assignments.first().stack_id
                    // If this is the last file on the stack, jump to the stack's top branch.
                    && operations::assigned_file_count_for_stack(ctx, stack_id)
                        .is_ok_and(|count| count == 1)
                    {
                        self.select_top_branch_for_stack_after_reload(stack_id)
                            .unwrap_or(SelectAfterReload::Stack(stack_id))
                    } else {
                        // Discarding only part of a file: select the previous selectable line.
                        self.cursor.select_previous_cli_id_or_uncommitted(
                            &self.status_lines,
                            &self.mode,
                            self.flags.show_files,
                        )
                    };

                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new("Discard uncommitted file?".into()),
                        self.theme,
                        move |ctx, messages| {
                            let hunk_assignments = uncommitted
                                .hunk_assignments
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>();
                            operations::discard_uncommitted_hunks_legacy(ctx, hunk_assignments)?;
                            messages.push(Message::Reload(
                                Some(select_after_reload),
                                ReloadCause::Mutation,
                            ));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Stack { stack_id, .. } => {
                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let stack_id = *stack_id;
                    let select_after_reload = self
                        .select_top_branch_for_stack_after_reload(stack_id)
                        .unwrap_or(SelectAfterReload::Stack(stack_id));
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new("Discard staged changes in this stack?".into()),
                        self.theme,
                        move |ctx, messages| {
                            operations::discard_stack(ctx, stack_id)?;
                            messages.push(Message::Reload(
                                Some(select_after_reload),
                                ReloadCause::Mutation,
                            ));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Commit { commit_id, .. } => {
                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let commit_id = *commit_id;
                    let select_after_reload = self
                        .cursor
                        .select_after_discarded_commit(&self.status_lines);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);
                    Confirm::new(
                        NonEmpty::new(
                            format!("Discard commit {}?", commit_id.to_hex_with_len(7)).into(),
                        ),
                        self.theme,
                        move |ctx, messages| {
                            let discard_result = operations::commit_discard(ctx, commit_id)?;
                            let select_after_reload =
                                select_after_reload.map(|selection| match selection {
                                    SelectAfterReload::Commit(target_commit_id) => {
                                        let remapped_target_commit_id = discard_result
                                            .workspace
                                            .replaced_commits
                                            .get(&target_commit_id)
                                            .copied()
                                            .unwrap_or(target_commit_id);
                                        SelectAfterReload::Commit(remapped_target_commit_id)
                                    }
                                    other => other,
                                });
                            messages
                                .push(Message::Reload(select_after_reload, ReloadCause::Mutation));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::Branch { name, stack_id, .. } => {
                    let Some(stack_id) = *stack_id else {
                        return Ok(());
                    };

                    let name = name.to_owned();

                    let commits = commits_on_branch(ctx, stack_id, &name)?;

                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let select_after_reload = self
                        .cursor
                        .select_after_discarded_branch(&self.status_lines);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);

                    Confirm::new(
                        NonEmpty::new(format!("Discard branch {name}?").into()),
                        self.theme,
                        move |ctx, messages| {
                            let mut meta = ctx.meta()?;
                            let snapshot_details =
                                SnapshotDetails::new(OperationKind::DeleteBranch);

                            let refname = Category::LocalBranch.to_full_name(&*name)?;
                            but_transaction::with_transaction(
                                ctx,
                                &mut meta,
                                snapshot_details,
                                DryRun::No,
                                |mut tx| {
                                    tx.remove_reference(refname.as_ref())?;
                                    if !commits.is_empty() {
                                        tx.discard_commits(
                                            commits.into_iter().map(|(commit, _, _)| commit),
                                        )?;
                                    }
                                    Ok(())
                                },
                            )?;

                            messages
                                .push(Message::Reload(select_after_reload, ReloadCause::Mutation));
                            drop(drop_to_be_discarded);
                            Ok(())
                        },
                    )
                }
                CliId::CommittedFile {
                    commit_id,
                    path,
                    id: _,
                } => {
                    let commit_id = *commit_id;
                    let path = path.to_owned();

                    self.to_be_discarded = Vec::from([Arc::clone(cli_id)]);
                    let drop_to_be_discarded =
                        message_on_drop::message_on_drop(Message::DropToBeDiscarded, messages);

                    Confirm::new(
                        NonEmpty::new(format!("Discard changes to {path}?").into()),
                        self.theme,
                        move |ctx, messages| {
                            let mut perm = ctx.exclusive_worktree_access();
                            let mut meta = ctx.meta()?;
                            let snapshot_details = SnapshotDetails::new(OperationKind::DiscardFile);

                            let changes = {
                                let context_lines = ctx.settings.context_lines;
                                let (repo, ws, mut db) =
                                    ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
                                let mut builder =
                                    DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
                                builder
                                    .push_changes_from_committed_file(commit_id, path.as_ref())?;
                                builder.into_diff_specs()
                            };

                            let (new_commit, _ws) = but_transaction::with_transaction_with_perm(
                                ctx,
                                &mut meta,
                                perm.write_permission(),
                                snapshot_details,
                                DryRun::No,
                                |mut tx| {
                                    let new_commit =
                                        tx.discard_changes_from_commit(commit_id, changes)?;
                                    Ok(but_transaction::Commit(new_commit))
                                },
                            )?;

                            let select_after_reload =
                                if operations::commit_is_empty(ctx, new_commit)? {
                                    SelectAfterReload::Commit(new_commit)
                                } else {
                                    SelectAfterReload::FirstFileInCommit(new_commit)
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
                CliId::PathPrefix { .. } => return Ok(()),
            },
        });

        Ok(())
    }

    pub fn handle_discard_marks(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Mode::Normal(normal_mode) = &*self.mode else {
            return Ok(());
        };

        if normal_mode.marks.is_empty() {
            return Ok(());
        }

        let commits_to_discard = normal_mode
            .marks
            .iter()
            .filter_map(|mark| match mark {
                MarkableRef::Commit(mark) => Some(mark.commit_id),
                _ => None,
            })
            .collect::<Vec<_>>();

        enum ChangesToDiscard {
            Uncommitted(Vec<DiffSpec>),
            CommittedFiles(ObjectId, Vec<DiffSpec>),
        }

        let changes_to_discard = {
            let context_lines = ctx.settings.context_lines;
            let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);

            match &normal_mode.marks {
                Marks::Empty | Marks::Commits(..) => None,
                Marks::Hunks(hunks) => {
                    for hunk in hunks {
                        builder.push_changes_from_uncommitted(hunk)?;
                    }
                    Some(ChangesToDiscard::Uncommitted(builder.into_diff_specs()))
                }
                Marks::CommittedFiles(files) => {
                    let commit = files.head.commit_id;
                    for file in files {
                        // One day the API will support this but it doesn't right now.
                        //
                        // Linear ticket: https://linear.app/gitbutler/issue/GB-1684/missing-api-moving-hunks-between-commits-with-multiple-sources
                        anyhow::ensure!(
                            commit == file.commit_id,
                            "BUG: it should not be possible to mark commits from multiple sources"
                        );

                        builder
                            .push_changes_from_committed_file(file.commit_id, file.path.as_ref())?;
                    }
                    Some(ChangesToDiscard::CommittedFiles(
                        commit,
                        builder.into_diff_specs(),
                    ))
                }
            }
        };

        self.to_be_discarded = normal_mode
            .marks
            .iter()
            .map(|mark| Arc::new(mark.to_owned().into_cli_id()))
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
                let mut meta = ctx.meta()?;
                let snapshot_details = SnapshotDetails::new(OperationKind::Discard);
                let workspace = but_transaction::with_transaction(
                    ctx,
                    &mut meta,
                    snapshot_details,
                    DryRun::No,
                    |mut tx| {
                        if !commits_to_discard.is_empty() {
                            tx.discard_commits(commits_to_discard)?;
                        }

                        if let Some(changes_to_discard) = changes_to_discard {
                            match changes_to_discard {
                                ChangesToDiscard::Uncommitted(changes) => {
                                    if !changes.is_empty() {
                                        but_workspace::discard_workspace_changes(
                                            tx.repo(),
                                            changes,
                                            tx.context_lines(),
                                        )?;
                                    }
                                }
                                ChangesToDiscard::CommittedFiles(commit, changes) => {
                                    if !changes.is_empty() {
                                        tx.discard_changes_from_commit(commit, changes)?;
                                    }
                                }
                            }
                        }

                        Ok(())
                    },
                )?;
                let select_after_reload = select_after_reload.map(|selection| match selection {
                    SelectAfterReload::Commit(target_commit_id) => {
                        let remapped_target_commit_id = workspace
                            .replaced_commits
                            .get(&target_commit_id)
                            .copied()
                            .unwrap_or(target_commit_id);
                        SelectAfterReload::Commit(remapped_target_commit_id)
                    }
                    SelectAfterReload::FirstFileInCommit(target_commit_id) => {
                        let remapped_target_commit_id = workspace
                            .replaced_commits
                            .get(&target_commit_id)
                            .copied()
                            .unwrap_or(target_commit_id);
                        SelectAfterReload::FirstFileInCommit(remapped_target_commit_id)
                    }
                    other => other,
                });

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
