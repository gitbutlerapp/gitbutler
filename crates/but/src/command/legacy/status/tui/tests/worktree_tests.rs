use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{IntoData as _, str};

use crate::command::legacy::status::tui::App;
use crate::command::legacy::status::tui::tests::utils::{
    TestTuiOptions, test_status_tui_with_options,
};
use crate::tui::test_utils::TestTui;

/// A workspace with one stack, plus a linked worktree branched off that stack's commit that
/// has an uncommitted change of its own.
fn worktree_tui() -> TestTui<App> {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings_slow("one-stack-with-worktree");
    env.setup_metadata(&["A"]);
    test_status_tui_with_options(
        env,
        TestTuiOptions {
            worktree_manipulation: true,
            ..Default::default()
        },
    )
}

/// The lane, its heading and its uncommitted file are all reachable with the cursor.
#[test]
fn worktree_lane_is_navigable() {
    let mut tui = worktree_tui();

    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊╭┄ v {wt-branch}"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊┊   ok A wt-file.txt"]);
}

/// A worktree heading names that checkout's uncommitted area, the way `zz` names the main
/// worktree's, so `c` on it offers those changes as a commit source.
#[test]
fn commit_source_from_a_worktree_heading() {
    let mut tui = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ v {wt-branch}"]);

    tui.input('c')
        .assert_current_line_eq(str!["┊┊╭┄ << source >> << noop >> v {wt-branch}"])
        .assert_rendered_contains("  commit  ");
}

/// GAP: a worktree's uncommitted file can be picked as a commit source, but the worktree's own
/// lane is not a selectable destination, so there is nowhere to put it. Moving up from the file
/// skips the `{wt-branch}` heading entirely and lands on the workspace stack, which means the
/// only offer is to commit the worktree's changes into the workspace.
#[test]
fn a_worktree_file_cannot_be_committed_onto_its_own_branch() {
    let mut tui = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊┊   ok A wt-file.txt"]);

    // The file is accepted as the source, so the source half works.
    tui.input('c')
        .assert_current_line_eq(str!["┊┊┊   << source >> << noop >> ok A wt-file.txt"]);

    // Moving towards the top of the worktree's lane jumps clean over its heading and out into
    // the workspace stack: `{wt-branch}` never becomes a destination.
    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_contains("<< commit to branch >>");

    // Nothing was committed to the worktree's branch.
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (wt-branch, A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );
}
