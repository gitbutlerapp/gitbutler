use std::collections::HashMap;

use but_rebase::graph_rebase::mutate::InsertSide;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use strum::IntoEnumIterator;

use crate::command::legacy::status::tui::{
    CommandMessage, ConfirmMessage, Message, Mode, ModeDiscriminant, RewordMessage, RubMessage,
};

use super::{BranchMessage, CommitMessage, DetailsMessage, FilesMessage, MoveMessage};

pub(super) fn default_key_binds() -> KeyBinds {
    let mut key_binds = KeyBinds::new();

    register_global_key_binds(&mut key_binds);

    for mode in ModeDiscriminant::iter() {
        match mode {
            ModeDiscriminant::Normal => {
                register_normal_mode_key_binds(&mut key_binds);
            }
            ModeDiscriminant::Rub => {
                register_rub_mode_key_binds(&mut key_binds);
            }
            ModeDiscriminant::RubButApi => {
                register_rub_but_api_mode_key_binds(&mut key_binds);
            }
            ModeDiscriminant::InlineReword => {
                register_inline_reword_mode_key_binds(&mut key_binds);
            }
            ModeDiscriminant::Command => {
                register_command_mode_key_binds(&mut key_binds);
            }
            ModeDiscriminant::Commit => {
                register_commit_mode_key_binds(&mut key_binds);
            }
            ModeDiscriminant::Move => {
                register_move_mode_key_binds(&mut key_binds);
            }
            ModeDiscriminant::Branch => {
                register_branch_mode_key_binds(&mut key_binds);
            }
        }
    }

    key_binds
}

pub(super) fn confirm_key_binds() -> KeyBinds {
    let mut key_binds = KeyBinds::new();

    let all_modes = ModeDiscriminant::iter().collect::<Vec<_>>();

    register_quit_key_binds(&mut key_binds, all_modes.clone());

    key_binds.register(StaticKeyBind {
        short_description: "select",
        chord_display: "enter",
        key_matcher: press().code(KeyCode::Enter),
        modes: all_modes.clone(),
        message: Message::Confirm(ConfirmMessage::Confirm),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "yes",
        chord_display: "y",
        key_matcher: press().code(KeyCode::Char('y')),
        modes: all_modes.clone(),
        message: Message::Confirm(ConfirmMessage::Yes),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "no",
        chord_display: "esc/n",
        key_matcher: press().code(KeyCode::Char('n')).alt_code(KeyCode::Esc),
        modes: all_modes.clone(),
        message: Message::Confirm(ConfirmMessage::No),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "left",
        chord_display: "←/h",
        key_matcher: press().code(KeyCode::Char('h')).alt_code(KeyCode::Left),
        modes: all_modes.clone(),
        message: Message::Confirm(ConfirmMessage::Left),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "right",
        chord_display: "→/l",
        key_matcher: press().code(KeyCode::Char('l')).alt_code(KeyCode::Right),
        modes: all_modes.clone(),
        message: Message::Confirm(ConfirmMessage::Right),
        hide_from_hotbar: false,
    });

    key_binds
}

fn register_global_key_binds(key_binds: &mut KeyBinds) {
    let all_except_text_input_modes = ModeDiscriminant::iter()
        .filter(|mode| match mode {
            ModeDiscriminant::Normal
            | ModeDiscriminant::Rub
            | ModeDiscriminant::RubButApi
            | ModeDiscriminant::Move
            | ModeDiscriminant::Branch
            | ModeDiscriminant::Commit => true,
            ModeDiscriminant::InlineReword | ModeDiscriminant::Command => false,
        })
        .collect::<Vec<_>>();

    let all_modes = ModeDiscriminant::iter().collect::<Vec<_>>();

    key_binds.register(StaticKeyBind {
        short_description: "down",
        chord_display: "↓/j",
        key_matcher: press().code(KeyCode::Char('j')).alt_code(KeyCode::Down),
        modes: all_except_text_input_modes.clone(),
        message: Message::MoveCursorDown,
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "up",
        chord_display: "↑/k",
        key_matcher: press().code(KeyCode::Char('k')).alt_code(KeyCode::Up),
        modes: all_except_text_input_modes.clone(),
        message: Message::MoveCursorUp,
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "next section",
        chord_display: "shift+j",
        key_matcher: press().shift().code(KeyCode::Char('J')),
        modes: all_except_text_input_modes.clone(),
        message: Message::MoveCursorNextSection,
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "prev section",
        chord_display: "shift+k",
        key_matcher: press().shift().code(KeyCode::Char('K')),
        modes: all_except_text_input_modes.clone(),
        message: Message::MoveCursorPreviousSection,
        hide_from_hotbar: true,
    });

    register_quit_key_binds(key_binds, all_except_text_input_modes.clone());

    key_binds.register(StaticKeyBind {
        short_description: "normal mode",
        chord_display: "ctrl+[",
        key_matcher: press().control().code(KeyCode::Char('[')),
        modes: all_modes,
        message: Message::EnterNormalMode,
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "scroll details down",
        chord_display: "ctrl+n",
        key_matcher: press().control().code(KeyCode::Char('n')),
        modes: all_except_text_input_modes.clone(),
        message: Message::Details(DetailsMessage::ScrollDown(1)),
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "scroll details up",
        chord_display: "ctrl+p",
        key_matcher: press().control().code(KeyCode::Char('p')),
        modes: all_except_text_input_modes.clone(),
        message: Message::Details(DetailsMessage::ScrollUp(1)),
        hide_from_hotbar: true,
    });

    let jump_distance = 30;

    key_binds.register(StaticKeyBind {
        short_description: "jump details down",
        chord_display: "ctrl+d",
        key_matcher: press().control().code(KeyCode::Char('d')),
        modes: all_except_text_input_modes.clone(),
        message: Message::Details(DetailsMessage::ScrollDown(jump_distance)),
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "jump details up",
        chord_display: "ctrl+u",
        key_matcher: press().control().code(KeyCode::Char('u')),
        modes: all_except_text_input_modes.clone(),
        message: Message::Details(DetailsMessage::ScrollUp(jump_distance)),
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "diff",
        chord_display: "d",
        key_matcher: press().code(KeyCode::Char('d')),
        modes: all_except_text_input_modes.clone(),
        message: Message::Details(DetailsMessage::ToggleVisibility),
        hide_from_hotbar: false,
    });
}

fn register_quit_key_binds(key_binds: &mut KeyBinds, modes: Vec<ModeDiscriminant>) {
    key_binds.register(StaticKeyBind {
        short_description: "quit",
        chord_display: "q",
        key_matcher: press().code(KeyCode::Char('q')),
        modes,
        message: Message::Quit,
        hide_from_hotbar: false,
    });
}

fn register_normal_mode_key_binds(key_binds: &mut KeyBinds) {
    key_binds.register(StaticKeyBind {
        short_description: "rub",
        chord_display: "r",
        key_matcher: press().code(KeyCode::Char('r')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Rub(RubMessage::Start {
            using_but_api: false,
        }),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "rub (but-api)",
        chord_display: "shift+r",
        key_matcher: press().shift().code(KeyCode::Char('R')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Rub(RubMessage::Start {
            using_but_api: true,
        }),
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "commit",
        chord_display: "c",
        key_matcher: press().code(KeyCode::Char('c')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Commit(CommitMessage::Start),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "new commit",
        chord_display: "n",
        key_matcher: press().code(KeyCode::Char('n')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Commit(CommitMessage::CreateEmpty),
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "move",
        chord_display: "m",
        key_matcher: press().code(KeyCode::Char('m')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Move(MoveMessage::Start),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "branch",
        chord_display: "b",
        key_matcher: press().code(KeyCode::Char('b')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Branch(BranchMessage::Start),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "reword inline",
        chord_display: "enter",
        key_matcher: press().code(KeyCode::Enter),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Reword(RewordMessage::InlineStart),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "reword",
        chord_display: "shift+m",
        key_matcher: press().shift().code(KeyCode::Char('M')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Reword(RewordMessage::WithEditor),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "files",
        chord_display: "f",
        key_matcher: press().code(KeyCode::Char('f')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Files(FilesMessage::ToggleFilesForCommit),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "show all files",
        chord_display: "shift+f",
        key_matcher: press().shift().code(KeyCode::Char('F')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Files(FilesMessage::ToggleGlobalFilesList),
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "command",
        chord_display: ":",
        key_matcher: press().code(KeyCode::Char(':')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Command(CommandMessage::Start),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "reload",
        chord_display: "ctrl+r",
        key_matcher: press().control().code(KeyCode::Char('r')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::Reload(None),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "copy",
        chord_display: "shift+c",
        key_matcher: press().shift().code(KeyCode::Char('C')),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::CopySelection,
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "esc",
        key_matcher: press().code(KeyCode::Esc),
        modes: Vec::from([ModeDiscriminant::Normal]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: true,
    });
}

fn register_rub_mode_key_binds(key_binds: &mut KeyBinds) {
    key_binds.register(StaticKeyBind {
        short_description: "confirm",
        chord_display: "enter",
        key_matcher: press().code(KeyCode::Enter),
        modes: Vec::from([ModeDiscriminant::Rub]),
        message: Message::Rub(RubMessage::Confirm),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "esc",
        key_matcher: press().code(KeyCode::Esc),
        modes: Vec::from([ModeDiscriminant::Rub]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "r",
        key_matcher: press().code(KeyCode::Char('r')),
        modes: Vec::from([ModeDiscriminant::Rub]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: true,
    });
}

fn register_rub_but_api_mode_key_binds(key_binds: &mut KeyBinds) {
    key_binds.register(StaticKeyBind {
        short_description: "confirm",
        chord_display: "enter",
        key_matcher: press().code(KeyCode::Enter),
        modes: Vec::from([ModeDiscriminant::RubButApi]),
        message: Message::Rub(RubMessage::Confirm),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "esc",
        key_matcher: press().code(KeyCode::Esc),
        modes: Vec::from([ModeDiscriminant::RubButApi]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: false,
    });
}

fn register_inline_reword_mode_key_binds(key_binds: &mut KeyBinds) {
    key_binds.register(StaticKeyBind {
        short_description: "confirm",
        chord_display: "enter",
        key_matcher: press().code(KeyCode::Enter),
        modes: Vec::from([ModeDiscriminant::InlineReword]),
        message: Message::Reword(RewordMessage::InlineConfirm),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "esc",
        key_matcher: press().code(KeyCode::Esc),
        modes: Vec::from([ModeDiscriminant::InlineReword]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: false,
    });
}

fn register_command_mode_key_binds(key_binds: &mut KeyBinds) {
    key_binds.register(StaticKeyBind {
        short_description: "run",
        chord_display: "enter",
        key_matcher: press().code(KeyCode::Enter),
        modes: Vec::from([ModeDiscriminant::Command]),
        message: Message::Command(CommandMessage::Confirm),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "esc",
        key_matcher: press().code(KeyCode::Esc),
        modes: Vec::from([ModeDiscriminant::Command]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: false,
    });
}

fn register_commit_mode_key_binds(key_binds: &mut KeyBinds) {
    key_binds.register(StaticKeyBind {
        short_description: "commit",
        chord_display: "enter",
        key_matcher: press().code(KeyCode::Enter),
        modes: Vec::from([ModeDiscriminant::Commit]),
        message: Message::Commit(CommitMessage::Confirm { with_message: true }),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "above",
        chord_display: "a",
        key_matcher: press().code(KeyCode::Char('a')),
        modes: Vec::from([ModeDiscriminant::Commit]),
        message: Message::Commit(CommitMessage::SetInsertSide(InsertSide::Above)),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "below",
        chord_display: "b",
        key_matcher: press().code(KeyCode::Char('b')),
        modes: Vec::from([ModeDiscriminant::Commit]),
        message: Message::Commit(CommitMessage::SetInsertSide(InsertSide::Below)),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "c",
        key_matcher: press().code(KeyCode::Char('c')),
        modes: Vec::from([ModeDiscriminant::Commit]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "esc",
        key_matcher: press().code(KeyCode::Esc),
        modes: Vec::from([ModeDiscriminant::Commit]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: false,
    });
}

fn register_move_mode_key_binds(key_binds: &mut KeyBinds) {
    key_binds.register(StaticKeyBind {
        short_description: "move",
        chord_display: "enter",
        key_matcher: press().code(KeyCode::Enter),
        modes: Vec::from([ModeDiscriminant::Move]),
        message: Message::Move(MoveMessage::Confirm),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "above",
        chord_display: "a",
        key_matcher: press().code(KeyCode::Char('a')),
        modes: Vec::from([ModeDiscriminant::Move]),
        message: Message::Move(MoveMessage::SetInsertSide(InsertSide::Above)),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "below",
        chord_display: "b",
        key_matcher: press().code(KeyCode::Char('b')),
        modes: Vec::from([ModeDiscriminant::Move]),
        message: Message::Move(MoveMessage::SetInsertSide(InsertSide::Below)),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "m",
        key_matcher: press().code(KeyCode::Char('m')),
        modes: Vec::from([ModeDiscriminant::Move]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "esc",
        key_matcher: press().code(KeyCode::Esc),
        modes: Vec::from([ModeDiscriminant::Move]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: false,
    });
}

fn register_branch_mode_key_binds(key_binds: &mut KeyBinds) {
    key_binds.register(StaticKeyBind {
        short_description: "new",
        chord_display: "n",
        key_matcher: press().code(KeyCode::Char('n')),
        modes: Vec::from([ModeDiscriminant::Branch]),
        message: Message::Branch(BranchMessage::New),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "move",
        chord_display: "m",
        key_matcher: press().code(KeyCode::Char('m')),
        modes: Vec::from([ModeDiscriminant::Branch]),
        message: Message::Move(MoveMessage::Start),
        hide_from_hotbar: false,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "b",
        key_matcher: press().code(KeyCode::Char('b')),
        modes: Vec::from([ModeDiscriminant::Branch]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: true,
    });

    key_binds.register(StaticKeyBind {
        short_description: "back",
        chord_display: "esc",
        key_matcher: press().code(KeyCode::Esc),
        modes: Vec::from([ModeDiscriminant::Branch]),
        message: Message::EnterNormalMode,
        hide_from_hotbar: false,
    });
}

#[derive(Clone, Copy, Debug)]
struct KeyBindId(usize);

#[derive(Debug)]
pub(super) struct KeyBinds {
    /// All registered key binds.
    all_key_binds: Vec<Box<dyn KeyBind>>,
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

    fn register<T>(&mut self, key_bind: T) -> KeyBindId
    where
        T: KeyBind,
    {
        let id = KeyBindId(self.all_key_binds.len());

        for mode in ModeDiscriminant::iter() {
            if key_bind.available_in_mode(mode) {
                self.mode_to_key_binds.entry(mode).or_default().push(id);
            }
        }

        self.all_key_binds.push(Box::new(key_bind));

        id
    }

    pub(super) fn iter_key_binds_available_in_mode(
        &self,
        mode: &Mode,
    ) -> impl Iterator<Item = &dyn KeyBind> {
        let mode = ModeDiscriminant::from(mode);
        self.mode_to_key_binds
            .get(&mode)
            .into_iter()
            .flatten()
            .copied()
            .map(|KeyBindId(idx)| &*self.all_key_binds[idx])
    }
}

pub(super) trait KeyBind: std::fmt::Debug + 'static {
    fn short_description(&self) -> &'static str;

    fn chord_display(&self) -> &'static str;

    fn hide_from_hotbar(&self) -> bool {
        false
    }

    fn available_in_mode(&self, mode: ModeDiscriminant) -> bool;

    fn matches(&self, ev: &KeyEvent) -> bool;

    fn message(&self) -> Message;
}

#[derive(Debug)]
struct StaticKeyBind {
    short_description: &'static str,
    chord_display: &'static str,
    key_matcher: KeyMatcher,
    modes: Vec<ModeDiscriminant>,
    message: Message,
    hide_from_hotbar: bool,
}

impl KeyBind for StaticKeyBind {
    fn short_description(&self) -> &'static str {
        self.short_description
    }

    fn chord_display(&self) -> &'static str {
        self.chord_display
    }

    fn available_in_mode(&self, mode: ModeDiscriminant) -> bool {
        self.modes.contains(&mode)
    }

    fn matches(&self, ev: &KeyEvent) -> bool {
        self.key_matcher.matches(ev)
    }

    fn message(&self) -> Message {
        self.message.clone()
    }

    fn hide_from_hotbar(&self) -> bool {
        self.hide_from_hotbar
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
