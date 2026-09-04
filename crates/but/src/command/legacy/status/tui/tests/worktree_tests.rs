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

/// `wt` is a strict prefix of its own area's `wt:@`, so typing it can never become the only
/// match; the ID typed out in full still jumps to the reference, and one more character reaches
/// the area.
#[test]
fn jump_to_a_worktree_reference_despite_its_area_extending_the_id() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input('/');
    tui.input("wt")
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);

    tui.input('/');
    tui.input("wt:")
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);
}

/// The same path dirty in the main worktree and a linked one is two rows, and the jump lands
/// on the linked worktree's rather than the main worktree's row above it.
#[test]
fn jump_to_a_file_that_is_also_dirty_in_the_main_worktree() {
    let (mut tui, _editor) = worktree_tui();
    tui.env().file("wt-file.txt", "main change");

    tui.reload();
    tui.input('/');
    tui.input("ok")
        .assert_current_line_eq(str!["┊┊┊   ok A wt-file.txt"]);
}

/// Both of the lane's headings, its uncommitted file and its commit are all reachable with the
/// cursor.
#[test]
fn worktree_lane_is_navigable() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊╭┄ wt:@ {worktree uncommitted}"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊┊   ok A wt-file.txt"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);
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

/// The two rows of a lane persist separately: they carry distinct remember keys, so a
/// collision between them would silently restore the wrong one.
#[test]
fn remember_selection_on_worktree_rows() {
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
        .assert_current_line_eq(str!["┊┊╭┄ wt:@ {worktree uncommitted}"]);
    tui.input('q');

    let mut tui = test_status_tui_with_options(tui.into_env(), options());
    tui.reload()
        .assert_current_line_eq(str!["┊┊╭┄ wt:@ {worktree uncommitted}"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);
    tui.input('q');

    let mut tui = test_status_tui_with_options(tui.into_env(), options());
    tui.reload()
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);
}

/// A worktree's uncommitted area names that worktree's changes the way `@` names the main
/// worktree's, so `c` on it offers those changes as a commit source - and marks itself a
/// no-op destination, exactly as `@` does.
#[test]
fn commit_source_from_a_worktree_area() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt:@ {worktree uncommitted}"]);

    tui.input('c')
        .assert_current_line_eq(str![
            "┊┊╭┄ << source >> << noop >> wt:@ {worktree uncommitted}"
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_source_from_a_worktree_area_final.svg"
        ]);
}

/// Committing every uncommitted change of a worktree at once: the area row is the source, and
/// the reference row two below it is the destination.
#[test]
fn commit_all_changes_of_a_worktree() {
    let (mut tui, editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt:@ {worktree uncommitted}"]);

    tui.input('c');
    // In commit mode the area's own file rows are not selectable, so one step down from the
    // area row reaches the reference row that receives the commit.
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);
    with_var("GIT_EDITOR", Some(editor), || {
        tui.input(KeyCode::Enter);
    });

    // The worktree's branch moved to the new commit and the stack tip stayed put.
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 27f2344 (wt-branch) commit from worktree
| * 998a235 add W
|/  
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

/// `c` on the reference row offers that worktree's own changes, the way `c` on a branch row
/// offers the main area's, so confirming right there commits everything in the worktree.
#[test]
fn commit_all_changes_of_a_worktree_from_its_reference() {
    let (mut tui, editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);

    tui.input('c')
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_all_changes_of_a_worktree_from_its_reference_001.svg"
        ]);
    with_var("GIT_EDITOR", Some(editor), || {
        tui.input(KeyCode::Enter);
    });

    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 27f2344 (wt-branch) commit from worktree
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

    // Down onto the worktree's reference row, which offers itself as the destination via the
    // `<< commit to worktree >>` extension line.
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"])
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
| * 27f2344 (wt-branch) commit from worktree
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
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);

    with_var("GIT_EDITOR", Some(editor), || {
        tui.input(KeyCode::Enter);
    });

    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * cdc9ff4 (wt-branch) commit from worktree
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

/// Marks spanning the main worktree and a linked worktree have no single source repository to
/// read from, so confirming such a commit is refused rather than mixing worktrees.
#[test]
fn marks_spanning_worktrees_are_refused() {
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
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);

    // The refusal shows as an error and nothing was committed.
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/marks_spanning_worktrees_are_refused.svg"]);
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

/// A detached worktree has no branch to move, so committing onto its reference row is refused
/// instead of silently committing somewhere else.
#[test]
fn commit_to_a_detached_worktree_reference_is_refused() {
    let (mut tui, _editor) = worktree_tui();

    but_testsupport::invoke_bash_at_dir(
        "git checkout -q --detach",
        &tui.env()
            .projects_root()
            .join(".git/gitbutler/test-worktrees/wt"),
    );

    // Detached, the reference row falls back to the worktree's name.
    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊╭┄ wt:@ {worktree uncommitted}"]);

    tui.input('c').assert_current_line_eq(str![
        "┊┊╭┄ << source >> << noop >> wt:@ {worktree uncommitted}"
    ]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊┊├┄ wt {wt}"]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/commit_to_a_detached_worktree_reference_is_refused.svg"
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

/// `n` on a worktree's reference row inserts an empty commit at the tip of that worktree's
/// branch, the way it does on a branch heading in the workspace.
#[test]
fn empty_commit_on_a_worktree_reference() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"]);

    tui.input('n')
        .assert_rendered_term_svg_eq(file![
            "snapshots/empty_commit_on_a_worktree_reference_final.svg"
        ])
        .assert_current_line_eq(str!["┊┊●   uxy (no commit message) (no changes)"]);
}

/// A worktree's reference row is a move target: confirming on it moves the commit to the tip
/// of the branch the worktree has checked out.
#[test]
fn move_commit_below_a_worktree_reference() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    // An empty commit moves without conflicts, so the move itself is all this test sees.
    tui.input('n')
        .assert_current_line_eq(str!["┊●   oun (no commit message) (no changes)"]);

    tui.input('m');
    // Past the worktree's own commit, onto its reference row.
    tui.input([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊┊├┄ wt {wt-branch}"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_commit_below_a_worktree_reference_001.svg"
        ]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_commit_below_a_worktree_reference_002.svg"
    ]);

    // The empty commit left the stack for the tip of the worktree's branch.
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* 6919fdf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 401b057 (wt-branch) 
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

/// A worktree's commit uncommits into that worktree's area, never the main one, and the cursor
/// lands on that area afterwards.
#[test]
fn uncommit_a_worktree_commit_into_its_own_area() {
    let (mut tui, _editor) = worktree_tui();

    tui.reload();
    tui.input([
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
    ])
    .assert_current_line_eq(str!["┊┊●   nll add W"]);
    tui.input('r');
    // Past the worktree's reference row and files to its own area.
    tui.input('k')
        .assert_current_line_eq(str!["┊┊╭┄ << uncommit >> wt:@ {worktree uncommitted}"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/uncommit_a_worktree_commit_into_its_own_area_001.svg"
        ]);
    // The main area is not a target for a worktree's commit, so the branch above is as far as
    // the cursor goes.
    tui.input('k')
        .assert_current_line_eq(str!["┊╭┄ << squash >> g0 [A]"]);
    tui.input('k')
        .assert_current_line_eq(str!["┊╭┄ << squash >> g0 [A]"]);

    tui.input('j')
        .assert_current_line_eq(str!["┊┊╭┄ << uncommit >> wt:@ {worktree uncommitted}"]);
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊┊╭┄ wt:@ {worktree uncommitted}"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/uncommit_a_worktree_commit_into_its_own_area_002.svg"
        ]);
    snapbox::assert_data_eq!(
        tui.env().git_log(),
        str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (wt-branch, A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}
