use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};
use temp_env::with_var;

use crate::command::legacy::status::tui::tests::utils::test_tui;

const TEST_EDITOR_MESSAGE: &str = "commit from tui test";

#[test]
fn commit_mode_enter_and_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< commit to branch >> g0 [A]"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"])
        .assert_rendered_eq(file!["snapshots/commit_mode_enter_and_escape_final.txt"]);
}

#[test]
fn commit_confirm_on_source_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"])
        .assert_rendered_eq(file![
            "snapshots/commit_confirm_on_source_is_noop_final.txt"
        ]);
}

#[test]
fn commit_mode_not_entered_from_non_commitable_rows() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["┊●   9477ae7 add A"])
        .assert_rendered_eq(file![
            "snapshots/commit_mode_not_entered_from_non_commitable_rows_final.txt"
        ]);
}

#[test]
fn commit_from_unstaged_changes_creates_commit_visible_in_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< commit to branch >> g0 [A]"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"])
        .assert_rendered_eq(file![
            "snapshots/commit_from_unstaged_changes_creates_commit_visible_in_tui_final.txt"
        ]);
}

#[test]
fn commit_mode_shows_commit_above_on_commit_rows() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│   << insert commit above >>"])
        .assert_rendered_eq(file![
            "snapshots/commit_mode_shows_commit_above_on_commit_rows_final.txt"
        ]);
}

#[test]
fn commit_mode_can_toggle_commit_target_insert_side() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│   << insert commit above >>"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('a')
        .assert_current_line_eq(str!["┊│   << insert commit above >>"])
        .assert_rendered_eq(file![
            "snapshots/commit_mode_can_toggle_commit_target_insert_side_final.txt"
        ]);
}

#[test]
fn commit_to_commit_above_creates_commit_visible_in_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│   << insert commit above >>"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"])
        .assert_rendered_eq(file![
            "snapshots/commit_to_commit_above_creates_commit_visible_in_tui_final.txt"
        ]);
}

#[test]
fn commit_to_commit_below_creates_commit_visible_in_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│   << insert commit above >>"]);

    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"])
        .assert_rendered_eq(file![
            "snapshots/commit_to_commit_below_creates_commit_visible_in_tui_final.txt"
        ]);
}

#[test]
fn commit_mode_from_staged_changes_stays_within_current_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> [..] [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄[..] [A]"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊  ╭┄[..] [staged to A]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["┊  ╭┄<< source >> << noop >> [..] [staged to A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< commit to branch >> [..] [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│   << insert commit above >>"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│   << insert commit above >>"])
        .assert_rendered_eq(file![
            "snapshots/commit_mode_from_staged_changes_stays_within_current_stack_final.txt"
        ]);
}

#[test]
fn staged_source_commit_cannot_be_forced_to_other_stack_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    env.file(
        ".git/editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_tui(env);

    tui.env.file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A test.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> [..] A test.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< assign hunks >> [..] [A]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄[..] [A]"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊  ╭┄[..] [staged to A]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["┊  ╭┄<< source >> << noop >> [..] [staged to A]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│   << insert commit above >>"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"])
        .assert_rendered_eq(file![
            "snapshots/staged_source_commit_cannot_be_forced_to_other_stack_target_final.txt"
        ]);
}

#[test]
fn commit_from_unassigned_multi_hunk_modified_file_commits_all_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
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

    // First commit: add the file so subsequent edits are true modifications.
    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);
    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< commit to branch >> g0 [A]"]);
    with_var("GIT_EDITOR", Some(editor_command.clone()), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    // Turn the committed file into a multi-hunk modification.
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

    // Move cursor back up to the unassigned section.
    for _ in 0..10 {
        tui.input_then_render(KeyCode::Up);
    }
    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unstaged changes]"]);

    // Second commit: this reproduces the bug for modified multi-hunk files.
    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unstaged changes]"]);
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄<< commit to branch >> g0 [A]"]);
    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    let status_after = tui.env.invoke_git("status --porcelain");
    assert!(
        !status_after.lines().any(|line| line.ends_with("multi-hunk.txt")),
        "after commit from unassigned via TUI, multi-hunk modified file should be fully committed\nstatus was:\n{status_after}"
    );
}
