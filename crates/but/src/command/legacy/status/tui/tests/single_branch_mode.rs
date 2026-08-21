use but_testsupport::Sandbox;
use snapbox::file;

use crate::command::legacy::status::tui::tests::utils::test_status_tui;

#[test]
fn shows_single_branch_mode_label_in_hot_bar() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("single-branch-mode");

    let mut tui = test_status_tui(env);

    tui.reload().assert_rendered_term_svg_eq(file![
        "snapshots/shows_single_branch_mode_label_in_hot_bar_001.svg"
    ]);
}
