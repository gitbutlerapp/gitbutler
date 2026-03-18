use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::test_tui;

mod utils;

#[test]
fn basic_cursor_movement() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_eq(file!["snapshots/basic_cursor_movement_001.txt"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┴ 0dc3733 [origin/main] 2000-01-02 add M"]);

    tui.input_then_render([
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
    ])
    .assert_current_line_eq(str!["┴ 0dc3733 [origin/main] 2000-01-02 add M"]);

    tui.input_then_render([
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
    ])
    .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
}

#[test]
fn movement_aliases_j_k() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('j')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('j')
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('k')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('k')
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
}

#[test]
fn section_jumps_shift_j_k() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┴ 0dc3733 [origin/main] 2000-01-02 add M"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
}

#[test]
fn creating_empty_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_eq(file!["snapshots/creating_empty_commits_001.txt"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_rendered_eq(file!["snapshots/creating_empty_commits_002.txt"])
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_rendered_eq(file!["snapshots/creating_empty_commits_003.txt"])
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);
}

#[test]
fn inline_reword() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_eq(file!["snapshots/inline_reword_001.txt"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_rendered_eq(file!["snapshots/inline_reword_002.txt"])
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_eq(file!["snapshots/inline_reword_003.txt"]);

    tui.input_then_render("foo")
        .assert_rendered_eq(file!["snapshots/inline_reword_004.txt"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_eq(file!["snapshots/inline_reword_005.txt"])
        .assert_current_line_eq(str!["┊●   [..] foo (no changes)"]);
}

#[test]
fn esc_leaves_rub_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);
}

#[test]
fn rubbing() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_eq(file!["snapshots/rubbing_001.txt"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_rendered_eq(file!["snapshots/rubbing_002.txt"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str![
            "┊●   << amend commit >> [..] (no commit message) (no changes)"
        ]);

    tui.input_then_render(KeyCode::Enter)
        // that you end up on zz is a bug but requires moving the rub implementation to use but-api
        // that work is in progress
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('f')
        .assert_rendered_eq(file!["snapshots/rubbing_003.txt"]);
}
