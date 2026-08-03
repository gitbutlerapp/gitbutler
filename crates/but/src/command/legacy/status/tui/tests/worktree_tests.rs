use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{IntoData as _, str};
use temp_env::with_var;

use crate::command::legacy::status::tui::App;
use crate::command::legacy::status::tui::tests::utils::{
    TestTuiOptions, test_status_tui_with_options,
};
use crate::tui::test_utils::TestTui;

const TEST_EDITOR_MESSAGE: &str = "commit from worktree";

/// A workspace with one stack, plus a linked worktree that branched off that stack's commit,
/// has a commit of its own, and an uncommitted change on top.
fn worktree_tui() -> (TestTui<App>, String) {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings_slow("one-stack-with-worktree");
    env.setup_metadata(&["A"]);
    // Outside the repository, or the script itself shows up as an uncommitted change.
    let editor_script = env.app_data_dir().join("editor.sh");
    std::fs::write(
        &editor_script,
        format!("printf '{TEST_EDITOR_MESSAGE}\\n' > \"$1\"\n"),
    )
    .expect("app data dir is writable");
    let editor_command = format!("sh {}", editor_script.display());
    let tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            worktree_manipulation: true,
            ..Default::default()
        },
    );
    (tui, editor_command)
}

/// The lane, its heading, its uncommitted file and its commit are all reachable with the cursor.
#[test]
fn worktree_lane_is_navigable() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊╭┄ v {wt-branch}"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊┊   ok A wt-file.txt"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊●   nll add W"]);
}

/// A worktree heading names that checkout's uncommitted area, the way `zz` names the main
/// worktree's, so `c` on it offers those changes as a commit source.
#[test]
fn commit_source_from_a_worktree_heading() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ v {wt-branch}"]);

    tui.input('c')
        .assert_current_line_eq(str![
            "┊┊╭┄ << source >> << commit to worktree >> v {wt-branch}"
        ])
        .assert_rendered_contains("  commit  ");
}

/// Committing every uncommitted change of a worktree at once, from its heading, which is both
/// the source and the destination.
#[test]
fn commit_all_changes_of_a_worktree() {
    let (mut tui, editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ v {wt-branch}"]);

    tui.input('c');
    with_var("GIT_EDITOR", Some(editor), || {
        tui.input(KeyCode::Enter);
    });

    // The worktree's branch moved to the new commit and the stack tip stayed put.
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 352ea1d (wt-branch) commit from worktree
| * 998a235 add W
|/  
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

/// A worktree's own lane is a commit destination, so a single file in it can be committed onto
/// the branch that worktree has checked out.
#[test]
fn commit_one_worktree_file_onto_its_own_branch() {
    let (mut tui, editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊┊   ok A wt-file.txt"]);

    tui.input('c')
        .assert_current_line_eq(str!["┊┊┊   << source >> << noop >> ok A wt-file.txt"]);

    // Up onto the worktree's own lane heading, which offers itself as the destination.
    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊┊╭┄ v {wt-branch}"])
        .assert_rendered_contains("<< commit to worktree >>");

    with_var("GIT_EDITOR", Some(editor), || {
        tui.input(KeyCode::Enter);
    });

    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 352ea1d (wt-branch) commit from worktree
| * 998a235 add W
|/  
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}
