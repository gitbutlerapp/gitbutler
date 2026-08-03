use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use snapbox::file;

use crate::{
    command::legacy::status::tui::{backstack::BackstackEntry, tests::utils::test_status_tui},
    tui::test_utils::Shift,
};

#[test]
fn squash_uncommitted_into_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'))
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_into_commit_001.svg"]);
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_into_commit_002.svg"]);
    tui.input('j');
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_into_commit_003.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_into_commit_004.svg"]);
}

#[test]
fn squash_branch_into_commit_on_same_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('c');
    tui.input('e');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_commit_on_same_branch_001.svg"
    ]);

    tui.input('k');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_commit_on_same_branch_002.svg"
    ]);
    tui.input('j');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_commit_on_same_branch_003.svg"
    ]);

    tui.input('u');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_commit_on_same_branch_004.svg"
    ]);
}

#[test]
fn squash_branch_into_commit_on_different_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_commit_on_different_branch_001.svg"
    ]);
    tui.input('j');
    tui.input('j');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_commit_on_different_branch_002.svg"
    ]);
    tui.input('u');
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_commit_on_different_branch_003.svg"
    ]);
}

#[test]
fn squash_branch_into_self() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('c');
    tui.input('e');
    tui.input(KeyCode::Enter);

    tui.input('k');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_branch_into_self_001.svg"]);
    tui.input('u');

    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_branch_into_self_002.svg"]);
}

#[test]
fn squash_branch_into_other_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input('n');
    tui.input('g');

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_branch_into_other_branch_001.svg"]);
    tui.input('u');
    tui.input('j');
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_branch_into_other_branch_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_branch_into_other_branch_003.svg"]);
}

#[test]
fn squash_with_target_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('r');
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_with_target_message_001.svg"]);
    tui.input('j');
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_with_target_message_002.svg"]);
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_with_target_message_003.svg"]);
}

#[test]
fn squash_commit_into_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_commit_into_commit_001.svg"]);
    tui.input('j');
    tui.input('j');
    tui.input('u');
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_commit_into_commit_002.svg"]);
}

#[test]
fn squash_uncommitted_hunk_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_hunk_to_commit_001.svg"]);
    tui.input('j');
    tui.input('j');
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_hunk_to_commit_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_hunk_to_commit_003.svg"]);
}

#[test]
fn squash_uncommitted_hunk_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_hunk_to_branch_001.svg"]);
    tui.input('j');
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_hunk_to_branch_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_hunk_to_branch_003.svg"]);
}

#[test]
fn squash_commit_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_commit_to_branch_001.svg"]);
    tui.input('j');
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_commit_to_branch_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_commit_to_branch_003.svg"]);
}

#[test]
fn squash_committed_file_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_committed_file_to_commit_001.svg"]);
    tui.input('j');
    tui.input('j');
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_committed_file_to_commit_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_committed_file_to_commit_003.svg"]);
}

#[test]
fn squash_committed_file_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_committed_file_to_branch_001.svg"]);
    tui.input('j');
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_committed_file_to_branch_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_committed_file_to_branch_003.svg"]);
}

#[test]
fn squash_uncommit_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommit_commit_001.svg"]);
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommit_commit_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommit_commit_003.svg"]);
}

#[test]
fn squash_uncommit_committed_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommit_committed_file_001.svg"]);
    tui.input('k');
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommit_committed_file_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommit_committed_file_003.svg"]);
}

#[test]
fn squash_marked_uncommitted_files_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "contents");
    env.file("two", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input(' ');
    tui.input(' ');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_marked_uncommitted_files_to_commit_001.svg"
    ]);
    tui.input('j');
    tui.input('j');
    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/squash_marked_uncommitted_files_to_commit_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squash_marked_uncommitted_files_to_commit_003.svg"
    ]);
}

#[test]
fn squash_marked_commits_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "contents");
    env.file("two", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    for _ in 0..2 {
        tui.input('g');
        tui.input('j');
        tui.input('c');
        tui.input('e');
        tui.input('j');
        tui.input(KeyCode::Enter);
    }

    tui.input(' ');
    tui.input(' ');

    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_marked_commits_to_commit_001.svg"]);
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_marked_commits_to_commit_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_marked_commits_to_commit_003.svg"]);
}

#[test]
fn squash_marked_committed_files_to_commit_via_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "contents");
    env.file("two", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('c');
    tui.input('e');
    tui.input('j');
    tui.input(KeyCode::Enter);
    tui.input('j');
    tui.input(' ');
    tui.input(' ');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_marked_committed_files_to_commit_via_global_file_list_001.svg"
    ]);
    tui.input('j');
    tui.input('j');
    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/squash_marked_committed_files_to_commit_via_global_file_list_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squash_marked_committed_files_to_commit_via_global_file_list_003.svg"
    ]);
}

#[test]
fn squash_marked_committed_files_to_commit_via_local_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "contents");
    env.file("two", "contents");
    env.file("three", "contents");

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('j');
    tui.input(KeyCode::Enter);
    tui.input('f');
    tui.input(' ');
    tui.input(' ');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_marked_committed_files_to_commit_via_local_file_list_001.svg"
    ]);
    tui.input('j');
    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/squash_marked_committed_files_to_commit_via_local_file_list_002.svg"
    ]);
    tui.input(KeyCode::Enter)
        .assert_backstack_eq([])
        .assert_rendered_term_svg_eq(file![
            "snapshots/squash_marked_committed_files_to_commit_via_local_file_list_003.svg"
        ]);
}

#[test]
fn squash_uncommitted_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "contents");
    env.file("two", "contents");

    let mut tui = test_status_tui(env);

    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_to_branch_001.svg"]);
    tui.input('j');
    tui.input('u')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_to_branch_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_uncommitted_to_branch_003.svg"]);
}

#[test]
fn reverse_squash() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "contents");
    env.file("two", "contents");

    let mut tui = test_status_tui(env);

    tui.input(Shift('f'));
    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input(Shift('r'))
        .assert_rendered_term_svg_eq(file!["snapshots/reverse_squash_001.svg"]);
    tui.input('u');
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/reverse_squash_002.svg"]);
}

#[test]
fn from_squash_mode_to_commit_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "contents");
    env.file("two", "contents");

    let mut tui = test_status_tui(env);

    tui.input('r')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode])
        .assert_rendered_term_svg_eq(file!["snapshots/from_squash_mode_to_commit_mode_001.svg"]);
    tui.input('c')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode])
        .assert_rendered_term_svg_eq(file!["snapshots/from_squash_mode_to_commit_mode_002.svg"]);
    tui.input('r')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode])
        .assert_rendered_term_svg_eq(file!["snapshots/from_squash_mode_to_commit_mode_003.svg"]);
}

#[test]
fn from_squash_mode_to_mode_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/from_squash_mode_to_mode_mode_001.svg"]);
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/from_squash_mode_to_mode_mode_002.svg"]);
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/from_squash_mode_to_mode_mode_003.svg"]);
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/from_squash_mode_to_mode_mode_004.svg"]);
}

#[test]
fn squash_branch_into_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_branch_into_uncommitted_001.svg"]);
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/squash_branch_into_uncommitted_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/squash_branch_into_uncommitted_003.svg"]);
}

#[test]
fn squash_branch_into_uncommitted_ignores_use_target_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_uncommitted_ignores_use_target_message_001.svg"
    ]);
    tui.input('u').assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_uncommitted_ignores_use_target_message_002.svg"
    ]);
    tui.input('k').assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_uncommitted_ignores_use_target_message_003.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/squash_branch_into_uncommitted_ignores_use_target_message_004.svg"
    ]);
}
