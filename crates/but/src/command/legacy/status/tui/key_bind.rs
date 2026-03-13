use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::command::legacy::status::tui::{Message, Mode, ModeDiscriminants};

/// The default set of key binds for the TUI.
pub(super) static KEY_BINDS: &[KeyBind] = &[
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Enter],
        },
        kind: KeyEventKind::Press,
        message: &Message::ConfirmRub,
        modes: KeyBindMode::Only(&[ModeDiscriminants::Rub]),
        short_description: "confirm",
        code_display: "enter",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Enter],
        },
        kind: KeyEventKind::Press,
        message: &Message::ConfirmInlineReword,
        modes: KeyBindMode::Only(&[ModeDiscriminants::InlineReword]),
        short_description: "confirm",
        code_display: "enter",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Char('j'), KeyCode::Down],
        },
        kind: KeyEventKind::Press,
        message: &Message::MoveCursorDown,
        modes: KeyBindMode::AllExceptInlineReword,
        short_description: "down",
        code_display: "↓/j",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Char('k'), KeyCode::Up],
        },
        kind: KeyEventKind::Press,
        message: &Message::MoveCursorUp,
        modes: KeyBindMode::AllExceptInlineReword,
        short_description: "up",
        code_display: "↑/k",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Char('r')],
        },
        kind: KeyEventKind::Press,
        message: &Message::StartRub,
        modes: KeyBindMode::Only(&[ModeDiscriminants::Normal]),
        short_description: "rub",
        code_display: "r",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Char('n')],
        },
        kind: KeyEventKind::Press,
        message: &Message::CreateEmptyCommit,
        modes: KeyBindMode::Only(&[ModeDiscriminants::Normal]),
        short_description: "new commit",
        code_display: "n",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Enter],
        },
        kind: KeyEventKind::Press,
        message: &Message::StartRewordInline,
        modes: KeyBindMode::Only(&[ModeDiscriminants::Normal]),
        short_description: "reword inline",
        code_display: "enter",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::SHIFT,
            keys: &[KeyCode::Enter],
        },
        kind: KeyEventKind::Press,
        message: &Message::RewordWithEditor,
        modes: KeyBindMode::Only(&[ModeDiscriminants::Normal]),
        short_description: "reword",
        code_display: "shift+enter",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Char('f')],
        },
        kind: KeyEventKind::Press,
        message: &Message::ToggleFiles,
        modes: KeyBindMode::AllExceptInlineReword,
        short_description: "files",
        code_display: "f",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Esc],
        },
        kind: KeyEventKind::Press,
        message: &Message::EnterNormalMode,
        modes: KeyBindMode::AllExceptNormal,
        short_description: "back",
        code_display: "esc",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::CONTROL,
            keys: &[KeyCode::Char('r')],
        },
        kind: KeyEventKind::Press,
        message: &Message::Reload(None),
        modes: KeyBindMode::Only(&[ModeDiscriminants::Normal]),
        short_description: "reload",
        code_display: "ctrl+r",
        hidden: false,
    },
    KeyBind {
        chord: KeyChord {
            modifiers: KeyModifiers::NONE,
            keys: &[KeyCode::Char('q')],
        },
        kind: KeyEventKind::Press,
        message: &Message::Quit,
        modes: KeyBindMode::AllExceptInlineReword,
        short_description: "quit",
        code_display: "q",
        hidden: false,
    },
];

/// A key binding for the TUI.
#[derive(Debug, Copy, Clone)]
pub(super) struct KeyBind {
    /// The chord required to pass.
    ///
    /// Either single keys or full modifier combos.
    pub(super) chord: KeyChord,
    /// Trigger on press or release?
    pub(super) kind: KeyEventKind,
    /// The message to send when the key binding is triggered.
    pub(super) message: &'static Message,
    /// The modes in which the key bind is available.
    pub(super) modes: KeyBindMode,
    /// The description of the key bind shown in the hotbar.
    pub(super) short_description: &'static str,
    /// The key code shown in the hotbar.
    // TODO: build this dynamically from `chord`
    pub(super) code_display: &'static str,
    /// Hidden key binds aren't shown in the hotbar.
    pub(super) hidden: bool,
}

impl KeyBind {
    pub(super) fn matches(self, ev: &KeyEvent, mode: &Mode) -> bool {
        if self.kind != ev.kind {
            return false;
        }

        if self.chord.modifiers != ev.modifiers {
            return false;
        }

        if !self.available_in_mode(mode) {
            return false;
        }

        if self
            .chord
            .keys
            .iter()
            .copied()
            .all(|key_code| key_code != ev.code)
        {
            return false;
        }

        true
    }

    pub(super) fn available_in_mode(self, mode: &Mode) -> bool {
        match self.modes {
            KeyBindMode::AllExceptInlineReword => !matches!(mode, Mode::InlineReword { .. }),
            KeyBindMode::Only(supported_modes) => supported_modes
                .iter()
                .copied()
                .any(|supported_mode| supported_mode == ModeDiscriminants::from(mode)),
            KeyBindMode::AllExceptNormal => !matches!(mode, Mode::Normal),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(super) struct KeyChord {
    pub(super) modifiers: KeyModifiers,
    pub(super) keys: &'static [KeyCode],
}

/// The modes a key binding is available in.
#[derive(Debug, Copy, Clone)]
pub(super) enum KeyBindMode {
    /// Available in all modes except inline reword.
    ///
    /// Inline reword is special since it shows a text area that eats all inputs.
    AllExceptInlineReword,
    /// Available in all modes except normal mode.
    AllExceptNormal,
    /// Only available in these modes.
    Only(&'static [ModeDiscriminants]),
}
