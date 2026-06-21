use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::str;

use crate::command::legacy::status::tui::tests::utils::test_tui;

#[test]
fn branch_key_from_unassigned_creates_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1] (no commits)"]);
}

#[test]
fn branch_key_from_commit_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   d3e2ba3 add B"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊●   d3e2ba3 add B"]);
}

#[test]
fn branch_key_from_branch_creates_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1] (no commits)"]);
}

#[test]
fn branch_key_keeps_global_file_list_open() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"])
        .assert_rendered_contains("94:tm A A");

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1] (no commits)"])
        .assert_rendered_contains("94:tm A A");
}

#[test]
fn focus_reload_preserves_branch_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.render_with_messages(Some(Event::FocusGained), Vec::new())
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn deleted_branch_name_can_be_reused_without_restoring_old_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('x')
        .assert_rendered_contains("Discard branch A?");

    tui.input_then_render('y');

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1] (no commits)"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1 ] (no commits)"]);

    for _ in 0..10 {
        tui.input_then_render(KeyCode::Backspace);
    }

    tui.input_then_render("A")
        .assert_current_line_eq(str!["┊╭┄br [A ] (no commits)"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A] (no commits)"]);

    let mut tui = tui.recreate();
    tui.reload().assert_rendered_contains("[A] (no commits)");
}

#[test]
fn focus_reload_preserves_merge_base_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    tui.render_with_messages(Some(Event::FocusGained), Vec::new())
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);
}

#[test]
fn inline_branch_reword_confirm_renames_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A ]"]);

    tui.input_then_render(KeyCode::Backspace)
        .assert_current_line_eq(str!["┊╭┄g0 [ ]"]);

    tui.input_then_render("new")
        .assert_current_line_eq(str!["┊╭┄g0 [new ]"]);

    // spaces get mapped to dashes
    tui.input_then_render(" ")
        .assert_current_line_eq(str!["┊╭┄g0 [new- ]"]);

    tui.input_then_render("name")
        .assert_current_line_eq(str!["┊╭┄g0 [new-name ]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ne [new-name]"]);
}

#[test]
fn inline_branch_reword_esc_cancels() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A ]"]);

    tui.input_then_render("new-name")
        .assert_current_line_eq(str!["┊╭┄g0 [Anew-name ]"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn inline_branch_reword_preserves_selection_after_reload_with_multiple_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A ]"]);

    tui.input_then_render(KeyCode::Backspace)
        .assert_current_line_eq(str!["┊╭┄g0 [ ]"]);

    tui.input_then_render("renamed-a")
        .assert_current_line_eq(str!["┊╭┄g0 [renamed-a ]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄re [renamed-a]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [B]"]);
}

#[test]
fn inline_branch_reword_space_before_close_bracket() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('j');

    // when the insertion point is at the end show a space before `]`
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A ]"]);

    // dont show a space when the cursor isn't at the end
    tui.input_then_render(KeyCode::Left)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}
