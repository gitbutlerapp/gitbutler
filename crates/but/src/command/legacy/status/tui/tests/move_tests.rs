use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::tests::utils::{
    TestTuiOptions, commit_with_two_hunks_in_file, test_status_tui, test_status_tui_with_options,
};

#[test]
fn esc_leaves_move_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊╭┄ << source >> << noop >> g0 [A]"]);

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/esc_leaves_move_mode_final.svg"]);
}

#[test]
fn move_mode_keeps_selected_commit_and_extension_visible_when_scrolled() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 100,
            height: 6,
            ..Default::default()
        },
    );

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   oun (no commit message) (no changes)"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   mul (no commit message) (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input(KeyCode::Up)
        .assert_rendered_contains("<< move commit above >>")
        .assert_rendered_contains("(no commit message) (no changes)")
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);
}

#[test]
fn move_commit_above_other_commit_reorders_tui() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   oun (no commit message) (no changes)"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   mul (no commit message) (no changes)"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> tpm add A"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    tui = tui.recreate();
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/move_commit_above_other_commit_reorders_tui_final.svg"
    ]);
}

#[test]
fn move_commit_down_from_source_selects_next_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   oun (no commit message) (no changes)"]);

    tui.input('n')
        .assert_current_line_eq(str!["┊●   mul (no commit message) (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   oun (no commit message) (no changes)"]);

    tui.input('m').assert_current_line_eq(str![
        "┊●   << source >> << noop >> oun (no commit message) (no changes)"
    ]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);
}

#[test]
fn move_commit_up_from_top_commit_selects_source_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊╭┄ h0 [C]"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   xwn add C"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> xwn add C"]);

    tui.input(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄ h0 [C]"])
        .assert_rendered_contains("<< move commit to branch >>");
}

#[test]
fn move_branch_onto_other_branch_reorders_stacks() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"]);

    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊╭┄ << source >> << noop >> g0 [A]"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊│  << stack branch >>"]);

    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊├┄ h0 [A]"]);

    tui = tui.recreate();
    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_onto_other_branch_reorders_stacks_final.svg"
    ]);
}

#[test]
fn move_branch_to_merge_base_tears_off_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "C", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input('m').assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_to_merge_base_tears_off_branch_001.svg"
    ]);
    tui.input('j');
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_to_merge_base_tears_off_branch_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_branch_to_merge_base_tears_off_branch_003.svg"
    ]);
}

#[test]
fn move_commit_to_merge_base_creates_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   ywx add second"]);

    tui.input('m')
        .assert_current_line_eq(str!["┊●   << source >> << noop >> ywx add second"]);

    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_commit_to_merge_base_creates_new_branch_001.svg"
        ]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_commit_to_merge_base_creates_new_branch_final.svg"
    ]);
}

#[test]
fn move_marked_commits_to_merge_base_creates_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input(' ');
    tui.input(KeyCode::Down);
    tui.input(' ');
    tui.input('m');

    tui.input(KeyCode::Down).assert_rendered_term_svg_eq(file![
        "snapshots/move_marked_commits_to_merge_base_creates_new_branch_001.svg"
    ]);

    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_marked_commits_to_merge_base_creates_new_branch_final.svg"
    ]);
}

#[test]
fn moving_multiple_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('b');
    tui.input('n');
    tui.input('g')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_001.svg"]);

    tui.input('j');
    tui.input('j');
    tui.input('j');
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_002.svg"]);

    tui.input('j');
    tui.input(' ')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_003.svg"]);

    tui.input('k');
    tui.input('m');
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_004.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_multiple_commits_005.svg"]);
}

#[test]
fn move_commit_to_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/move_commit_to_new_branch_001.svg"]);
    tui.input('k');
    tui.input('b')
        .assert_rendered_term_svg_eq(file!["snapshots/move_commit_to_new_branch_002.svg"]);
}

#[test]
fn move_multiple_commits_to_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input(' ');
    tui.input('j');
    tui.input(' ').assert_rendered_term_svg_eq(file![
        "snapshots/move_multiple_commits_to_new_branch_001.svg"
    ]);
    tui.input('m');
    tui.input('k');
    tui.input('k');
    tui.input('k').assert_rendered_term_svg_eq(file![
        "snapshots/move_multiple_commits_to_new_branch_002.svg"
    ]);
    tui.input('b').assert_rendered_term_svg_eq(file![
        "snapshots/move_multiple_commits_to_new_branch_003.svg"
    ]);
}

#[test]
fn committed_file_roundtrips_through_squash_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-files");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input('m').assert_rendered_term_svg_eq(file![
        "snapshots/committed_file_roundtrips_through_squash_mode_001.svg"
    ]);
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/committed_file_roundtrips_through_squash_mode_002.svg"
    ]);
    tui.input('m');
    tui.input('k')
        .assert_current_line_eq(str!["┊│   << move file above >>[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/committed_file_roundtrips_through_squash_mode_003.svg"
        ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/committed_file_roundtrips_through_squash_mode_004.svg"
    ]);
}

#[test]
fn marked_committed_files_roundtrip_through_squash_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-files");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input(' ');
    tui.input(' ');
    tui.input('m').assert_rendered_term_svg_eq(file![
        "snapshots/marked_committed_files_roundtrip_through_squash_mode_001.svg"
    ]);
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/marked_committed_files_roundtrip_through_squash_mode_002.svg"
    ]);
    tui.input('m');
    tui.input('k');
    tui.input('k')
        .assert_current_line_eq(str!["┊│   << move files above >>[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marked_committed_files_roundtrip_through_squash_mode_003.svg"
        ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/marked_committed_files_roundtrip_through_squash_mode_004.svg"
    ]);
}

#[test]
fn move_committed_files_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/move_committed_files_to_commit_001.svg"]);
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/move_committed_files_to_commit_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/move_committed_files_to_commit_003.svg"]);
}

#[test]
fn move_committed_files_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/move_committed_files_to_branch_001.svg"]);
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/move_committed_files_to_branch_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/move_committed_files_to_branch_003.svg"]);
}

#[test]
fn move_committed_files_to_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input('m').assert_rendered_term_svg_eq(file![
        "snapshots/move_committed_files_to_new_branch_001.svg"
    ]);
    tui.input('k');
    tui.input('k').assert_rendered_term_svg_eq(file![
        "snapshots/move_committed_files_to_new_branch_002.svg"
    ]);
    tui.input('b').assert_rendered_term_svg_eq(file![
        "snapshots/move_committed_files_to_new_branch_003.svg"
    ]);
}

#[test]
fn move_committed_files_to_unstacked_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input('m').assert_rendered_term_svg_eq(file![
        "snapshots/move_committed_files_to_unstacked_branch_001.svg"
    ]);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/move_committed_files_to_unstacked_branch_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_committed_files_to_unstacked_branch_003.svg"
    ]);
}

#[test]
fn move_two_files_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-files");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input(' ');
    tui.input(' ');
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_commit_001.svg"]);
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_commit_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_commit_003.svg"]);
}

#[test]
fn move_two_files_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-files");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input(' ');
    tui.input(' ');
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_branch_001.svg"]);
    tui.input('k');
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_branch_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_branch_003.svg"]);
}

#[test]
fn move_two_files_to_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-files");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input(' ');
    tui.input(' ');
    tui.input('m')
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_new_branch_001.svg"]);
    tui.input('k');
    tui.input('k');
    tui.input('k')
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_new_branch_002.svg"]);
    tui.input('b')
        .assert_rendered_term_svg_eq(file!["snapshots/move_two_files_to_new_branch_003.svg"]);
}

#[test]
fn move_two_files_to_unstacked_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-files");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);

    tui.input('d');
    tui.input('j');
    tui.input('j');
    tui.input('f');
    tui.input(' ');
    tui.input(' ');
    tui.input('m').assert_rendered_term_svg_eq(file![
        "snapshots/move_two_files_to_unstacked_branch_001.svg"
    ]);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/move_two_files_to_unstacked_branch_002.svg"
    ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_two_files_to_unstacked_branch_003.svg"
    ]);
}

#[test]
fn move_marked_committed_hunks_above_commit() {
    let mut tui = commit_with_two_hunks_in_file();

    // mark the bottom hunk of `file` and the only hunk of `other`
    tui.input('d');
    tui.input('l');
    tui.input('j');
    tui.input('j');
    tui.input(' ');
    tui.input(' ');

    tui.input('m').assert_rendered_term_svg_eq(file![
        "snapshots/move_marked_committed_hunks_above_commit_001.svg"
    ]);
    tui.input('j')
        .assert_current_line_eq(str!["┊│   << move hunks above >>[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunks_above_commit_002.svg"
        ]);

    // the marked hunks move to a new commit between the two commits
    tui.input(KeyCode::Enter)
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunks_above_commit_003.svg"
        ]);

    // only the top hunk of `file` is left in the source commit
    tui.input('k').assert_rendered_term_svg_eq(file![
        "snapshots/move_marked_committed_hunks_above_commit_004.svg"
    ]);
}

#[test]
fn move_marked_committed_hunks_above_their_commit() {
    let mut tui = commit_with_two_hunks_in_file();

    // mark the bottom hunk of `file` and the only hunk of `other`
    tui.input('d');
    tui.input('l');
    tui.input('j');
    tui.input('j');
    tui.input(' ');
    tui.input(' ');

    // moving the hunks above their own commit splits them off into a new commit
    tui.input('m')
        .assert_current_line_eq(str!["┊│   << move hunks above >>[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunks_above_their_commit_001.svg"
        ]);
    tui.input(KeyCode::Enter)
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunks_above_their_commit_002.svg"
        ]);
    tui.input('j').assert_rendered_term_svg_eq(file![
        "snapshots/move_marked_committed_hunks_above_their_commit_003.svg"
    ]);
}

#[test]
fn move_marked_committed_hunks_to_branch() {
    let mut tui = commit_with_two_hunks_in_file();

    // commit `third` to a new branch stacked on top to move to
    tui.input('k');
    tui.input('b');
    tui.input('n')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-2] (no commits)"]);
    tui.env().file("third", "third\n");
    tui.reload();
    tui.input('g');
    tui.input('c');
    tui.input('e');
    tui.input('j')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-2] (no commits)"]);
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   uxx (no commit message)"]);

    // mark the bottom hunk of `file` and the only hunk of `other`
    tui.input('j');
    tui.input('j')
        .assert_current_line_eq(str!["┊●   qxn (no commit message)"]);
    tui.input('d');
    tui.input('l');
    tui.input('j');
    tui.input('j');
    tui.input(' ');
    tui.input(' ');

    tui.input('m');
    tui.input('k');
    tui.input('k');
    tui.input('k')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-2][..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunks_to_branch_001.svg"
        ]);

    // the marked hunks move to a new commit at the tip of the other branch
    tui.input(KeyCode::Enter)
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunks_to_branch_002.svg"
        ]);
}

#[test]
fn move_marked_committed_hunks_to_new_branch_above_branch() {
    let mut tui = commit_with_two_hunks_in_file();

    // mark the bottom hunk of `file` and the only hunk of `other`
    tui.input('d');
    tui.input('l');
    tui.input('j');
    tui.input('j');
    tui.input(' ');
    tui.input(' ');

    tui.input('m');
    tui.input('k')
        .assert_current_line_eq(str!["┊╭┄ br [c-branch-1][..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunks_to_new_branch_above_branch_001.svg"
        ]);

    // the marked hunks move to a new commit on a new branch stacked on top
    tui.input('b')
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunks_to_new_branch_above_branch_002.svg"
        ]);
}

#[test]
fn move_marked_committed_hunk_to_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "line\n");
    env.file("other", "line\n");

    let mut tui = test_status_tui(env);

    // commit both files, then mark the hunk that adds `other`
    tui.input('c');
    tui.input('e');
    tui.input('b')
        .assert_current_line_eq(str!["┊●   pwz (no commit message)"]);
    tui.input('d');
    tui.input('l');
    tui.input('j');
    tui.input('j');
    tui.input(' ');

    tui.input('m');
    tui.input('j');
    tui.input('j')
        .assert_current_line_eq(str!["│ << move hunk to new branch >>[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunk_to_new_branch_001.svg"
        ]);

    // the marked hunk moves to a new commit on a new branch of its own
    tui.input(KeyCode::Enter)
        .assert_marks_count_eq(0)
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_marked_committed_hunk_to_new_branch_002.svg"
        ]);
}

#[test]
fn move_committed_hunk_under_cursor_from_details() {
    let mut tui = commit_with_two_hunks_in_file();

    // select the bottom hunk of `file` without marking it
    tui.input('d');
    tui.input('l');
    tui.input('j');
    tui.input('j');

    tui.input('m');
    tui.input('j')
        .assert_current_line_eq(str!["┊│   << move hunk above >>[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/move_committed_hunk_under_cursor_from_details_001.svg"
        ]);

    // only the selected hunk moves to a new commit between the two commits
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/move_committed_hunk_under_cursor_from_details_002.svg"
    ]);
    tui.input('k').assert_rendered_term_svg_eq(file![
        "snapshots/move_committed_hunk_under_cursor_from_details_003.svg"
    ]);
}

#[test]
fn marked_committed_hunks_roundtrip_through_squash_mode() {
    let mut tui = commit_with_two_hunks_in_file();

    // mark the bottom hunk of `file` and the only hunk of `other`
    tui.input('d');
    tui.input('l');
    tui.input('j');
    tui.input('j');
    tui.input(' ');
    tui.input(' ');

    tui.input('m');
    tui.input('r').assert_rendered_term_svg_eq(file![
        "snapshots/marked_committed_hunks_roundtrip_through_squash_mode_001.svg"
    ]);
    tui.input('m');
    tui.input('j')
        .assert_current_line_eq(str!["┊│   << move hunks above >>[..]"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marked_committed_hunks_roundtrip_through_squash_mode_002.svg"
        ]);
    tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
        "snapshots/marked_committed_hunks_roundtrip_through_squash_mode_003.svg"
    ]);
}
