use crate::{command::undo::run_mutate_undo_roundtrip_test, utils::Sandbox};

// TODO: `but commit empty` doesn't support `--message`, it should so we don't need this hack
pub(super) fn commit_empty_with_message(env: &Sandbox, message: &str) -> String {
    #[derive(serde::Deserialize)]
    struct CommitEmptyJson {
        commit_id: String,
    }

    #[derive(serde::Deserialize)]
    struct RewordJson {
        new_commit_id: String,
    }

    let output = env.but("commit empty A --format json").assert().success();
    let output = output.get_output();
    let commit_id = serde_json::from_slice::<CommitEmptyJson>(&output.stdout)
        .unwrap()
        .commit_id;

    let output = env
        .but("reword")
        .args([&commit_id, "-m", message, "--format", "json"])
        .assert()
        .success();
    let output = output.get_output();
    serde_json::from_slice::<RewordJson>(&output.stdout)
        .unwrap()
        .new_commit_id
}

#[test]
fn can_undo_but_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file'").assert().success();
    });
}

#[test]
fn can_undo_but_commit_on_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' A").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' --create").assert().success();

        // TODO: only create one snapshot so additional undo isn't required
        env.but("undo").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' --create my-new-branch")
            .assert()
            .success();

        // TODO: only create one snapshot so additional undo isn't required
        env.but("undo").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_create_existing_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' --create A")
            .assert()
            .success();
    });
}

#[test]
#[ignore = "undoing assignments dont work. https://linear.app/gitbutler/issue/GB-1468/undoing-but-commit-only-to-commit-only-assigned-changes-doesnt-work"]
fn can_undo_but_commit_dash_dash_only() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();

    env.file("assigned.txt", "assigned content");
    env.but("rub assigned.txt A").assert().success();

    env.file("unassigned.txt", "unassigned content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' --only").assert().success();
    });
}

#[test]
fn can_undo_but_commit_dash_dash_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();

    env.file("new-file.txt", "content");
    env.file("other-new-file.txt", "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit -m 'Add file' --changes new-file.txt")
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit empty").assert().success();
    });
}

#[test]
fn can_undo_but_commit_empty_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();

    env.file("new-file.txt", "content");

    env.but("branch new my-new-branch").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("commit empty my-new-branch").assert().success();
    });
}

#[test]
fn can_undo_but_commit_empty_dash_dash_before() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();

    env.file("new-file.txt", "content");

    commit_empty_with_message(&env, "one");
    commit_empty_with_message(&env, "two");
    let target = commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("commit empty --before {target}"))
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_commit_empty_dash_dash_after() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();

    env.file("new-file.txt", "content");

    commit_empty_with_message(&env, "one");
    let target = commit_empty_with_message(&env, "two");
    commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("commit empty --after {target}"))
            .assert()
            .success();
    });
}
