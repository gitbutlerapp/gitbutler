use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::{
    Message,
    tests::utils::{test_tui, test_tui_with_size},
};

#[test]
fn esc_leaves_move_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"])
        .assert_rendered_eq(file!["snapshots/esc_leaves_move_mode_final.txt"]);
}

#[test]
fn move_mode_keeps_selected_commit_and_extension_visible_when_scrolled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 6);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_rendered_contains("<< move commit above >>")
        .assert_rendered_contains("(no commit message) (no changes)")
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);
}

#[test]
fn move_commit_above_other_commit_reorders_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    let env = tui.env;
    let mut tui = test_tui(env);
    tui.input_then_render(None).assert_rendered_eq(file![
        "snapshots/move_commit_above_other_commit_reorders_tui_final.txt"
    ]);
}

#[test]
fn move_commit_below_other_commit_reorders_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    let env = tui.env;
    let mut tui = test_tui(env);
    tui.input_then_render(None).assert_rendered_eq(file![
        "snapshots/move_commit_below_other_commit_reorders_tui_final.txt"
    ]);
}

#[test]
fn move_branch_onto_other_branch_reorders_stacks() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )
    .unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊├┄<< move branch >> [..] [B]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊├┄[..] [A]"]);

    let env = tui.env;
    let mut tui = test_tui(env);
    tui.input_then_render(None).assert_rendered_eq(file![
        "snapshots/move_branch_onto_other_branch_reorders_stacks_final.txt"
    ]);
}

#[test]
fn move_branch_to_merge_base_tears_off_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )
    .unwrap();
    env.setup_metadata(&["A", "C", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄h0 [C]"]);

    tui.input_then_render('m')
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> h0 [C]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str![
            "┴ << tear off branch >> [..] [origin/main] 2000-01-02 add M"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    let env = tui.env;
    let mut tui = test_tui(env);
    tui.render_with_messages(
        None,
        Vec::from([Message::EnterNormalMode, Message::Reload(None)]),
    )
    .assert_rendered_eq(file![
        "snapshots/move_branch_to_merge_base_tears_off_branch_final.txt"
    ]);
}
