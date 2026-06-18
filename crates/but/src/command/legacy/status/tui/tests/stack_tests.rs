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

#[test]
fn moving_stacks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    let mut tui = test_tui(env);

    for name in ["one", "two", "three"] {
        tui.input_then_render('g');
        tui.input_then_render('b');
        tui.input_then_render(KeyCode::Enter);
        for _ in 0..100 {
            tui.input_then_render(KeyCode::Backspace);
        }
        tui.input_then_render(name);
        tui.input_then_render(KeyCode::Enter);
        tui.input_then_render('g');
    }

    tui.reload()
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_001.svg"]);

    tui.input_then_render('j');
    tui.input_then_render('s');
    tui.input_then_render('m')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_002.svg"]);
    tui.input_then_render('j')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_003.svg"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_004.svg"]);

    tui.input_then_render('s');
    tui.input_then_render('m');
    tui.input_then_render('k')
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_005.svg"]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/moving_stacks_006.svg"]);
}
