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

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/commit_mode_enter_and_escape_final.svg"]);
}

#[test]
fn commit_confirm_on_source_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_confirm_on_source_is_noop_final.svg"
        ]);
}

#[test]
fn commiting_with_no_unassigned_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│   << insert commit >>"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    tui.input_then_render('e')
        .assert_current_line_eq(str!["┊│ << insert commit (empty message) >>"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] (no commit message) (no changes)"])
        .assert_rendered_term_svg_eq(file!["snapshots/commiting_with_no_unassigned_changes.svg"]);
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

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_from_unstaged_changes_creates_commit_visible_in_tui_final.svg"
        ]);
}

#[test]
fn commit_from_unstaged_changes_to_new_branch_creates_branch_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render('b')
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_from_unstaged_changes_to_new_branch_creates_branch_and_commit_final.svg"
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

    tui.env().file("multi-hunk.txt", &base);

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

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
    tui.env().file("multi-hunk.txt", changed);

    tui.input_then_render(None);
    tui.input_then_render(std::array::repeat::<_, 20>(KeyCode::Up));
    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    let status = tui.env().invoke_git("status --porcelain");
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

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│   << insert commit >>"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_mode_shows_commit_above_on_commit_rows_final.svg"
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

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│   << insert commit >>"]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_to_commit_above_creates_commit_visible_in_tui_final.svg"
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

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│   << insert commit >>"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"]);
    });

    tui.input_then_render(None)
        .assert_current_line_eq(str!["┊●   [..] commit from tui test[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_to_commit_below_creates_commit_visible_in_tui_final.svg"
        ]);
}

#[test]
fn commit_mode_from_staged_changes_stays_within_current_stack() {
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

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes] (no changes)"]);

    tui.input_then_render('c').assert_current_line_eq(str![
        "╭┄<< source >> << noop >> zz [unassigned changes] (no changes)"
    ]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│   << insert commit >>"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│ << insert commit >>"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_mode_from_staged_changes_stays_within_current_stack_final.svg"
        ]);
}

#[test]
fn commit_with_inline_reword() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    tui.input_then_render(None)
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    tui.input_then_render('e')
        .assert_current_line_eq(str!["┊│ << insert commit (empty message) >>"]);

    tui.input_then_render('i')
        .assert_current_line_eq(str!["┊│ << insert commit (reword inline) >>"]);

    tui.input_then_render('i')
        .assert_current_line_eq(str!["┊│ << insert commit >>"]);

    tui.input_then_render('i')
        .assert_current_line_eq(str!["┊│ << insert commit (reword inline) >>"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..]"]);

    tui.input_then_render("commit message here")
        .assert_current_line_eq(str!["┊●   [..] commit message here"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] commit message here"]);
}

#[test]
fn commit_moved_file_from_unassigned_changes_line() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    // show files in commits
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')));

    // commit test.txt
    tui.env().file("test.txt", "content");
    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    tui.input_then_render('i');
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("add test.txt");
    tui.input_then_render(KeyCode::Enter);

    // go back to top to normalize inputs
    tui.input_then_render('g');

    // move the file
    tui.env().rename_file("test.txt", "moved-test.txt");

    // commit the moved file
    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    tui.input_then_render('i');
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("move test.txt to moved-test.txt");
    tui.input_then_render(KeyCode::Enter);

    // there should be no more changes to commit
    tui.input_then_render(None)
        .assert_rendered_contains("zz [unassigned changes] (no changes)");
}

#[test]
fn commit_moved_file_from_file_line() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    // show files in commits
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')));

    // commit test.txt
    tui.env().file("test.txt", "content");
    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    tui.input_then_render('i');
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("add test.txt");
    tui.input_then_render(KeyCode::Enter);

    // go back to top to normalize inputs
    tui.input_then_render('g');

    // move the file
    tui.env().rename_file("test.txt", "moved-test.txt");

    // commit the moved file via the file list, not [unassigned changes]
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str![["┊   [..] R moved-test.txt"]]);
    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    tui.input_then_render('i');
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("move test.txt to moved-test.txt");
    tui.input_then_render(KeyCode::Enter);

    // there should be no more changes to commit
    tui.input_then_render(None)
        .assert_rendered_contains("zz [unassigned changes] (no changes)");
}

#[test]
fn commit_moved_and_modified_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    // show files in commits
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')));

    // commit test.txt
    tui.env().file("test.txt", "");
    for _ in 0..100 {
        tui.env().append_file("test.txt", "content\n");
    }

    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    tui.input_then_render('i');
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("add test.txt");
    tui.input_then_render(KeyCode::Enter);

    // go back to top to normalize inputs
    tui.input_then_render('g');

    // move and modify the file
    tui.env().rename_file("test.txt", "moved-test.txt");
    tui.env().append_file("moved-test.txt", "new content\n");

    // commit the moved file
    tui.input_then_render('c');
    tui.input_then_render(KeyCode::Down);
    tui.input_then_render('i');
    tui.input_then_render(KeyCode::Enter);
    tui.input_then_render("move test.txt to moved-test.txt");
    tui.input_then_render(KeyCode::Enter);

    // there should be no more changes to commit
    tui.input_then_render(None)
        .assert_rendered_contains("zz [unassigned changes] (no changes)");
}

#[test]
fn cannot_select_unassigned_files_with_commits_marked() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    // mark the commit
    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render(' ')
        .assert_current_line_eq(str![["┊✔︎   [..] add A"]]);

    // moving up selects the branch
    tui.input_then_render('k')
        .assert_current_line_eq(str![["┊╭┄g0 [A]"]]);

    // cannot move further up, stays on the branch
    tui.input_then_render('k')
        .assert_current_line_eq(str![["┊╭┄g0 [A]"]]);
}

#[test]
fn cannot_select_committed_files_with_commits_marked() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    // mark the commit
    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render(' ')
        .assert_current_line_eq(str![["┊✔︎   [..] add A"]]);

    // cannot open the file list with marked commits
    tui.input_then_render('f')
        .assert_current_line_eq(str![["┊✔︎   [..] add A"]]);
}

#[test]
fn cannot_select_committed_files_from_global_listing_with_commits_marked() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.env().file("test.txt", "content");

    // mark the commit
    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render(' ')
        .assert_current_line_eq(str![["┊✔︎   [..] add A"]]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('F')))
        .assert_current_line_eq(str![["┊✔︎   [..] add A"]]);

    tui.input_then_render('j')
        .assert_current_line_eq(str![["┊✔︎   [..] add A"]])
        .assert_rendered_term_svg_eq(file!["snapshots/cannot_select_committed_files_from_global_listing_with_commits_marked_showing_global_file_list.svg"]);

    // the global file list can be closed with f
    tui.input_then_render('f')
        .assert_rendered_term_svg_eq(file!["snapshots/cannot_select_committed_files_from_global_listing_with_commits_marked_final.svg"]);
}
