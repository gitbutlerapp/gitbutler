use but_api::open::program::USER_DEFINED_PROGRAMS_FILENAME;
use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};
use snapbox::{file, str};
use temp_env::with_var;

use crate::command::legacy::status::tui::tests::utils::{Shift, test_tui};

#[test]
fn open_uncommitted_file_in_program() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("open-me.txt", "I have new content");

    let mut tui = test_tui(env);
    tui.input('g');
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   ps A open-me.txt"]);

    let app_data_dir = tui.env().projects_root().display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input(Shift('o')).assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_in_program_001.svg"
        ]);
        tui.input("touch").assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_in_program_002.svg"
        ]);
        tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_in_program_003.svg"
        ]);
        tui.reload().assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_in_program_004.svg"
        ]);
    });

    tui.input("g");
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊   wv A open-me.txt.touch"]);
}

#[test]
fn open_uncommitted_file_in_program_chooses_program_by_extension_automatically_if_unambiguous() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("open-me.txt", "I have new content");

    let mut tui = test_tui(env);
    tui.input('g');
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   ps A open-me.txt"]);

    let app_data_dir = tui.env().projects_root();
    let config_dir = app_data_dir.join("gitbutler");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join(USER_DEFINED_PROGRAMS_FILENAME),
        r#"[
  {
    "id": "touch",
    "extensions": ["txt"]
  }
]"#,
    )
    .unwrap();

    let app_data_dir = app_data_dir.display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input('o').assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_in_program_chooses_program_by_extension_automatically_if_unambiguous_001.svg"
        ]);
        tui.reload().assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_in_program_chooses_program_by_extension_automatically_if_unambiguous_002.svg"
        ]);
    });

    tui.input("g");
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊   ps A open-me.txt"]);
}

#[test]
fn open_uncommitted_file_in_program_shows_only_programs_that_match_extension() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("open-me.txt", "I have new content");

    let mut tui = test_tui(env);
    tui.input('g');
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   ps A open-me.txt"]);

    let app_data_dir = tui.env().projects_root();
    let config_dir = app_data_dir.join("gitbutler");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join(USER_DEFINED_PROGRAMS_FILENAME),
        r#"[
  {
    "id": "touch",
    "extensions": ["txt"]
  },
  {
    "id": "echo",
    "extensions": ["txt"]
  }
]"#,
    )
    .unwrap();

    let app_data_dir = app_data_dir.display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input(Shift('o')).assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_in_program_shows_only_programs_that_match_extension.svg"
        ]);
    });
}

#[test]
fn open_uncommitted_file_with_multiple_hunks_in_program_from_details_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let original_content = "this\nis\nsome\ncontent\nto\ndiff\nwith\nadded\nlines\n";
    env.file("open-me.txt", original_content);

    let mut tui = test_tui(env);
    tui.input('c');
    tui.input(KeyCode::Down);
    tui.input('i');
    tui.input(KeyCode::Enter);
    tui.input("Add file");
    tui.input(KeyCode::Enter);

    tui.env().file(
        "open-me.txt",
        format!("new first\n{original_content}new last"),
    );

    tui.reload();
    tui.input('g');
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   ps M open-me.txt"]);

    tui.input((KeyModifiers::SHIFT, 'D'));
    tui.input(KeyCode::Down);
    tui.input(KeyCode::Down);

    let app_data_dir = tui.env().projects_root().display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input(Shift('o')).assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_with_multiple_hunks_in_program_from_details_view_001.svg"
        ]);
        tui.input("touch").assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_with_multiple_hunks_in_program_from_details_view_002.svg"
        ]);
        tui.input(KeyCode::Enter).assert_rendered_term_svg_eq(file![
            "snapshots/open_uncommitted_file_with_multiple_hunks_in_program_from_details_view_003.svg"
        ]);
        tui.input((KeyModifiers::SHIFT, 'D'));
        tui.reload().assert_rendered_term_svg_eq(
            file!["snapshots/open_uncommitted_file_with_multiple_hunks_in_program_from_details_view_004.svg"]
        );
    });

    tui.input("g");
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊   nv A open-me.txt.touch.11"]);
}

#[test]
fn open_committed_file_in_program() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);
    tui.input('f')
        .assert_current_line_eq(str!["┊│     t:t A A"]);

    let app_data_dir = tui.env().projects_root().display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input(Shift('o'))
            .assert_rendered_term_svg_eq(file!["snapshots/open_committed_file_in_program_001.svg"]);
        tui.input("touch")
            .assert_rendered_term_svg_eq(file!["snapshots/open_committed_file_in_program_002.svg"]);
        tui.input(KeyCode::Enter)
            .assert_rendered_term_svg_eq(file!["snapshots/open_committed_file_in_program_003.svg"]);
        tui.reload()
            .assert_rendered_term_svg_eq(file!["snapshots/open_committed_file_in_program_004.svg"]);
    });

    tui.input(["f", "g"]);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊   rx A A.touch"]);
}

#[test]
fn cannot_open_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);
    tui.reload()
        .assert_current_line_eq(str!["╭┄ zz [uncommitted] (no changes)"]);

    let app_data_dir = tui.env().projects_root().display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input(Shift('o'))
            .assert_rendered_term_svg_eq(file!["snapshots/cannot_open_uncommitted_area.svg"]);
    });
}

#[test]
fn cannot_open_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);
    tui.input(KeyCode::Down)
        .assert_current_line_eq(str!["┊╭┄ g0 [A]"]);

    let app_data_dir = tui.env().projects_root().display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input(Shift('o'))
            .assert_rendered_term_svg_eq(file!["snapshots/cannot_open_branch.svg"]);
    });
}

#[test]
fn cannot_open_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);
    tui.input([KeyCode::Down, KeyCode::Down])
        .assert_current_line_eq(str!["┊●   tpm add A"]);

    let app_data_dir = tui.env().projects_root().display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input(Shift('o'))
            .assert_rendered_term_svg_eq(file!["snapshots/cannot_open_commit.svg"]);
    });
}

#[test]
fn cannot_open_common_base() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_tui(env);
    tui.input((KeyModifiers::SHIFT, 'G'))
        .assert_current_line_eq(str!["┴ 0dc3733 (common base) 2000-01-02 add M"]);

    let app_data_dir = tui.env().projects_root().display().to_string();
    with_var("E2E_TEST_APP_DATA_DIR", Some(app_data_dir), || {
        tui.input(Shift('o'))
            .assert_rendered_term_svg_eq(file!["snapshots/cannot_open_common_base.svg"]);
    });
}
