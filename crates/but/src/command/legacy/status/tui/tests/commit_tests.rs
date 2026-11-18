use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};
use temp_env::with_var;

use crate::{
    command::legacy::status::tui::tests::utils::test_status_tui,
    tui::test_utils::{Control, Shift},
};

const TEST_EDITOR_MESSAGE: &str = "commit from tui test";

#[test]
fn commit_mode_enter_and_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ << commit to new branch above >>"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/commit_mode_enter_and_escape_final.svg"]);
}

#[test]
fn commit_confirm_on_source_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_confirm_on_source_is_noop_final.svg"
        ]);
}

#[test]
fn commiting_with_no_uncommitted_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('c').assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commiting_with_no_uncommitted_changes_001.svg"
        ]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('e')
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commiting_with_no_uncommitted_changes_002.svg"
        ]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   oun (no commit message) (no changes)"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commiting_with_no_uncommitted_changes_003.svg"
        ]);
}

#[test]
fn commit_from_unstaged_changes_creates_commit_visible_in_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   oqn commit from tui test"]);
    });

    tui.reload()
        .assert_current_line_eq(str!["┊●   oqn commit from tui test"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_from_unstaged_changes_creates_commit_visible_in_tui_final.svg"
        ]);
}

#[test]
fn commit_from_unstaged_changes_to_new_branch_creates_branch_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ << commit to new branch above >>"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   oqn commit from tui test"]);
    });

    tui.reload()
        .assert_current_line_eq(str!["┊●   oqn commit from tui test"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_from_unstaged_changes_to_new_branch_creates_branch_and_commit_final.svg"
        ]);
}

#[test]
fn commit_from_unstaged_changes_to_new_branch_enters_workspace_in_single_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.invoke_git("checkout A");
    env.file("test.txt", "content");

    let mut tui = test_status_tui(env);

    tui.reload();

    tui.input('c');
    tui.input('e');
    tui.input(Shift('g'));
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commit_from_unstaged_changes_to_new_branch_enters_workspace_in_single_branch_mode_final.svg"
    ]);

    // HEAD enters the workspace, with the empty-message commit on a branch independent of A.
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
*   e4771af (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
| * 9477ae7 (A) add A
* | 80b9bc6 (c-branch-1) 
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn commit_from_unstaged_changes_with_multiple_hunks_in_same_file_commits_all_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

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

    let mut tui = test_status_tui(env);

    tui.env().file("multi-hunk.txt", &base);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    with_var("GIT_EDITOR", Some(editor_command.clone()), || {
        tui.input(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   kmy commit from tui test"]);
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

    tui.reload();
    tui.input(std::array::repeat::<_, 20>(KeyCode::Up));
    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   mwu commit from tui test"]);
    });

    let status = tui.env().invoke_git("status --porcelain");
    assert_eq!(
        status, "",
        "expected all @ changes to be committed, but worktree still has:\n{status}"
    );
}

#[test]
fn commit_mode_shows_commit_below_on_commit_rows() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_mode_shows_commit_below_on_commit_rows_final.svg"
        ]);
}

#[test]
fn commit_to_new_branch_below_stacks_it_under_the_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input('e');

    // Past the branch tip and the stop right below A's only commit is the new branch below A.
    tui.input(['j', 'j', 'j', 'j'])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_to_new_branch_below_stacks_it_under_the_branch_001.svg"
        ]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commit_to_new_branch_below_stacks_it_under_the_branch_002.svg"
    ]);
}

#[test]
fn commit_to_commit_below_creates_commit_visible_in_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(
        "editor.sh",
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    );
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_to_commit_below_creates_commit_visible_in_tui_001.svg"
        ])
        .assert_rendered_contains("┊│   << commit below >>");

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   zym commit from tui test"]);
    });

    tui.reload()
        .assert_current_line_eq(str!["┊●   zym commit from tui test"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_to_commit_below_creates_commit_visible_in_tui_final.svg"
        ]);
}

#[test]
fn commit_mode_from_staged_changes_stays_within_current_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   vo A test.txt"]);

    tui.input('r')
        .assert_current_line_eq(str!["┊   << source >> vo A test.txt"]);

    tui.input(KeyCode::Down);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   << amend >> tpm add A"]);

    tui.input('u');

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input('c').assert_current_line_eq(str![
        "╭┄ << source >> << noop >> @ [uncommitted] (no changes)"
    ]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ << commit to new branch above >>"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_mode_from_staged_changes_stays_within_current_stack_001.svg"
        ]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   tpm add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_mode_from_staged_changes_stays_within_current_stack_final.svg"
        ]);
}

#[test]
fn commit_with_inline_reword() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('e')
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/commit_with_inline_reword_001.svg"]);

    tui.input('i')
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/commit_with_inline_reword_002.svg"]);

    tui.input('i')
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/commit_with_inline_reword_003.svg"]);

    tui.input('i')
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/commit_with_inline_reword_004.svg"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   nsl"]);

    tui.input("commit message here")
        .assert_current_line_eq(str!["┊●   nsl commit message here"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   nsl commit message here"]);
}

#[test]
fn commit_moved_file_from_uncommitted_changes_line() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    // show files in commits
    tui.input((KeyModifiers::SHIFT, 'F'));

    // commit test.txt
    tui.env().file("test.txt", "content");
    tui.reload();
    tui.input('c');
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("add test.txt");
    tui.input(KeyCode::Enter);

    // go back to top to normalize inputs
    tui.input('g');

    // move the file
    tui.env().rename_file("test.txt", "moved-test.txt");
    tui.reload();

    // commit the moved file
    tui.input('c');
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("move test.txt to moved-test.txt");
    tui.input(KeyCode::Enter);

    // there should be no more changes to commit
    tui.reload()
        .assert_rendered_contains("@ [uncommitted] (no changes)");
}

#[test]
fn commit_moved_file_from_file_line() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    // show files in commits
    tui.input((KeyModifiers::SHIFT, 'F'));

    // commit test.txt
    tui.env().file("test.txt", "content");
    tui.reload();
    tui.input('c');
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("add test.txt");
    tui.input(KeyCode::Enter);

    // go back to top to normalize inputs
    tui.input('g');

    // move the file
    tui.env().rename_file("test.txt", "moved-test.txt");
    tui.reload();

    // commit the moved file via the file list, not [uncommitted]
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   yw R moved-test.txt"]);
    tui.input('c');
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("move test.txt to moved-test.txt");
    tui.input(KeyCode::Enter);

    // there should be no more changes to commit
    tui.reload()
        .assert_rendered_contains("@ [uncommitted] (no changes)");
}

#[test]
fn commit_moved_and_modified_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    // show files in commits
    tui.input((KeyModifiers::SHIFT, 'F'));

    // commit test.txt
    tui.env().file("test.txt", "");
    for _ in 0..100 {
        tui.env().append_file("test.txt", "content\n");
    }

    tui.reload();
    tui.input('c');
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("add test.txt");
    tui.input(KeyCode::Enter);

    // go back to top to normalize inputs
    tui.input('g');

    // move and modify the file
    tui.env().rename_file("test.txt", "moved-test.txt");
    tui.env().append_file("moved-test.txt", "new content\n");
    tui.reload();

    // commit the moved file
    tui.input('c');
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("move test.txt to moved-test.txt");
    tui.input(KeyCode::Enter);

    // there should be no more changes to commit
    tui.reload()
        .assert_rendered_contains("@ [uncommitted] (no changes)");
}

#[test]
fn cannot_select_uncommitted_files_with_commits_marked() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    // mark the commit
    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input(' ')
        .assert_current_line_eq(str!["┊✔︎   tpm add A"]);

    // cannot move futher up since the branch and files aren't selectable
    tui.input('k')
        .assert_current_line_eq(str!["┊✔︎   tpm add A"]);
}

#[test]
fn cannot_select_committed_files_with_commits_marked() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    // mark the commit
    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input(' ')
        .assert_current_line_eq(str!["┊✔︎   tpm add A"]);

    // cannot open the file list with marked commits
    tui.input('f')
        .assert_current_line_eq(str!["┊✔︎   tpm add A"]);
}

#[test]
fn cannot_select_committed_files_from_global_listing_with_commits_marked() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    // mark the commit
    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input(' ')
        .assert_current_line_eq(str!["┊✔︎   tpm add A"]);

    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_current_line_eq(str!["┊✔︎   tpm add A"]);

    tui.input('j')
        .assert_current_line_eq(str!["┊✔︎   tpm add A"])
        .assert_rendered_term_svg_eq(file!["snapshots/cannot_select_committed_files_from_global_listing_with_commits_marked_showing_global_file_list.svg"]);

    // the global file list can be closed with f
    tui.input('f').assert_rendered_term_svg_eq(file![
        "snapshots/cannot_select_committed_files_from_global_listing_with_commits_marked_final.svg"
    ]);
}

#[test]
fn escape_from_commit_mode_preserves_marks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("one", "content");
    tui.env().file("two", "content");
    tui.reload();

    tui.input('j');
    tui.input(' ').assert_rendered_contains("✔︎");

    tui.input('c').assert_rendered_contains("✔︎");

    tui.input(KeyCode::Esc).assert_rendered_contains("✔︎");
}

#[test]
fn mark_and_commit_multiple_uncommitted_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("one", "content");
    tui.env().file("two", "content");
    tui.env().file("three", "content");

    tui.reload();

    tui.input('j');
    tui.input(' ');
    tui.input(' ');
    tui.input('c').assert_rendered_term_svg_eq(file![
        "snapshots/mark_and_commit_multiple_uncommitted_files_001.svg"
    ]);

    tui.input(['j', 'j']);
    tui.input('e');
    tui.input(KeyCode::Enter);
    tui.input((KeyModifiers::SHIFT, 'F'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/mark_and_commit_multiple_uncommitted_files_final.svg"
        ]);
}

#[test]
fn committing_steps_through_every_stop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.input('c');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_steps_through_every_stop_001.svg"
    ]);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_steps_through_every_stop_002.svg"
    ]);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_steps_through_every_stop_003.svg"
    ]);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_steps_through_every_stop_004.svg"
    ]);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_steps_through_every_stop_005.svg"
    ]);

    // The new stack at the merge base is the last stop.
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_steps_through_every_stop_005.svg"
    ]);

    tui.input('k').assert_rendered_term_svg_eq(file![
        "snapshots/committing_steps_through_every_stop_004.svg"
    ]);
}

#[test]
fn committing_steps_over_merged_branches() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata(&["A", "B"]);
    env.set_target_sha("refs/heads/base");

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");
    tui.reload();

    // A is merged upstream, so the first stop is above B.
    tui.input('c');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/committing_steps_over_merged_branches_001.svg"
    ]);
}

#[test]
fn a_reload_keeps_the_stop_stepped_to() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");
    tui.reload();

    tui.input('c');
    tui.input('j');

    // Another changed file moves A's line down, and the new branch above A stays selected.
    tui.env().file("other.txt", "content");
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/a_reload_keeps_the_stop_stepped_to_001.svg"
    ]);
}

#[test]
fn picking_the_branch_the_cursor_is_on_commits_to_it() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");
    tui.reload();

    // Stepped to the new branch above A, picking A lands on A itself.
    tui.input('c');
    tui.input('j');
    tui.input('t');
    tui.input(Control('n'));
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/picking_the_branch_the_cursor_is_on_commits_to_it_001.svg"
    ]);
}

#[test]
fn commit_to_new_branch_from_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui(env);

    tui.env().file("test.txt", "content");

    tui.reload();

    tui.input('c');
    tui.input('e');
    tui.input(Shift('g'));
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commit_to_new_branch_from_uncommitted_001.svg"
    ]);
}
