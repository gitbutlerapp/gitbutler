use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use snapbox::file;

use crate::{command::legacy::status::tui::tests::test_status_tui, tui::test_utils::Shift};

#[test]
fn cherry_pick_commit_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('p')
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_commit_to_branch_001.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_commit_to_branch_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_commit_to_branch_003.svg"]);
}

#[test]
fn cherry_pick_commit_to_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('p');
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_commit_to_new_branch_001.svg"]);
    tui.input('b')
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_commit_to_new_branch_002.svg"]);
}

#[test]
fn cherry_pick_commit_to_below_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('p').assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_below_commit_001.svg"
    ]);
    tui.input('j');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_below_commit_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_below_commit_003.svg"
    ]);
}

#[test]
fn cherry_pick_commit_to_above_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('p');
    tui.input('j');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_above_commit_001.svg"
    ]);
    tui.input('a').assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_above_commit_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_above_commit_003.svg"
    ]);
}

#[test]
fn cherry_pick_marks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_marks_001.svg"]);
    tui.input('p')
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_marks_002.svg"]);
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_marks_003.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_marks_004.svg"]);
}

#[test]
fn commits_are_cherry_picked_in_order() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("A", "new A content");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('b');
    tui.input('g');
    tui.input('c');
    tui.input('j');
    tui.input('j');
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("change A");
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commits_are_cherry_picked_in_order_001.svg"
    ]);

    tui.input(' ');
    tui.input(' ');
    tui.input('p').assert_rendered_term_svg_eq(file![
        "snapshots/commits_are_cherry_picked_in_order_002.svg"
    ]);
    tui.input('k');
    tui.input('k');
    tui.input('k').assert_rendered_term_svg_eq(file![
        "snapshots/commits_are_cherry_picked_in_order_003.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commits_are_cherry_picked_in_order_004.svg"
    ]);
}
