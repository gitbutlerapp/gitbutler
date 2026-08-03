use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::{
    TestTuiOptions, test_status_tui, test_status_tui_with_options,
};

#[test]
fn esc_leaves_move_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊╭┄ << source >> << noop >> g0 [A]"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/esc_leaves_move_mode_final.svg"]);
}

#[test]
fn move_mode_keeps_selected_commit_and_extension_visible_when_scrolled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 6,
            ..Default::default()
        },
    );

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   1#0 (no commit message) (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input(KeyCode::Up)
        .assert_rendered_contains("<< move commit above >>")
        .assert_rendered_contains("(no commit message) (no changes)")
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);
}

#[test]
fn move_commit_above_other_commit_reorders_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   1#0 (no commit message) (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui = tui.recreate();
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/move_commit_above_other_commit_reorders_tui_final.svg"
    ]);
}

#[test]
fn move_commit_down_from_source_selects_next_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   1#0 (no commit message) (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   1#1 (no commit message) (no changes)"]);

    tui.input('m').assert_current_line_eq(str![
        "┊●   << source >> << noop >> 1#1 (no commit message) (no changes)"
    ]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);
}

#[test]
fn move_commit_up_from_top_commit_selects_source_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄ h0 [C]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   xwn add C"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> xwn add C"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄ h0 [C]"])
        .assert_rendered_contains("<< move commit to branch >>");
}

#[test]
fn move_branch_onto_other_branch_reorders_stacks() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊╭┄ << source >> << noop >> g0 [A]"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│  << stack branch >>"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊├┄ h0 [A]"]);

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

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input('m').assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_to_merge_base_tears_off_branch_001.svg"
    ]);
    tui.input('j');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_to_merge_base_tears_off_branch_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_to_merge_base_tears_off_branch_003.svg"
    ]);
}

#[test]
fn moving_multiple_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_001.svg"]);

    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_002.svg"]);

    tui.input('j');
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_003.svg"]);

    tui.input('k');
    tui.input('m');
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_004.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_005.svg"]);
}
