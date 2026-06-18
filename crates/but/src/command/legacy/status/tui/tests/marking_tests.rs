use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::{file, str};

use crate::command::legacy::status::tui::{BackstackEntry, tests::utils::test_tui};

#[test]
fn marking_individual_commit_toggles_mark_indicator() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(' ')
        .assert_current_line_eq(str!["┊✔︎   9477ae7 add A"]);

    tui.input_then_render(' ')
        .assert_current_line_eq(str!["┊●   [..] add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_individual_commit_toggles_mark_indicator_final.svg"
        ]);
}

#[test]
fn marking_branch_toggles_all_commits_in_that_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(' ')
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊✔︎   9477ae7 add A"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/marking_branch_toggles_all_commits_in_that_branch_final.svg"
        ]);
}

#[test]
fn marking_unassigned_toggles_all_unassigned_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.file("a.txt", "content");
    env.file("b.txt", "content");

    let mut tui = test_tui(env);

    tui.reload()
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(' ')
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊✔︎  [..] A a.txt"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊✔︎  [..] A b.txt"]);

    tui.input_then_render('g');
    tui.input_then_render(' ');

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊   [..] A a.txt"]);
}

#[test]
fn multi_squash_marked_commits_into_selected_marked_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   [..] add A"]);

    tui.input_then_render(' ')
        .assert_current_line_eq(str!["┊✔︎   9477ae7 add A"]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_current_line_eq(str!["┊╭┄h0 [B]"]);

    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊●   [..] add B"]);

    tui.input_then_render(' ')
        .assert_current_line_eq(str!["┊✔︎   d3e2ba3 add B"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊✔︎   << source >> << squash >> d3e2ba3 add B"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   [..] add B"])
        .assert_rendered_term_svg_eq(file![
            "snapshots/multi_squash_marked_commits_into_selected_marked_target_final.svg"
        ]);
}

#[test]
fn marks_still_show_in_split_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    env.file("one", "content of one");
    env.file("two", "content of two");

    let mut tui = test_tui(env);

    // mark some things
    tui.input_then_render('j');
    tui.input_then_render(' ')
        .assert_rendered_contains("┊✔︎    kl A one")
        .assert_rendered_contains("┊   twop A two")
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_001.svg"]);

    // open details view and still see the marks
    tui.input_then_render('d')
        .assert_rendered_contains("+content of two")
        .assert_rendered_contains("┊✔︎    kl A one")
        .assert_rendered_contains("┊   twop A two")
        .assert_backstack_eq([BackstackEntry::OpenSplitDetailsView, BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_002.svg"]);
    tui.input_then_render('d')
        .assert_backstack_eq([BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_003.svg"]);

    // opening and focusing details should still show marks
    tui.input_then_render('l')
        .assert_rendered_contains("details")
        .assert_rendered_contains("+content of two")
        .assert_rendered_contains("┊✔︎    kl A one")
        .assert_rendered_contains("┊   twop A two")
        .assert_backstack_eq([
            BackstackEntry::LeaveNormalMode,
            BackstackEntry::OpenSplitDetailsView,
            BackstackEntry::Mark,
        ])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_004.svg"]);

    // going back to normal mode should retain marks and keep details open
    tui.input_then_render('h')
        .assert_rendered_contains("normal")
        .assert_rendered_contains("+content of two")
        .assert_rendered_contains("┊✔︎    kl A one")
        .assert_rendered_contains("┊   twop A two")
        .assert_backstack_eq([BackstackEntry::OpenSplitDetailsView, BackstackEntry::Mark])
        .assert_rendered_term_svg_eq(file!["snapshots/marks_still_show_in_split_details_005.svg"]);
}
