use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use snapbox::file;

use crate::{command::legacy::status::tui::tests::test_status_tui, tui::test_utils::Shift};

#[test]
fn cherry_pick_commit_to_merge_base_creates_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);
    let mut tui = test_status_tui(env);

    tui.input("jjp");
    // Below the other commit, then a new branch below A, then a new stack.
    tui.input("jjj")
        .assert_current_line_eq(snapbox::str!["│ << pick to new stack >>"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cherry_pick_commit_to_merge_base_creates_new_branch_001.svg"
        ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_merge_base_creates_new_branch_final.svg"
    ]);
}

#[test]
fn cherry_pick_marked_commits_to_merge_base_creates_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);
    let mut tui = test_status_tui(env);

    tui.input("jj");
    tui.input(' ');
    tui.input('j');
    tui.input(' ');
    tui.input('p');
    // Past the new branch below A, which the marked bottom commit can still go to.
    tui.input(['j', 'j']).assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_marked_commits_to_merge_base_creates_new_branch_001.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_marked_commits_to_merge_base_creates_new_branch_final.svg"
    ]);
}

#[test]
fn cherry_pick_commit_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('p')
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_commit_to_branch_001.svg"]);
    tui.input(['j', 'j', 'j'])
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
    // Past the new branch below A, which the picked commit can go to too.
    tui.input(['j', 'j'])
        .assert_rendered_term_svg_eq(file!["snapshots/cherry_pick_commit_to_new_branch_001.svg"]);
    tui.input(KeyCode::Enter)
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
    tui.input(['j', 'j', 'j']);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_below_commit_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_below_commit_003.svg"
    ]);
}

#[test]
fn cherry_pick_commit_to_new_branch_below() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('p');
    tui.input(['j', 'j', 'j', 'j', 'j'])
        .assert_rendered_term_svg_eq(file![
            "snapshots/cherry_pick_commit_to_new_branch_below_001.svg"
        ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/cherry_pick_commit_to_new_branch_below_002.svg"
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
    tui.input(['j', 'j', 'j'])
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
    tui.input(Shift('g'));
    tui.input('n');
    tui.input('g');
    tui.input('c');
    // Past c-branch-1's stops and the new branch above A, to A's tip.
    tui.input(['j', 'j', 'j', 'j', 'j']);
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
    // Up past the stops of A and the new branch below c-branch-1, to c-branch-1's tip.
    tui.input(['k', 'k', 'k', 'k', 'k'])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commits_are_cherry_picked_in_order_003.svg"
        ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commits_are_cherry_picked_in_order_004.svg"
    ]);
}
