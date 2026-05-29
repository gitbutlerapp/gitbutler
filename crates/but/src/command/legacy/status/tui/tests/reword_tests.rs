use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};

use crate::command::legacy::status::tui::tests::utils::{TestTui, test_tui, test_tui_with_size};

fn reword_selected_commit_to_multiline(tui: &mut TestTui) {
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("first body line");
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("second body line");
    tui.input_then_render((KeyModifiers::CONTROL, KeyCode::Char('s')));
}

fn reword_selected_commit_to_many_lines(tui: &mut TestTui) {
    tui.input_then_render(KeyCode::Enter);
    for idx in 1..=8 {
        tui.input_then_render(KeyCode::Enter);
        tui.input_then_render(format!("body line {idx}").as_str());
    }
    tui.input_then_render((KeyModifiers::CONTROL, KeyCode::Char('s')));
}

#[test]
fn commit_inline_reword_enter_inserts_newline_and_ctrl_s_saves() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(snapbox::str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_contains("ctrl+s confirm")
        .assert_current_line_eq(snapbox::str!["┊●   [..] add A"]);

    tui.input_then_render([
        KeyCode::Enter,
        KeyCode::Char('b'),
        KeyCode::Char('o'),
        KeyCode::Char('d'),
        KeyCode::Char('y'),
    ])
    .assert_rendered_contains("body");

    tui.input_then_render((KeyModifiers::CONTROL, KeyCode::Char('s')))
        .assert_current_line_eq(snapbox::str!["┊●   [..] add A"]);

    let messages = tui.env().invoke_git("log --all --format=%B");
    assert!(
        messages.contains("add A\nbody"),
        "inline reword should save the full multi-line commit message, got {messages:?}"
    );
}

#[test]
fn branch_inline_reword_enter_still_confirms() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(snapbox::str!["┊╭┄g0 [A]"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_contains("ctrl+s confirm");
    tui.input_then_render(KeyCode::Backspace);
    tui.input_then_render("renamed");
    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_contains("[renamed]");
}

#[test]
fn existing_multiline_commit_message_opens_inline_and_pushes_rows_down_with_cursor_at_end() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down]);
    reword_selected_commit_to_multiline(&mut tui);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_contains("add A")
        .assert_rendered_contains("first body line")
        .assert_rendered_contains("second body line")
        .assert_rendered_contains("┊│           first body line")
        .assert_rendered_contains("┊│           second body line");

    tui.input_then_render(" appended");
    tui.input_then_render((KeyModifiers::CONTROL, KeyCode::Char('s')));
    let messages = tui.env().invoke_git("log --all --format=%B");
    assert!(
        messages.contains("second body line appended"),
        "typing after entering reword should append at the end of the full message, got {messages:?}"
    );
}

#[test]
fn multiline_inline_reword_renders_when_top_is_scrolled_offscreen() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 3);

    tui.input_then_render([KeyCode::Down, KeyCode::Down]);
    reword_selected_commit_to_multiline(&mut tui);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_contains("second body line")
        .assert_rendered_contains("┊│           second body line");
}

#[test]
fn long_inline_reword_scrolls_textarea_to_cursor_when_message_exceeds_viewport() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 5);

    tui.input_then_render([KeyCode::Down, KeyCode::Down]);
    reword_selected_commit_to_many_lines(&mut tui);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_contains("body line 8");

    tui.input_then_render(" appended");
    tui.input_then_render((KeyModifiers::CONTROL, KeyCode::Char('s')));
    let messages = tui.env().invoke_git("log --all --format=%B");
    assert!(
        messages.contains("body line 8 appended"),
        "typing after opening a long message should append at the visible cursor, got {messages:?}"
    );
}

#[test]
fn multiline_inline_reword_clips_when_bottom_is_offscreen() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 4);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Enter])
        .assert_current_line_eq(snapbox::str!["┊●   [..] add A"]);
    tui.input_then_render([
        KeyCode::Enter,
        KeyCode::Char('b'),
        KeyCode::Char('o'),
        KeyCode::Char('d'),
        KeyCode::Char('y'),
    ])
    .assert_rendered_contains("body");
}
