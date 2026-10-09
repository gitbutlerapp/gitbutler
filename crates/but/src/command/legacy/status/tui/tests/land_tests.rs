use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use snapbox::{file, str};

use crate::{command::legacy::status::tui::tests::utils::test_status_tui, tui::test_utils::Shift};

/// Landing from branch mode with several branches marked asks for one confirmation with a tickbox
/// per stack, and cancelling it publishes nothing. The land itself is covered by the `but merge` tests.
#[test]
fn marking_multiple_branches_and_cancelling_land() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let target_before = env.invoke_git("rev-parse origin/main");

    let mut tui = test_status_tui(env);

    tui.input('g');
    tui.input(Shift('j'));
    tui.input(' ');
    tui.input(Shift('j'));
    tui.input(' ');
    tui.input('b');
    tui.input(Shift('l'))
        .assert_rendered_contains("This lands 2 branches — A, B — directly onto origin/main")
        .assert_rendered_contains("[x] A")
        .assert_rendered_contains("[x] B");
    tui.input('n')
        .assert_rendered_not_contains("Land onto origin/main:")
        .assert_rendered_contains("[A]")
        .assert_rendered_contains("[B]");

    assert_eq!(
        tui.env().invoke_git("rev-parse origin/main"),
        target_before,
        "a cancelled land must not move the target"
    );
}

/// With nothing marked, land targets the branch under the cursor.
#[test]
fn land_without_marks_uses_selected_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui(env);

    tui.input('g');
    tui.input(Shift('j'));
    tui.input('b');
    tui.input(Shift('l'))
        .assert_rendered_contains("This lands A directly onto origin/main")
        .assert_rendered_contains("[x] A");
}

/// Each stack is a row listing what lands with it bottom-up: a branch stacked on another lands
/// together with it, while the bottom of another stack lands on its own. Unticking a row leaves
/// that stack out.
#[test]
fn land_has_a_tickbox_per_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "C"]);
    let target_before = env.invoke_git("rev-parse origin/main");

    let mut tui = test_status_tui(env);

    tui.input('g');
    tui.input(Shift('j'));
    tui.input(' ');
    tui.input(Shift('j'));
    tui.input('k').assert_current_line_eq(str!["┊╭┄ h0 [C]"]);
    tui.input(' ');
    tui.input('b');
    tui.input(Shift('l'))
        .assert_rendered_term_svg_eq(file!["snapshots/land_has_a_tickbox_per_stack_001.svg"]);

    tui.input([KeyCode::Down, KeyCode::Char(' ')])
        .assert_rendered_term_svg_eq(file!["snapshots/land_has_a_tickbox_per_stack_002.svg"]);

    tui.input(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file!["snapshots/land_has_a_tickbox_per_stack_003.svg"]);

    assert_eq!(
        tui.env().invoke_git("rev-parse origin/main"),
        target_before,
        "a cancelled land must not move the target"
    );
}
