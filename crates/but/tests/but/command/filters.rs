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

#[test]
fn resolve_finish_stores_filtered_files_once_and_writes_them_in_worktree_form() {
    let env = resolve_conflicted_commit_with_filtered_file_after(|env| {
        env.file("c.secret", "untracked before resolving\n");
    });
    env.file("file.txt", "resolved\n");
    env.file("a.secret", "resolved secret\n");

    env.but("resolve finish").assert().success();
    assert_eq!(
        env.invoke_git("cat-file -p A:a.secret")
            .as_bytes()
            .as_bstr(),
        "SECRET\0erfbyirq frperg",
        "the commit stores the resolved file cleaned once"
    );
    assert_eq!(
        read(&env, "a.secret").as_bstr(),
        "resolved secret\n",
        "the committed file is written smudged"
    );
    assert_eq!(
        read(&env, "c.secret").as_bstr(),
        "untracked before resolving\n",
        "the uncommitted file comes back smudged"
    );
    assert_eq!(
        env.git_status(),
        "?? c.secret\n",
        "the worktree matches the workspace and its uncommitted file"
    );
}

#[test]
fn resolve_finish_keeps_the_stat_of_filtered_files_it_leaves_alone() {
    let env = resolve_conflicted_commit_with_filtered_file_after(|env| {
        // The fixture's files were copied, so let status record their stat first, as it had.
        // A stat no older than the index is racy and never kept, so date the file back.
        std::fs::File::options()
            .write(true)
            .open(env.projects_root().join("b.secret"))
            .unwrap()
            .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(10))
            .unwrap();
        env.git_status();
    });
    env.file("file.txt", "resolved\n");
    env.but("resolve finish").assert().success();

    let git_dir = env.projects_root().join(".git");
    // Age the index so status relies on the recorded stat, as it does a moment later.
    std::fs::File::options()
        .write(true)
        .open(git_dir.join("index"))
        .unwrap()
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2))
        .unwrap();
    std::fs::write(git_dir.join("clean-calls"), "").unwrap();
    assert_eq!(env.git_status(), "", "the worktree matches the workspace");
    assert!(
        !std::fs::read_to_string(git_dir.join("clean-calls"))
            .unwrap()
            .contains("b.secret"),
        "the index keeps the stat of the untouched filtered file, so status doesn't clean it"
    );
}
