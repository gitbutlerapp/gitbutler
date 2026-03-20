use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};
use temp_env::with_var;

use crate::command::legacy::status::tui::tests::utils::test_tui;

// Tests RubOperation::UnassignedToCommit.
#[test]
fn rub_api_unassigned_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str![
            "┊●   << amend >> [..] (no commit message) (no changes)"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] (no commit message)"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"])
        .assert_rendered_eq(file!["snapshots/rub_api_unassigned_to_commit.txt"]);
}

// Tests RubOperation::UnassignedToBranch.
#[test]
fn rub_api_unassigned_to_branch_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> [..] [A]"]);
}

// Tests RubOperation::UnassignUncommitted.
#[test]
fn rub_api_unassign_uncommitted_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["╭┄<< unassign hunks >> zz [unstaged changes]"]);
}

// Tests RubOperation::UncommittedToBranch.
#[test]
fn rub_api_uncommitted_to_branch_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> [..] [A]"]);
}

// Tests RubOperation::UncommittedToCommit.
#[test]
fn rub_api_uncommitted_to_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> [..] add A"]);
}

// Tests RubOperation::BranchToUnassigned.
#[test]
fn rub_api_branch_to_unassigned_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄[..] [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> [..] [A]"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["╭┄<< unassign hunks >> zz [unstaged changes]"]);
}

// Tests RubOperation::BranchToCommit.
#[test]
fn rub_api_branch_to_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄[..] [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> [..] [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> [..] add A"]);
}

// Tests RubOperation::BranchToBranch.
#[test]
fn rub_api_branch_to_branch_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄[..] [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> [..] [A]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄<< reassign hunks >> h0 [B]"]);
}

// Tests RubOperation::MoveCommitToBranch.
#[test]
fn rub_api_move_commit_to_branch_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄<< move commit >> [..] [A]"]);
}

// Tests RubOperation::UndoCommit.
#[test]
fn rub_api_undo_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄<< undo commit >> zz [unstaged changes]"]);
}

// Tests RubOperation::SquashCommits.
#[test]
fn rub_api_squash_commits_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊●   << source >> << noop >> [..] add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << SquashCommits is not supported >> [..] add B"]);
}

// Tests RubOperation::CommittedFileToCommit.
#[test]
fn rub_api_committed_file_to_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊│     << source >> << noop >> [..] A A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊●   << move file >> [..] add A"]);
}

// Tests RubOperation::CommittedFileToBranch.
#[test]
fn rub_api_committed_file_to_branch_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊│     << source >> << noop >> [..] A A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊╭┄<< uncommit file >> [..] [A]"]);
}

// Tests RubOperation::CommittedFileToUnassigned.
#[test]
fn rub_api_committed_file_to_unassigned_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     [..] A A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊│     << source >> << noop >> [..] A A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄<< uncommit file >> zz [unstaged changes]"]);
}

// Tests RubOperation::UnassignedToStack.
#[test]
fn rub_api_unassigned_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("a.txt", "content");
    tui.env.file("z.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A a.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A a.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊  ╭┄<< assign hunks >> [..] [staged to A]"]);
}

// Tests RubOperation::UncommittedToStack.
#[test]
fn rub_api_uncommitted_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("a.txt", "content");
    tui.env.file("z.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A a.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A a.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   [..] A z.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A z.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊  ╭┄<< assign hunks >> [..] [staged to A]"]);
}

// Tests RubOperation::BranchToStack.
#[test]
fn rub_api_branch_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("a.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A a.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A a.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊  ╭┄<< reassign hunks >> [..] [staged to A]"]);
}

// Tests RubOperation::StackToUnassigned.
#[test]
fn rub_api_stack_to_unassigned_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊  │ [..] A test.txt"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊  ╭┄[..] [staged to A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊  ╭┄<< source >> << noop >> [..] [staged to A]"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["╭┄<< unassign hunks >> zz [unstaged changes]"]);
}

// Tests RubOperation::StackToBranch.
#[test]
fn rub_api_stack_to_branch_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊  │ [..] A test.txt"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊  ╭┄[..] [staged to A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊  ╭┄<< source >> << noop >> [..] [staged to A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< reassign hunks >> g0 [A]"]);
}

// Tests RubOperation::StackToStack.
#[test]
fn rub_api_stack_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("A", "content");
    tui.env.file("B", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A[..]"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A[..]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render([
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
        KeyCode::Up,
    ])
    .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] B[..]"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] B[..]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> h0 [B]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_current_line_eq(str!["┊  ╭┄[..] [staged to B]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊  ╭┄<< source >> << noop >> [..] [staged to B]"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊  ╭┄<< reassign hunks >> [..] [staged to A]"]);
}

#[test]
fn rub_api_unassigned_to_commit_multi_hunk_modified_file_commits_all_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        "printf 'commit from tui test\n' > \"$1\"\n".to_string(),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let original = (1..=300)
        .map(|line| format!("line-{line}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    env.file("multi-hunk.txt", &original);

    let mut tui = test_tui(env);

    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter);
    });

    let modified = (1..=300)
        .map(|line| match line {
            1 => "line-1-modified".to_string(),
            250 => "line-250-modified".to_string(),
            _ => format!("line-{line}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    tui.env.file("multi-hunk.txt", &modified);

    for _ in 0..10 {
        tui.input_then_render(KeyCode::Up);
    }
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] multi-hunk.txt"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] multi-hunk.txt"]);
    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> [..]"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..]"]);

    let status_after = tui.env.invoke_git("status --porcelain");
    assert!(
        !status_after
            .lines()
            .any(|line| line.ends_with("multi-hunk.txt")),
        "after Shift+R unassigned->commit amend, multi-hunk modified file should be fully committed\nstatus was:\n{status_after}"
    );
}

#[test]
fn rub_api_uncommitted_to_commit_multi_hunk_modified_file_commits_all_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        "printf 'commit from tui test\n' > \"$1\"\n".to_string(),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let original = (1..=300)
        .map(|line| format!("line-{line}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    env.file("multi-hunk.txt", &original);

    let mut tui = test_tui(env);

    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter);
    });

    let modified = (1..=300)
        .map(|line| match line {
            1 => "line-1-modified".to_string(),
            250 => "line-250-modified".to_string(),
            _ => format!("line-{line}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    tui.env.file("multi-hunk.txt", &modified);

    for _ in 0..10 {
        tui.input_then_render(KeyCode::Up);
    }
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] multi-hunk.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] multi-hunk.txt"]);
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊  │ [..] multi-hunk.txt"]);
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊  │ << source >> << noop >> [..] multi-hunk.txt"]);
    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> [..]"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..]"]);

    let status_after = tui.env.invoke_git("status --porcelain");
    assert!(
        !status_after
            .lines()
            .any(|line| line.ends_with("multi-hunk.txt")),
        "after Shift+R uncommitted->commit amend, multi-hunk modified file should be fully committed\nstatus was:\n{status_after}"
    );
}

#[test]
fn rub_api_branch_to_commit_multi_hunk_modified_file_commits_all_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        "printf 'commit from tui test\n' > \"$1\"\n".to_string(),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let original = (1..=300)
        .map(|line| format!("line-{line}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    env.file("multi-hunk.txt", &original);

    let mut tui = test_tui(env);

    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter);
    });

    let modified = (1..=300)
        .map(|line| match line {
            1 => "line-1-modified".to_string(),
            250 => "line-250-modified".to_string(),
            _ => format!("line-{line}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    tui.env.file("multi-hunk.txt", &modified);

    for _ in 0..10 {
        tui.input_then_render(KeyCode::Up);
    }
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] multi-hunk.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] multi-hunk.txt"]);
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> g0 [A]"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊╭┄<< source >> << noop >> g0 [A]"]);
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> [..]"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..]"]);

    let status_after = tui.env.invoke_git("status --porcelain");
    assert!(
        !status_after
            .lines()
            .any(|line| line.ends_with("multi-hunk.txt")),
        "after Shift+R branch->commit amend, multi-hunk modified file should be fully committed\nstatus was:\n{status_after}"
    );
}
