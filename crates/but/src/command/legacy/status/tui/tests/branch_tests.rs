use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::test_tui;

#[test]
fn branch_mode_from_unassigned_jumps_to_first_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/branch_mode_from_unassigned_jumps_to_first_branch_final.svg"
        ]);
}

#[test]
fn branch_mode_from_commit_jumps_to_nearest_preceding_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add B"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄<< target >> h0 [B]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/branch_mode_from_commit_jumps_to_nearest_preceding_branch_final.svg"
        ]);
}

#[test]
fn esc_leaves_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/esc_leaves_branch_mode_final.svg"]);
}

#[test]
fn branch_mode_down_moves_from_branch_to_merge_base_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┴ << target >> [..] [origin/main] 2000-01-02 add M"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/branch_mode_down_moves_from_branch_to_merge_base_target_final.svg"
        ]);
}

#[test]
fn entering_branch_mode_closes_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/entering_branch_mode_closes_global_file_list_final.svg"
        ]);
}

#[test]
fn new_branch_from_merge_base_in_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┴ << target >> [..] [origin/main] 2000-01-02 add M"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1] (no commits)"]);

    tui.input_then_render(None)
        .assert_rendered_term_svg_eq(file![
            "snapshots/new_branch_from_merge_base_in_branch_mode_final.svg"
        ]);
}

#[test]
fn focus_reload_in_branch_mode_preserves_branch_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"]);

    tui.render_with_messages(Some(Event::FocusGained), Vec::new())
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/focus_reload_in_branch_mode_preserves_branch_selection_final.svg"
        ]);
}

#[test]
fn focus_reload_in_branch_mode_preserves_merge_base_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┴ << target >> [..] [origin/main] 2000-01-02 add M"]);

    tui.render_with_messages(Some(Event::FocusGained), Vec::new())
        .assert_current_line_eq(str!["┴ << target >> [..] [origin/main] 2000-01-02 add M"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/focus_reload_in_branch_mode_preserves_merge_base_selection_final.svg"
        ]);
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
        .assert_current_line_eq(str!["[..] [new-name]"]);
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
        .assert_current_line_eq(str!["[..] [renamed-a]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["[..] [B]"]);
}
