use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::test_tui;

// Tests RubOperation::UncommittedAreaToCommit.
#[test]
fn rub_api_uncommitted_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vot A test.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   vot A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vot A test.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 1 (no commit message) (no changes)"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   1 (no commit message)"])
        .assert_rendered_term_svg_eq(file!["snapshots/rub_api_uncommitted_to_commit.svg"]);
}

#[test]
fn rub_api_uncommitted_to_commit_preserves_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vot A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vot A test.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/rub_api_uncommitted_to_commit_preserves_global_file_list_final.svg"
        ]);
}

#[test]
fn rub_api_cannot_unassign_uncommitted_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vot A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vot A test.txt"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊   << source >> << noop >> vot A test.txt"]);
}

// Tests RubOperation::UncommittedToCommit.
#[test]
fn rub_api_uncommitted_to_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vot A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vot A test.txt"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> lrm add B"]);
}

#[test]
fn mark_and_rub_multiple_uncommitted_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("one", "content");
    tui.env().file("two", "content");
    tui.env().file("three", "content");

    tui.reload();

    tui.input('j');
    tui.input(' ');
    tui.input(' ');

    tui.input('r')
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    let status = tui.env().invoke_git("status --porcelain");
    assert_eq!(
        status, "?? two",
        "expected only unmarked file to remain uncommitted after rubbing marked files"
    );
}

// Ensure rub mode does not offer branch destinations.
#[test]
fn rub_api_cannot_rub_into_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["╭┄<< undo commit >> zz [uncommitted] (no changes)"]);
}

// Tests RubMessage::StartReverse on a commit when uncommitted has changes.
#[test]
fn rub_api_reverse_rub_uses_uncommitted_source_when_uncommitted_has_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");
    tui.env().invoke_git("add test.txt");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input((KeyModifiers::SHIFT, 'J'))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'R'))
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [uncommitted]"]);
}

// Tests RubMessage::StartReverse with uncommitted source when stack has no assigned changes.
#[test]
fn rub_api_reverse_rub_uses_uncommitted_source_when_stack_has_no_assigned_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'R'))
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str![
            "╭┄<< source >> << noop >> zz [uncommitted] (no changes)"
        ]);
}

// Tests RubMessage::StartReverse is a no-op when not selecting a commit.
#[test]
fn rub_api_reverse_rub_is_noop_on_non_commit_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input((KeyModifiers::SHIFT, 'R'))
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);
}

// Tests RubOperation::UndoCommit.
#[test]
fn rub_api_undo_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄<< undo commit >> zz [uncommitted] (no changes)"]);
}

// Tests RubOperation::SquashCommits.
#[test]
fn rub_api_squash_commits_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> lrm add B"]);
}

#[test]
fn rub_api_squash_commits_toggles_message_strategy_labels() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'T'))
        .assert_current_line_eq(str!["┊●   << squash (use this message) >> lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'T'))
        .assert_current_line_eq(str!["┊●   << squash >> lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'S'))
        .assert_current_line_eq(str!["┊●   << squash (discard this message) >> lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'S'))
        .assert_current_line_eq(str!["┊●   << squash >> lrm add B"]);
}

#[test]
fn rub_api_squash_commits_can_keep_target_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'T'))
        .assert_current_line_eq(str!["┊●   << squash (use this message) >> lrm add B"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   lrm add B"]);
}

#[test]
fn rub_api_squash_commits_can_keep_source_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'S'))
        .assert_current_line_eq(str!["┊●   << squash (discard this message) >> lrm add B"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   lrm add A"]);
}

// Tests RubOperation::CommittedFileToCommit.
#[test]
fn rub_api_committed_file_to_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊│     << source >> << noop >> t:t A A"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊●   << move file >> tpm add A"]);
}

// Tests RubOperation::CommittedFileToUncommittedArea.
#[test]
fn rub_api_committed_file_to_uncommitted_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted] (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊│     << source >> << noop >> t:t A A"]);

    tui.input([KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄<< uncommit file >> zz [uncommitted] (no changes)"]);
}

// Tests RubOperation::UncommittedAreaToStack.
#[test]
fn rub_api_uncommitted_area_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("a.txt", "content");
    tui.env().file("z.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   nkn A a.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> nkn A a.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);
}

// Tests RubOperation::UncommittedToStack.
#[test]
fn rub_api_uncommitted_hunk_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("a.txt", "content");
    tui.env().file("z.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   nkn A a.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> nkn A a.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);
}

// Tests RubOperation::StackToUncommittedArea.
#[test]
fn rub_api_stack_to_uncommitted_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vot A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vot A test.txt"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);
}

// Tests RubOperation::StackToStack.
#[test]
fn rub_api_stack_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.env().file("A", "content");
    tui.env().file("B", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   tmn M A"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> tmn M A"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input([
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
    ])
    .assert_current_line_eq(str!["╭┄zz [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   plv M B"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> plv M B"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> lrm add B"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   lrm add B"]);

    tui.input((KeyModifiers::SHIFT, 'K'))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input('r').assert_current_line_eq(str!["┊╭┄h0 [B]"]);
}

#[test]
fn rub_multiple_commits_into_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_tui(env);

    tui.env().file("A", "content");
    tui.env().file("B", "content");
    tui.reload();

    tui.input('j');
    tui.input('c');
    tui.input('e');
    tui.input('b');

    tui.input('g');
    tui.input('j');
    tui.input('c');
    tui.input('e');
    tui.input('j');
    tui.input(KeyCode::Enter);

    tui.input(' ');
    tui.input(' ');

    tui.input('r');
    tui.input('g').assert_rendered_term_svg_eq(file![
        "snapshots/rub_multiple_commits_into_uncommitted_001.svg"
    ]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/rub_multiple_commits_into_uncommitted_final.svg"
    ]);
}

#[test]
fn marks_are_maintained_after_leaving_rub_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);

    tui.reload();

    tui.input('j');
    tui.input('n');
    tui.input('n');
    tui.input('n').assert_rendered_term_svg_eq(file![
        "snapshots/marks_are_maintained_after_leaving_rub_mode_001.svg"
    ]);

    tui.input(' ');
    tui.input(' ');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/marks_are_maintained_after_leaving_rub_mode_002.svg"
    ]);

    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/marks_are_maintained_after_leaving_rub_mode_003.svg"
    ]);

    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/marks_are_maintained_after_leaving_rub_mode_004.svg"
    ]);
}

#[test]
fn moves_cursor_back_into_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moves_cursor_back_into_file_list_001.svg"]);

    tui.input('f')
        .assert_rendered_term_svg_eq(file!["snapshots/moves_cursor_back_into_file_list_002.svg"]);

    tui.input('r');
    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/moves_cursor_back_into_file_list_003.svg"]);

    tui.input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/moves_cursor_back_into_file_list_004.svg"]);
}

#[test]
fn moves_the_cursor_back_to_a_valid_location_when_going_back() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('n');
    tui.input('n');
    tui.input('n').assert_rendered_term_svg_eq(file![
        "snapshots/moves_the_cursor_back_to_a_valid_location_when_going_back_001.svg"
    ]);

    tui.input(' ');
    tui.input(' ');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/moves_the_cursor_back_to_a_valid_location_when_going_back_002.svg"
    ]);

    tui.input('g').assert_rendered_term_svg_eq(file![
        "snapshots/moves_the_cursor_back_to_a_valid_location_when_going_back_003.svg"
    ]);

    tui.input(KeyCode::Esc).assert_rendered_term_svg_eq(file![
        "snapshots/moves_the_cursor_back_to_a_valid_location_when_going_back_004.svg"
    ]);
}
