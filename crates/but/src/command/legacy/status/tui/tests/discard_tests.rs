use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};
use snapbox::{file, str};

use crate::{
    command::legacy::status::tui::{backstack::BackstackEntry, tests::utils::test_status_tui},
    tui::test_utils::Shift,
};

#[test]
fn discard_prompt_can_be_cancelled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('x')
        .assert_rendered_contains("Discard uncommitted changes?")
        .assert_rendered_contains("<< discard >>");

    tui.input('n');

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"])
        .assert_rendered_term_svg_eq(file!["snapshots/discard_prompt_can_be_cancelled_final.svg"]);
}

#[test]
fn discard_uncommitted_confirm_yes_discards_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('x')
        .assert_rendered_contains("Discard uncommitted changes?");

    tui.input('y');

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    let status = tui.env().invoke_git("status --porcelain");
    assert_eq!(status, "");

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/discard_uncommitted_confirm_yes_discards_changes_final.svg"
    ]);
}

#[test]
fn discard_uncommitted_cancel_keeps_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('x')
        .assert_rendered_contains("Discard uncommitted changes?");

    tui.input('n');

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    let status = tui.env().invoke_git("status --porcelain");
    assert!(
        status.contains("test.txt"),
        "expected uncommitted changes to remain, got: {status:?}"
    );

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/discard_uncommitted_cancel_keeps_changes_final.svg"
    ]);
}

#[test]
fn discard_commit_confirm_yes_removes_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('x')
        .assert_rendered_contains("Discard commit")
        .assert_rendered_contains("<< discard >>");

    tui.input('y');
    tui.reload();

    let log = tui.env().invoke_git("log --oneline");
    assert!(
        !log.contains("add A"),
        "expected discarded commit to be removed from history, got:\n{log}"
    );

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/discard_commit_confirm_yes_removes_commit_final.svg"
    ]);
}

#[test]
fn discard_top_commit_selects_next_commit_in_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   oun (no commit message) (no changes)"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   mul (no commit message) (no changes)"]);

    tui.input('x')
        .assert_rendered_contains("Discard commit")
        .assert_rendered_contains("<< discard >>");

    tui.input('y');

    tui.reload()
        .assert_current_line_eq(str!["┊●   oun (no commit message) (no changes)"]);
}

#[test]
fn discard_bottom_commit_selects_rewritten_commit_above() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    for message in ["one", "two"] {
        tui.input('n');
        tui.input(KeyCode::Enter);
        tui.input(message);
        tui.input(KeyCode::Enter);
    }

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('x').assert_rendered_contains("Discard commit");
    tui.input('y')
        .assert_current_line_eq(str!["┊●   oun one (no changes)"]);
}

#[test]
fn discard_stack_confirm_yes_discards_staged_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> vo A test.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ << amend >> g0 [A]"]);

    tui.input('u');
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input('x')
        .assert_rendered_contains("Discard uncommitted changes?")
        .assert_rendered_contains("<< discard >>");

    tui.input('y');

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    let status = tui.env().invoke_git("status --porcelain");
    assert_eq!(status, "");

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/discard_stack_confirm_yes_discards_staged_changes_final.svg"
    ]);
}

#[test]
fn discard_branch_confirm_yes_removes_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"]);

    tui.input('x')
        .assert_rendered_contains("Discard branch c-branch-1?")
        .assert_rendered_contains("<< discard >>");

    tui.input('y');

    tui.reload().assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    let branches = tui.env().invoke_git("branch --list");
    assert!(
        !branches.contains("c-branch-1"),
        "expected branch c-branch-1 to be removed, got: {branches:?}"
    );

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/discard_branch_confirm_yes_removes_branch_final.svg"
    ]);
}

#[test]
fn discard_branch_cancel_keeps_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"]);

    tui.input('x')
        .assert_rendered_contains("Discard branch c-branch-1?");

    tui.input('n');

    tui.reload()
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"]);

    let branches = tui.env().invoke_git("branch --list");
    assert!(
        branches.contains("c-branch-1"),
        "expected branch c-branch-1 to remain, got: {branches:?}"
    );

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/discard_branch_cancel_keeps_branch_final.svg"
    ]);
}

#[test]
fn discard_multiple_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1] (no commits)"]);

    for msg in ["one", "two", "three"] {
        tui.input('n');
        tui.input(KeyCode::Enter);
        tui.input(msg);
        tui.input(KeyCode::Enter);
    }

    tui.input(' ');
    tui.input(KeyCode::Down);
    tui.input(' ');

    tui.input('x').assert_rendered_contains("Discard?");

    tui.input('y');

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/discard_multiple_commits_final.svg"]);
}

#[test]
fn mark_and_discard_uncommitted_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input(' ');
    tui.input('j');
    tui.input(' ');

    tui.input('x');
    tui.input('y')
        .assert_current_line_eq(str!["┊   or A three"]);

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/mark_and_discard_uncommitted_files_final.svg"
    ]);
}

#[test]
fn discard_individual_committed_files_from_local_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.input('f').assert_rendered_term_svg_eq(file![
        "snapshots/discard_individual_committed_files_from_local_file_list_001.svg"
    ]);

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_individual_committed_files_from_local_file_list_002.svg"
    ]);
    tui.input('y')
        .assert_current_line_eq(str!["┊│     zt:o A three"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_individual_committed_files_from_local_file_list_003.svg"
        ])
        .assert_backstack_eq([BackstackEntry::ShowFileList]);

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_individual_committed_files_from_local_file_list_004.svg"
    ]);
    tui.input('y')
        .assert_current_line_eq(str!["┊│     zt:t A two"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_individual_committed_files_from_local_file_list_005.svg"
        ])
        .assert_backstack_eq([BackstackEntry::ShowFileList]);

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_individual_committed_files_from_local_file_list_006.svg"
    ]);
    tui.input('y')
        .assert_current_line_eq(str!["┊●   ztn (no commit message) (no changes)"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_individual_committed_files_from_local_file_list_007.svg"
        ])
        .assert_backstack_eq([]);
}

#[test]
fn discard_individual_committed_files_from_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');
    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_individual_committed_files_from_global_file_list_001.svg"
        ]);

    tui.input('j');

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_individual_committed_files_from_global_file_list_002.svg"
    ]);
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_individual_committed_files_from_global_file_list_003.svg"
    ]);

    tui.input('x');
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_individual_committed_files_from_global_file_list_004.svg"
    ]);

    tui.input('x');
    tui.input('y')
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_individual_committed_files_from_global_file_list_005.svg"
        ])
        .assert_backstack_eq([]);
}

#[test]
fn discard_marked_committed_files_from_local_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');
    tui.input('f').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_committed_files_from_local_file_list_001.svg"
    ]);

    tui.input(' ');
    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::ShowFileList])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_marked_committed_files_from_local_file_list_002.svg"
        ]);

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_committed_files_from_local_file_list_003.svg"
    ]);
    tui.input('y')
        .assert_current_line_eq(str!["┊│     zt:t A two"])
        .assert_backstack_eq([BackstackEntry::ShowFileList])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_marked_committed_files_from_local_file_list_004.svg"
        ]);

    tui.input(' ')
        .assert_backstack_eq([BackstackEntry::Mark, BackstackEntry::ShowFileList]);

    tui.input('x');
    tui.input('y')
        .assert_current_line_eq(str!["┊●   ztn (no commit message) (no changes)"])
        .assert_backstack_eq([])
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_marked_committed_files_from_local_file_list_005.svg"
        ]);
}

#[test]
fn discard_marked_committed_files_from_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');
    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/discard_marked_committed_files_from_global_file_list_001.svg"
        ]);

    tui.input('j');
    tui.input(' ');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_committed_files_from_global_file_list_002.svg"
    ]);

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_committed_files_from_global_file_list_003.svg"
    ]);
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_committed_files_from_global_file_list_004.svg"
    ]);

    tui.input('k').assert_rendered_term_svg_eq(file![
        "snapshots/discard_marked_committed_files_from_global_file_list_005.svg"
    ]);
}

#[test]
fn global_file_list_stays_open_after_discarding_the_last_file_in_a_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/global_file_list_stays_open_after_discarding_the_last_file_in_a_commit_001.svg"
        ]);

    // discard the file in the top commit
    tui.input('j');
    tui.input('j');
    tui.input('j')
        .assert_current_line_eq(str!["┊│     t:t A A"]);
    tui.input('x');
    tui.input('y');

    // after discarding the last file in the commit the global file list should still be open
    tui.input('g')
        .assert_rendered_term_svg_eq(file![
            "snapshots/global_file_list_stays_open_after_discarding_the_last_file_in_a_commit_002.svg"
        ])
        .assert_backstack_eq([BackstackEntry::ShowFileList]);
}

#[test]
fn global_file_list_stays_open_after_marking_and_discarding_all_files_in_a_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'F'));

    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input(' ');
    tui.input('x')
        .assert_rendered_term_svg_eq(file![
            "snapshots/global_file_list_stays_open_after_marking_and_discarding_all_files_in_a_commit_001.svg"
        ]);
    tui.input('y')
        .assert_rendered_term_svg_eq(file![
            "snapshots/global_file_list_stays_open_after_marking_and_discarding_all_files_in_a_commit_002.svg"
        ])
        .assert_backstack_eq([BackstackEntry::ShowFileList]);
}

#[test]
fn marking_and_discarding_multiple_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('b');
    tui.input('n');
    tui.input('n');

    tui.input('g');
    tui.input('b');
    tui.input('n');
    tui.input('n').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_branches_001.svg"
    ]);

    tui.input('g');
    tui.input(Shift('j'));
    tui.input(Shift('j'));
    tui.input(' ');
    tui.input(' ');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_branches_002.svg"
    ]);

    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_branches_003.svg"
    ]);
    tui.input('y').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_branches_004.svg"
    ]);
}

#[test]
fn marking_and_discarding_multiple_branches_fails_with_dependencies() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "content");

    let mut tui = test_status_tui(env);

    // create a stack of two branches where the top has a dependency on the bottom
    tui.input('c');
    tui.input('e');
    tui.input('b');
    tui.env().append_file("file", "new line");
    tui.reload();
    tui.input('g');
    tui.input('c');
    tui.input('e');
    tui.input('j');
    tui.input('b');

    // create a new branch without any dependencies
    tui.input('g');
    tui.input('b');
    tui.input('n');
    tui.input('n');

    tui.input(Shift('f')).assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_branches_fails_with_dependencies_001.svg"
    ]);

    // mark and discard two branches:
    // 1. the one without dependencies
    // 2. the bottom branch in the stack which the top dependes on
    tui.input('g');
    tui.input(Shift('j'));
    tui.input(' ');
    tui.input(Shift('j'));
    tui.input(' ');
    tui.input('x').assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_branches_fails_with_dependencies_002.svg"
    ]);
    tui.input('y');
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/marking_and_discarding_multiple_branches_fails_with_dependencies_003.svg"
    ]);
}
