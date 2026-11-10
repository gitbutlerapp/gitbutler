//! Workspace operations in repositories with clean/smudge filters, like unlocked git-crypt repositories.

use bstr::ByteSlice;

use super::util::{find_branch, status_json};
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

/// Pull so that stack A's commit, which changes the filtered `a.secret`, conflicts on
/// `file.txt`, then enter edit mode on it with `but resolve`.
fn resolve_conflicted_commit_with_filtered_file() -> Sandbox {
    resolve_conflicted_commit_with_filtered_file_after(|_| {})
}

/// Like [`resolve_conflicted_commit_with_filtered_file()`], but call `prepare` on the
/// workspace before entering edit mode.
fn resolve_conflicted_commit_with_filtered_file_after(prepare: impl FnOnce(&Sandbox)) -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "upstream-conflicted-with-filtered-files",
    );
    env.setup_metadata_at_target(&["A"], "refs/heads/base");
    env.invoke_git("remote set-url origin .");
    env.but("pull").assert().success();
    let status = status_json(&env);
    let commit = find_branch(&status, "A")["commits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|commit| commit["conflicted"].as_bool() == Some(true))
        .and_then(|commit| commit["cliId"].as_str())
        .expect("pulling conflicts A's commit")
        .to_owned();
    prepare(&env);
    env.but(format!("resolve {commit}")).assert().success();
    env
}

#[test]
fn resolve_writes_filtered_files_in_worktree_form() {
    let env = resolve_conflicted_commit_with_filtered_file();
    assert_eq!(
        read(&env, "a.secret").as_bstr(),
        "changed on A\n",
        "the commit's change to the filtered file is written smudged"
    );
}

#[test]
fn resolve_cancel_restores_uncommitted_filtered_files_in_worktree_form() {
    let env = resolve_conflicted_commit_with_filtered_file_after(|env| {
        env.file("a.secret", "edited before resolving\n");
        env.file("untracked.txt", "untracked\n");
    });

    env.but("resolve cancel --force").assert().success();
    assert_eq!(
        read(&env, "a.secret").as_bstr(),
        "edited before resolving\n",
        "the uncommitted edit comes back smudged"
    );
    assert_eq!(
        env.git_status(),
        " M a.secret\n?? untracked.txt\n",
        "the worktree and index are as before resolving"
    );
}
