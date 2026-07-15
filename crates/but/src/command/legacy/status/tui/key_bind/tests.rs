use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::legacy::status::tui::key_bind::press;
use crate::command::legacy::status::tui::{
    CommitMessage, Message, RubMessage, mode::ModeDiscriminant,
};

#[test]
fn chord_display_for_plain_and_modified_keys() {
    assert_eq!(press().code(KeyCode::Char('q')).chord_display(), "q");
    assert_eq!(press().code(KeyCode::Enter).chord_display(), "enter");
    assert_eq!(press().code(KeyCode::Esc).chord_display(), "esc");
    assert_eq!(
        press().control().code(KeyCode::Char('r')).chord_display(),
        "ctrl+r"
    );
    assert_eq!(
        press().control().code(KeyCode::Char('[')).chord_display(),
        "ctrl+["
    );
    assert_eq!(
        press().alt().code(KeyCode::Char('e')).chord_display(),
        "alt+e"
    );
    assert_eq!(
        press().shift().code(KeyCode::Char('J')).chord_display(),
        "shift+j"
    );
}

#[test]
fn chord_display_for_alternate_codes() {
    assert_eq!(
        press()
            .code(KeyCode::Char('j'))
            .alt_code(KeyCode::Down)
            .chord_display(),
        "↓/j"
    );
    assert_eq!(
        press()
            .code(KeyCode::Char('k'))
            .alt_code(KeyCode::Up)
            .chord_display(),
        "↑/k"
    );
    assert_eq!(
        press()
            .code(KeyCode::Char('h'))
            .alt_code(KeyCode::Left)
            .chord_display(),
        "←/h"
    );
    assert_eq!(
        press()
            .code(KeyCode::Char('l'))
            .alt_code(KeyCode::Right)
            .chord_display(),
        "→/l"
    );
    assert_eq!(
        press()
            .code(KeyCode::Char('n'))
            .alt_code(KeyCode::Esc)
            .chord_display(),
        "esc/n"
    );
    assert_eq!(
        press()
            .shift()
            .code(KeyCode::Char('J'))
            .alt_code(KeyCode::Down)
            .chord_display(),
        "shift+↓/j"
    );
    assert_eq!(
        press()
            .control()
            .code(KeyCode::Char('j'))
            .alt_code(KeyCode::Down)
            .chord_display(),
        "ctrl+↓/j"
    );
    assert_eq!(
        press()
            .alt()
            .code(KeyCode::Char('n'))
            .alt_code(KeyCode::Esc)
            .chord_display(),
        "alt+esc/n"
    );
}

#[test]
fn matcher_still_matches_primary_and_alternate_codes() {
    let matcher = press().code(KeyCode::Char('j')).alt_code(KeyCode::Down);

    assert!(matcher.matches(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)));
    assert!(matcher.matches(&KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    assert!(!matcher.matches(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SHIFT)));
}

#[test]
fn rub_is_available_while_details_are_focused() {
    let bindings = super::default_key_binds();
    let event = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    assert!(
        bindings
            .iter_key_binds_available_in_mode(ModeDiscriminant::Details)
            .any(|binding| binding.matches(&event)
                && matches!(binding.message(), Message::Rub(RubMessage::Start)))
    );
}

#[test]
fn commit_is_available_while_details_are_focused() {
    let bindings = super::default_key_binds();
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
    assert!(
        bindings
            .iter_key_binds_available_in_mode(ModeDiscriminant::Details)
            .any(|binding| binding.matches(&event)
                && matches!(binding.message(), Message::Commit(CommitMessage::Start)))
    );
}
