use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use snapbox::{file, str};

use super::utils::test_tui;

#[test]
fn command_mode_runs_successful_command_and_returns_to_normal_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(':')
        .assert_rendered_eq(file!["snapshots/command_mode_success_001.txt"]);

    tui.input_then_render("--help")
        .assert_rendered_eq(file!["snapshots/command_mode_success_002.txt"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_eq(file!["snapshots/command_mode_success_003.txt"]);
}

#[test]
fn command_mode_keeps_input_when_command_exits_non_zero() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(':');

    tui.input_then_render("--definitely-not-a-real-flag");

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_eq(file!["snapshots/command_mode_failure_001.txt"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
}
