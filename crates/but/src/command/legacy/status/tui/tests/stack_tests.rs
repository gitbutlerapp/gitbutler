use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::test_status_tui;

#[test]
fn unapply_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('b');
    tui.input('n');

    tui.input((KeyModifiers::SHIFT, 'G'));
    tui.input('b');
    tui.input('n');

    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_001.svg"]);

    tui.input('s')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_002.svg"]);

    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_003.svg"]);

    tui.input('k');
    tui.input('u')
        .assert_current_line_eq(str!["┊├┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_004.svg"]);
}

#[test]
fn unapply_stack_selects_base_branch_when_next_stack_has_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('b');
    tui.input('n');
    tui.input('n');

    tui.input((KeyModifiers::SHIFT, 'G'));
    tui.input('b');
    tui.input('n');

    tui.input('g');
    tui.input('s');
    tui.input('u').assert_current_line_eq(str!["┊├┄ g0 [A]"]);
}

#[test]
fn enter_stack_mode_from_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('s')
        .assert_rendered_term_svg_eq(file!["snapshots/enter_stack_mode_from_commits_001.svg"]);
}

#[test]
fn moving_stacks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui(env);

    for name in ["one", "two", "three"] {
        tui.input('g');
        tui.input('b');
        tui.input('n');
        tui.input(KeyCode::Enter);
        for _ in 0..100 {
            tui.input(KeyCode::Backspace);
        }
        tui.input(name);
        tui.input(KeyCode::Enter);
        tui.input('g');
    }

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_001.svg"]);

    tui.input('j');
    tui.input('s');
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_002.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_003.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_004.svg"]);

    tui.input('s');
    tui.input('m');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_005.svg"]);

    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_006.svg"]);
}

#[test]
fn applying_stacks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui(env);

    for name in ["one", "two"] {
        tui.input('g');
        tui.input('b');
        tui.input('n');
        tui.input(KeyCode::Enter);
        for _ in 0..100 {
            tui.input(KeyCode::Backspace);
        }
        tui.input(name);
        tui.input(KeyCode::Enter);
        tui.input('g');
    }

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_001.svg"]);

    for _ in 0..2 {
        tui.input('s');
        tui.input('u');
        tui.input('g');
    }

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_002.svg"]);

    tui.input('s');
    tui.input('a')
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_003.svg"]);
    tui.input("two")
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_004.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/applying_stacks_005.svg"]);
}

#[test]
fn escape_moves_cursor_back_to_valid_position() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j').assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input('s');
    tui.input('m');
    tui.input('j');

    // cancelling should put the cursor at a valid position
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str![["┴ 0dc3733 (common base) 2000-01-02 add M"]]);
}

#[test]
fn maintains_cursor_position_if_on_source() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j').assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input('s');
    tui.input('m');

    // cancelling should put the cursor at a valid position
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
}
