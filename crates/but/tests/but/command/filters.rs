//! Workspace operations in repositories with clean/smudge filters, like unlocked git-crypt repositories.

use bstr::ByteSlice;

use crate::utils::Sandbox;

fn read(env: &Sandbox, path: &str) -> Vec<u8> {
    std::fs::read(env.projects_root().join(path)).unwrap()
}

#[test]
fn unapply_and_apply_write_filtered_files_in_worktree_form() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-with-filtered-files");
    env.setup_metadata(&["A"]);

    env.but("unapply A").assert().success();
    assert_eq!(
        read(&env, "a.secret").as_bstr(),
        "base\n",
        "unapplying restores the smudged target version"
    );
    assert_eq!(env.git_status(), "", "the smudged file matches the target");

    env.but("apply A").assert().success();
    assert_eq!(
        read(&env, "a.secret").as_bstr(),
        "changed on A\n",
        "applying writes the smudged branch version"
    );
    assert_eq!(
        env.git_status(),
        "",
        "the smudged file matches the workspace"
    );
}

#[test]
fn pull_writes_filtered_files_in_worktree_form() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-with-filtered-files");
    env.setup_metadata(&["A"]);

    env.but("pull").assert().success();
    assert_eq!(
        read(&env, "b.secret").as_bstr(),
        "changed upstream\n",
        "the upstream change arrives smudged"
    );
    assert_eq!(
        read(&env, "a.secret").as_bstr(),
        "changed on A\n",
        "the stack's change stays smudged"
    );
    assert_eq!(
        env.git_status(),
        "",
        "the smudged files match the updated workspace"
    );
}
