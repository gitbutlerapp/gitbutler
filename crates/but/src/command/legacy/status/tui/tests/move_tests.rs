use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::{
    Message, ReloadCause,
    tests::utils::{TestTuiOptions, test_tui, test_tui_with_options},
};

#[test]
fn esc_leaves_move_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/esc_leaves_move_mode_final.svg"]);
}

#[test]
fn move_mode_keeps_selected_commit_and_extension_visible_when_scrolled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 6,
            ..Default::default()
        },
    );

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   9638f28 (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_rendered_contains("<< move commit above >>")
        .assert_rendered_contains("(no commit message) (no changes)")
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);
}

#[test]
fn move_commit_above_other_commit_reorders_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   9638f28 (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   2c72e58 add A"]);

    tui = tui.recreate();
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/move_commit_above_other_commit_reorders_tui_final.svg"
    ]);
}

#[test]
fn move_commit_down_from_source_selects_next_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   9638f28 (no commit message) (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render('m').assert_current_line_eq(str![
        "┊●   << source >> << noop >> f184fc7 (no commit message) (no changes)"
    ]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);
}

#[test]
fn move_commit_up_from_top_commit_selects_source_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄h0 [C]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   3842fc0 add C"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 3842fc0 add C"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄h0 [C]"])
        .assert_rendered_contains("<< move commit to branch >>");
}

#[test]
fn move_branch_onto_other_branch_reorders_stacks() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│ << stack branch >>"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊├┄h0 [A]"]);

    tui = tui.recreate();
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_onto_other_branch_reorders_stacks_final.svg"
    ]);
}

#[test]
fn move_branch_to_merge_base_tears_off_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "C", "B"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄h0 [C]"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> h0 [C]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str![
            "┴ << unstack branch >> 0dc3733 (common base) 2000-01-02 add M"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄i0 [C]"]);

    tui = tui.recreate();
    tui.render_with_messages(
        None,
        Vec::from([
            Message::EnterNormalModeAfterConfirmingOperation,
            Message::Reload(None, ReloadCause::Mutation),
        ]),
    )
    .assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_to_merge_base_tears_off_branch_final.svg"
    ]);
}

#[test]
fn moving_multiple_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render('b');
    tui.input_then_render('g')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_001.svg"]);

    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render(' ');

    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_002.svg"]);

    tui.input_then_render('m')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_003.svg"]);
    tui.input_then_render('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_004.svg"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_005.svg"]);
}
