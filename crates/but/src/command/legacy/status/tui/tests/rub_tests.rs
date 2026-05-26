use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::test_tui;

// Tests RubOperation::UnassignedToCommit.
#[test]
fn rub_api_unassigned_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str![
            "┊●   << amend >> [..] (no commit message) (no changes)"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] (no commit message)"])
        .assert_rendered_term_svg_eq(file!["snapshots/rub_api_unassigned_to_commit.svg"]);
}

#[test]
fn rub_api_unassigned_to_commit_preserves_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/rub_api_unassigned_to_commit_preserves_global_file_list_final.svg"
        ]);
}

// Tests RubOperation::UnassignUncommitted.
#[test]
fn rub_api_unassign_uncommitted_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["╭┄<< unassign hunks >> zz [unassigned changes]"]);
}

// Tests RubOperation::UncommittedToCommit.
#[test]
fn rub_api_uncommitted_to_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> [..] add [..]"]);
}

// Ensure rub mode does not offer branch destinations.
#[test]
fn rub_api_cannot_rub_into_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str![
            "╭┄<< undo commit >> zz [unassigned changes] (no changes)"
        ]);
}

// Tests RubMessage::StartReverse on a commit when unassigned has changes.
#[test]
fn rub_api_reverse_rub_uses_unassigned_source_when_unassigned_has_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");
    tui.env().invoke_git("add test.txt");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊●   << amend >> [..] add A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);
}

// Tests RubMessage::StartReverse with unassigned source when stack has no assigned changes.
#[test]
fn rub_api_reverse_rub_uses_unassigned_source_when_stack_has_no_assigned_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊●   << amend >> [..] add A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str![
            "╭┄<< source >> << noop >> zz [unassigned changes] (no changes)"
        ]);
}

// Tests RubMessage::StartReverse is a no-op when not selecting a commit.
#[test]
fn rub_api_reverse_rub_is_noop_on_non_commit_selection() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);
}

// Tests RubOperation::UndoCommit.
#[test]
fn rub_api_undo_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str![
            "╭┄<< undo commit >> zz [unassigned changes] (no changes)"
        ]);
}

// Tests RubOperation::SquashCommits.
#[test]
fn rub_api_squash_commits_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> [..] add B"]);
}

#[test]
fn rub_api_squash_commits_toggles_message_strategy_labels() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> [..] add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('T')))
        .assert_current_line_eq(str!["┊●   << squash (use this message) >> [..] add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('T')))
        .assert_current_line_eq(str!["┊●   << squash >> [..] add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('S')))
        .assert_current_line_eq(str!["┊●   << squash (discard this message) >> [..] add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('S')))
        .assert_current_line_eq(str!["┊●   << squash >> [..] add B"]);
}

#[test]
fn rub_api_squash_commits_can_keep_target_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> [..] add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('T')))
        .assert_current_line_eq(str!["┊●   << squash (use this message) >> [..] add B"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add B"]);
}

#[test]
fn rub_api_squash_commits_can_keep_source_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> [..] add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('S')))
        .assert_current_line_eq(str!["┊●   << squash (discard this message) >> [..] add B"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);
}

// Tests RubOperation::CommittedFileToCommit.
#[test]
fn rub_api_committed_file_to_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊│     << source >> << noop >> [..] A A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊●   << move file >> [..] add A"]);
}

// Tests RubOperation::CommittedFileToUnassigned.
#[test]
fn rub_api_committed_file_to_unassigned_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊│     << source >> << noop >> [..] A A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str![
            "╭┄<< uncommit file >> zz [unassigned changes] (no changes)"
        ]);
}

// Tests RubOperation::UnassignedToStack.
#[test]
fn rub_api_unassigned_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("a.txt", "content");
    tui.env().file("z.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A a.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A a.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);
}

// Tests RubOperation::UncommittedToStack.
#[test]
fn rub_api_uncommitted_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("a.txt", "content");
    tui.env().file("z.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A a.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A a.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);
}

// Tests RubOperation::StackToUnassigned.
#[test]
fn rub_api_stack_to_unassigned_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);
}

// Tests RubOperation::StackToStack.
#[test]
fn rub_api_stack_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("A", "content");
    tui.env().file("B", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A[..]"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A[..]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render([
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
    ])
    .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] B[..]"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] B[..]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> d3e2ba3 add B"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);
}

#[test]
fn cannot_yet_rub_multiple_uncommitted_files() {
    // some day we will allow rubbing multiple uncommitted files

    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("file-one", "content");
    tui.env().file("file-two", "content");

    tui.input_then_render(KeyCode::Down);
    tui.input_then_render(' ');
    tui.input_then_render(' ');

    tui.input_then_render('r')
        .assert_rendered_term_svg_eq(file!["snapshots/cannot_yet_rub_multiple_files_final.svg"]);
}
