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

    tui.input('/')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_001.svg"]);
    // Non-matching lines are dimmed and the matched characters are underlined.
    tui.input("g0")
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_002.svg"]);
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_backstack_eq([]);

    // cycling through matches
    tui.input('/');
    tui.input("kl")
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_003.svg"]);
    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_004.svg"]);
    tui.input((KeyModifiers::CONTROL, 'p'))
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_005.svg"]);
    tui.input(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_006.svg"]);

    // jumping to @
    tui.input('/');
    tui.input('@');
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["╭┄ @ [uncommitted]"]);
}

/// Typing a line's only match moves the cursor there, but only Enter leaves jump mode.
#[test]
fn jump_waits_for_enter_to_confirm() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('/');
    tui.input("lrm")
        .assert_current_line_eq(str!["┊●   lrm add B"])
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode])
        .assert_rendered_term_svg_eq(file!["snapshots/jump_waits_for_enter_to_confirm_001.svg"]);
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   lrm add B"])
        .assert_backstack_eq([]);
}

#[test]
fn jump_to_branch_by_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('/');
    // The commit "add C" also matches, but the branch comes first.
    tui.input('C')
        .assert_rendered_term_svg_eq(file!["snapshots/jump_to_branch_by_name_001.svg"]);
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ h0 [C]"]);
}

#[test]
fn jump_to_commit_by_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('/');
    tui.input("add B")
        .assert_rendered_term_svg_eq(file!["snapshots/jump_to_commit_by_message_001.svg"]);
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   lrm add B"]);
}

#[test]
fn jump_to_file_by_path() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('/');
    tui.input("three")
        .assert_rendered_term_svg_eq(file!["snapshots/jump_to_file_by_path_001.svg"]);
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊   or   A three"]);
}

#[test]
fn jump_matches_fuzzily() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('/');
    tui.input("adC")
        .assert_rendered_term_svg_eq(file!["snapshots/jump_matches_fuzzily_001.svg"]);
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   xwn add C"]);
}

/// An exact short ID wins over earlier lines that only match fuzzily.
#[test]
fn jump_prefers_exact_short_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("kl", "");

    let mut tui = test_status_tui(env);

    tui.input('/');
    // The file `klx` above also contains an `x`.
    tui.input('x')
        .assert_current_line_eq(str!["┊●   xwn add C"])
        .assert_rendered_term_svg_eq(file!["snapshots/jump_prefers_exact_short_id_001.svg"]);
}

/// Without a match there's nothing to confirm, so Enter keeps jump mode open.
#[test]
fn jump_without_matches_cannot_be_confirmed() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('/');
    tui.input("qqq").assert_rendered_term_svg_eq(file![
        "snapshots/jump_without_matches_cannot_be_confirmed_001.svg"
    ]);
    tui.input(KeyCode::Enter)
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode]);
}

/// The first match is scrolled into view even when it's below the viewport.
#[test]
fn jump_scrolls_to_offscreen_match() {
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

    tui.input('/');
    tui.input("add B")
        .assert_current_line_eq(str!["┊●   lrm add B"])
        .assert_rendered_term_svg_eq(file!["snapshots/jump_scrolls_to_offscreen_match_001.svg"]);
}

#[test]
fn jump_to_merge_base_by_sha() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('/');
    tui.input("0dc3733");
    tui.input(KeyCode::Enter).assert_current_line_eq(str![
        "┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M"
    ]);
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
    tui.input('x');
    tui.input(KeyCode::Enter)
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
fn escape_puts_cursor_back_where_jump_started() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('j').assert_current_line_eq(str!["┊╭┄ g0 [A]"]);
    tui.input('/');
    tui.input("add B")
        .assert_current_line_eq(str!["┊●   lrm add B"]);
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"])
        .assert_backstack_eq([])
        .assert_rendered_term_svg_eq(file![
            "snapshots/escape_puts_cursor_back_where_jump_started_001.svg"
        ]);
}

/// Escape finds the starting line again after a reload removed the lines above it.
#[test]
fn escape_after_reload_puts_cursor_back_on_starting_line() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('/');
    tui.input("0dc3733");
    tui.input(KeyCode::Enter).assert_current_line_eq(str![
        "┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M"
    ]);
    tui.input('/');
    tui.input("g0").assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.env().remove_file("one");
    tui.env().remove_file("two");
    tui.env().remove_file("three");
    tui.reload();

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str![
            "┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M"
        ])
        .assert_rendered_term_svg_eq(file![
            "snapshots/escape_after_reload_puts_cursor_back_on_starting_line_001.svg"
        ]);
}

/// When a reload removed the starting line, Escape falls back like a reload does.
#[test]
fn escape_after_starting_line_is_removed() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("one", "");
    env.file("two", "");
    env.file("three", "");

    let mut tui = test_status_tui(env);

    tui.input('/');
    tui.input("three");
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊   or   A three"]);
    tui.input('/');
    tui.input("g0").assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    tui.env().remove_file("one");
    tui.env().remove_file("two");
    tui.env().remove_file("three");
    tui.reload();

    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["╭┄ @ [uncommitted] (no changes)"])
        .assert_backstack_eq([]);
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
    tui.input('x');
    tui.input(KeyCode::Enter)
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);
}

#[test]
fn highlights_matches_when_file_list_is_open() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input((KeyModifiers::SHIFT, 'F'));
    tui.input('/');
    tui.input('t').assert_rendered_term_svg_eq(file![
        "snapshots/highlights_matches_when_file_list_is_open_001.svg"
    ]);

    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/highlights_matches_when_file_list_is_open_002.svg"
        ]);

    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_rendered_term_svg_eq(file![
            "snapshots/highlights_matches_when_file_list_is_open_003.svg"
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
    tui.input("r");
    tui.input(KeyCode::Enter)
        .assert_current_line_eq(str!["┊╭┄ rr [rr-branch]"]);

    tui.input("g");
    tui.input("/");
    tui.input("rz");
    tui.input(KeyCode::Enter)
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

    // Escape puts the cursor back where it was before entering jump.
    tui.input('/');
    tui.input((KeyModifiers::CONTROL, 'n'));
    tui.input((KeyModifiers::CONTROL, 'n'))
        .assert_current_line_eq(str!["[..]lrm add B"]);
    tui.input(KeyCode::Esc)
        .assert_current_line_eq(str!["[..]xwn add C"])
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
    tui.input("zt:t");
    tui.input(KeyCode::Enter)
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

    // IDs outside the list cannot move the cursor, cycle, or confirm outside it.
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
    tui.input('@');
    tui.input(KeyCode::Enter)
        .assert_current_line_eq("╭┄ @ [uncommitted] (no changes)")
        .assert_backstack_eq([BackstackEntry::ShowFileList]);
}
