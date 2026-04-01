use std::sync::Arc;

use anyhow::anyhow;
use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};
use temp_env::with_var;

use crate::command::legacy::status::tui::Message;
use crate::command::legacy::status::tui::tests::utils::{test_tui, test_tui_with_size};

mod branch_tests;
mod command_tests;
mod commit_tests;
mod details_tests;
mod move_tests;
mod rub_tests;
mod utils;

fn assert_cursor_context_rows(
    tui: &utils::TestTui,
    visible_height: usize,
    preferred_context: usize,
) {
    let selected_rows = tui
        .app
        .selected_row_range()
        .expect("selected row should be in bounds");
    let selected_height = selected_rows.end.saturating_sub(selected_rows.start);
    let effective_context =
        preferred_context.min(visible_height.saturating_sub(selected_height) / 2);

    let total_rows = tui.app.total_rendered_height();
    let available_above = selected_rows.start;
    let available_below = total_rows.saturating_sub(selected_rows.end);

    let required_above = effective_context.min(available_above);
    let required_below = effective_context.min(available_below);

    let actual_above = selected_rows.start.saturating_sub(tui.app.scroll_top);
    let viewport_bottom = tui.app.scroll_top.saturating_add(visible_height);
    let actual_below = viewport_bottom.saturating_sub(selected_rows.end);

    assert!(
        actual_above >= required_above,
        "expected at least {required_above} rows above cursor, got {actual_above}"
    );
    assert!(
        actual_below >= required_below,
        "expected at least {required_below} rows below cursor, got {actual_below}"
    );
}

#[test]
fn shows_full_error_when_message_wraps() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.render_with_messages(
        None,
        Vec::from([
            Message::Reload(None),
            Message::ShowError(Arc::new(anyhow!(
                "error-with-end-marker: this is a deliberately long error message that should wrap over multiple lines without clipping and it must include END-MARKER"
            ))),
        ]),
    )
    .assert_rendered_term_svg_eq(file!["snapshots/shows_full_error_when_message_wraps_001.svg"]);
}

#[test]
fn shows_full_error_cause_chain_with_multiple_contexts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    let err = anyhow!("root-cause-END-MARKER")
        .context("context-level-1")
        .context("context-level-2")
        .context("context-level-3");

    tui.render_with_messages(
        None,
        Vec::from([Message::Reload(None), Message::ShowError(Arc::new(err))]),
    )
    .assert_rendered_term_svg_eq(file![
        "snapshots/shows_full_error_cause_chain_with_multiple_contexts_001.svg"
    ]);
}

#[test]
fn format_error_for_tui_shows_cause_chain_without_backtrace() {
    let err = anyhow!("root-cause")
        .context("context-level-1")
        .context("context-level-2");

    let rendered = super::format_error_for_tui(&err);

    assert_eq!(
        rendered,
        "context-level-2\n\nCaused by:\n    0: context-level-1\n    1: root-cause"
    );
    assert!(!rendered.contains("Stack backtrace"));
}

#[test]
fn format_error_for_tui_shows_single_message_for_leaf_error() {
    let err = anyhow!("leaf-error");

    let rendered = super::format_error_for_tui(&err);

    assert_eq!(rendered, "leaf-error");
    assert!(!rendered.contains("Caused by:"));
}

#[test]
fn basic_cursor_movement() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_term_svg_eq(file!["snapshots/basic_cursor_movement_001.svg"])
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
fn shift_k_from_commit_moves_to_current_section_header_first() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
}

#[test]
fn shift_k_from_second_stack_commit_moves_to_its_header() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   [..]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);
}

#[test]
fn cursor_movement_scrolls_viewport_down() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 8);

    tui.input_then_render(None)
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_down_001.svg"
        ])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_down_002.svg"
        ])
        .assert_current_line_eq(str!["┊●   [..] add B"]);
}

#[test]
fn cursor_movement_scrolls_viewport_up() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 8);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_up_001.svg"
        ])
        .assert_current_line_eq(str!["┊●   [..] add B"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_up_002.svg"
        ])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
}

#[test]
fn scrolling_keeps_three_rows_of_context_when_possible() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 8);
    let visible_height = 7;

    tui.input_then_render(None);
    assert_cursor_context_rows(&tui, visible_height, 3);

    for _ in 0..10 {
        tui.input_then_render(KeyCode::Down);
        assert_cursor_context_rows(&tui, visible_height, 3);
    }

    for _ in 0..10 {
        tui.input_then_render(KeyCode::Up);
        assert_cursor_context_rows(&tui, visible_height, 3);
    }
}

#[test]
fn section_jumps_scroll_viewport_when_target_is_offscreen() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 8);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/section_jumps_scroll_viewport_when_target_is_offscreen_001.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/section_jumps_scroll_viewport_when_target_is_offscreen_002.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/section_jumps_scroll_viewport_when_target_is_offscreen_003.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn reload_preserves_visible_selection_when_scrolled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 8);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down]);

    tui.render_with_messages(None, Vec::from([Message::Reload(None)]))
        .assert_rendered_term_svg_eq(file![
            "snapshots/reload_preserves_visible_selection_when_scrolled_001.svg"
        ])
        .assert_current_line_eq(str!["┊●   [..] add B"]);
}

#[test]
fn inline_reword_renders_on_visible_row_when_scrolled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 8);

    tui.input_then_render([
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Enter,
    ])
    .assert_rendered_term_svg_eq(file![
        "snapshots/inline_reword_renders_on_visible_row_when_scrolled_001.svg"
    ])
    .assert_current_line_eq(str!["┊●   [..] add B"]);
}

#[test]
fn creating_empty_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_002.svg"])
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_003.svg"])
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);
}

#[test]
fn inline_reword() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_002.svg"])
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_003.svg"]);

    tui.input_then_render("foo")
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_004.svg"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_005.svg"])
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
fn mode_toggle_key_r_enters_and_leaves_rub_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_r_enters_and_leaves_rub_mode_001.svg"
        ])
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   vo A test.txt"]);
}

#[test]
fn mode_toggle_key_c_enters_and_leaves_commit_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render('c')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_c_enters_and_leaves_commit_mode_001.svg"
        ])
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
}

#[test]
fn mode_toggle_key_m_enters_and_leaves_move_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('m')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_m_enters_and_leaves_move_mode_001.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn mode_toggle_key_b_enters_and_leaves_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('b')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_b_enters_and_leaves_branch_mode_001.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄<< target >> g0 [A]"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn rubbing() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_002.svg"])
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

    tui.input_then_render(KeyCode::Enter);
    // that you end up on zz is a bug but requires moving the rub implementation to use but-api
    // that work is in progress
    tui.input_then_render([
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
    ])
    .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_003.svg"]);
}

#[test]
fn global_file_list_does_not_restrict_cursor() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/global_file_list_does_not_restrict_cursor_final.svg"
        ]);
}

#[test]
fn commit_file_list_scopes_cursor_to_files_in_selected_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊│     [..] A A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_list_scopes_cursor_to_files_in_selected_commit_final.svg"
        ]);
}

#[test]
fn commit_file_toggle_on_commit_without_files_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 8);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    with_var("GIT_AUTHOR_DATE", Some("2000-01-01T00:00:00Z"), || {
        with_var("GIT_COMMITTER_DATE", Some("2000-01-01T00:00:00Z"), || {
            tui.input_then_render('n')
                .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);
        });
    });

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┴ 0dc3733 [origin/main] 2000-01-02 add M"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_toggle_on_commit_without_files_is_noop_final.svg"
        ]);
}

#[test]
fn commit_file_list_rub_can_escape_scope_and_esc_reenters_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊│     << source >> << noop >> [..] A A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊●   << move file >> [..] add A"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊│     [..] A A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_list_rub_can_escape_scope_and_esc_reenters_file_list_final.svg"
        ]);
}

#[test]
fn confirm_rub_closes_commit_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊│     << source >> << noop >> [..] A A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);
}

#[test]
fn esc_in_normal_mode_closes_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊●   [..] add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/esc_in_normal_mode_closes_global_file_list_final.svg"
        ]);
}

#[test]
fn esc_in_normal_mode_closes_commit_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊●   [..] add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/esc_in_normal_mode_closes_commit_file_list_final.svg"
        ]);
}

#[test]
fn commit_file_toggle_off_from_commit_row_preserves_commit_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊●   [..] add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_toggle_off_from_commit_row_preserves_commit_selection_final.svg"
        ]);
}
