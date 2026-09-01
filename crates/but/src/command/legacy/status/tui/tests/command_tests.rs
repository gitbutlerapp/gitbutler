use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use snapbox::{file, str};

use crate::command::legacy::status::tui::BackstackEntry;

use super::utils::test_status_tui;

#[test]
fn command_mode_runs_successful_command_and_returns_to_normal_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(':')
        .assert_rendered_term_svg_eq(file!["snapshots/command_mode_success_001.svg"]);

    tui.input("--help")
        .assert_rendered_term_svg_eq(file!["snapshots/command_mode_success_002.svg"]);

    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/command_mode_success_003.svg"]);
}

#[test]
fn leaving_command_mode_from_normal_preserves_marks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_contains("┊✔︎  kl   A one");

    tui.input(':')
        .assert_backstack_eq([BackstackEntry::LeaveCommandMode, BackstackEntry::Mark])
        .assert_rendered_contains("┊✔︎  kl   A one");

    tui.input(KeyCode::Esc)
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_contains("┊✔︎  kl   A one");
}

#[test]
fn command_parse_error_preserves_status_marks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("one", "content of one");
    env.file("two", "content of two");

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input(' ');
    tui.input(':');
    tui.input('\'');
    tui.input(KeyCode::Enter)
        .assert_rendered_contains("but '")
        .assert_rendered_contains("┊✔︎  kl   A one");
}

#[test]
fn command_parse_error_preserves_details_marks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('l');
    tui.input('j');
    tui.input(' ');
    tui.input('k');
    tui.input(':');
    tui.input('\'');
    tui.input(KeyCode::Enter)
        .assert_rendered_contains("but '")
        .assert_rendered_contains("✔︎");
}

#[test]
fn command_mode_keeps_input_when_command_exits_non_zero() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(':');

    tui.input("--definitely-not-a-real-flag");

    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/command_mode_failure_001.svg"])
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);
}

#[test]
fn dims_unselectable_lines_while_in_command_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    let mut tui = test_status_tui(env);

    tui.input(' ');

    tui.input(':').assert_rendered_term_svg_eq(file![
        "snapshots/dims_unselectable_lines_while_in_command_mode_001.svg"
    ]);
}
