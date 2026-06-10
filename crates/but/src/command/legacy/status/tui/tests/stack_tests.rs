use but_testsupport::Sandbox;
use crossterm::event::*;
use snapbox::file;

use crate::command::legacy::status::tui::tests::utils::test_tui;

#[test]
fn unapply_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('j');
    tui.input_then_render('b');

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('G')));
    tui.input_then_render('b');

    tui.input_then_render('g')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_001.svg"]);

    tui.input_then_render('s')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_002.svg"]);

    tui.input_then_render('j')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_003.svg"]);

    tui.input_then_render('k');
    tui.input_then_render('u')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_004.svg"]);

    tui.input_then_render('y')
        .assert_rendered_term_svg_eq(file!["snapshots/unapply_stack_005.svg"]);
}

#[test]
fn enter_stack_mode_from_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('j');
    tui.input_then_render('j');
    tui.input_then_render('s')
        .assert_rendered_term_svg_eq(file!["snapshots/enter_stack_mode_from_commits_001.svg"]);
}
