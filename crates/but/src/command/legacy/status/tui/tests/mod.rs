use anyhow::anyhow;
use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{ToDebug, file, str};
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
mod copy_tests;
mod details_tests;
mod discard_tests;
mod jump_tests;
mod marking_tests;
mod move_tests;
mod rub_tests;
mod stack_tests;
mod utils;

#[test]
fn git_activity_only_reloads_for_a_new_head() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);
    let mut ctx = tui.env().context();
    let project_id = ctx.legacy_project.id.clone();
    let head_sha = super::operations::head_sha(&mut ctx).unwrap();
    drop(ctx);

    tui.env().file("external-change.txt", "content");

    tui.render_with_messages(
        None,
        vec![Message::WatcherEvent(
            gitbutler_watcher::Change::GitActivity {
                project_id: project_id.clone(),
                head_sha,
            },
        )],
    )
    .assert_rendered_not_contains("external-change.txt");

    tui.render_with_messages(
        None,
        vec![Message::WatcherEvent(
            gitbutler_watcher::Change::GitActivity {
                project_id,
                head_sha: "new-head".to_owned(),
            },
        )],
    )
    .assert_rendered_contains("external-change.txt");
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
            Message::ShowError(anyhow!(
                "error-with-end-marker: this is a deliberately long error message that should wrap over multiple lines without clipping and it must include END-MARKER"
            )),
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
            Message::ShowError(err),
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

    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/narrow_hotbar_keeps_help_and_quit_visible_in_modal_modes.svg"
    ]);
}

#[test]
fn help_popup_opens_over_status_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input('?')
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_opens_over_status_view_001.svg"]);

    tui.input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_opens_over_status_view_002.svg"]);
}

#[test]
fn help_popup_searches_descriptions() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input('?');
    tui.input('/')
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_searches_descriptions_001.svg"]);

    tui.input('s');
    tui.input(KeyCode::Enter);
    tui.input("hm")
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_searches_descriptions_002.svg"]);

    tui.input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_searches_descriptions_003.svg"]);

    tui.input('/');
    tui.input('s');
    tui.input((KeyModifiers::CONTROL, 'm'));
    tui.input("hm")
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_searches_descriptions_002.svg"]);
    tui.input('/')
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_searches_descriptions_004.svg"]);

    tui.input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_searches_descriptions_005.svg"]);
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

    tui.input('?')
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_scrolls_001.svg"]);

    tui.input((KeyModifiers::CONTROL, 'd'))
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_scrolls_002.svg"]);

    tui.input((KeyModifiers::CONTROL, 'u'))
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_scrolls_003.svg"]);

    tui.input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/help_popup_scrolls_004.svg"]);
}

#[test]
fn undo_opens_confirm_for_latest_snapshot() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");
    tui.input('c');
    tui.input(KeyCode::Down);
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("commit for undo prompt test");
    tui.input(KeyCode::Enter);

    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/undo_opens_confirm_for_latest_snapshot_001.svg"
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
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    tui.input([
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
    ])
    .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    tui.input([
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
    ])
    .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);
}

#[test]
fn movement_aliases_j_k() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input('j').assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input('j')
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('k').assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input('k')
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);
}

#[test]
fn section_jumps_shift_j_k() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    tui.input((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);
}

#[test]
fn shift_k_from_commit_moves_to_current_section_header_first() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);
}

#[test]
fn shift_k_from_second_stack_commit_moves_to_its_header() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'K'))
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
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_down_002.svg"
        ])
        .assert_current_line_eq(str!["┊●   lrm add B"]);
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

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_up_001.svg"
        ])
        .assert_current_line_eq(str!["┊●   lrm add B"]);

    tui.input([KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cursor_movement_scrolls_viewport_up_002.svg"
        ])
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);
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

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/section_jumps_scroll_viewport_when_target_is_offscreen_001.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/section_jumps_scroll_viewport_when_target_is_offscreen_002.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input((KeyModifiers::SHIFT, 'K'))
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

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
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

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down]);

    tui.render_with_messages(
        None,
        Vec::from([Message::Reload(None, ReloadCause::Mutation)]),
    )
    .assert_rendered_term_svg_eq(file![
        "snapshots/reload_preserves_visible_selection_when_scrolled_001.svg"
    ])
    .assert_current_line_eq(str!["┊●   lrm add B"]);
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

    tui.input([
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Enter,
    ])
    .assert_rendered_term_svg_eq(file![
        "snapshots/inline_reword_renders_on_visible_row_when_scrolled_001.svg"
    ])
    .assert_current_line_eq(str!["┊●   lrm add B"]);
}

#[test]
fn creating_empty_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input('n')
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_002.svg"])
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input('n')
        .assert_rendered_term_svg_eq(file!["snapshots/creating_empty_commits_003.svg"])
        .assert_current_line_eq(str!["┊●   1#0 (no commit message) (no changes)"]);
}

#[test]
fn inline_reword() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input('n')
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_002.svg"])
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_003.svg"]);

    tui.input("foo")
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_004.svg"]);

    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/inline_reword_005.svg"])
        .assert_current_line_eq(str!["┊●   1 foo (no changes)"]);
}

#[test]
fn inline_reword_open_editor_keeps_inline_message_when_editor_makes_no_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(".git/editor.sh", "exit 0\n");
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input(KeyCode::Enter);
    tui.input(" updated")
        .assert_rendered_contains("add A updated");

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input((KeyModifiers::ALT, 'e'))
            .assert_current_line_eq(str!["┊●   tpm add A updated"]);
    });
}

#[test]
fn esc_leaves_rub_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   v A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> v A test.txt"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊   v A test.txt"]);
}

#[test]
fn mode_key_r_enters_and_escape_leaves_rub_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   v A test.txt"]);

    tui.input('r')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_r_enters_and_leaves_rub_mode_001.svg"
        ])
        .assert_current_line_eq(str!["┊   << source >> << noop >> v A test.txt"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊   v A test.txt"]);
}

#[test]
fn rub_mode_shift_j_lands_on_first_selectable_in_next_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   v A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> v A test.txt"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);
}

#[test]
fn rub_mode_shift_j_can_jump_between_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   v A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> v A test.txt"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> lrm add B"]);
}

#[test]
fn rub_mode_shift_k_jumps_to_first_selectable_in_previous_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   v A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> v A test.txt"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊●   << amend >> lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);
}

#[test]
fn mode_key_c_enters_and_escape_leaves_commit_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input('c')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_c_enters_and_leaves_commit_mode_001.svg"
        ])
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [uncommitted]"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);
}

#[test]
fn mode_key_m_enters_and_escape_leaves_move_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input('m')
        .assert_rendered_term_svg_eq(file![
            "snapshots/mode_toggle_key_m_enters_and_leaves_move_mode_001.svg"
        ])
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
}

#[test]
fn key_b_creates_new_branch_from_selected_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input('b')
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1] (no commits)"]);
}

#[test]
fn rubbing() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_001.svg"])
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_002.svg"])
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   v A test.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   v A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> v A test.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 1 (no commit message) (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input(KeyCode::Enter);
    // that you end up on zz is a bug but requires moving the rub implementation to use but-api
    // that work is in progress
    tui.input([
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
    ])
    .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_003.svg"]);
}

#[test]
fn global_file_list_does_not_restrict_cursor() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
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

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('f')
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊│     t:t A A"])
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

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    with_var("GIT_AUTHOR_DATE", Some("2000-01-01T00:00:00Z"), || {
        with_var("GIT_COMMITTER_DATE", Some("2000-01-01T00:00:00Z"), || {
            tui.input('n')
                .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);
        });
    });

    tui.input('f')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down])
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

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('f')
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input((KeyModifiers::SHIFT, 'R'))
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_file_list_rub_esc_leaves_rub_and_closes_file_list_final.svg"
        ]);
}

#[test]
fn confirm_rub_keeps_commit_file_list_open() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('f')
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input((KeyModifiers::SHIFT, 'R'))
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     t:t A A"]);
}

#[test]
fn esc_in_normal_mode_closes_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/esc_in_normal_mode_closes_global_file_list_final.svg"
        ]);
}

#[test]
fn esc_in_normal_mode_closes_commit_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('f')
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/esc_in_normal_mode_closes_commit_file_list_final.svg"
        ]);
}

#[test]
fn commit_file_toggle_off_from_commit_row_preserves_commit_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('f')
        .assert_current_line_eq(str!["┊●   tpm add A"])
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

    tui.input('j');
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/pick_changes_mode_002.svg"]);
    let outcome = tui.input(KeyCode::Enter).take_outcome().unwrap();

    let cli_ids = match outcome {
        TuiOutcome::CliIds(cli_ids) => cli_ids,
        _ => panic!("unexpected outcome {outcome:#?}"),
    };

    let actual = cli_ids
        .into_iter()
        .map(|id| {
            if let CliId::UncommittedHunkOrFile(hunk) = id {
                hunk
            } else {
                panic!("expected hunk, got {id:#?}")
            }
        })
        .map(|hunk| {
            (
                hunk.id,
                hunk.hunk_assignments.head.path,
                hunk.is_entire_file,
            )
        })
        .collect::<Vec<_>>();
    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
[
    (
        "k",
        "one",
        true,
    ),
    (
        "k:synthetic-id-0",
        "one",
        false,
    ),
]

"#]]
    );
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
    tui.input('j');
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file![
            "snapshots/stays_in_pick_change_mode_after_full_screen_details_002.svg"
        ]);

    tui.input((KeyModifiers::SHIFT, 'D'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/stays_in_pick_change_mode_after_full_screen_details_003.svg"
        ])
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenFullScreenDetailsView,
            BackstackEntry::Mark,
        ]);

    tui.input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/stays_in_pick_change_mode_after_full_screen_details_004.svg"
        ])
        .assert_backstack_eq([BackstackEntry::Mark]);

    // ensure the changes are still marked after returning from details mode
    let outcome = tui.input(KeyCode::Enter).take_outcome().unwrap();

    let cli_ids = match outcome {
        TuiOutcome::CliIds(cli_ids) => cli_ids,
        _ => panic!("unexpected outcome {outcome:#?}"),
    };

    let actual = cli_ids
        .into_iter()
        .map(|id| {
            if let CliId::UncommittedHunkOrFile(hunk) = id {
                hunk
            } else {
                panic!("expected hunk, got {id:#?}")
            }
        })
        .map(|hunk| {
            (
                hunk.id,
                hunk.hunk_assignments.head.path,
                hunk.is_entire_file,
            )
        })
        .collect::<Vec<_>>();
    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
[
    (
        "k",
        "one",
        true,
    ),
    (
        "k:synthetic-id-0",
        "one",
        false,
    ),
]

"#]]
    );
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

    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/pick_changes_mode_supports_focusing_details_view_001.svg"
    ]);
}

#[test]
fn consistent_commit_shas_in_tests() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_tui(env);

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);
}

#[test]
fn jumping_up_down() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input('j');
    for n in 1..=12 {
        tui.input('n');
        tui.input(KeyCode::Enter);
        tui.input(format!("commit #{n}"));
        tui.input(KeyCode::Enter);
    }

    tui.reload()
        .assert_current_line_eq("┊●   1#0 commit #12 (no changes)");

    tui.input((KeyModifiers::CONTROL, 'd'))
        .assert_current_line_eq("┊●   1#10 commit #2 (no changes)");
    tui.input((KeyModifiers::CONTROL, 'u'))
        .assert_current_line_eq("┊●   1#0 commit #12 (no changes)");
}

#[test]
fn jumping_up_down_non_normal_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "");

    let mut tui = test_tui(env);

    tui.input('j');
    tui.input('j');
    for n in 1..=12 {
        tui.input('n');
        tui.input(KeyCode::Enter);
        tui.input(format!("commit #{n}"));
        tui.input(KeyCode::Enter);
    }

    tui.input('g');
    tui.input('r');

    tui.input((KeyModifiers::CONTROL, 'd'))
        .assert_current_line_eq("┊●   << amend >> 1#9 commit #3 (no changes)");
    tui.input((KeyModifiers::CONTROL, 'u'))
        .assert_current_line_eq("╭┄<< source >> << noop >> zz [uncommitted]");
}

#[test]
fn pressing_l_doesnt_unfocus_the_detail_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    // open and focus the detail view
    tui.input('d');
    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/pressing_l_doesnt_unfocus_the_detail_view_001.svg"
    ]);

    // pressing `l` again should do nothing since we're already focused on the detail view
    tui.input('l').assert_rendered_term_svg_eq(file![
        "snapshots/pressing_l_doesnt_unfocus_the_detail_view_001.svg"
    ]);
}

#[test]
fn maintains_selection_using_change_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_tui(env);

    // create a new branch with a commit
    tui.input('b');
    tui.input('n');

    // reword the commit
    tui.input(KeyCode::Enter);
    tui.input("target");
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   1 target (no changes)"]);

    // undo the reword, this shouldn't change the selection
    tui.input('u')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);
}
