use but_testsupport::Sandbox;
use crossterm::event::{KeyCode, KeyModifiers};

use crate::command::legacy::status::tui::tests::utils::{
    TestTuiOptions, test_status_tui, test_status_tui_with_options,
};

const COPY_MORE: (KeyModifiers, char) = (KeyModifiers::SHIFT, 'Y');

#[test]
fn copying_change_id_doesnt_include_disambiguation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let mut tui = test_status_tui_with_options(
        env,
        TestTuiOptions {
            testing_change_id: Some("1"),
            ..Default::default()
        },
    );

    tui.input('b');
    tui.input('n');
    tui.input('n');
    tui.input('n');

    // Ideally this would include the disambiguation but the change id in CliId::Commit doesn't
    // include it. In the future it will.
    tui.input('y').assert_copied_text_eq("1");
}

#[test]
fn copies_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);
    tui.input(KeyCode::Down);

    tui.input('y').assert_copied_text_eq("A");
}

#[test]
fn copies_committed_file_path() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('f');
    tui.input(KeyCode::Down);

    tui.input('y').assert_copied_text_eq("A");
}

#[test]
fn copies_uncommitted_file_path() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("copy.txt", "copied content\n");

    let mut tui = test_status_tui(env);
    tui.input(KeyCode::Down);

    tui.input('y').assert_copied_text_eq("copy.txt");
}

#[test]
fn copies_every_commit_value() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("copy.txt", "copied content\n");

    let mut tui = test_status_tui(env);
    tui.input('b');
    tui.input('n');
    tui.input('n');
    tui.input(KeyCode::Enter);
    tui.input("Copy title");
    tui.input(KeyCode::Enter);

    tui.input((KeyModifiers::SHIFT, 'K'));
    tui.input((KeyModifiers::SHIFT, 'K'));
    tui.input(KeyCode::Down);
    tui.input('r');
    tui.input((KeyModifiers::SHIFT, 'J'));
    tui.input('u');
    tui.input(KeyCode::Enter);
    tui.reload();
    tui.input((KeyModifiers::SHIFT, 'K'));
    tui.input(KeyCode::Down);

    let commit_id = tui.env().invoke_git("rev-parse refs/heads/c-branch-1");

    tui.input(COPY_MORE);
    tui.input(KeyCode::Enter).assert_copied_text_eq(&commit_id);

    tui.input(COPY_MORE);
    tui.input(KeyCode::Down);
    tui.input(KeyCode::Enter)
        .assert_copied_text_eq(&commit_id[..7]);

    tui.input(COPY_MORE);
    tui.input([KeyCode::Down; 2]);
    tui.input(KeyCode::Enter)
        .assert_copied_text_eq("omyuzposspozzzlpxqkzstlkmxpvxzqs");

    tui.input(COPY_MORE);
    tui.input([KeyCode::Down; 3]);
    tui.input(KeyCode::Enter)
        .assert_copied_text_eq("Copy title");

    tui.input(COPY_MORE);
    tui.input([KeyCode::Down; 4]);
    tui.input(KeyCode::Enter)
        .assert_copied_text_eq("Copy title");

    tui.input(COPY_MORE);
    tui.input([KeyCode::Down; 5]);
    tui.input(KeyCode::Enter)
        .assert_copied_text_eq("committer <committer@example.com>");

    tui.input(COPY_MORE);
    tui.input([KeyCode::Down; 6]);
    tui.input(KeyCode::Enter)
        .assert_copied_text_eq("--- /dev/null\n+++ b/copy.txt\n@@ -1,0 +1,1 @@\n+copied content\n");
}

#[test]
fn copies_every_branch_value() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);
    tui.input(KeyCode::Down);

    tui.input(COPY_MORE);
    tui.input(KeyCode::Enter).assert_copied_text_eq("A");

    tui.input(COPY_MORE);
    tui.input([KeyCode::Down; 2]);
    tui.input(KeyCode::Enter)
        .assert_copied_text_eq("--- /dev/null\n+++ b/A\n@@ -1,0 +1,1 @@\n+A\n");
}

#[test]
fn copies_every_committed_file_value() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let mut tui = test_status_tui(env);
    tui.input([KeyCode::Down, KeyCode::Down]);
    tui.input('f');
    tui.input(KeyCode::Down);

    tui.input(COPY_MORE);
    tui.input(KeyCode::Enter).assert_copied_text_eq("t:t");

    tui.input(COPY_MORE);
    tui.input(KeyCode::Down);
    tui.input(KeyCode::Enter).assert_copied_text_eq("A");
}

#[test]
fn copies_hunk_from_detail_view() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("copy.txt", "copied content\n");

    let mut tui = test_status_tui(env);
    tui.input('d');
    tui.input('l');
    tui.input((KeyModifiers::SHIFT, 'G'));

    tui.input('y')
        .assert_copied_text_eq("copy.txt\n\n@@ -1,0 +1,1 @@\n+copied content\n");
}

#[test]
fn copies_every_uncommitted_file_value() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("copy.txt", "copied content\n");

    let mut tui = test_status_tui(env);
    tui.input(KeyCode::Down);

    tui.input(COPY_MORE);
    tui.input(KeyCode::Enter).assert_copied_text_eq("ry");

    tui.input(COPY_MORE);
    tui.input(KeyCode::Down);
    tui.input(KeyCode::Enter).assert_copied_text_eq(
        "diff --git a/copy.txt b/copy.txt\n--- a/copy.txt\n+++ b/copy.txt\n@@ -1,0 +1,1 @@\n+copied content\n",
    );

    tui.input(COPY_MORE);
    tui.input([KeyCode::Down; 2]);
    tui.input(KeyCode::Enter).assert_copied_text_eq("copy.txt");
}
