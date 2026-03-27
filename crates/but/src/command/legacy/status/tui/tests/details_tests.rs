use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};
use snapbox::file;
use temp_env::with_var;

use crate::command::legacy::status::tui::tests::utils::{test_tui, test_tui_with_size};

#[test]
fn toggle_details_view_for_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_details_view_for_commit_001.term.svg"
        ]);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_details_view_for_commit_002.term.svg"
        ]);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_details_view_for_commit_003.term.svg"
        ]);
}

#[test]
fn details_view_updates_with_selection_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_updates_with_selection_changes_001.term.svg"
        ]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_updates_with_selection_changes_002.term.svg"
        ]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_updates_with_selection_changes_003.term.svg"
        ]);
}

#[test]
fn details_view_supports_scroll_controls() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let file_contents = (1..=120)
        .map(|line| format!("line-{line:03}\n"))
        .collect::<String>();
    env.file("large.txt", file_contents);

    let mut tui = test_tui_with_size(env, 100, 10);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_001.term.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('n')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_002.term.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('d')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_003.term.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('u')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_004.term.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('p')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_005.term.svg"
        ]);
}

#[test]
fn commit_message_wraps_in_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 80, 14);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_message_wraps_in_details_view_001.term.svg"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_message_wraps_in_details_view_002.term.svg"
        ]);

    tui.input_then_render(" this commit message is intentionally long so the details pane has to wrap the text across multiple visual lines")
        .assert_rendered_term_svg_eq(file!["snapshots/commit_message_wraps_in_details_view_003.term.svg"]);

    with_var("GIT_AUTHOR_DATE", Some("2000-01-01T00:00:00Z"), || {
        with_var("GIT_COMMITTER_DATE", Some("2000-01-01T00:00:00Z"), || {
            tui.input_then_render(KeyCode::Enter);
        });
    });

    tui.input_then_render('d');

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('n')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_message_wraps_in_details_view_005.term.svg"
        ]);
}

#[test]
fn details_view_renders_multiple_hunks_and_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let first_file = (1..=8)
        .map(|line| format!("alpha-{line}\n"))
        .collect::<String>();
    let second_file = (1..=8)
        .map(|line| format!("beta-{line}\n"))
        .collect::<String>();

    env.file("alpha.txt", first_file);
    env.file("beta.txt", second_file);

    let mut tui = test_tui_with_size(env, 100, 18);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_renders_multiple_hunks_and_files_001.term.svg"
        ]);
}

#[test]
fn toggling_details_off_and_on_resets_scroll_position() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();
    let file_contents = (1..=80)
        .map(|line| format!("line-{line:03}\n"))
        .collect::<String>();
    env.file("large.txt", file_contents);

    let mut tui = test_tui_with_size(env, 100, 10);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_001.term.svg"
        ]);

    tui.input_then_render((KeyModifiers::CONTROL, KeyCode::Char('d')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_002.term.svg"
        ]);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_003.term.svg"
        ]);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_004.term.svg"
        ]);
}
