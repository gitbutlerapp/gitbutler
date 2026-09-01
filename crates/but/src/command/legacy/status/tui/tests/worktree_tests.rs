use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{IntoData as _, file, str};
use temp_env::with_var;

use crate::command::legacy::status::tui::tests::utils::{
    TestTuiOptions, test_status_tui_with_options,
};
use crate::command::legacy::status::{TuiLaunchOptions, tui::App};
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

/// Sibling worktrees nested below a dirty worktree's first commit keep a blank lane between them.
#[test]
fn sibling_worktree_lanes_are_separated_after_uncommitted_files() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings_slow("one-stack-with-worktree");
    env.setup_metadata(&["A"]);
    env.invoke_git(
        "worktree add -b wt-child-one .git/gitbutler/test-worktrees/zz-child-one wt-branch",
    );
    env.invoke_git(
        "worktree add -b wt-child-two .git/gitbutler/test-worktrees/zz-child-two wt-branch",
    );
    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            worktree_manipulation: true,
            ..Default::default()
        },
    );

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/sibling_worktree_lanes_are_separated_after_uncommitted_files_001.svg"
    ]);
}

/// The lane, its heading, its uncommitted file and its commit are all reachable with the cursor.
#[test]
fn worktree_lane_is_navigable() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊┊   ok A wt-file.txt"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊●   nll add W"])
        .assert_rendered_term_svg_eq(file!["snapshots/worktree_lane_is_navigable_final.svg"]);
}

#[test]
fn stack_highlighting_with_a_nested_worktree_lane() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input('s').assert_rendered_term_svg_eq(file![
        "snapshots/stack_highlighting_with_a_nested_worktree_lane_001.svg"
    ]);
}

#[test]
fn remember_selection_on_worktree_heading() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings_slow("one-stack-with-worktree");
    env.setup_metadata(&["A"]);
    let launch_options = TuiLaunchOptions {
        remember_selection: true,
        ..Default::default()
    };
    let options = || TestTuiOptions {
        launch_options: launch_options.clone(),
        worktree_manipulation: true,
        ..Default::default()
    };
    let mut tui = test_status_tui_with_options(env, options());

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"]);
    tui.input('q');

    let mut tui = test_status_tui_with_options(tui.into_env(), options());
    tui.reload()
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"]);
}

/// A worktree heading names that checkout's uncommitted area, the way `@` names the main
/// worktree's, so `c` on it offers those changes as a commit source.
#[test]
fn commit_source_from_a_worktree_heading() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"]);

    // The heading claims the source inline; its extension line advertises the destination.
    tui.input('c')
        .assert_current_line_eq(str!["┊┊╭┄ << source >> wt {wt-branch}"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_source_from_a_worktree_heading_final.svg"
        ]);
}

/// Committing every uncommitted change of a worktree at once, from its heading, which is both
/// the source and the destination.
#[test]
fn commit_all_changes_of_a_worktree() {
    let (mut tui, editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"]);

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

    // Up onto the worktree's own lane heading, which offers itself as the destination via the
    // `<< commit to worktree >>` extension line.
    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_one_worktree_file_onto_its_own_branch_001.svg"
        ]);

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

/// The heading accepts changes from other checkouts too: a main-worktree change committed onto
/// it leaves the main uncommitted area and lands on the branch the worktree has checked out.
#[test]
fn commit_a_main_worktree_change_onto_a_worktree() {
    let (mut tui, editor) = worktree_tui();

    tui.env().file("main-file.txt", "content");
    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    tui.input('c')
        .assert_current_line_eq(str!["╭┄ << source >> << noop >> @ [uncommitted]"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"]);

    with_var("GIT_EDITOR", Some(editor), || {
        tui.input(KeyCode::Enter);
    });

    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 9fa1815 (wt-branch) commit from worktree
| * 998a235 add W
|/  
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
    // The change left the main checkout for the worktree's branch, so only the worktree's own
    // dirty file remains anywhere.
    snapbox::assert_data_eq!(tui.env().git_status(), str![""]);
}

/// Marks spanning the main checkout and a linked worktree have no single source repository to
/// read from, so confirming such a commit is refused rather than mixing checkouts.
#[test]
fn marks_spanning_checkouts_are_refused() {
    let (mut tui, _editor) = worktree_tui();

    tui.env().file("main-file.txt", "content");
    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);

    // Mark the main worktree's file and the linked worktree's file.
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   nu A main-file.txt"]);
    tui.input(' ');
    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊┊   ok A wt-file.txt"]);
    tui.input(' ');

    tui.input('c');
    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"]);

    // The refusal shows as an error and nothing was committed.
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/marks_spanning_checkouts_are_refused.svg"]);
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 998a235 (wt-branch) add W
|/  
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );
}

/// A detached worktree has no branch to move, so committing onto its heading is refused
/// instead of silently committing somewhere else.
#[test]
fn commit_to_a_detached_worktree_heading_is_refused() {
    let (mut tui, _editor) = worktree_tui();

    but_testsupport::invoke_bash_at_dir(
        "git checkout -q --detach",
        &tui.env()
            .projects_root()
            .join(".git/gitbutler/test-worktrees/wt"),
    );

    // Detached, the heading falls back to the worktree's name.
    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt}"]);

    tui.input('c')
        .assert_current_line_eq(str!["┊┊╭┄ << source >> wt {wt}"]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commit_to_a_detached_worktree_heading_is_refused.svg"
    ]);
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 998a235 (wt-branch) add W
|/  
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );
}

/// `n` on a worktree heading inserts an empty commit at the tip of the worktree's branch, the
/// way it does on a branch heading in the workspace.
#[test]
fn empty_commit_on_a_worktree_heading() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"]);

    tui.input('n')
        .assert_rendered_term_svg_eq(file![
            "snapshots/empty_commit_on_a_worktree_heading_final.svg"
        ])
        .assert_current_line_eq(str!["┊┊●   1 (no commit message) (no changes)"]);
}

/// A worktree heading is a move target: confirming on it moves the commit to the tip of the
/// branch the worktree has checked out.
#[test]
fn move_commit_below_a_worktree_heading() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    // An empty commit moves without conflicts, so the move itself is all this test sees.
    tui.input('n')
        .assert_current_line_eq(str!["┊●   1 (no commit message) (no changes)"]);

    tui.input('m');
    // Past the worktree's own commit, onto its heading.
    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊┊╭┄ wt {wt-branch}"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_commit_below_a_worktree_heading_001.svg"
        ]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_commit_below_a_worktree_heading_002.svg"
    ]);

    // The empty commit left the stack for the tip of the worktree's branch.
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* 6919fdf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 1ce4908 (wt-branch) 
| * 20da4fb add W
|/  
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

#[test]
fn cherry_picking_commits_into_worktrees() {
    let (mut tui, _editor) = worktree_tui();

    tui.input('b');
    tui.input('n');
    tui.input('n');

    tui.input('p');
    tui.input('j');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/cherry_picking_commits_into_worktrees_001.svg"
    ]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/cherry_picking_commits_into_worktrees_002.svg"
    ]);
}
