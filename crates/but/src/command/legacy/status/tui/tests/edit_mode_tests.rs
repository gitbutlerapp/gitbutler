use but_testsupport::Sandbox;
use crossterm::event::KeyCode;
use gitbutler_operating_modes::OperatingMode;
use snapbox::str;
use temp_env::with_var;

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
fn edit_mode_enter_with_uncommitted_changes_uses_backend_result() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();
    env.file("uncommitted.txt", "uncommitted\n");

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);

    tui.input_then_render('e')
        .assert_rendered_contains("Enter edit mode for commit 9477ae7?")
        .assert_rendered_not_contains("Cannot enter edit mode with uncommitted changes");

    tui.input_then_render('y')
        .assert_rendered_contains("  edit  ")
        .assert_backstack_eq([BackstackEntry::AbortEditMode]);

    let mode =
        but_api::legacy::modes::operating_mode(&tui.env().context().expect("context should open"))
            .expect("operating mode should load")
            .operating_mode;
    assert!(
        matches!(mode, OperatingMode::Edit(_)),
        "the backend should decide whether uncommitted changes can enter edit mode"
    );
}

#[test]
fn edit_mode_can_create_commit_from_unassigned_changes() {
    const EDIT_MODE_COMMIT_MESSAGE: &str = "edit mode commit from tui test";

    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();
    let editor_command = format!("sh -c 'printf {EDIT_MODE_COMMIT_MESSAGE:?} > \"$1\"' editor");

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down]);
    tui.input_then_render('e');
    tui.input_then_render('y')
        .assert_rendered_contains("  edit  ")
        .assert_backstack_eq([BackstackEntry::AbortEditMode]);

    tui.env().file("edit-mode-new-file.txt", "content\n");
    tui.reload()
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);
    tui.input_then_render([KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["╭┄zz [unassigned changes]"]);

    tui.input_then_render('c')
        .assert_current_line_eq(str!["╭┄<< source >> << noop >> zz [unassigned changes]"]);
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);

    with_var("GIT_EDITOR", Some(editor_command), || {
        tui.input_then_render(KeyCode::Enter)
            .assert_current_line_eq(str!["┊●   716b7f4 edit mode commit from tui test"]);
    });

    let ctx = tui.env().context().expect("context should open");
    let repo = ctx.repo.get().expect("repo should open");
    let head_message = repo
        .head_commit()
        .expect("HEAD should resolve to a commit")
        .decode()
        .expect("HEAD commit should decode")
        .message
        .to_string();
    assert_eq!(head_message, EDIT_MODE_COMMIT_MESSAGE);
}

#[test]
fn edit_mode_can_amend_commit_from_unassigned_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down]);
    tui.input_then_render('e');
    tui.input_then_render('y')
        .assert_rendered_contains("  edit  ")
        .assert_backstack_eq([BackstackEntry::AbortEditMode]);

    tui.env().file("edit-mode-amend.txt", "content\n");
    tui.reload()
        .assert_current_line_eq(str!["┊●   9477ae7 add A"]);
    tui.input_then_render([KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   km A edit-mode-amend.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> km A edit-mode-amend.txt"]);
    tui.input_then_render([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   << amend >> 9477ae7 add A"]);
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   26e33f3 add A"]);

    let ctx = tui.env().context().expect("context should open");
    let repo = ctx.repo.get().expect("repo should open");
    let blob = repo
        .rev_parse_single(b"HEAD:edit-mode-amend.txt")
        .expect("amended file should be in HEAD")
        .object()
        .expect("amended file should resolve to an object");
    assert_eq!(&blob.data[..], b"content\n");
}

#[test]
fn edit_mode_save_after_create_amend_and_move_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down]);
    tui.input_then_render('e');
    tui.input_then_render('y')
        .assert_rendered_contains("  edit  ")
        .assert_backstack_eq([BackstackEntry::AbortEditMode]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
    tui.input_then_render('n')
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);

    tui.env().file("edit-mode-moved.txt", "content\n");
    tui.reload()
        .assert_current_line_eq(str!["┊●   f184fc7 (no commit message) (no changes)"]);
    tui.input_then_render([KeyCode::Up, KeyCode::Up, KeyCode::Up])
        .assert_current_line_eq(str!["┊   ox A edit-mode-moved.txt"]);

    tui.input_then_render('r')
        .assert_current_line_eq(str!["┊   << source >> << noop >> ox A edit-mode-moved.txt"]);
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str![
            "┊●   << amend >> f184fc7 (no commit message) (no changes)"
        ]);
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   6f0be43 (no commit message)"]);

    tui.input_then_render('m').assert_current_line_eq(str![
        "┊●   << source >> << noop >> 6f0be43 (no commit message)"
    ]);
    tui.input_then_render(KeyCode::Down)
        .assert_current_line_eq(str!["┊│   << move commit above >>"]);
    tui.input_then_render('a')
        .assert_current_line_eq(str!["┊●   9477ae7 add A"])
        .assert_rendered_contains("┊│   << move commit below >>");
    tui.input_then_render(KeyCode::Enter)
        .assert_current_line_eq(str!["┊●   33b3ede (no commit message)"]);

    tui.input_then_render('e')
        .assert_rendered_contains("Save changes and exit edit mode?");
    tui.input_then_render('y')
        .assert_backstack_eq([])
        .assert_rendered_not_contains("  edit  ")
        .assert_rendered_contains("╭┄zz [unassigned changes] (no changes)")
        .assert_rendered_contains("┊●   8f0cbdf add A")
        .assert_rendered_contains("┊●   33b3ede (no commit message)")
        .assert_rendered_contains("┴ 0dc3733 (common base) 2000-01-02 add M");
}

#[test]
fn edit_mode_can_create_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render([KeyCode::Down, KeyCode::Down]);
    tui.input_then_render('e');
    tui.input_then_render('y')
        .assert_rendered_contains("  edit  ")
        .assert_backstack_eq([BackstackEntry::AbortEditMode]);

    tui.input_then_render(KeyCode::Up)
        .assert_current_line_eq(str!["┊╭┄g0 [A]"]);
    tui.input_then_render('b')
        .assert_current_line_eq(str!["┊╭┄br [c-branch-1] (no commits)"]);
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
