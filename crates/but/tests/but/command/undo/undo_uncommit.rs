use crate::{command::undo::run_mutate_undo_roundtrip_test, utils::Sandbox};

#[test]
fn can_undo_but_uncommit_commit_add() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit 6a30821").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_commit_modify() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    let path = "new-file.txt";
    env.file(path, "changed content");

    env.but("commit -m 'Change file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit c0aac71").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_commit_delete() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    env.remove_file(path);

    env.but("commit -m 'Remove file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit 1f27386").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_file_add() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit 6a:xk").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_file_modify() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    env.file(path, "new content");
    env.but("commit -m 'Modify file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit 31:xk").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_file_delete() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    env.remove_file(path);
    env.but("commit -m 'Remove file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit 1f:xk").assert().success();
    });
}
