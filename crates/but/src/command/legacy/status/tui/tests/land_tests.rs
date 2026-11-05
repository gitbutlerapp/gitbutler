use but_testsupport::Sandbox;

use crate::{
    command::legacy::status::tui::tests::utils::{
        TestTuiOptions, test_status_tui, test_status_tui_with_options,
    },
    tui::test_utils::Shift,
};

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
        .assert_rendered_contains("This lands 2 branches — A, B — directly onto")
        .assert_rendered_contains("Land A, B onto");
    tui.input('n')
        .assert_rendered_not_contains("Land A, B onto")
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
        .assert_rendered_contains("This lands A directly onto")
        .assert_rendered_contains("Land A onto");
}

/// A confirmation taller than the terminal scrolls instead of clipping what the user confirms.
#[test]
fn long_land_confirmation_scrolls() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            width: 60,
            height: 9,
            ..Default::default()
        },
    );

    tui.input('g');
    tui.input(Shift('j'));
    tui.input('b');
    tui.input(Shift('l'))
        .assert_rendered_contains("This lands A")
        .assert_rendered_contains("more line(s)")
        .assert_rendered_not_contains("Land A onto");
    for _ in 0..20 {
        tui.input('j');
    }
    tui.input('j')
        .assert_rendered_contains("Land A onto")
        .assert_rendered_not_contains("This lands A");
}
