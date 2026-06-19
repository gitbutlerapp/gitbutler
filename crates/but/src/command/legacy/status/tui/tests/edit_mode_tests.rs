use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use gitbutler_operating_modes::OperatingMode;
use snapbox::str;

use crate::command::legacy::status::tui::{BackstackEntry, tests::utils::test_tui};

#[test]
fn edit_mode_enter_save_and_abort_confirmations() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('e')
        .assert_rendered_contains("Enter edit mode for commit 9477ae7?")
        .assert_rendered_contains("Yes    No");

    tui.input_then_render('y')
        .assert_rendered_contains("  edit  ")
        .assert_backstack_eq([BackstackEntry::AbortEditMode]);

    let mode =
        but_api::legacy::modes::operating_mode(&tui.env().context().expect("context should open"))
            .expect("operating mode should load")
            .operating_mode;
    assert!(
        matches!(mode, OperatingMode::Edit(_)),
        "confirming enter should put the workspace in edit mode"
    );

    tui.input_then_render('e')
        .assert_rendered_contains("Save changes and exit edit mode?")
        .assert_rendered_contains("Yes    No");

    tui.input_then_render(KeyCode::Esc)
        .assert_backstack_eq([BackstackEntry::AbortEditMode]);

    tui.input_then_render(KeyCode::Esc)
        .assert_rendered_contains("Abort edit mode?")
        .assert_rendered_contains("Changes made in edit mode will be discarded.")
        .assert_rendered_contains("Yes    No");

    tui.input_then_render('y').assert_backstack_eq([]);

    let mode =
        but_api::legacy::modes::operating_mode(&tui.env().context().expect("context should open"))
            .expect("operating mode should load")
            .operating_mode;
    assert!(
        !matches!(mode, OperatingMode::Edit(_)),
        "confirming abort should leave edit mode"
    );
}

#[test]
fn edit_mode_save_confirmation_exits_edit_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down]);
    tui.input_then_render('e');
    tui.input_then_render('y')
        .assert_rendered_contains("  edit  ")
        .assert_backstack_eq([BackstackEntry::AbortEditMode]);

    tui.input_then_render('e')
        .assert_rendered_contains("Save changes and exit edit mode?");
    tui.input_then_render('y').assert_backstack_eq([]);

    let mode =
        but_api::legacy::modes::operating_mode(&tui.env().context().expect("context should open"))
            .expect("operating mode should load")
            .operating_mode;
    assert!(
        !matches!(mode, OperatingMode::Edit(_)),
        "confirming save should leave edit mode"
    );
}

#[test]
fn edit_mode_enter_with_uncommitted_changes_shows_error_toast() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();
    env.file("uncommitted.txt", "uncommitted\n");

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('e')
        .assert_rendered_contains("Cannot enter edit mode with uncommitted changes")
        .assert_rendered_not_contains("Enter edit mode for commit");

    let mode =
        but_api::legacy::modes::operating_mode(&tui.env().context().expect("context should open"))
            .expect("operating mode should load")
            .operating_mode;
    assert!(
        !matches!(mode, OperatingMode::Edit(_)),
        "uncommitted changes should prevent entering edit mode"
    );
}

#[test]
fn edit_mode_on_non_commit_selection_is_noop() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.render_with_messages(None, Vec::new())
        .assert_rendered_not_contains("Enter edit mode for commit");
    tui.input_then_render('e')
        .assert_rendered_not_contains("Enter edit mode for commit")
        .assert_rendered_not_contains("Cannot enter edit mode");
}
