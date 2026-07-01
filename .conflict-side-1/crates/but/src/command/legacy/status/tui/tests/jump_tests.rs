use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};
use snapbox::file;

use crate::command::legacy::status::tui::{BackstackEntry, tests::test_tui};

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

    let mut tui = test_tui(env);

    // jumping straight to the matching line
    tui.input_then_render('/')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_001.svg"]);
    tui.input_then_render("h0")
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_002.svg"]);

    // cycling through matches
    tui.input_then_render('/');
    tui.input_then_render("kl")
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_003.svg"]);
    tui.input_then_render((KeyModifiers::CONTROL, 'p'))
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_004.svg"]);
    tui.input_then_render((KeyModifiers::CONTROL, 'p'))
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_005.svg"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_006.svg"]);

    // jumping to zz
    tui.input_then_render('/');
    tui.input_then_render('z')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_007.svg"]);
    tui.input_then_render('z')
        .assert_rendered_term_svg_eq(file!["snapshots/jumping_around_008.svg"]);
}

#[test]
fn jump_from_other_modes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("one", "");

    let mut tui = test_tui(env);

    tui.input_then_render('r')
        .assert_rendered_term_svg_eq(file!["snapshots/jump_from_other_modes_001.svg"]);
    tui.input_then_render('/')
        .assert_rendered_term_svg_eq(file!["snapshots/jump_from_other_modes_002.svg"]);
    tui.input_then_render("38")
        .assert_rendered_term_svg_eq(file!["snapshots/jump_from_other_modes_003.svg"]);
}

#[test]
fn clears_backstack_on_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_tui(env);

    tui.input_then_render('/')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode]);
    tui.input_then_render(KeyCode::Esc).assert_backstack_eq([]);
}

#[test]
fn restores_backstack_from_previous_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B"]);

    env.file("one", "");

    let mut tui = test_tui(env);

    tui.input_then_render(' ');
    tui.input_then_render('r')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);

    tui.input_then_render('/')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);
    tui.input_then_render(KeyCode::Esc)
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);

    tui.input_then_render('/')
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);
    tui.input_then_render("38")
        .assert_backstack_eq([BackstackEntry::LeaveNormalMode, BackstackEntry::Mark]);
}
