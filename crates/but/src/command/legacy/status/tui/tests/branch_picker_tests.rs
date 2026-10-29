use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::test_status_tui;

#[test]
fn opens_branch_picker_popup_layout() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('t')
        .assert_rendered_term_svg_eq(file!["snapshots/opens_branch_picker_popup_layout_001.svg"])
        .assert_rendered_contains("> ")
        .assert_rendered_contains("A")
        .assert_rendered_contains("B");
}

#[test]
fn branch_picker_filters_and_highlights_matches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('t');

    tui.input("B")
        .assert_rendered_term_svg_eq(file![
            "snapshots/branch_picker_filters_and_highlights_matches_001.svg"
        ])
        .assert_rendered_contains("> B")
        .assert_rendered_contains("B");
}

#[test]
fn branch_picker_cursor_movement_updates_selected_row() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('t');

    tui.input(KeyCode::Down).assert_rendered_term_svg_eq(file![
        "snapshots/branch_picker_cursor_movement_updates_selected_row_001.svg"
    ]);
}

#[test]
fn esc_closes_branch_picker_without_changing_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input('t').assert_rendered_contains("> ");

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);
}

#[test]
fn confirm_selects_highlighted_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('t');

    tui.input(KeyCode::Down);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
}

#[test]
fn confirm_with_no_matches_keeps_picker_open() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('t');

    tui.input("zzz-not-found")
        .assert_rendered_contains("> zzz-not-found");

    tui.input(KeyCode::Enter)
        .assert_rendered_contains("> zzz-not-found");
}

#[test]
fn pick_and_goto_noop_when_commit_file_list_open() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('f')
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input('t')
        .assert_rendered_term_svg_eq(file![
            "snapshots/pick_and_goto_noop_when_commit_file_list_open_001.svg"
        ])
        .assert_current_line_eq(str!["┊│     t:t A A"]);
}

#[test]
fn goto_uncommitted_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('t');

    tui.input("uncommitted");

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);
}
