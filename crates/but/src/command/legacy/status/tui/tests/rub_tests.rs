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

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str![
            "┊●   << amend >> f184fc7 (no commit message) (no changes)"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   6bdd3d2 (no commit message)"])
        .assert_rendered_term_svg_eq(file!["snapshots/rub_api_unassigned_to_commit.svg"]);
}

#[test]
fn rub_api_unassigned_to_commit_preserves_global_file_list() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
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
        .assert_current_line_eq(str!["┊●   8474410 add A"])
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

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

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

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> d3e2ba3 add B"]);
}

#[test]
fn mark_and_rub_multiple_uncommitted_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("one", "content");
    tui.env().file("two", "content");
    tui.env().file("three", "content");

    tui.reload();

    tui.input_then_render('j');
    tui.input_then_render(' ');
    tui.input_then_render(' ');

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   91784b3 add A"]);

    let status = tui.env().invoke_git("status --porcelain");
    assert_eq!(
        status, "?? two",
        "expected only unmarked file to remain uncommitted after rubbing marked files"
    );
}

// Ensure rub mode does not offer branch destinations.
#[test]
fn rub_api_cannot_rub_into_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 9477ae7 add A"]);

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

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);
}

// Tests RubMessage::StartReverse with unassigned source when stack has no assigned changes.
#[test]
fn rub_api_reverse_rub_uses_unassigned_source_when_stack_has_no_assigned_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('R')))
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

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

    tui.reload()
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

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 9477ae7 add A"]);

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

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 9477ae7 add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> d3e2ba3 add B"]);
}

#[test]
fn rub_api_squash_commits_toggles_message_strategy_labels() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 9477ae7 add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> d3e2ba3 add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('T')))
        .assert_current_line_eq(str!["┊●   << squash (use this message) >> d3e2ba3 add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('T')))
        .assert_current_line_eq(str!["┊●   << squash >> d3e2ba3 add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('S')))
        .assert_current_line_eq(str![
            "┊●   << squash (discard this message) >> d3e2ba3 add B"
        ]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('S')))
        .assert_current_line_eq(str!["┊●   << squash >> d3e2ba3 add B"]);
}

#[test]
fn rub_api_squash_commits_can_keep_target_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 9477ae7 add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> d3e2ba3 add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('T')))
        .assert_current_line_eq(str!["┊●   << squash (use this message) >> d3e2ba3 add B"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   4788772 add B"]);
}

#[test]
fn rub_api_squash_commits_can_keep_source_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> 9477ae7 add A"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << squash >> d3e2ba3 add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('S')))
        .assert_current_line_eq(str![
            "┊●   << squash (discard this message) >> d3e2ba3 add B"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   75eb901 add A"]);
}

// Tests RubOperation::CommittedFileToCommit.
#[test]
fn rub_api_committed_file_to_commit_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊│     << source >> << noop >> 94:tm A A"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊●   << move file >> 9477ae7 add A"]);
}

// Tests RubOperation::CommittedFileToUnassigned.
#[test]
fn rub_api_committed_file_to_unassigned_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│     94:tm A A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊│     << source >> << noop >> 94:tm A A"]);

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

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   nk A a.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> nk A a.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   55d8e41 add A"]);
}

// Tests RubOperation::UncommittedToStack.
#[test]
fn rub_api_uncommitted_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("a.txt", "content");
    tui.env().file("z.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   nk A a.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> nk A a.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   55d8e41 add A"]);
}

// Tests RubOperation::StackToUnassigned.
#[test]
fn rub_api_stack_to_unassigned_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> vo A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   8474410 add A"]);
}

// Tests RubOperation::StackToStack.
#[test]
fn rub_api_stack_to_stack_operation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("A", "content");
    tui.env().file("B", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   tm M A"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> tm M A"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   056a77b add A"]);

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
        .assert_current_line_eq(str!["┊   pl M B"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> pl M B"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> d3e2ba3 add B"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   7f2e16d add B"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('K')))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);
}

#[test]
fn rub_multiple_commits_into_unassigned() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("A", "content");
    tui.env().file("B", "content");
    tui.reload();

    tui.input_then_render('j');
    tui.input_then_render('c');
    tui.input_then_render('e');
    tui.input_then_render('b');

    tui.input_then_render('g');
    tui.input_then_render('j');
    tui.input_then_render('c');
    tui.input_then_render('e');
    tui.input_then_render('j');
    tui.input_then_render(KeyCode::Enter);

    tui.input_then_render(' ');
    tui.input_then_render(' ');

    tui.input_then_render('r');
    tui.input_then_render('g')
        .assert_rendered_term_svg_eq(file![
            "snapshots/rub_multiple_commits_into_unassigned_001.svg"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file![
            "snapshots/rub_multiple_commits_into_unassigned_final.svg"
        ]);
}
