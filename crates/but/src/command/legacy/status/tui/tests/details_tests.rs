use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};
use snapbox::{file, str};
use temp_env::with_var;

use crate::command::legacy::status::tui::tests::utils::{test_tui, test_tui_with_size};

#[test]
fn toggle_details_view_for_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file!["snapshots/toggle_details_view_for_commit_001.svg"]);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file!["snapshots/toggle_details_view_for_commit_002.svg"]);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file!["snapshots/toggle_details_view_for_commit_003.svg"]);
}

#[test]
fn details_view_updates_with_selection_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks").unwrap();
    env.setup_metadata(&["A", "B"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_updates_with_selection_changes_001.svg"
        ]);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_updates_with_selection_changes_002.svg"
        ]);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_updates_with_selection_changes_003.svg"
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
            "snapshots/details_view_supports_scroll_controls_001.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('n')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_002.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('d')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_003.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('u')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_004.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('p')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_supports_scroll_controls_005.svg"
        ]);
}

#[test]
fn commit_message_wraps_in_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 80, 14);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_message_wraps_in_details_view_001.svg"
        ]);

    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_message_wraps_in_details_view_002.svg"
        ]);

    tui.input_then_render(" this commit message is intentionally long so the details pane has to wrap the text across multiple visual lines")
        .assert_rendered_term_svg_eq(file!["snapshots/commit_message_wraps_in_details_view_003.svg"]);

    with_var("GIT_AUTHOR_DATE", Some("2000-01-01T00:00:00Z"), || {
        with_var("GIT_COMMITTER_DATE", Some("2000-01-01T00:00:00Z"), || {
            tui.input_then_render(KeyCode::Enter);
        });
    });

    tui.input_then_render('d');

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('n')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/commit_message_wraps_in_details_view_005.svg"
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
            "snapshots/details_view_renders_multiple_hunks_and_files_001.svg"
        ]);
}

#[test]
fn details_diff_svg_shows_plus_and_minus_backgrounds() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("A", "A-changed\n");

    let mut tui = test_tui_with_size(env, 100, 12);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_diff_svg_shows_plus_and_minus_backgrounds_001.svg"
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
            "snapshots/toggling_details_off_and_on_resets_scroll_position_001.svg"
        ]);

    tui.input_then_render((KeyModifiers::CONTROL, KeyCode::Char('d')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_002.svg"
        ]);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_003.svg"
        ]);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggling_details_off_and_on_resets_scroll_position_004.svg"
        ]);
}

#[test]
fn details_view_syntax_highlighting_survives_scrolling() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let rust_code = (1..=120)
        .map(|line| {
            format!(
                "fn function_{line:03}(value: i32) -> i32 {{ let answer = match value {{ 0 => 41, _ => value + 1 }}; println!(\"line-{line:03}: {{answer}}\"); answer }} // comment-{line:03}\n"
            )
        })
        .collect::<String>();
    env.file("syntax.rs", rust_code);

    let mut tui = test_tui_with_size(env, 100, 10);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_syntax_highlighting_survives_scrolling_001.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('d')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_syntax_highlighting_survives_scrolling_002.svg"
        ]);

    tui.render_with_messages((KeyModifiers::CONTROL, KeyCode::Char('u')), Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_syntax_highlighting_survives_scrolling_003.svg"
        ]);
}

#[test]
fn details_view_can_grow_and_shrink() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 16);

    tui.input_then_render('d');
    tui.input_then_render("++-")
        .assert_rendered_term_svg_eq(file!["snapshots/details_view_can_grow_and_shrink_001.svg"]);
}

#[test]
fn details_view_resize_clamps_to_max_and_min_width() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui_with_size(env, 100, 16);

    tui.input_then_render('d');
    tui.input_then_render("++++++++++++++++++++");
    tui.input_then_render("--------------------")
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_view_resize_clamps_to_max_and_min_width_001.svg"
        ]);
}

#[test]
fn details_cursor_stays_visible_after_resizing() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let long_lines = (1..=80)
        .map(|line| format!("this is a deliberately long line in alpha.txt #{line:03} that should wrap in narrow detail panes\n"))
        .collect::<String>();

    env.file("alpha.txt", long_lines);
    env.file("beta.txt", "beta\n");

    let mut tui = test_tui_with_size(env, 80, 10);

    tui.input_then_render('d');
    tui.input_then_render('l');
    tui.input_then_render("----------");
    tui.input_then_render('j');

    tui.input_then_render("++++++++++")
        .assert_rendered_term_svg_eq(file![
            "snapshots/details_cursor_stays_visible_after_resizing_001.svg"
        ]);
}

#[test]
fn toggle_full_screen_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    // can open details with shift+d
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_showing_full_screen_details.svg"
        ]);

    // full screen details don't close when pressing h
    tui.input_then_render('h')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_showing_full_screen_details.svg"
        ]);

    // can close details with shift+d
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_closed_full_screen_details.svg"
        ]);

    // can close full screen details with escape
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_showing_full_screen_details.svg"
        ]);
    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_closed_full_screen_details.svg"
        ]);

    // can close full screen details with d
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_showing_full_screen_details.svg"
        ]);
    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_closed_full_screen_details.svg"
        ]);

    // shift+d with split details in normal mode opens full screen details
    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_split_details.svg"
        ]);
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_showing_full_screen_details.svg"
        ]);
    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_closed_full_screen_details.svg"
        ]);

    // shift+d with split details in details mode opens full screen details
    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_split_details.svg"
        ]);
    tui.input_then_render('l')
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_split_details_mode.svg"
        ]);
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_showing_full_screen_details.svg"
        ]);
    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_closed_full_screen_details.svg"
        ]);
}

#[test]
fn rubbing_from_full_screen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')))
        .assert_rendered_term_svg_eq(file![
            "snapshots/rubbing_from_full_screen_details_details_open.svg"
        ]);

    // avoid `input_then_render` here: its synthetic reload resets the details
    // selection that the idle frame below lets us establish.
    tui.render_with_messages(None, Vec::new());
    tui.render_with_messages('r', Vec::new())
        .assert_rendered_term_svg_eq(file![
            "snapshots/rubbing_from_full_screen_details_rubbing.svg"
        ]);

    tui.input_then_render('j')
        .assert_current_line_eq(str![["┊●   << amend >> [..] add A[..]"]]);

    // The details view should close. The split shouldn't show either
    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file![
            "snapshots/rubbing_from_full_screen_details_final.svg"
        ]);
}

#[test]
fn rubbing_from_split_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input_then_render('l')
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_from_split_details_open.svg"]);

    // avoid `input_then_render` here: its synthetic reload resets the details
    // selection that the idle frame below lets us establish.
    tui.render_with_messages(None, Vec::new());
    tui.render_with_messages('r', Vec::new())
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_from_split_details_rubbing.svg"]);

    tui.input_then_render('j')
        .assert_current_line_eq(str![["┊●   << amend >> [..] add A[..]"]]);

    // the details view should remain open
    tui.input_then_render(KeyCode::Enter)
        .assert_rendered_term_svg_eq(file!["snapshots/rubbing_from_split_details_final.svg"]);
}

#[test]
fn details_view_with_no_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')));

    tui.render_with_messages(None, Vec::new())
        .assert_rendered_contains("No changes");
}

#[test]
fn unfocusing_split_details_with_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input_then_render('l')
        .assert_rendered_term_svg_eq(file![
            "snapshots/unfocusing_split_details_with_escape_focused.svg"
        ]);

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/unfocusing_split_details_with_escape_unfocused.svg"
        ]);
}

#[test]
fn close_split_details_with_escape() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input_then_render('d')
        .assert_rendered_term_svg_eq(file!["snapshots/close_split_details_with_escape_open.svg"]);

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/close_split_details_with_escape_closed.svg"
        ]);
}

#[test]
fn escape_after_toggling_split_details_closed_does_not_reopen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input_then_render('d');
    tui.input_then_render('d');

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/close_split_details_with_escape_closed.svg"
        ]);
}

#[test]
fn escape_after_toggling_full_screen_details_closed_does_not_reopen_details() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')));
    tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('D')));

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/toggle_full_screen_details_view_for_commit_closed_full_screen_details.svg"
        ]);
}

#[test]
fn open_and_focus_details_split_can_be_closed_with_esc() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "content");

    let mut tui = test_tui(env);

    tui.input_then_render('l')
        .assert_rendered_term_svg_eq(file![
            "snapshots/open_and_focus_details_split_can_be_closed_with_esc_focused.svg"
        ]);

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_term_svg_eq(file![
            "snapshots/open_and_focus_details_split_can_be_closed_with_esc_open.svg"
        ]);

    tui.input_then_render(KeyCode::Esc);
}
