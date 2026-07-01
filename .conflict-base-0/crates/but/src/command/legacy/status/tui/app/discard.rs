use std::sync::Arc;

use but_core::{DryRun, ref_metadata::StackId};
use but_ctx::Context;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::refs::Category;
use nonempty::NonEmpty;

use crate::{
    CliId,
    command::legacy::status::tui::{
        Message, ReloadCause, SelectAfterReload,
        app::{App, Modal, mark::commits_on_branch},
        confirm::Confirm,
        key_bind::confirm_key_binds,
        marking::Markable,
        message_on_drop,
        mode::Mode,
        operations,
    },
    utils::diff_specs::DiffSpecBuilder,
};

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
                Some(SelectAfterReload::CliId(Arc::clone(cli_id)))
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
        if self.marks().is_none_or(|marks| marks.is_empty()) {
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
                                            commits.into_iter().map(|(commit, _)| commit),
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
                CliId::PathPrefix { .. } | CliId::CommittedFile { .. } => return Ok(()),
            },
            key_binds: confirm_key_binds(),
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

        let commits = normal_mode
            .marks
            .iter()
            .filter_map(|mark| match mark {
                Markable::Commit { commit_id, .. } => Some(*commit_id),
                Markable::Uncommitted(..) => None,
            })
            .collect::<Vec<_>>();

        let uncommitted_diff_specs = {
            let context_lines = ctx.settings.context_lines;
            let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
            let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);

            for mark in &normal_mode.marks {
                match mark {
                    Markable::Uncommitted(uncommitted) => {
                        builder.push_changes_from_uncommitted(uncommitted)?;
                    }
                    Markable::Commit { .. } => {}
                }
            }

            builder.into_diff_specs()
        };

        self.to_be_discarded = normal_mode
            .marks
            .iter()
            .cloned()
            .map(|mark| Arc::new(mark.into_cli_id()))
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
                        if !commits.is_empty() {
                            tx.discard_commits(commits)?;
                        }

                        if !uncommitted_diff_specs.is_empty() {
                            but_workspace::discard_workspace_changes(
                                tx.repo(),
                                uncommitted_diff_specs,
                                tx.context_lines(),
                            )?;
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
                    other => other,
                });

                drop(drop_to_be_discarded);
                messages.extend([
                    Message::ClearNormalModeMarks,
                    Message::Reload(select_after_reload, ReloadCause::Mutation),
                ]);
                Ok(())
            },
        );

        self.modal = Some(Modal::Confirm {
            confirm,
            key_binds: confirm_key_binds(),
        });

        Ok(())
    }
}
