use crate::{command::undo::run_mutate_undo_roundtrip_test, utils::Sandbox};

pub(super) fn commit_empty_with_message(env: &Sandbox, message: &str) -> String {
    #[derive(serde::Deserialize)]
    struct CommitJson {
        commit: String,
    }

    let output = env
        .but("commit --empty -b A --json")
        .args(["-m", message])
        .assert()
        .success();
    let output = output.get_output();
    serde_json::from_slice::<CommitJson>(&output.stdout)
        .unwrap()
        .commit
}

#[test]
fn can_undo_but_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file'").assert().success();
    });
}

#[test]
fn can_undo_but_commit_on_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' -b A").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' -b").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' -b my-new-branch")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create_existing_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' -b A").assert().success();
    });
}

#[test]
#[ignore = "undoing assignments dont work. https://linear.app/gitbutler/issue/GB-1468/undoing-but-commit-only-to-commit-only-assigned-changes-doesnt-work"]
fn can_undo_but_commit_dash_dash_only() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("assigned.txt", "assigned content");
    env.but("stage assigned.txt A").assert().success();

    env.file("uncommitted.txt", "uncommitted content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' assigned.txt")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("new-file.txt", "content");
    env.file("other-new-file.txt", "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' new-file.txt")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit --empty --no-message").assert().success();
    });
}

#[test]
fn can_undo_but_commit_empty_with_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit --empty -m 'Plan empty slot'")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("new-file.txt", "content");

    env.but("branch new my-new-branch").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit --empty --no-message -b my-new-branch")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty_dash_dash_before() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("new-file.txt", "content");

    commit_empty_with_message(&env, "one");
    commit_empty_with_message(&env, "two");
    let target = commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("commit --empty --no-message --below {target}"))
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty_dash_dash_after() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.file("new-file.txt", "content");

    commit_empty_with_message(&env, "one");
    let target = commit_empty_with_message(&env, "two");
    commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("commit --empty --no-message --above {target}"))
            .assert()
            .success();
    });
}
