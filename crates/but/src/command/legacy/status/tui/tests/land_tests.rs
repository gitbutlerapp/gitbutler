use but_testsupport::Sandbox;

use crate::{command::legacy::status::tui::tests::utils::test_status_tui, tui::test_utils::Shift};

/// Landing from branch mode with several branches marked asks for one confirmation naming every
/// branch, and cancelling it publishes nothing. The land itself is covered by the `but merge` tests.
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
        .assert_rendered_contains("Land A, B onto origin/main?");
    tui.input('n')
        .assert_rendered_not_contains("Land A, B onto origin/main?")
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
        .assert_rendered_contains("Land A onto origin/main?");
}
