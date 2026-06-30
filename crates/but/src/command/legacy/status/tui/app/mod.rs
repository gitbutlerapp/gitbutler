use std::{
    borrow::Cow,
    rc::Rc,
    sync::{Arc, mpsc::Receiver},
    time::Instant,
};

use anyhow::Context as _;
use bstr::ByteSlice;
use but_ctx::Context;
use gitbutler_operating_modes::OperatingMode;
use gix::refs::{Category, FullName};
use nonempty::NonEmpty;
use ratatui::prelude::*;
use tracing::Level;

use crate::{
    CliId,
    command::legacy::status::{
        FilesStatusFlag, StatusFlags, StatusOutputLine, TuiLaunchOptions, TuiOutcome,
        TuiRunOptions, output::StatusOutputLineData,
    },
    theme::Theme,
    tui::TerminalGuard,
};

use super::{
    BackstackEntry, DETAILS_SIZE_ADJUSTMENT_PERCENTAGE, DetailsLayoutMessage, FilesMessage,
    Message, ReloadCause, SelectAfterReload, TuiInputOutputChannel,
    backstack::{Backstack, RememberToUpdateBackstack},
    confirm::Confirm,
    copy_selection_picker,
    copy_selection_picker::CopySelectionItem,
    cursor,
    cursor::{Cursor, is_selectable_in_mode},
    details::Details,
    file_browser::FileBrowser,
    fps::FpsCounter,
    fuzzy_picker::{Col, FuzzyPicker, FuzzyPickerItem, SearchableToken},
    help::Help,
    highlight::Highlights,
    key_bind::{
        KeyBinds, default_key_binds, fuzzy_picker_key_binds, help_key_binds,
        normal_with_marks_key_binds,
    },
    marking::MarkClasses,
    mode::Mode,
    operations,
    render::{details_viewport, ensure_cursor_visible, status_viewport_height},
    toast::{ToastKind, Toasts},
};

mod details_layout;
mod discard;
mod mark;
mod undo_redo;

mod command_mode;
pub use command_mode::*;

mod commit_mode;
pub use commit_mode::*;

mod reword;
pub use reword::*;

mod jump_mode;
pub use jump_mode::*;

mod move_mode;
pub use move_mode::*;

mod normal_mode;
pub use normal_mode::*;

mod pick_changes_mode;
pub use pick_changes_mode::*;

mod rub_mode;
pub use rub_mode::*;

mod stack_mode;
pub use stack_mode::*;

#[derive(Debug)]
pub struct App {
    pub status_lines: Vec<StatusOutputLine>,
    pub flags: StatusFlags,
    pub outcome: Option<TuiOutcome>,
    pub should_render: bool,
    pub cursor: Cursor,
    pub scroll_top: usize,
    pub mode: RememberToUpdateBackstack<Mode>,
    pub toasts: Toasts,
    pub renders: u64,
    pub updates: u64,
    pub app_key_binds: AppKeyBinds,
    pub highlight: Highlights<CliId>,
    pub modal: Option<Modal>,
    pub details: Details,
    pub is_details_visible: bool,
    pub launch_options: TuiLaunchOptions,
    pub delayed_messages: Vec<Message>,
    pub incoming_out_of_band_messages: Vec<Rc<Receiver<Message>>>,
    pub fps: FpsCounter,
    pub to_be_discarded: Vec<Arc<CliId>>,
    pub status_width_percentage: u16,
    pub theme: &'static Theme,
    pub has_focus: bool,
    pub backstack: Backstack,
    pub previous_reload_caused_by_mutation_timestamp: Option<Instant>,
    pub file_browser: Option<FileBrowser>,
}

impl App {
    pub fn new(
        status_lines: Vec<StatusOutputLine>,
        flags: StatusFlags,
        launch_options: TuiLaunchOptions,
        run_options: TuiRunOptions,
        show_file_browser: bool,
    ) -> Self {
        let cursor = if let Some(object_id) = launch_options.select_commit {
            Cursor::select_commit(object_id, &status_lines)
                .unwrap_or_else(|| Cursor::new(&status_lines))
        } else {
            Cursor::new(&status_lines)
        };

        let theme = crate::theme::get();

        let (mut details, is_details_visible) = (Details::new(theme), launch_options.show_diff);
        if is_details_visible {
            details.mark_dirty();
        }

        let app_key_binds = AppKeyBinds {
            key_binds: default_key_binds(),
            normal_with_marks_key_binds: normal_with_marks_key_binds(),
        };

        let mode = RememberToUpdateBackstack::new(match run_options {
            TuiRunOptions::Normal => Mode::default(),
            TuiRunOptions::PickChanges => Mode::PickChanges(Default::default()),
        });

        let file_browser = show_file_browser.then(FileBrowser::default);

        Self {
            status_lines,
            flags,
            cursor,
            scroll_top: 0,
            outcome: None,
            should_render: true,
            mode,
            toasts: Default::default(),
            renders: 0,
            updates: 0,
            app_key_binds,
            highlight: Default::default(),
            delayed_messages: Default::default(),
            incoming_out_of_band_messages: Default::default(),
            to_be_discarded: Default::default(),
            modal: Default::default(),
            backstack: Default::default(),
            previous_reload_caused_by_mutation_timestamp: Default::default(),
            fps: FpsCounter::new(),
            details,
            is_details_visible,
            launch_options,
            status_width_percentage: 50,
            theme,
            has_focus: true,
            file_browser,
        }
    }

    pub fn active_key_binds(&self) -> &KeyBinds {
        match &self.modal {
            Some(Modal::Confirm { key_binds, .. })
            | Some(Modal::GotoBranchPicker { key_binds, .. })
            | Some(Modal::ApplyStackPicker { key_binds, .. })
            | Some(Modal::CopySelectionPicker { key_binds, .. })
            | Some(Modal::Help { key_binds, .. }) => key_binds,
            None => {
                if let Mode::Normal(NormalMode { marks }) = &*self.mode
                    && !marks.is_empty()
                {
                    &self.app_key_binds.normal_with_marks_key_binds
                } else {
                    &self.app_key_binds.key_binds
                }
            }
        }
    }

    #[tracing::instrument(level = Level::TRACE, skip(self, ctx, out, mode, terminal_guard, messages))]
    pub fn handle_message<T>(
        &mut self,
        ctx: &mut Context,
        out: &mut dyn TuiInputOutputChannel,
        mode: &OperatingMode,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
        msg: Message,
    ) where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        if let Err(err) = self.try_handle_message(ctx, out, mode, terminal_guard, messages, msg) {
            messages.push(Message::ShowError(Arc::new(err)));
        }
    }

    fn try_handle_message<T>(
        &mut self,
        ctx: &mut Context,
        out: &mut dyn TuiInputOutputChannel,
        mode: &OperatingMode,
        terminal_guard: &mut T,
        messages: &mut Vec<Message>,
        msg: Message,
    ) -> anyhow::Result<()>
    where
        T: TerminalGuard,
        anyhow::Error: From<<T::Backend as Backend>::Error>,
    {
        self.should_render = true;
        let terminal_area: Rect = terminal_guard.terminal_mut().size()?.into();
        let visible_height = status_viewport_height(self, terminal_area);

        if self
            .details
            .needs_update_after_message(self.is_details_visible, &msg)
        {
            self.details.mark_dirty();
        }

        match msg {
            Message::Quit => {
                self.handle_quit();
            }
            Message::ConfirmAndQuit => {
                self.handle_confirm_and_quit();
            }
            Message::JustRender => {}
            Message::MoveCursorUp(count) => {
                for _ in 0..count {
                    if let Some(new_cursor) =
                        self.cursor
                            .move_up(&self.status_lines, &self.mode, self.flags.show_files)
                    {
                        self.cursor = new_cursor;
                    } else {
                        break;
                    }
                }
            }
            Message::MoveCursorDown(count) => {
                for _ in 0..count {
                    if let Some(new_cursor) =
                        self.cursor
                            .move_down(&self.status_lines, &self.mode, self.flags.show_files)
                    {
                        self.cursor = new_cursor;
                    } else {
                        break;
                    }
                }
            }
            Message::MoveCursorPreviousSection => {
                if let Some(new_cursor) = self.cursor.move_previous_section(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                }
            }
            Message::MoveCursorNextSection => {
                if let Some(new_cursor) = self.cursor.move_next_section(
                    &self.status_lines,
                    &self.mode,
                    self.flags.show_files,
                ) {
                    self.cursor = new_cursor;
                }
            }
            Message::SelectBranch(branch_name) => {
                if let Some(new_cursor) =
                    Cursor::select_branch(&branch_name.shorten().to_str_lossy(), &self.status_lines)
                {
                    self.cursor = if matches!(&*self.mode, Mode::Rub(_)) {
                        new_cursor
                            .move_down_within_section(
                                &self.status_lines,
                                &self.mode,
                                self.flags.show_files,
                            )
                            .unwrap_or(new_cursor)
                    } else {
                        new_cursor
                    };
                }
            }
            Message::SelectUncommitted => {
                let new_cursor = Cursor::new(&self.status_lines);
                if let Some(uncommitted_line) = new_cursor.selected_line(&self.status_lines)
                    && cursor::is_selectable_in_mode(
                        uncommitted_line,
                        &self.mode,
                        self.flags.show_files,
                    )
                {
                    self.cursor = new_cursor;
                }
            }
            Message::SelectMergeBase => {
                let Some(new_cursor) = Cursor::select_merge_base(&self.status_lines) else {
                    return Ok(());
                };
                if let Some(merge_base_line) = new_cursor.selected_line(&self.status_lines)
                    && cursor::is_selectable_in_mode(
                        merge_base_line,
                        &self.mode,
                        self.flags.show_files,
                    )
                {
                    self.cursor = new_cursor;
                }
            }
            Message::Rub(rub_message) => self.handle_rub(rub_message, ctx, messages)?,
            Message::Back => {
                self.handle_back(messages);
            }
            Message::UnfocusDetails => {
                self.handle_unfocus_details(messages);
            }
            Message::EnterNormalModeAfterConfirmingOperation => {
                self.handle_enter_normal_mode_after_confirming_operation(messages);
            }
            Message::DetailsLayout(details_layout_message) => match details_layout_message {
                DetailsLayoutMessage::Focus { full_screen } => {
                    self.handle_focus_details(full_screen, messages);
                }
                DetailsLayoutMessage::ToggleFullScreen => {
                    self.handle_toggle_details_full_screen(messages);
                }
                DetailsLayoutMessage::ToggleVisibility => {
                    self.handle_toggle_details_visibility(messages);
                }
                DetailsLayoutMessage::Dismiss => {
                    self.handle_dismiss_details(messages);
                }
            },
            Message::Files(files_message) => match files_message {
                FilesMessage::ToggleGlobalFilesList => {
                    self.handle_files_toggle_global_files_list(messages)
                }
                FilesMessage::ToggleFilesForCommit => {
                    self.handle_files_toggle_files_for_commit(ctx, messages)?
                }
            },
            Message::Reload(select_after_reload, cause) => {
                self.handle_reload(ctx, out, mode, select_after_reload, cause)?
            }
            Message::ShowError(err) => self.handle_show_error(err, messages),
            Message::Commit(commit_message) => {
                self.handle_commit(commit_message, ctx, terminal_guard, messages)?
            }
            Message::Reword(reword_message) => {
                self.handle_reword(reword_message, ctx, terminal_guard, messages)?
            }
            Message::Command(command_message) => {
                self.handle_command(command_message, terminal_guard, out, messages)?
            }
            Message::Move(move_message) => self.handle_move(move_message, ctx, messages)?,
            Message::NewBranch => {
                self.handle_new_branch(ctx, messages)?;
            }
            Message::CopySelection => {
                self.handle_copy_selection()?;
            }
            Message::CopySelectionPicker => {
                self.handle_copy_selection_picker()?;
            }
            Message::ShowToast { kind, text } => {
                self.toasts.insert(kind, text);
            }
            Message::Confirm(confirm_message) => match self.modal.take() {
                Some(Modal::Confirm { confirm, key_binds }) => {
                    self.modal = confirm
                        .handle_message(confirm_message, ctx, messages)?
                        .map(|confirm| Modal::Confirm { confirm, key_binds });
                }
                modal => self.modal = modal,
            },
            Message::FuzzyPicker(fuzzy_picker_message) => {
                if let Some(modal) = self.modal.take() {
                    match modal {
                        Modal::GotoBranchPicker { picker, key_binds } => {
                            self.modal = picker
                                .handle_message(fuzzy_picker_message, ctx, messages)?
                                .map(|picker| Modal::GotoBranchPicker {
                                    picker: Box::new(picker),
                                    key_binds,
                                });
                        }
                        Modal::ApplyStackPicker { picker, key_binds } => {
                            self.modal = picker
                                .handle_message(fuzzy_picker_message, ctx, messages)?
                                .map(|picker| Modal::ApplyStackPicker {
                                    picker: Box::new(picker),
                                    key_binds,
                                });
                        }
                        Modal::CopySelectionPicker { picker, key_binds } => {
                            self.modal = picker
                                .handle_message(fuzzy_picker_message, ctx, messages)?
                                .map(|picker| Modal::CopySelectionPicker {
                                    picker: Box::new(picker),
                                    key_binds,
                                });
                        }
                        Modal::Confirm { .. } | Modal::Help { .. } => {
                            self.modal = Some(modal);
                        }
                    }
                }
            }
            Message::Help(help_message) => match self.modal.take() {
                Some(Modal::Help { help, key_binds }) => {
                    self.modal = help
                        .handle_message(help_message, terminal_area)?
                        .map(|help| Modal::Help { help, key_binds });
                }
                modal => self.modal = modal,
            },
            Message::Details(details_message) => {
                let details_viewport = details_viewport(self, terminal_area);
                self.details
                    .try_handle_message(details_message, details_viewport, messages)?;
            }
            Message::RegisterOutOfBandMessage(rx) => {
                self.incoming_out_of_band_messages.push(rx);
            }
            Message::WithOneFrameDelay(msg) => {
                self.delayed_messages.push(*msg);
            }
            Message::Discard => {
                self.handle_discard(ctx, messages)?;
            }
            Message::DropToBeDiscarded => {
                self.to_be_discarded.clear();
            }
            Message::AndThen { lhs, rhs } => {
                self.try_handle_message(ctx, out, mode, terminal_guard, messages, *lhs)?;

                // Push `rhs` to the end of the queue. That way any messages enqueued by `lhs` will
                // be handled first.
                messages.push(*rhs);
            }
            Message::Debug(text) => {
                messages.push(Message::ShowToast {
                    kind: ToastKind::Debug,
                    text: text.to_owned().into(),
                });
            }
            Message::GrowDetails => {
                self.update_status_width_percentage(
                    self.status_width_percentage
                        .saturating_sub(DETAILS_SIZE_ADJUSTMENT_PERCENTAGE),
                    terminal_area,
                );
            }
            Message::ShrinkDetails => {
                self.update_status_width_percentage(
                    self.status_width_percentage
                        .saturating_add(DETAILS_SIZE_ADJUSTMENT_PERCENTAGE),
                    terminal_area,
                );
            }
            Message::PickAndGotoBranch => {
                self.handle_pick_and_goto_branch(ctx)?;
            }
            Message::ToggleHelp => {
                self.handle_toggle_help();
            }
            Message::Mark => {
                self.handle_mark(ctx)?;
            }
            Message::ClearNormalModeMarks => {
                self.handle_clear_normal_mode_marks();
            }
            Message::SetHasFocus(has_focus) => {
                self.has_focus = has_focus;
            }
            Message::Undo => {
                self.handle_undo(ctx, messages)?;
            }
            Message::Redo => {
                self.handle_redo(ctx, messages)?;
            }
            Message::Stack(stack_message) => self.handle_stack(stack_message, ctx, messages)?,
            Message::Jump(jump_message) => self.handle_jump(jump_message, messages),
        }

        ensure_cursor_visible(self, visible_height);

        Ok(())
    }

    fn handle_quit(&mut self) {
        self.outcome = Some(TuiOutcome::None);
    }

    fn handle_confirm_and_quit(&mut self) {
        self.outcome = match &*self.mode {
            Mode::Normal(..)
            | Mode::Rub(..)
            | Mode::InlineReword(..)
            | Mode::Command(..)
            | Mode::Commit(..)
            | Mode::Move(..)
            | Mode::Details(..)
            | Mode::Stack(..)
            | Mode::Jump(..)
            | Mode::MoveStack(..) => Some(TuiOutcome::None),
            Mode::PickChanges(PickChangesMode { marks }) => {
                let ids = marks
                    .iter()
                    .cloned()
                    .map(|mark| mark.into_cli_id())
                    .collect();
                Some(TuiOutcome::CliIds(ids))
            }
        };
    }

    fn handle_enter_normal_mode_after_confirming_operation(&mut self, messages: &mut Vec<Message>) {
        let mut entries_to_handle = Vec::new();
        self.mode.update(&mut self.backstack, |backstack, mode| {
            backstack.retain(|entry| match entry {
                BackstackEntry::ShowFileList => {
                    // this keeps the global file list open after performing operations such as
                    // committing or rubbing
                    true
                }
                BackstackEntry::LeaveNormalMode | BackstackEntry::Mark => {
                    entries_to_handle.push(entry);
                    false
                }
                BackstackEntry::OpenSplitDetailsView => true,
                BackstackEntry::OpenFullScreenDetailsView => {
                    entries_to_handle.push(entry);
                    false
                }
            });

            *mode = Mode::Normal(NormalMode::default());
        });

        for entry in entries_to_handle {
            self.handle_backstack_entry(entry, messages);
        }

        self.maybe_move_cursor_into_file_list();
    }

    fn handle_back(&mut self, messages: &mut Vec<Message>) {
        if let Some(entry) = self.backstack.pop() {
            self.handle_backstack_entry(entry, messages);
        }
    }

    fn handle_backstack_entry(&mut self, entry: BackstackEntry, messages: &mut Vec<Message>) {
        match entry {
            BackstackEntry::LeaveNormalMode => {
                if !self.restore_mode_before_details(messages) && !self.restore_mode_before_jump() {
                    let marks = self.marks().cloned().unwrap_or_default();
                    self.mode.update(&mut self.backstack, |backstack, mode| {
                        let _ = backstack;
                        *mode = Mode::Normal(NormalMode { marks });
                    });
                }
                self.maybe_move_cursor_into_file_list();
            }
            BackstackEntry::ShowFileList => {
                self.flags.show_files = FilesStatusFlag::None;
                messages.push(Message::Reload(None, ReloadCause::ViewOnly));
            }
            BackstackEntry::Mark => match self
                .mode
                .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
            {
                Mode::Normal(normal_mode) => {
                    normal_mode.marks.clear();
                }
                Mode::PickChanges(pick_uncommitted_mode) => {
                    pick_uncommitted_mode.marks.clear();
                }
                Mode::Rub(..) => {
                    *self
                        .mode
                        .get_mut_without_updating_backstack_and_i_promise_not_to_change_state() =
                        Mode::Normal(NormalMode::default());
                    self.backstack.remove_mark();
                    self.backstack.remove_leave_normal_mode();
                    self.maybe_move_cursor_into_file_list();
                }
                Mode::InlineReword(..)
                | Mode::Command(..)
                | Mode::Commit(..)
                | Mode::Move(..)
                | Mode::Stack(..)
                | Mode::MoveStack(..)
                | Mode::Jump(..)
                | Mode::Details(..) => {}
            },
            BackstackEntry::OpenSplitDetailsView | BackstackEntry::OpenFullScreenDetailsView => {
                messages.push(Message::DetailsLayout(
                    DetailsLayoutMessage::ToggleVisibility,
                ));
            }
        }
    }

    fn maybe_move_cursor_into_file_list(&mut self) {
        match self.flags.show_files {
            FilesStatusFlag::Commit(object_id) => {
                // When viewing files in a commit cursor movement is constrained to only those
                // files. However you can start a rub which then enables moving outside the file
                // list, while keeping the file list visible. Thus when entering normal mode
                // (perhaps from cancelling the rub) we need to potentially move the cursor back to
                // the file list.
                let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
                    return;
                };

                if let Some(cli_id) = selection.data.cli_id()
                    && let CliId::CommittedFile { commit_id, .. } = &**cli_id
                    && *commit_id == object_id
                {
                    // cursor is already within the file list
                } else {
                    self.cursor =
                        Cursor::select_first_file_in_commit(object_id, &self.status_lines)
                            .unwrap_or(self.cursor);
                }
            }
            FilesStatusFlag::None | FilesStatusFlag::All => {}
        }
    }

    fn handle_files_toggle_global_files_list(&mut self, messages: &mut Vec<Message>) {
        self.flags.show_files = match self.flags.show_files {
            FilesStatusFlag::None => {
                self.backstack.push_show_file_list();
                FilesStatusFlag::All
            }
            FilesStatusFlag::All | FilesStatusFlag::Commit(_) => {
                self.backstack.remove_show_file_list();
                FilesStatusFlag::None
            }
        };
        messages.push(Message::Reload(None, ReloadCause::ViewOnly));
    }

    fn handle_files_toggle_files_for_commit(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        if let Mode::Normal(normal_mode) = &*self.mode
            && !normal_mode.marks.is_empty()
        {
            let MarkClasses {
                marked_commits,
                // with marked uncommitted files the cursor cannot move out of the uncommitted
                // changes section, thus you can never toggle files for a commit because that
                // requires selecting the commit
                marked_uncommitted: _,
            } = normal_mode.marks.classify();
            if marked_commits {
                match self.flags.show_files {
                    FilesStatusFlag::None => {
                        return Ok(());
                    }
                    FilesStatusFlag::Commit(_) => {}
                    FilesStatusFlag::All => {
                        self.flags.show_files = FilesStatusFlag::None;
                        self.backstack.remove_show_file_list();
                        messages.push(Message::Reload(None, ReloadCause::ViewOnly));
                        return Ok(());
                    }
                }
            }
        }

        if let Some(selection) = self.cursor.selected_line(&self.status_lines)
            && let Some(cli_id) = selection.data.cli_id()
            && let CliId::Commit { commit_id, .. } = &**cli_id
        {
            if !operations::commit_is_empty(ctx, *commit_id)? {
                let select_after_reload = match self.flags.show_files {
                    FilesStatusFlag::None => {
                        self.flags.show_files = FilesStatusFlag::Commit(*commit_id);
                        self.backstack.push_show_file_list();
                        Some(SelectAfterReload::FirstFileInCommit(*commit_id))
                    }
                    FilesStatusFlag::All | FilesStatusFlag::Commit(_) => {
                        self.flags.show_files = FilesStatusFlag::None;
                        self.backstack.remove_show_file_list();
                        Some(SelectAfterReload::Commit(*commit_id))
                    }
                };
                messages.push(Message::Reload(select_after_reload, ReloadCause::ViewOnly));
            }
        } else {
            self.flags.show_files = FilesStatusFlag::None;
            self.backstack.remove_show_file_list();
            messages.push(Message::Reload(None, ReloadCause::ViewOnly));
        };

        Ok(())
    }

    /// Handles reloading status output and restoring selection.
    fn handle_reload(
        &mut self,
        ctx: &mut Context,
        out: &mut dyn TuiInputOutputChannel,
        mode: &OperatingMode,
        select_after_reload: Option<SelectAfterReload>,
        cause: ReloadCause,
    ) -> anyhow::Result<()> {
        let new_lines = operations::reload_legacy(ctx, out, mode, self.flags, self.launch_options)?;

        self.cursor = if let Some(select_after_reload) = select_after_reload {
            match select_after_reload {
                SelectAfterReload::Commit(commit_id) => {
                    Cursor::select_commit(commit_id, &new_lines)
                }
                SelectAfterReload::Branch(branch) => Cursor::select_branch(&branch, &new_lines),
                SelectAfterReload::Uncommitted => Cursor::select_uncommitted(&new_lines),
                SelectAfterReload::UncommittedFile { path, stack_id } => {
                    Cursor::select_uncommitted_file(path.as_ref(), stack_id, &new_lines)
                }
                SelectAfterReload::FirstFileInCommit(commit_id) => {
                    Cursor::select_first_file_in_commit(commit_id, &new_lines)
                }
                SelectAfterReload::Stack(stack_id) => Cursor::select_stack(stack_id, &new_lines),
                SelectAfterReload::CliId(cli_id) => Cursor::restore(&cli_id, &new_lines),
            }
        } else {
            let selected_merge_base = self
                .cursor
                .selected_line(&self.status_lines)
                .is_some_and(|line| matches!(line.data, StatusOutputLineData::MergeBase));

            let default_restore = || {
                self.cursor
                    .selection_cli_id_for_reload(&self.status_lines, self.flags.show_files)
                    .and_then(|previously_selected_cli_id| {
                        Cursor::restore(previously_selected_cli_id, &new_lines)
                    })
            };

            if selected_merge_base {
                Cursor::select_merge_base(&new_lines).or_else(default_restore)
            } else {
                default_restore()
            }
        }
        .unwrap_or_else(|| Cursor::new(&new_lines));

        self.status_lines = new_lines;

        match cause {
            ReloadCause::Watcher | ReloadCause::ViewOnly | ReloadCause::Manual => {}
            ReloadCause::Mutation => {
                self.previous_reload_caused_by_mutation_timestamp = Some(Instant::now());
            }
        }

        Ok(())
    }

    /// Handles showing a transient UI error.
    fn handle_show_error(&mut self, err: Arc<anyhow::Error>, messages: &mut Vec<Message>) {
        self.toasts
            .insert(ToastKind::Error, format_error_for_tui(&err));

        // ensure we always enter normal mode when something does wrong
        // so we don't get stuck in whatever mode we were in previously
        messages.push(Message::EnterNormalModeAfterConfirmingOperation);
    }

    /// Handles creating an empty commit relative to the current selection.
    fn handle_commit_create_empty(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        match &selection.data {
            StatusOutputLineData::Branch { cli_id } => {
                let CliId::Branch { name, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_result = operations::create_empty_commit_relative_to_branch(ctx, name)?;

                messages.push(Message::Reload(
                    Some(SelectAfterReload::Commit(commit_result.new_commit)),
                    ReloadCause::Mutation,
                ));
            }
            StatusOutputLineData::Commit { cli_id, .. } => {
                let CliId::Commit { commit_id, .. } = &**cli_id else {
                    return Ok(());
                };

                let commit_result =
                    operations::create_empty_commit_relative_to_commit(ctx, *commit_id)?;

                messages.push(Message::Reload(
                    Some(SelectAfterReload::Commit(commit_result.new_commit)),
                    ReloadCause::Mutation,
                ));
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::UncommittedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => {}
        }

        Ok(())
    }

    fn handle_new_branch(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };

        let new_name = match &selection.data {
            StatusOutputLineData::Branch { cli_id } => {
                let CliId::Branch { name, .. } = &**cli_id else {
                    return Ok(());
                };
                operations::create_branch_anchored_legacy(ctx, name.to_owned())?
            }
            StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UncommittedFile { .. } => {
                operations::create_branch_legacy(ctx)?
            }
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::Commit { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::File { .. }
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => return Ok(()),
        };

        messages.push(Message::Reload(
            Some(SelectAfterReload::Branch(new_name)),
            ReloadCause::Mutation,
        ));

        Ok(())
    }

    fn handle_copy_selection(&mut self) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selected_line(&self.status_lines) else {
            return Ok(());
        };
        let Some(cli_id) = selection.data.cli_id() else {
            return Ok(());
        };

        let what_to_copy = match &**cli_id {
            CliId::Branch { name, .. } => Cow::Borrowed(&**name),
            CliId::Commit { commit_id, .. } => Cow::Owned(commit_id.to_hex_with_len(7).to_string()),
            CliId::CommittedFile { path, .. } => path.to_str_lossy(),
            CliId::UncommittedHunkOrFile(uncommitted) => {
                Cow::Borrowed(&*uncommitted.hunk_assignments.first().path)
            }
            CliId::PathPrefix { .. } | CliId::Uncommitted { .. } | CliId::Stack { .. } => {
                return Ok(());
            }
        };

        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(what_to_copy))
            .context("failed to copy to system clipboard")?;

        self.highlight
            .insert(Arc::unwrap_or_clone(Arc::clone(cli_id)));

        Ok(())
    }

    fn handle_copy_selection_picker(&mut self) -> anyhow::Result<()> {
        let Some(selection) = self
            .cursor
            .selected_line(&self.status_lines)
            .and_then(|selection| selection.data.cli_id())
        else {
            return Ok(());
        };

        let picker = match &**selection {
            CliId::Commit { commit_id, .. } => {
                let commit_id = *commit_id;
                copy_selection_picker::commit_picker(commit_id, self.theme)
            }
            CliId::Branch { name, .. } => {
                let branch = Category::LocalBranch.to_full_name(&**name)?;
                copy_selection_picker::branch_picker(branch, self.theme)
            }
            CliId::UncommittedHunkOrFile(hunk) => {
                copy_selection_picker::uncommitted_hunk_picker(hunk.clone(), self.theme)
            }
            CliId::CommittedFile {
                path,
                id,
                commit_id: _,
            } => copy_selection_picker::committed_file_picker(
                path.to_owned(),
                id.to_owned(),
                self.theme,
            ),
            CliId::PathPrefix { .. } | CliId::Uncommitted { .. } | CliId::Stack { .. } => {
                return Ok(());
            }
        };
        self.modal = Some(Modal::CopySelectionPicker {
            picker: Box::new(picker),
            key_binds: fuzzy_picker_key_binds(),
        });

        Ok(())
    }

    /// Returns the currently selected commit id when the selected line is a commit.
    fn selected_commit_id(&self) -> Option<gix::ObjectId> {
        let selection = self.cursor.selected_line(&self.status_lines)?;

        let StatusOutputLineData::Commit { cli_id, .. } = &selection.data else {
            return None;
        };

        let CliId::Commit { commit_id, .. } = &**cli_id else {
            return None;
        };

        Some(*commit_id)
    }

    fn handle_pick_and_goto_branch(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        match self.flags.show_files {
            FilesStatusFlag::None | FilesStatusFlag::All => {}
            FilesStatusFlag::Commit(_) => return Ok(()),
        }

        let head_info = {
            let meta = ctx.meta()?;
            but_workspace::head_info(
                &*ctx.repo.get()?,
                &meta,
                but_workspace::ref_info::Options {
                    project_meta: ctx.project_meta()?,
                    ..Default::default()
                },
            )?
        };

        let branch_names = head_info
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .filter_map(|segment| {
                let ref_info = segment.ref_info.as_ref()?;
                Some(&ref_info.ref_name)
            })
            .filter(|name| {
                if matches!(&*self.mode, Mode::Rub(_)) {
                    true
                } else {
                    // not all branches are selectable all the time, for example if we're committing
                    // changes assigned to a stack then we cannot select branches outside the stack
                    self.status_lines
                        .iter()
                        .find(|line| {
                            if let Some(id) = line.data.cli_id()
                                && let CliId::Branch {
                                    name: name_on_line, ..
                                } = &**id
                                && name_on_line == name.shorten()
                            {
                                true
                            } else {
                                false
                            }
                        })
                        .is_none_or(|line| {
                            is_selectable_in_mode(line, &self.mode, self.flags.show_files)
                        })
                }
            })
            .map(|name| name.to_owned())
            .collect::<Vec<_>>();

        if let Some(branch_names) = NonEmpty::from_vec(branch_names) {
            let include_uncommitted = Cursor::select_uncommitted(&self.status_lines)
                .and_then(|cursor| cursor.selected_line(&self.status_lines))
                .is_some_and(|uncommitted| {
                    is_selectable_in_mode(uncommitted, &self.mode, self.flags.show_files)
                });

            let picker_items = if include_uncommitted {
                let mut mapped_items = NonEmpty::new(GotoBranchItem::Uncommitted);
                mapped_items.extend(branch_names.map(GotoBranchItem::Branch));
                mapped_items
            } else {
                branch_names.map(GotoBranchItem::Branch)
            };

            self.modal = Some(Modal::GotoBranchPicker {
                picker: Box::new(FuzzyPicker::new(
                    picker_items,
                    self.theme,
                    |item, _ctx, messages| {
                        match item {
                            GotoBranchItem::Branch(branch_name) => {
                                messages.push(Message::SelectBranch(branch_name));
                            }
                            GotoBranchItem::Uncommitted => {
                                messages.push(Message::SelectUncommitted);
                            }
                        }
                        Ok(())
                    },
                )),
                key_binds: fuzzy_picker_key_binds(),
            });
        }

        Ok(())
    }

    fn handle_toggle_help(&mut self) {
        if matches!(self.modal, Some(Modal::Help { .. })) {
            self.modal = None;
        } else {
            self.modal = Some(Modal::Help {
                help: Help::new([&self.app_key_binds.key_binds], self.theme),
                key_binds: help_key_binds(),
            });
        }
    }
}

#[derive(Debug)]
pub struct AppKeyBinds {
    key_binds: KeyBinds,
    normal_with_marks_key_binds: KeyBinds,
}

#[derive(Debug)]
pub enum Modal {
    Confirm {
        confirm: Confirm,
        key_binds: KeyBinds,
    },
    CopySelectionPicker {
        picker: Box<FuzzyPicker<CopySelectionItem>>,
        key_binds: KeyBinds,
    },
    GotoBranchPicker {
        picker: Box<FuzzyPicker<GotoBranchItem>>,
        key_binds: KeyBinds,
    },
    ApplyStackPicker {
        picker: Box<FuzzyPicker<ApplyBranchItem>>,
        key_binds: KeyBinds,
    },
    Help {
        help: Help,
        key_binds: KeyBinds,
    },
}

/// Formats an error for display in the terminal UI without including backtraces.
///
/// The output always starts with the top-level error message and, when available,
/// appends a `Caused by:` section containing every error in the cause chain.
pub fn format_error_for_tui(err: &anyhow::Error) -> String {
    let mut causes = err.chain();

    let Some(top_level) = causes.next() else {
        return "unknown error".to_owned();
    };

    let cause_lines: Vec<String> = causes.map(|cause| cause.to_string()).collect();
    if cause_lines.is_empty() {
        return top_level.to_string();
    }

    let mut output = top_level.to_string();
    output.push_str("\n\nCaused by:\n");

    for (idx, cause) in cause_lines.iter().enumerate() {
        output.push_str(&format!("    {idx}: {cause}"));
        if idx + 1 < cause_lines.len() {
            output.push('\n');
        }
    }

    output
}

#[derive(Debug, Clone)]
pub enum GotoBranchItem {
    Branch(FullName),
    Uncommitted,
}

impl FuzzyPickerItem for GotoBranchItem {
    fn columns(&self, searchable: SearchableToken) -> impl IntoIterator<Item = Col<'_>> {
        match self {
            Self::Branch(full_name) => [Col {
                text: full_name.shorten().to_str_lossy(),
                searchable: Some(searchable),
            }],
            Self::Uncommitted => [Col {
                text: "uncommitted".into(),
                searchable: Some(searchable),
            }],
        }
    }

    fn style(&self, theme: &'static Theme) -> Style {
        match self {
            Self::Branch(..) => theme.local_branch,
            Self::Uncommitted => theme.info,
        }
    }
}
