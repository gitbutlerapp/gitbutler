use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::str;
use temp_env::with_var;

use crate::command::legacy::status::tui::tests::utils::test_tui;

const TEST_EDITOR_MESSAGE: &str = "commit from tui test";

#[test]
fn commit_mode_enter_and_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< commit >> g0 [A]"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn commit_confirm_on_source_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
}

#[test]
fn commit_mode_not_entered_from_non_commitable_rows() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);
}

#[test]
fn commit_from_unstaged_changes_creates_commit_visible_in_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< commit >> g0 [A]"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
}
