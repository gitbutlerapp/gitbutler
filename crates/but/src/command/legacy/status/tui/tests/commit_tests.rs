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
fn commit_from_unstaged_changes_with_multiple_hunks_in_same_file_commits_all_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        ".git/editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let base = (1..=20)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    let mut tui = test_tui(env);

    tui.env.file("multi-hunk.txt", &base);

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

    let changed = base
        .lines()
        .enumerate()
        .map(|(idx, line)| match idx {
            1 => "line-2-modified".to_string(),
            17 => "line-18-modified".to_string(),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    tui.env.file("multi-hunk.txt", changed);

    tui.input_then_render(None);
    tui.input_then_render(std::array::repeat::<_, 20>(KeyCode::Up));
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

    let status = tui.env.invoke_git("status --porcelain");
    assert_eq!(
        status, "",
        "expected all zz changes to be committed, but worktree still has:\n{status}"
    );
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
