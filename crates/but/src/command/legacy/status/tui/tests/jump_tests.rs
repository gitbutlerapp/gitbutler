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

    // jumping straight to the matching line
    tui.input('/')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_001.svg"]);
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

    // jumping to zz
    tui.input('/');
    tui.input('z')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_008.svg"]);
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
