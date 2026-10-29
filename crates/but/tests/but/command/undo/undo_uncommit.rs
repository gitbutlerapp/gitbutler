use crate::{command::undo::run_mutate_undo_roundtrip_test, utils::Sandbox};

#[test]
fn can_undo_but_uncommit_commit_add() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit uxn").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_commit_modify() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    let path = "new-file.txt";
    env.file(path, "changed content");

    env.but("commit -m 'Change file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit qnr").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_commit_delete() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    env.remove_file(path);

    env.but("commit -m 'Remove file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit orr").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_file_add() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit uxn:x").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_file_modify() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    env.file(path, "new content");
    env.but("commit -m 'Modify file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit zkn:x").assert().success();
    });
}

#[test]
fn can_undo_but_uncommit_file_delete() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");
    let path = "new-file.txt";
    env.file(path, "content");

    env.but("commit -m 'Add file'").assert().success();

    env.remove_file(path);
    env.but("commit -m 'Remove file'").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("uncommit orr:x").assert().success();
    });
}
