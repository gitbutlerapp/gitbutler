use anyhow::Context as _;
use bstr::{BString, ByteSlice as _};
use but_ctx::Context;
use gix::prelude::ObjectIdExt as _;
use nonempty::NonEmpty;
use ratatui::{style::Style, text::Span};

use crate::{
    CliId,
    command::{
        legacy::status::{
            FilesStatusFlag,
            output::StatusOutputLineData,
            tui::{
                Message, ReloadCause, SelectAfterReload,
                app::{App, Modal},
                confirm::Confirm,
                fuzzy_picker::{Col, FuzzyPicker, FuzzyPickerItem, SearchableToken},
                key_bind::fuzzy_picker_key_binds,
                mode::Mode,
                render::{
                    ModeRender, RenderSingleLineSpans, SpanExt as _, worktree_operation_display,
                },
                toast::ToastKind,
            },
        },
        worktree::{self, archive::ArchivalOperation, new::NewOperation},
    },
    theme::Theme,
};

use super::MoveCursorDiration;

#[derive(Debug, Clone)]
pub struct WorktreeMode {}

impl ModeRender for WorktreeMode {
    fn render_operation_target_marker(
        &self,
        app: &App,
        data: &StatusOutputLineData,
        line: &mut RenderSingleLineSpans<'_, '_>,
    ) {
        if let Some(display) = worktree_operation_display(data, self) {
            line.extend([
                Span::raw("<< ").mode_colors(&*app.mode, app.theme),
                Span::raw(display).mode_colors(&*app.mode, app.theme),
                Span::raw(" >>").mode_colors(&*app.mode, app.theme),
                Span::raw(" "),
            ]);
        }
    }
}

impl WorktreeMode {}

#[derive(Debug)]
pub enum WorktreeMessage {
    Start,
    New,
    Archive,
    ShowUnarchivePicker,
}

impl App {
    #[allow(warnings, clippy::ptr_arg)]
    pub fn handle_worktree(
        &mut self,
        worktree_message: WorktreeMessage,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match worktree_message {
            WorktreeMessage::Start => self.handle_worktree_start(),
            WorktreeMessage::New => self.handle_worktree_new(ctx, messages)?,
            WorktreeMessage::Archive => self.handle_worktree_archive(),
            WorktreeMessage::ShowUnarchivePicker => {
                self.handle_worktree_show_unarchive_picker(ctx)?
            }
        }

        Ok(())
    }

    fn handle_worktree_start(&mut self) {
        match self.flags.show_files {
            FilesStatusFlag::Commit(..) => return,
            FilesStatusFlag::None | FilesStatusFlag::All => {}
        }

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Worktree(WorktreeMode {});
            });
        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Down);
    }

    fn handle_worktree_archive(&mut self) {
        let Some(CliId::Worktree { name, .. }) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|line| line.data.cli_id())
            .map(|id| &**id)
        else {
            return;
        };
        let worktree_name = name.clone();

        self.modal = Some(Modal::Confirm {
            confirm: Confirm::new(
                NonEmpty::new(
                    format!("Archive {worktree_name}? The directory will not be deleted").into(),
                ),
                self.theme,
                move |ctx, messages| {
                    let guard = ctx.shared_worktree_access();
                    _ = worktree::archive::run(
                        ctx,
                        guard.read_permission(),
                        ArchivalOperation {
                            worktree: worktree_name,
                            archived: true,
                        },
                    )?;

                    messages.extend([
                        Message::EnterNormalModeAfterConfirmingOperation,
                        Message::Reload(None, ReloadCause::Mutation),
                    ]);
                    Ok(())
                },
            ),
        });
    }

    fn handle_worktree_show_unarchive_picker(&mut self, ctx: &Context) -> anyhow::Result<()> {
        let worktrees = {
            let guard = ctx.shared_worktree_access();
            but_api::worktrees::worktrees_list_with_perm(ctx, guard.read_permission())
                .context("Failed to list archived worktrees")?
                .archived
        };
        let items = worktrees
            .into_iter()
            .map(|worktree| UnarchiveWorktreeItem {
                name: worktree.name,
            })
            .collect::<Vec<_>>();
        let Some(items) = NonEmpty::from_vec(items) else {
            self.toasts.insert(ToastKind::Info, "No archived worktrees");
            return Ok(());
        };

        let picker = FuzzyPicker::new(items, self.theme, |item, ctx, messages| {
            let guard = ctx.shared_worktree_access();
            _ = worktree::archive::run(
                ctx,
                guard.read_permission(),
                ArchivalOperation {
                    worktree: item.name.clone(),
                    archived: false,
                },
            )?;

            messages.extend([
                Message::EnterNormalModeAfterConfirmingOperation,
                Message::Reload(
                    Some(SelectAfterReload::Worktree(item.name)),
                    ReloadCause::Mutation,
                ),
            ]);
            Ok(())
        });
        self.modal = Some(Modal::UnarchiveWorktreePicker {
            picker: Box::new(picker),
            key_binds: fuzzy_picker_key_binds(),
        });
        Ok(())
    }

    fn handle_worktree_new(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let operation = match &selection.data {
            StatusOutputLineData::MergeBase => NewOperation {
                ref_name: None,
                base: None,
            },
            StatusOutputLineData::Commit { cli_id, .. } => {
                let CliId::Commit { commit, .. } = &**cli_id else {
                    return Ok(());
                };

                let repo = ctx.repo.get()?;
                let selected_commit = but_core::Commit::from_id(commit.commit_id.attach(&repo))?;
                if selected_commit.is_conflicted() {
                    self.toasts.insert(
                        ToastKind::Info,
                        "Cannot create a worktree from a conflicted commit",
                    );
                    return Ok(());
                }

                NewOperation {
                    ref_name: None,
                    base: Some(commit.commit_id),
                }
            }

            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::Worktree { .. }
            | StatusOutputLineData::WorktreeUncommitted { .. }
            | StatusOutputLineData::UncommittedFile { .. }
            | StatusOutputLineData::Branch { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        let mut guard = ctx.exclusive_worktree_access();
        let outcome = worktree::new::run(ctx, guard.write_permission(), operation)?;

        messages.extend([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(
                Some(SelectAfterReload::Worktree(outcome.created.name)),
                ReloadCause::Mutation,
            ),
        ]);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UnarchiveWorktreeItem {
    name: BString,
}

impl FuzzyPickerItem for UnarchiveWorktreeItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        [Col {
            text: self.name.to_str_lossy(),
            searchable: Some(searchable),
        }]
    }

    fn style(&self, theme: &'static Theme) -> Style {
        theme.default
    }
}
