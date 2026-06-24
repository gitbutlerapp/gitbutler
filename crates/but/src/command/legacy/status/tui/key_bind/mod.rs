use std::{borrow::Cow, collections::HashMap};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use strum::IntoEnumIterator;

use crate::command::legacy::status::tui::{
    CommandMessage, CommitMessageComposer, ConfirmMessage, DetailsLayoutMessage,
    FuzzyPickerMessage, Message, RewordMessage, RubMessage, StackMessage, help::HelpMessage,
    mode::ModeDiscriminant,
};

use super::{
    CommandModeKind, CommitMessage, DetailsMessage, FilesMessage, MoveMessage, ReloadCause,
};

#[cfg(test)]
mod tests;

pub(super) fn default_key_binds() -> KeyBinds {
    let mut key_binds = KeyBinds::new();

    for mode in ModeDiscriminant::iter() {
        let mut builder = key_binds.for_modes([mode]);
        match mode {
            ModeDiscriminant::Normal => {
                register_normal_mode_key_binds(&mut builder, true);
            }
            ModeDiscriminant::PickChanges => {
                builder.mark().register();
                builder.confirm_and_quit().register();
                register_non_mode_specific_key_binds(&mut builder, WithFocusDetails::Yes);
            }
            ModeDiscriminant::Rub => {
                builder.rub_confirm().register();
                builder.rub_use_target_message().register();
                builder.rub_use_source_message().register();
                builder.mark().register();
                register_non_mode_specific_key_binds(&mut builder, WithFocusDetails::No);
            }
            ModeDiscriminant::Commit => {
                builder.commit_confirm().register();
                builder.commit_empty_message().register();
                builder.commit_reword_inline().register();
                builder.commit_toggle_insert_side().register();
                builder.commit_to_new_branch().register();
                register_non_mode_specific_key_binds(&mut builder, WithFocusDetails::No);
            }
            ModeDiscriminant::Move => {
                builder.move_confirm().register();
                builder.move_toggle_insert_side().register();
                register_non_mode_specific_key_binds(&mut builder, WithFocusDetails::No);
            }
            ModeDiscriminant::Stack => {
                builder.apply().register();
                builder.unapply().register();
                builder.reorder().register();
                register_non_mode_specific_key_binds(&mut builder, WithFocusDetails::No);
            }
            ModeDiscriminant::MoveStack => {
                builder.reorder_confirm().register();
                register_non_mode_specific_key_binds(&mut builder, WithFocusDetails::No);
            }
            ModeDiscriminant::Details => {
                builder.details_scroll_up().register();
                builder.details_scroll_down().register();
                builder.details_next_hunk().register();
                builder.details_prev_hunk().register();
                builder.details_jump_up().register();
                builder.details_jump_down().register();

                builder.details_rub().register();
                builder.details_copy().register();
                builder.details_top().register();
                builder.details_bottom().register();
                builder.toggle_full_screen_details().register();

                builder
                    .key_bind(
                        "hide details",
                        press().code(KeyCode::Char('d')),
                        Message::DetailsLayout(DetailsLayoutMessage::Dismiss),
                    )
                    .register();
                builder.grow_details().register();
                builder.shrink_details().register();
                builder.details_focus_status().register();
                builder.focus_details().hide_from_hotbar().register();

                builder.command().register();
                builder.shell_command().register();

                builder.help().register();
                builder.quit().register();

                builder.normal_mode().register();
                builder.back().register();
            }
            ModeDiscriminant::InlineReword => {
                builder.reword_confirm().register();
                builder.reword_open_editor().register();

                builder.normal_mode().register();
                builder.back().register();
            }
            ModeDiscriminant::Command => {
                builder.command_confirm().register();

                builder.normal_mode().register();
                builder.back().register();
            }
        }
    }

    key_binds
}

pub(super) fn confirm_key_binds() -> KeyBinds {
    let mut key_binds = KeyBinds::new();

    let mut builder = key_binds.for_all_modes();

    builder
        .key_bind(
            "select",
            press().code(KeyCode::Enter),
            Message::Confirm(ConfirmMessage::Confirm),
        )
        .register();

    builder
        .key_bind(
            "yes",
            press().code(KeyCode::Char('y')),
            Message::Confirm(ConfirmMessage::Yes),
        )
        .register();

    builder
        .key_bind(
            "no",
            press().code(KeyCode::Char('n')).alt_code(KeyCode::Esc),
            Message::Confirm(ConfirmMessage::No),
        )
        .register();

    builder
        .key_bind(
            "left",
            press().code(KeyCode::Char('h')).alt_code(KeyCode::Left),
            Message::Confirm(ConfirmMessage::Left),
        )
        .register();

    builder
        .key_bind(
            "right",
            press().code(KeyCode::Char('l')).alt_code(KeyCode::Right),
            Message::Confirm(ConfirmMessage::Right),
        )
        .register();

    builder.quit().register();

    key_binds
}

pub(super) fn fuzzy_picker_key_binds() -> KeyBinds {
    let mut key_binds = KeyBinds::new();

    let mut builder = key_binds.for_all_modes();

    builder
        .key_bind(
            "up",
            press().alt_code(KeyCode::Up),
            Message::FuzzyPicker(FuzzyPickerMessage::MoveCursorUp),
        )
        .register();

    builder
        .key_bind(
            "down",
            press().alt_code(KeyCode::Down),
            Message::FuzzyPicker(FuzzyPickerMessage::MoveCursorDown),
        )
        .register();

    builder
        .key_bind(
            "up",
            press().control().code(KeyCode::Char('p')),
            Message::FuzzyPicker(FuzzyPickerMessage::MoveCursorUp),
        )
        .register();

    builder
        .key_bind(
            "down",
            press().control().code(KeyCode::Char('n')),
            Message::FuzzyPicker(FuzzyPickerMessage::MoveCursorDown),
        )
        .register();

    builder
        .key_bind(
            "confirm",
            press().code(KeyCode::Enter),
            Message::FuzzyPicker(FuzzyPickerMessage::Confirm),
        )
        .register();

    builder
        .key_bind(
            "back",
            press().code(KeyCode::Esc),
            Message::FuzzyPicker(FuzzyPickerMessage::Close),
        )
        .register();

    builder
        .key_bind(
            "back",
            press().control().code(KeyCode::Char('[')),
            Message::FuzzyPicker(FuzzyPickerMessage::Close),
        )
        .hide_from_hotbar()
        .register();

    key_binds
}

pub(super) fn help_key_binds() -> KeyBinds {
    let mut key_binds = KeyBinds::new();

    let mut builder = key_binds.for_all_modes();

    builder
        .up_with(Message::Help(HelpMessage::ScrollUp(1)))
        .register();

    builder
        .down_with(Message::Help(HelpMessage::ScrollDown(1)))
        .register();

    builder
        .key_bind(
            "jump up",
            press().control().code(KeyCode::Char('u')),
            Message::Help(HelpMessage::ScrollUp(KeyBindsBuilder::JUMP_DISTANCE)),
        )
        .register();

    builder
        .key_bind(
            "jump down",
            press().control().code(KeyCode::Char('d')),
            Message::Help(HelpMessage::ScrollDown(KeyBindsBuilder::JUMP_DISTANCE)),
        )
        .register();

    builder
        .key_bind(
            "back",
            press().code(KeyCode::Char('?')).alt_code(KeyCode::Esc),
            Message::Help(HelpMessage::Close),
        )
        .register();

    builder
        .key_bind(
            "back",
            press().control().code(KeyCode::Char('[')),
            Message::Help(HelpMessage::Close),
        )
        .hide_from_hotbar()
        .register();

    builder.quit().register();

    key_binds
}

pub(super) fn normal_with_marks_key_binds() -> KeyBinds {
    let mut key_binds = KeyBinds::new();

    let mut builder = key_binds.for_modes(Vec::from([ModeDiscriminant::Normal]));

    register_normal_mode_key_binds(&mut builder, false);

    key_binds
}

#[derive(Clone, Copy, Debug)]
struct KeyBindId(usize);

#[derive(Debug)]
pub(super) struct KeyBinds {
    /// All registered key binds.
    all_key_binds: Vec<KeyBind>,
    /// Which key binds are available in which modes?
    mode_to_key_binds: HashMap<ModeDiscriminant, Vec<KeyBindId>>,
}

impl KeyBinds {
    fn new() -> Self {
        KeyBinds {
            mode_to_key_binds: Default::default(),
            all_key_binds: Default::default(),
        }
    }

    fn for_modes(
        &mut self,
        modes: impl IntoIterator<Item = ModeDiscriminant>,
    ) -> KeyBindsBuilder<'_> {
        KeyBindsBuilder {
            key_binds: self,
            modes: modes.into_iter().collect(),
        }
    }

    fn for_all_modes(&mut self) -> KeyBindsBuilder<'_> {
        self.for_modes(ModeDiscriminant::iter())
    }

    fn register(&mut self, key_bind: KeyBind) -> KeyBindId {
        let id = KeyBindId(self.all_key_binds.len());

        for mode in ModeDiscriminant::iter() {
            if key_bind.available_in_mode(mode) {
                self.mode_to_key_binds.entry(mode).or_default().push(id);
            }
        }

        self.all_key_binds.push(key_bind);

        id
    }

    pub(super) fn iter_key_binds_available_in_mode(
        &self,
        mode: ModeDiscriminant,
    ) -> impl Iterator<Item = &KeyBind> {
        self.mode_to_key_binds
            .get(&mode)
            .into_iter()
            .flatten()
            .copied()
            .map(|KeyBindId(idx)| &self.all_key_binds[idx])
    }
}

#[derive(Debug)]
struct KeyBindsBuilder<'a> {
    key_binds: &'a mut KeyBinds,
    modes: Vec<ModeDiscriminant>,
}

impl KeyBindsBuilder<'_> {
    fn key_bind(
        &mut self,
        short_description: &'static str,
        key_matcher: KeyMatcher,
        message: Message,
    ) -> KeyBindsInModesBuilder<'_> {
        KeyBindsInModesBuilder {
            key_binds: self.key_binds,
            short_description,
            long_description: None,
            key_matcher,
            modes: self.modes.clone(),
            message,
            hide_from_hotbar: false,
            show_only_in_normal_mode_help_section: false,
            always_show_in_hot_bar: false,
        }
    }

    fn down(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.down_with(Message::MoveCursorDown(1))
            .show_only_in_normal_mode_help_section()
    }

    fn down_with(&mut self, msg: Message) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "down",
            press().code(KeyCode::Char('j')).alt_code(KeyCode::Down),
            msg,
        )
    }

    fn up(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.up_with(Message::MoveCursorUp(1))
            .show_only_in_normal_mode_help_section()
    }

    fn up_with(&mut self, msg: Message) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "up",
            press().code(KeyCode::Char('k')).alt_code(KeyCode::Up),
            msg,
        )
    }

    fn next_section(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "next section",
            press().shift().code(KeyCode::Char('J')),
            Message::MoveCursorNextSection,
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn prev_section(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "prev section",
            press().shift().code(KeyCode::Char('K')),
            Message::MoveCursorPreviousSection,
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    const JUMP_DISTANCE: usize = 10;

    fn jump_up(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "jump up",
            press().control().code(KeyCode::Char('u')),
            Message::MoveCursorUp(Self::JUMP_DISTANCE),
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn jump_down(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "jump down",
            press().control().code(KeyCode::Char('d')),
            Message::MoveCursorDown(Self::JUMP_DISTANCE),
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn toggle_details(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "details",
            press().code(KeyCode::Char('d')),
            Message::DetailsLayout(DetailsLayoutMessage::ToggleVisibility),
        )
        .long_description("Toggle the details view")
        .show_only_in_normal_mode_help_section()
    }

    fn toggle_full_screen_details(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "full screen details",
            press().shift().code(KeyCode::Char('D')),
            Message::DetailsLayout(DetailsLayoutMessage::ToggleFullScreen),
        )
        .hide_from_hotbar()
        .long_description("Toggle the full screen details view")
        .show_only_in_normal_mode_help_section()
    }

    fn normal_mode(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "normal mode",
            press().control().code(KeyCode::Char('[')),
            Message::Back,
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn grow_details(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "grow details",
            press().code(KeyCode::Char('+')),
            Message::GrowDetails,
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn shrink_details(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "shrink details",
            press().code(KeyCode::Char('-')),
            Message::ShrinkDetails,
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn undo(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind("undo", press().code(KeyCode::Char('u')), Message::Undo)
            .show_only_in_normal_mode_help_section()
            .long_description("Undo the last operation")
    }

    fn redo(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "redo",
            press().shift().code(KeyCode::Char('U')),
            Message::Redo,
        )
        .show_only_in_normal_mode_help_section()
        .hide_from_hotbar()
        .long_description("Redo the last undo")
    }

    fn mark(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind("mark", press().code(KeyCode::Char(' ')), Message::Mark)
            .show_only_in_normal_mode_help_section()
            .long_description("Mark and rub multiple items")
    }

    fn quit(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind("quit", press().code(KeyCode::Char('q')), Message::Quit)
            .show_only_in_normal_mode_help_section()
            .always_show_in_hot_bar()
    }

    fn help(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "help",
            press().code(KeyCode::Char('?')),
            Message::ToggleHelp,
        )
        .show_only_in_normal_mode_help_section()
        .long_description("Show this help menu")
        .always_show_in_hot_bar()
    }

    fn uncommitted_area(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "goto uncommitted",
            press().code(KeyCode::Char('g')),
            Message::SelectUncommitted,
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn merge_base(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "goto merge base",
            press().shift().code(KeyCode::Char('G')),
            Message::SelectMergeBase,
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn branch_picker(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "pick branch",
            press().code(KeyCode::Char('t')),
            Message::PickAndGotoBranch,
        )
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
        .long_description("Fuzzy search for branches")
    }

    fn rub(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "rub",
            press().code(KeyCode::Char('r')),
            Message::Rub(RubMessage::Start),
        )
        .long_description("Squash or undo commits")
    }

    fn reverse_rub(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "reverse rub",
            press().shift().code(KeyCode::Char('R')),
            Message::Rub(RubMessage::StartReverse),
        )
        .long_description("Rub uncommitted changes into selection")
        .hide_from_hotbar()
    }

    fn commit(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "commit",
            press().code(KeyCode::Char('c')),
            Message::Commit(CommitMessage::Start),
        )
        .long_description("Create a new commit")
    }

    fn new_commit(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "empty commit",
            press().code(KeyCode::Char('n')),
            Message::Commit(CommitMessage::CreateEmpty),
        )
        .long_description("Insert empty commit")
        .hide_from_hotbar()
    }

    fn move_mode(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "move",
            press().code(KeyCode::Char('m')),
            Message::Move(MoveMessage::Start),
        )
        .long_description("Move selection somewhere else")
    }

    fn move_toggle_insert_side(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "above/below",
            press().code(KeyCode::Char('a')),
            Message::Move(MoveMessage::ToggleInsertSide),
        )
        .long_description("Toggle moving above or below")
    }

    fn branch(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "branch",
            press().code(KeyCode::Char('b')),
            Message::NewBranch,
        )
        .long_description("Create a new branch")
    }

    fn stack(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "stack",
            press().code(KeyCode::Char('s')),
            Message::Stack(StackMessage::Enter),
        )
        .long_description("Enter stack mode")
    }

    fn focus_details(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "focus details",
            press().code(KeyCode::Char('l')),
            Message::DetailsLayout(DetailsLayoutMessage::Focus { full_screen: false }),
        )
    }

    fn reword_inline(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "reword",
            press().code(KeyCode::Enter),
            Message::Reword(RewordMessage::InlineStart),
        )
        .long_description("Reword commit or branch inline")
    }

    fn reword_editor(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "reword with editor",
            press().shift().code(KeyCode::Char('M')),
            Message::Reword(RewordMessage::WithEditor),
        )
        .long_description("Reword commit with the configured editor")
        .hide_from_hotbar()
    }

    fn files(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "files",
            press().code(KeyCode::Char('f')),
            Message::Files(FilesMessage::ToggleFilesForCommit),
        )
        .long_description("Show files in commit")
    }

    fn all_files(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "show all files",
            press().shift().code(KeyCode::Char('F')),
            Message::Files(FilesMessage::ToggleGlobalFilesList),
        )
        .long_description("Show files in all commits")
        .hide_from_hotbar()
    }

    fn discard(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "discard",
            press().code(KeyCode::Char('x')),
            Message::Discard,
        )
        .long_description("Discard the selection")
    }

    fn command(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "but command",
            press().code(KeyCode::Char(':')),
            Message::Command(CommandMessage::Start(CommandModeKind::But)),
        )
        .long_description("Run a `but` command")
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn shell_command(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "shell command",
            press().code(KeyCode::Char('!')),
            Message::Command(CommandMessage::Start(CommandModeKind::Shell)),
        )
        .long_description("Run any shell command")
        .hide_from_hotbar()
        .show_only_in_normal_mode_help_section()
    }

    fn reload(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "reload",
            press().control().code(KeyCode::Char('r')),
            Message::Reload(None, ReloadCause::Manual),
        )
        .hide_from_hotbar()
    }

    fn copy(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "copy",
            press().code(KeyCode::Char('y')),
            Message::CopySelection,
        )
        .hide_from_hotbar()
        .long_description("Copy selection")
    }

    fn copy_picker(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "copy more",
            press().shift().code(KeyCode::Char('Y')),
            Message::CopySelectionPicker,
        )
        .hide_from_hotbar()
        .long_description("Copy selection picker")
    }

    fn back(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind("back", press().code(KeyCode::Esc), Message::Back)
            .hide_from_hotbar()
            .show_only_in_normal_mode_help_section()
    }

    fn confirm_and_quit(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "confirm",
            press().code(KeyCode::Enter).alt_code(KeyCode::Char('c')),
            Message::ConfirmAndQuit,
        )
        .long_description("Rub target into selection")
    }

    fn rub_use_target_message(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "use target message",
            press().shift().code(KeyCode::Char('T')),
            Message::Rub(RubMessage::UseTargetMessage),
        )
        .long_description("When squashing use target message and discard source message")
    }

    fn rub_use_source_message(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "use source message",
            press().shift().code(KeyCode::Char('S')),
            Message::Rub(RubMessage::UseSourceMessage),
        )
        .long_description("When squashing use source message and discard target message")
    }

    fn rub_confirm(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "confirm",
            press().code(KeyCode::Enter),
            Message::Rub(RubMessage::Confirm),
        )
        .long_description("Rub target into selection")
    }

    fn reword_open_editor(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "open editor",
            press().alt().code(KeyCode::Char('e')),
            Message::Reword(RewordMessage::OpenEditor),
        )
    }

    fn reword_confirm(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "confirm",
            press().code(KeyCode::Enter),
            Message::Reword(RewordMessage::InlineConfirm),
        )
    }

    fn command_confirm(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "run command",
            press().code(KeyCode::Enter),
            Message::Command(CommandMessage::Confirm),
        )
    }

    fn commit_confirm(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "confirm",
            press().code(KeyCode::Enter),
            Message::Commit(CommitMessage::Confirm),
        )
    }

    fn commit_toggle_insert_side(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "above/below",
            press().code(KeyCode::Char('a')),
            Message::Commit(CommitMessage::ToggleInsertSide),
        )
        .long_description("Toggle committing above or below")
    }

    fn commit_empty_message(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "empty message",
            press().code(KeyCode::Char('e')),
            Message::Commit(CommitMessage::ToggleMessageComposer(
                CommitMessageComposer::Empty,
            )),
        )
        .long_description("When creating commit, leave message empty")
    }

    fn commit_reword_inline(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "reword inline",
            press().code(KeyCode::Char('i')),
            Message::Commit(CommitMessage::ToggleMessageComposer(
                CommitMessageComposer::Inline,
            )),
        )
        .long_description("When creating commit, reword it inline")
    }

    fn commit_to_new_branch(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "commit to new branch",
            press().code(KeyCode::Char('b')),
            Message::Commit(CommitMessage::CommitToNewBranch),
        )
        .long_description("Create a new branch, then commit to it")
    }

    fn move_confirm(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "confirm",
            press().code(KeyCode::Enter),
            Message::Move(MoveMessage::Confirm),
        )
    }

    fn apply(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "apply",
            press().code(KeyCode::Char('a')),
            Message::Stack(StackMessage::ShowApplyPicker),
        )
        .long_description("Apply stack")
    }

    fn unapply(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "unapply",
            press().code(KeyCode::Char('u')),
            Message::Stack(StackMessage::Unapply),
        )
        .long_description("Unapply stack")
    }

    fn reorder(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "move",
            press().code(KeyCode::Char('m')),
            Message::Stack(StackMessage::MoveStart),
        )
        .long_description("Move stack")
    }

    fn reorder_confirm(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "confirm",
            press().code(KeyCode::Enter),
            Message::Stack(StackMessage::MoveConfirm),
        )
    }

    fn details_next_hunk(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "next hunk",
            press().shift().code(KeyCode::Char('J')),
            Message::Details(DetailsMessage::SelectNextSection),
        )
    }

    fn details_prev_hunk(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "prev hunk",
            press().shift().code(KeyCode::Char('K')),
            Message::Details(DetailsMessage::SelectPrevSection),
        )
    }

    fn details_scroll_up(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "up",
            press().code(KeyCode::Char('k')).alt_code(KeyCode::Up),
            Message::Details(DetailsMessage::ScrollUp(1)),
        )
    }

    fn details_scroll_down(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "down",
            press().code(KeyCode::Char('j')).alt_code(KeyCode::Down),
            Message::Details(DetailsMessage::ScrollDown(1)),
        )
    }

    fn details_jump_up(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "jump up",
            press().control().code(KeyCode::Char('u')),
            Message::Details(DetailsMessage::ScrollUp(Self::JUMP_DISTANCE)),
        )
    }

    fn details_jump_down(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "jump down",
            press().control().code(KeyCode::Char('d')),
            Message::Details(DetailsMessage::ScrollDown(Self::JUMP_DISTANCE)),
        )
    }

    fn details_rub(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "rub",
            press().code(KeyCode::Char('r')),
            Message::Details(DetailsMessage::StartRub),
        )
    }

    fn details_copy(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "copy hunk",
            press().code(KeyCode::Char('y')),
            Message::Details(DetailsMessage::CopyCurrentHunk),
        )
        .hide_from_hotbar()
        .long_description("Copy current hunk to clipboard")
    }

    fn details_focus_status(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "focus status",
            press().code(KeyCode::Char('h')),
            Message::UnfocusDetails,
        )
    }

    fn details_top(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "goto top",
            press().code(KeyCode::Char('g')),
            Message::Details(DetailsMessage::GotoTop),
        )
    }

    fn details_bottom(&mut self) -> KeyBindsInModesBuilder<'_> {
        self.key_bind(
            "goto bottom",
            press().shift().code(KeyCode::Char('G')),
            Message::Details(DetailsMessage::GotoBottom),
        )
    }
}

fn register_normal_mode_key_binds(builder: &mut KeyBindsBuilder<'_>, without_marks: bool) {
    builder.up().register();
    builder.down().register();
    builder.next_section().register();
    builder.prev_section().register();
    builder.jump_up().register();
    builder.jump_down().register();

    builder.rub().register();

    if without_marks {
        builder.reverse_rub().register();
    }

    builder.commit().register();

    if without_marks {
        builder.new_commit().register();
    }

    builder.move_mode().register();

    if without_marks {
        builder.branch().register();
        builder.stack().register();
    }

    builder.toggle_details().register();
    builder.toggle_full_screen_details().register();
    builder.focus_details().register();
    builder.grow_details().register();
    builder.shrink_details().register();

    if without_marks {
        builder.reword_inline().register();
        builder.reword_editor().register();
    }

    builder.files().register();
    builder.all_files().register();

    builder.discard().register();

    builder.mark().register();

    builder.undo().register();
    builder.redo().register();

    builder.branch_picker().register();
    builder.uncommitted_area().register();
    builder.merge_base().register();
    builder.command().register();
    builder.shell_command().register();

    if without_marks {
        builder.copy().register();
        builder.copy_picker().register();
    }

    builder.reload().register();
    builder.back().hide_from_hotbar().register();
    builder.help().register();
    builder.quit().register();
}

enum WithFocusDetails {
    Yes,
    No,
}

fn register_non_mode_specific_key_binds(
    builder: &mut KeyBindsBuilder<'_>,
    with_focus_details: WithFocusDetails,
) {
    builder.up().register();
    builder.down().register();
    builder.next_section().register();
    builder.prev_section().register();
    builder.jump_up().register();
    builder.jump_down().register();
    builder.toggle_details().register();
    builder.toggle_full_screen_details().register();

    if matches!(with_focus_details, WithFocusDetails::Yes) {
        builder.focus_details().register();
    }

    builder.grow_details().register();
    builder.shrink_details().register();
    builder.branch_picker().register();
    builder.uncommitted_area().register();
    builder.merge_base().register();
    builder.help().register();
    builder.quit().register();
    builder.normal_mode().register();
    builder.back().register();
}

#[derive(Debug)]
#[must_use]
struct KeyBindsInModesBuilder<'a> {
    key_binds: &'a mut KeyBinds,
    short_description: &'static str,
    long_description: Option<&'static str>,
    key_matcher: KeyMatcher,
    modes: Vec<ModeDiscriminant>,
    message: Message,
    hide_from_hotbar: bool,
    show_only_in_normal_mode_help_section: bool,
    always_show_in_hot_bar: bool,
}

impl KeyBindsInModesBuilder<'_> {
    fn hide_from_hotbar(mut self) -> Self {
        self.hide_from_hotbar = true;
        self
    }

    fn always_show_in_hot_bar(mut self) -> Self {
        self.always_show_in_hot_bar = true;
        self
    }

    /// Normally `?` shows key binds in all the methods they're available in. However that results
    /// in a lot of noise since many key binds (such as moving the cursor) are available in all
    /// modes.
    ///
    /// This methods is used to hide such key binds from non-normal modes to reduce that noise.
    ///
    /// The intention is that shared key binds are shown in normal mode so other modes can only
    /// show key binds specific to them.
    fn show_only_in_normal_mode_help_section(mut self) -> Self {
        self.show_only_in_normal_mode_help_section = true;
        self
    }

    #[expect(dead_code)]
    fn short_description(mut self, short_description: &'static str) -> Self {
        self.short_description = short_description;
        self
    }

    fn long_description(mut self, long_description: &'static str) -> Self {
        self.long_description = Some(long_description);
        self
    }

    fn register(self) -> KeyBindId {
        let KeyBindsInModesBuilder {
            key_binds,
            short_description,
            long_description,
            key_matcher,
            modes,
            message,
            hide_from_hotbar,
            show_only_in_normal_mode_help_section,
            always_show_in_hot_bar,
        } = self;

        key_binds.register(KeyBind {
            short_description,
            long_description,
            chord_display: key_matcher.chord_display(),
            key_matcher,
            modes,
            message,
            hide_from_hotbar,
            show_only_in_normal_mode_help_section,
            always_show_in_hot_bar,
        })
    }
}

#[derive(Debug)]
pub(super) struct KeyBind {
    short_description: &'static str,
    long_description: Option<&'static str>,
    chord_display: Cow<'static, str>,
    key_matcher: KeyMatcher,
    modes: Vec<ModeDiscriminant>,
    message: Message,
    hide_from_hotbar: bool,
    show_only_in_normal_mode_help_section: bool,
    always_show_in_hot_bar: bool,
}

impl KeyBind {
    pub(super) fn short_description(&self) -> &str {
        self.short_description
    }

    pub(super) fn long_description(&self) -> Option<&str> {
        self.long_description
    }

    pub(super) fn chord_display(&self) -> &str {
        &self.chord_display
    }

    pub(super) fn available_in_mode(&self, mode: ModeDiscriminant) -> bool {
        self.modes.contains(&mode)
    }

    pub(super) fn matches(&self, ev: &KeyEvent) -> bool {
        self.key_matcher.matches(ev)
    }

    pub(super) fn message(&self) -> Message {
        self.message.clone()
    }

    pub(super) fn hide_from_hotbar(&self) -> bool {
        self.hide_from_hotbar
    }

    pub(super) fn always_show_in_hot_bar(&self) -> bool {
        self.always_show_in_hot_bar
    }

    pub(super) fn show_only_in_normal_mode_help_section(&self) -> bool {
        self.show_only_in_normal_mode_help_section
    }
}

#[inline]
fn press() -> KeyMatcher {
    KeyMatcher {
        kind: KeyEventKind::Press,
        modifiers: KeyModifiers::NONE,
        codes: [None, None],
    }
}

#[derive(Debug, Copy, Clone)]
struct KeyMatcher {
    kind: KeyEventKind,
    modifiers: KeyModifiers,
    codes: [Option<KeyCode>; 2],
}

impl KeyMatcher {
    #[inline]
    fn alt(self) -> Self {
        self.modifiers(KeyModifiers::ALT)
    }

    #[inline]
    fn shift(self) -> Self {
        self.modifiers(KeyModifiers::SHIFT)
    }

    #[inline]
    fn control(self) -> Self {
        self.modifiers(KeyModifiers::CONTROL)
    }

    #[inline]
    fn modifiers(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    #[inline]
    fn code(mut self, code: KeyCode) -> Self {
        self.codes[0] = Some(code);
        self
    }

    #[inline]
    fn alt_code(mut self, code: KeyCode) -> Self {
        self.codes[1] = Some(code);
        self
    }

    /// Render this matcher into the hotbar chord display format.
    fn chord_display(&self) -> Cow<'static, str> {
        let mut codes = self.codes.into_iter().flatten().collect::<Vec<_>>();
        codes.sort_by_key(|code| self.display_sort_key(*code));

        let displays = codes
            .into_iter()
            .map(|code| self.format_code(code))
            .collect::<Vec<_>>();
        Cow::Owned(displays.join("/"))
    }

    /// Return the sort key used to produce a stable, user-facing display order.
    fn display_sort_key(&self, code: KeyCode) -> u8 {
        match code {
            KeyCode::Char(_) => 1,
            _ => 0,
        }
    }

    /// Format a single key code together with this matcher's modifiers.
    fn format_code(&self, code: KeyCode) -> String {
        let mut prefixes = Vec::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            prefixes.push("ctrl");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            prefixes.push("alt");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            prefixes.push("shift");
        }

        let key = format_key_code(code, self.modifiers.contains(KeyModifiers::SHIFT));
        if prefixes.is_empty() {
            key
        } else {
            format!("{}+{key}", prefixes.join("+"))
        }
    }

    #[inline]
    fn matches(self, ev: &KeyEvent) -> bool {
        if self.kind != ev.kind {
            return false;
        }

        if self.modifiers != ev.modifiers {
            return false;
        }

        self.codes
            .into_iter()
            .flatten()
            .any(|key_code| key_code == ev.code)
    }
}

/// Format a key code into the hotbar display representation.
fn format_key_code(code: KeyCode, shifted: bool) -> String {
    match code {
        KeyCode::Backspace => "backspace".to_owned(),
        KeyCode::Enter => "enter".to_owned(),
        KeyCode::Left => "←".to_owned(),
        KeyCode::Right => "→".to_owned(),
        KeyCode::Up => "↑".to_owned(),
        KeyCode::Down => "↓".to_owned(),
        KeyCode::Home => "home".to_owned(),
        KeyCode::End => "end".to_owned(),
        KeyCode::PageUp => "pageup".to_owned(),
        KeyCode::PageDown => "pagedown".to_owned(),
        KeyCode::Tab => "tab".to_owned(),
        KeyCode::BackTab => "backtab".to_owned(),
        KeyCode::Delete => "del".to_owned(),
        KeyCode::Insert => "ins".to_owned(),
        KeyCode::Esc => "esc".to_owned(),
        KeyCode::Char(' ') => "space".to_owned(),
        KeyCode::Char(ch) => normalize_char_for_display(ch, shifted).to_string(),
        KeyCode::Null => "null".to_owned(),
        KeyCode::CapsLock => "capslock".to_owned(),
        KeyCode::ScrollLock => "scrolllock".to_owned(),
        KeyCode::NumLock => "numlock".to_owned(),
        KeyCode::PrintScreen => "printscreen".to_owned(),
        KeyCode::Pause => "pause".to_owned(),
        KeyCode::Menu => "menu".to_owned(),
        KeyCode::KeypadBegin => "keypadbegin".to_owned(),
        KeyCode::Media(_) => "media".to_owned(),
        KeyCode::Modifier(_) => "modifier".to_owned(),
        KeyCode::F(number) => format!("f{number}"),
    }
}

/// Normalize a character for chord display rendering.
fn normalize_char_for_display(ch: char, shifted: bool) -> char {
    if shifted && ch.is_ascii_alphabetic() {
        ch.to_ascii_lowercase()
    } else {
        ch
    }
}
