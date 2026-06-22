use std::sync::Arc;

use anyhow::anyhow;
use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};
use temp_env::with_var;

use crate::CliId;
use crate::command::legacy::status::tui::tests::utils::{
    TestTuiOptions, test_tui, test_tui_with_options,
};
use crate::command::legacy::status::tui::{BackstackEntry, Message, ReloadCause};
use crate::command::legacy::status::{TuiOutcome, TuiRunOptions};

mod branch_picker_tests;
mod branch_tests;
mod command_tests;
mod commit_tests;
mod details_tests;
mod discard_tests;
mod marking_tests;
mod move_tests;
mod rub_tests;
mod stack_tests;
mod utils;

fn assert_cursor_context_rows(
    tui: &utils::TestTui,
    visible_height: usize,
    preferred_context: usize,
) {
    let selected_rows =
        super::render::selected_row_range(&tui.app).expect("selected row should be in bounds");
    let selected_height = selected_rows.end.saturating_sub(selected_rows.start);
    let effective_context =
        preferred_context.min(visible_height.saturating_sub(selected_height) / 2);

    let total_rows = super::render::total_rendered_height(&tui.app);
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.render_with_messages(
        None,
        Vec::from([
            Message::Reload(None, ReloadCause::Mutation),
            Message::ShowError(Arc::new(anyhow!(
                "error-with-end-marker: this is a deliberately long error message that should wrap over multiple lines without clipping and it must include END-MARKER"
            ))),
        ]),
    )
    .assert_rendered_term_svg_eq(file!["snapshots/shows_full_error_when_message_wraps_001.svg"]);
}

#[test]
fn shows_full_error_cause_chain_with_multiple_contexts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    let err = anyhow!("root-cause-END-MARKER")
        .context("context-level-1")
        .context("context-level-2")
        .context("context-level-3");

    tui.render_with_messages(
        None,
        Vec::from([
            Message::Reload(None, ReloadCause::Mutation),
            Message::ShowError(Arc::new(err)),
        ]),
    )
    .assert_rendered_term_svg_eq(file![
        "snapshots/shows_full_error_cause_chain_with_multiple_contexts_001.svg"
    ]);
}

#[test]
fn narrow_hotbar_prioritizes_help_and_quit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 42,
            height: 20,
            ..Default::default()
        },
    );

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/narrow_hotbar_prioritizes_help_and_quit.svg"
    ]);
}

#[test]
fn narrow_hotbar_keeps_help_and_quit_visible_in_modal_modes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 36,
            height: 20,
            ..Default::default()
        },
    );

    tui.input_then_render('r')
        .assert_rendered_term_svg_eq(file![
            "snapshots/narrow_hotbar_keeps_help_and_quit_visible_in_modal_modes.svg"
        ]);
}

#[test]
fn help_popup_opens_over_status_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input_then_render('?')
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_opens_over_status_view_001.svg"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_opens_over_status_view_002.svg"]);
}

#[test]
fn help_popup_scrolls() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 10,
            ..Default::default()
        },
    );

    tui.input_then_render('?')
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_scrolls_001.svg"]);

    tui.input_then_render((KeyModifiers::CONTROL, 'd'))
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_scrolls_002.svg"]);

    tui.input_then_render((KeyModifiers::CONTROL, 'u'))
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_scrolls_003.svg"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_scrolls_004.svg"]);
}

#[test]
fn undo_opens_confirm_for_latest_snapshot() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");
    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    tui.input_then_render('i');
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("commit for undo prompt test");
    tui.input_then_render(KeyCode::Enter);

    tui.input_then_render('u')
        .assert_rendered_term_svg_eq(file![
            "snapshots/undo_opens_confirm_for_latest_snapshot_001.svg"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file![
            "snapshots/undo_opens_confirm_for_latest_snapshot_002.svg"
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/basic_cursor_movement_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    tui.input_then_render([
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
    ])
    .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    tui.input_then_render([
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
    ])
    .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);
}

#[test]
fn movement_aliases_j_k() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render('j')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('j')
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('k')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('k')
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);
}

#[test]
fn section_jumps_shift_j_k() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);
}

#[test]
fn shift_k_from_commit_moves_to_current_section_header_first() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);
}

#[test]
fn shift_k_from_second_stack_commit_moves_to_its_header() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   d3e2ba3 add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);
}

#[test]
fn cursor_movement_scrolls_viewport_down() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 8,
            ..Default::default()
        },
    );

    tui.reload()
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_down_001.svg"
        ])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_down_002.svg"
        ])
        .assert_current_line_eq(str!["┊●   d3e2ba3 add B"]);
}

#[test]
fn cursor_movement_scrolls_viewport_up() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 8,
            ..Default::default()
        },
    );

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_up_001.svg"
        ])
        .assert_current_line_eq(str!["┊●   d3e2ba3 add B"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_up_002.svg"
        ])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);
}

#[test]
fn scrolling_keeps_three_rows_of_context_when_possible() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 8,
            ..Default::default()
        },
    );
    let visible_height = 6;

    tui.reload();
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 8,
            ..Default::default()
        },
    );

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/section_jumps_scroll_viewport_when_target_is_offscreen_001.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/section_jumps_scroll_viewport_when_target_is_offscreen_002.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'K'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/section_jumps_scroll_viewport_when_target_is_offscreen_003.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn moving_to_merge_base_scrolls_to_keep_selection_visible() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 8,
            ..Default::default()
        },
    );

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);
}

#[test]
fn reload_preserves_visible_selection_when_scrolled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 8,
            ..Default::default()
        },
    );

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down]);

    tui.render_with_messages(
        None,
        Vec::from([Message::Reload(None, ReloadCause::Mutation)]),
    )
    .assert_rendered_term_svg_eq(file![
        "snapshots/reload_preserves_visible_selection_when_scrolled_001.svg"
    ])
    .assert_current_line_eq(str!["┊●   d3e2ba3 add B"]);
}

#[test]
fn inline_reword_renders_on_visible_row_when_scrolled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 8,
            ..Default::default()
        },
    );

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
    .assert_current_line_eq(str!["┊●   d3e2ba3 add B"]);
}

#[test]
fn creating_empty_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_002.svg"])
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_003.svg"])
        .assert_current_line_eq(str!["┊●   9638f28 (no commit message) (no changes)"]);
}

#[test]
fn inline_reword() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_002.svg"])
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_003.svg"]);

    tui.input_then_render("foo")
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_004.svg"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_005.svg"])
        .assert_current_line_eq(str!["┊●   cb96911 foo (no changes)"]);
}

#[test]
fn inline_reword_open_editor_keeps_inline_message_when_editor_makes_no_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(".git/editor.sh", "exit 0\n");
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render(" updated")
        .assert_rendered_contains("add A updated");

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render((KeyModifiers::ALT, 'e'))
            .assert_current_line_eq(str!["┊●   711ccd7 add A updated"]);
    });
}

#[test]
fn esc_leaves_rub_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);
}

#[test]
fn mode_key_r_enters_and_escape_leaves_rub_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_r_enters_and_leaves_rub_mode_001.svg"
        ])
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);
}

#[test]
fn rub_mode_shift_j_lands_on_first_selectable_in_next_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);
}

#[test]
fn rub_mode_shift_j_can_jump_between_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> d3e2ba3 add B"]);
}

#[test]
fn rub_mode_shift_k_jumps_to_first_selectable_in_previous_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> d3e2ba3 add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);
}

#[test]
fn mode_key_c_enters_and_escape_leaves_commit_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input_then_render('c')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_c_enters_and_leaves_commit_mode_001.svg"
        ])
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);
}

#[test]
fn mode_key_m_enters_and_escape_leaves_move_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('m')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_m_enters_and_leaves_move_mode_001.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn key_b_creates_new_branch_from_selected_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1] (no commits)"]);
}

#[test]
fn rubbing() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_002.svg"])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str![
            "┊●   << amend >> f184fc7 (no commit message) (no changes)"
        ]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

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
    .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'F'))
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_003.svg"]);
}

#[test]
fn global_file_list_does_not_restrict_cursor() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/global_file_list_does_not_restrict_cursor_final.svg"
        ]);
}

#[test]
fn commit_file_list_scopes_cursor_to_files_in_selected_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊│     94:tm A A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_list_scopes_cursor_to_files_in_selected_commit_final.svg"
        ]);
}

#[test]
fn commit_file_toggle_on_commit_without_files_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 12,
            ..Default::default()
        },
    );

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    with_var("GIT_AUTHOR_DATE", Some("2000-01-01T00:00:00Z"), || {
        with_var("GIT_COMMITTER_DATE", Some("2000-01-01T00:00:00Z"), || {
            tui.input_then_render('n')
                .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);
        });
    });

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_toggle_on_commit_without_files_is_noop_final.svg"
        ]);
}

#[test]
fn commit_file_list_rub_esc_leaves_rub_and_closes_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'R'))
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊●   9477ae7 add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_list_rub_esc_leaves_rub_and_closes_file_list_final.svg"
        ]);
}

#[test]
fn confirm_rub_keeps_commit_file_list_open() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'R'))
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     94:tm A A"]);
}

#[test]
fn esc_in_normal_mode_closes_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊●   9477ae7 add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/esc_in_normal_mode_closes_global_file_list_final.svg"
        ]);
}

#[test]
fn esc_in_normal_mode_closes_commit_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊●   9477ae7 add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/esc_in_normal_mode_closes_commit_file_list_final.svg"
        ]);
}

#[test]
fn commit_file_toggle_off_from_commit_row_preserves_commit_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('f')
        .assert_current_line_eq(str!["┊●   9477ae7 add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_toggle_off_from_commit_row_preserves_commit_selection_final.svg"
        ]);
}

#[test]
fn pick_changes_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            run_options: TuiRunOptions::PickChanges,
            ..Default::default()
        },
    );

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/pick_changes_mode_001.svg"]);

    tui.input_then_render('j');
    tui.input_then_render(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/pick_changes_mode_002.svg"]);
    let outcome = tui
        .input_then_render(KeyCode::Enter)
        .take_outcome()
        .unwrap();

    let cli_ids = match outcome {
        TuiOutcome::CliIds(cli_ids) => cli_ids,
        _ => panic!("unexpected outcome {outcome:#?}"),
    };

    for id in &cli_ids {
        assert!(matches!(dbg!(id), CliId::Uncommitted(..)));
    }
    assert_eq!(cli_ids.len(), 1);
}

#[test]
fn stays_in_pick_change_mode_after_full_screen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            run_options: TuiRunOptions::PickChanges,
            ..Default::default()
        },
    );

    tui.reload()
        .assert_rendered_term_svg_eq(file![
            "snapshots/stays_in_pick_change_mode_after_full_screen_details_001.svg"
        ])
        .assert_backstack_eq([]);

    // mark some changes
    tui.input_then_render('j');
    tui.input_then_render(' ')
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/stays_in_pick_change_mode_after_full_screen_details_002.svg"
        ]);

    tui.input_then_render((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/stays_in_pick_change_mode_after_full_screen_details_003.svg"
        ])
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenFullScreenDetailsView,
            BackstackEntry::Mark,
        ]);

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/stays_in_pick_change_mode_after_full_screen_details_004.svg"
        ])
        .assert_backstack_eq([BackstackEntry::Mark]);

    // ensure the changes are still marked after returning from details mode
    let outcome = tui
        .input_then_render(KeyCode::Enter)
        .take_outcome()
        .unwrap();

    let cli_ids = match outcome {
        TuiOutcome::CliIds(cli_ids) => cli_ids,
        _ => panic!("unexpected outcome {outcome:#?}"),
    };

    for id in &cli_ids {
        assert!(matches!(dbg!(id), CliId::Uncommitted(..)));
    }
    assert_eq!(cli_ids.len(), 1);
}

#[test]
fn pick_changes_mode_supports_focusing_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            run_options: TuiRunOptions::PickChanges,
            ..Default::default()
        },
    );

    tui.input_then_render('l')
        .assert_rendered_term_svg_eq(file![
            "snapshots/pick_changes_mode_supports_focusing_details_view_001.svg"
        ]);
}

#[test]
fn consistent_commit_shas_in_tests() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_tui(env);

    tui.input_then_render('b');
    tui.input_then_render('n')
        .assert_current_line_eq("┊●   0b42c46 (no commit message) (no changes)");
}

#[test]
fn jumping_up_down() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input_then_render('j');
    for n in 1..=30 {
        tui.input_then_render('n');
        tui.input_then_render(KeyCode::Enter);
        tui.input_then_render(format!("commit #{n}"));
        tui.input_then_render(KeyCode::Enter);
    }

    tui.reload()
        .assert_current_line_eq("┊●   1a89cbc commit #30 (no changes)");

    tui.input_then_render((KeyModifiers::CONTROL, 'd'))
        .assert_current_line_eq("┊●   90ce384 commit #20 (no changes)");
    tui.input_then_render((KeyModifiers::CONTROL, 'u'))
        .assert_current_line_eq("┊●   1a89cbc commit #30 (no changes)");
}

#[test]
fn jumping_up_down_non_normal_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "");

    let mut tui = test_tui(env);

    tui.input_then_render('j');
    tui.input_then_render('j');
    for n in 1..=30 {
        tui.input_then_render('n');
        tui.input_then_render(KeyCode::Enter);
        tui.input_then_render(format!("commit #{n}"));
        tui.input_then_render(KeyCode::Enter);
    }

    tui.input_then_render('g');
    tui.input_then_render('r');

    tui.input_then_render((KeyModifiers::CONTROL, 'd'))
        .assert_current_line_eq("┊●   << amend >> 9d9282f commit #21 (no changes)");
    tui.input_then_render((KeyModifiers::CONTROL, 'u'))
        .assert_current_line_eq("╭┄<< source >> << noop >> zz [unassigned changes]");
}
