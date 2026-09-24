use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};
use snapbox::{file, str};

use crate::command::legacy::status::tui::{BackstackEntry, tests::test_status_tui};

#[test]
fn jumping_around() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");
    env.file("kl", "");

    let mut tui = test_status_tui(env);

    // Unique next keys (such as g) have green backgrounds; shared prefixes (such as k) do not.
    tui.input('/')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_001.svg"]);
    tui.input('g').assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input('/');
    tui.input("h0")
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_002.svg"]);

    // cycling through matches
    tui.input('/');
    tui.input("kl")
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_003.svg"]);
    tui.input((KeyModifiers::CONTROL, 'p'))
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_004.svg"]);
    tui.input((KeyModifiers::CONTROL, 'p'))
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_005.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_006.svg"]);

    // jumping to @
    tui.input('/');
    tui.input('@')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_008.svg"]);
}

/// Offscreen matches must prevent a visible hint from promising an immediate jump.
#[test]
fn jump_hints_include_offscreen_matches() {
    use super::utils::{TestTuiOptions, test_status_tui_with_options};

    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);
    env.file("one", "");
    env.file("two", "");
    env.file("three", "");
    env.file("kl", "");
    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            height: 8,
            ..Default::default()
        },
    );

    // The file twop is visible, but the commit tpm is below the viewport.
    tui.input('/').assert_rendered_term_svg_eq(file![
        "snapshots/jump_hints_include_offscreen_matches_001.svg"
    ]);
    // After t, w will immediately select the file, even though t alone would not.
    tui.input('t').assert_rendered_term_svg_eq(file![
        "snapshots/jump_hints_include_offscreen_matches_002.svg"
    ]);
    tui.input('p')
        .assert_current_line_eq(str!["┊●   tpm add A"]);
}

#[test]
fn jump_to_merge_base_by_commit_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let merge_base_id = env
        .open_repo()
        .rev_parse_single("origin/main")
        .unwrap()
        .detach()
        .to_string();
    let mut tui = test_status_tui(env);

    tui.input('/');
    for _ in 0..6 {
        tui.input((KeyModifiers::CONTROL, 'n'));
    }
    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_current_line_eq(str!["[..] (common base) [..]"]);
    tui.input(KeyCode::Esc);

    tui.input('/');
    tui.input(&merge_base_id[..12])
        .assert_current_line_eq(str!["[..] (common base) [..]"]);
}

#[test]
fn jump_from_other_modes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("one", "");

    let mut tui = test_status_tui(env);

    tui.input('r')
        .assert_rendered_term_svg_eq(file!["snapshots/jump_from_other_modes_001.svg"]);
    tui.input('/')
        .assert_rendered_term_svg_eq(file!["snapshots/jump_from_other_modes_002.svg"]);
    tui.input('x')
        .assert_rendered_term_svg_eq(file!["snapshots/jump_from_other_modes_003.svg"]);
}

#[test]
fn clears_backstack_on_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('/')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode]);
    tui.input(KeyCode::Esc).assert_backstack_eq([]);
}

#[test]
fn restores_backstack_from_previous_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("one", "");

    let mut tui = test_status_tui(env);

    tui.input(' ');
    tui.input('r')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);

    tui.input('/')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);
    tui.input(KeyCode::Esc)
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);

    tui.input('/')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);
    tui.input('x')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);
}

#[test]
fn highlights_exact_matches_when_file_list_is_open() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'F'));
    tui.input('/');
    tui.input('t').assert_rendered_term_svg_eq(file![
        "snapshots/highlights_exact_matches_when_file_list_is_open_001.svg"
    ]);

    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/highlights_exact_matches_when_file_list_is_open_002.svg"
        ]);

    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/highlights_exact_matches_when_file_list_is_open_003.svg"
        ]);
}

#[test]
fn when_branch_short_code_and_commit_change_id_have_same_initial_character() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "branch-short-code-matches-commit-change-id",
    );
    env.setup_metadata(&["rr-branch"]);

    let mut tui = test_status_tui(env);

    tui.input("g");
    tui.input("/");
    tui.input("r").assert_rendered_term_svg_eq(file![
        "snapshots/when_branch_short_code_matches_commit_sha_without_change_id_001.svg"
    ]);
    tui.input("r")
        .assert_current_line_eq(str!["┊╭┄ rr [rr-branch]"]);

    tui.input("g");
    tui.input("/");
    tui.input("r");
    tui.input("z")
        .assert_current_line_eq(str!["┊●   rzr add branch 814"]);
}

#[test]
fn cancelling_jump_during_file_squash_does_not_move_cursor() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);
    let mut tui = test_status_tui(env);

    tui.input("jjf");
    tui.input('r');
    tui.input("jj")
        .assert_current_line_eq(str!["[..]xwn add C"]);
    tui.input('/');
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["[..]xwn add C"])
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::ShowFileList,
        ]);

    // Escape keeps the current cursor, not the position from before entering jump.
    tui.input('/');
    tui.input((KeyModifiers::CONTROL, 'n'));
    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_current_line_eq(str!["[..]lrm add B"]);
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["[..]lrm add B"])
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::ShowFileList,
        ]);

    tui.input(None).assert_rendered_term_svg_eq(file![
        "snapshots/cancelling_jump_during_file_squash_does_not_move_cursor_001.svg"
    ]);

    // Cancelling the operation itself still returns to the scoped file list.
    tui.input(KeyCode::Esc)
        .assert_current_line_eq("┊│     t:t A A")
        .assert_backstack_eq([BackstackEntry::ShowFileList]);
}

#[test]
fn jumping_in_file_lists() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('c');
    tui.input('e');
    tui.input('b');
    tui.input('f');
    tui.input('/').assert_backstack_eq([
        BackstackEntry::LeaveNormalMode,
        BackstackEntry::ShowFileList,
    ]);
    tui.input("zt:t")
        .assert_current_line_eq("┊│     zt:t A two")
        .assert_backstack_eq([BackstackEntry::ShowFileList]);

    // An empty query can cycle through files, but neither boundary leaves the list.
    tui.input('/')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_in_file_lists_001.svg"]);
    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_current_line_eq("┊│     zt:t A two");
    tui.input((KeyModifiers::CONTROL, 'p'))
        .assert_current_line_eq("┊│     zt:o A three");
    tui.input((KeyModifiers::CONTROL, 'p'))
        .assert_current_line_eq("┊│     zt:k A one");
    tui.input((KeyModifiers::CONTROL, 'p'))
        .assert_current_line_eq("┊│     zt:k A one");
    tui.input(KeyCode::Enter)
        .assert_current_line_eq("┊│     zt:k A one")
        .assert_backstack_eq([BackstackEntry::ShowFileList]);

    // The commit's own ID is also a prefix of its files: Enter must select a file.
    tui.input('/');
    tui.input("zt")
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_in_file_lists_002.svg"]);
    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_current_line_eq("┊│     zt:o A three");
    tui.input(KeyCode::Enter)
        .assert_current_line_eq("┊│     zt:o A three")
        .assert_backstack_eq([BackstackEntry::ShowFileList]);

    // IDs outside the list cannot auto-jump, cycle, or confirm outside it.
    for query in ["br", "@", "0dc3733"] {
        tui.input('/');
        tui.input(query)
            .assert_current_line_eq("┊│     zt:o A three");
        tui.input((KeyModifiers::CONTROL, 'n'))
            .assert_current_line_eq("┊│     zt:o A three");
        tui.input((KeyModifiers::CONTROL, 'p'))
            .assert_current_line_eq("┊│     zt:o A three");
        tui.input(KeyCode::Enter).assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::ShowFileList,
        ]);
        tui.input(KeyCode::Esc)
            .assert_current_line_eq("┊│     zt:o A three")
            .assert_backstack_eq([BackstackEntry::ShowFileList]);
    }
    tui.input(None)
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_in_file_lists_003.svg"]);

    // Closing the scoped list and showing all files restores global jump targets.
    tui.input((KeyModifiers::SHIFT, 'F'));
    tui.input((KeyModifiers::SHIFT, 'F'));
    tui.input('/');
    tui.input('@')
        .assert_current_line_eq("╭┄ @ [uncommitted] (no changes)")
        .assert_backstack_eq([BackstackEntry::ShowFileList]);
}
