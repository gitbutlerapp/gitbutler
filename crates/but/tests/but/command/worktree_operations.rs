use std::path::{Path, PathBuf};

use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn move_workspace_commit_to_worktree_branch() -> anyhow::Result<()> {
    let env = worktree_env();
    env.file("ws-move.txt", "one hunk from workspace\n");
    env.but("commit B -m 'workspace move source'")
        .assert()
        .success()
        .stdout_eq(str!["✓ Created commit [..] on branch B\n\n"])
        .stderr_eq(str![]);

    let source = revision(&env, "B")?;
    env.but(format!("move {source} {}", worktree_id(&env)?))
        .assert()
        .success()
        .stdout_eq(str!["Moved [..] → [feat]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:ws-move.txt", b"one hunk from workspace\n")?;
    assert_missing(&env, "B:ws-move.txt");
    assert_eq!(
        std::fs::read(worktree_dir(&env).join("ws-move.txt"))?,
        b"one hunk from workspace\n",
        "the linked checkout follows its rewritten branch"
    );
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree_dir(&env))?,
        "",
        "the linked checkout stays clean"
    );
    assert_eq!(tip_message(&env, "feat")?, "workspace move source");
    Ok(())
}

#[test]
fn move_worktree_commit_to_workspace_branch() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_file(
        &worktree_dir(&env),
        "wt-move.txt",
        "one hunk from worktree\n",
        "worktree move source",
    );

    let source = revision(&env, "feat")?;
    env.but(format!("move {source} {}", workspace_branch_id(&env, "B")?))
        .assert()
        .success()
        .stdout_eq(str!["Moved [..] → [B]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "B:wt-move.txt", b"one hunk from worktree\n")?;
    assert_missing(&env, "feat:wt-move.txt");
    assert!(
        !worktree_dir(&env).join("wt-move.txt").exists(),
        "the linked checkout removes the moved source"
    );
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree_dir(&env))?,
        "",
        "the linked checkout stays clean"
    );
    assert_eq!(tip_message(&env, "B")?, "worktree move source");
    Ok(())
}

#[test]
fn move_one_hunk_from_workspace_commit_to_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_empty(&worktree_dir(&env), "worktree target");
    env.file("ws-hunk.txt", "the only hunk from workspace\n");
    env.file(
        "ws-remains.txt",
        "another one-hunk file stays in workspace\n",
    );
    env.but("commit B -m 'workspace hunk source'")
        .assert()
        .success()
        .stdout_eq(str!["✓ Created commit [..] on branch B\n\n"])
        .stderr_eq(str![]);

    let source = revision(&env, "B")?;
    let target = revision(&env, "feat")?;
    env.but(format!("move {source}:ws-hunk.txt {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved files between commits!\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:ws-hunk.txt", b"the only hunk from workspace\n")?;
    assert_missing(&env, "B:ws-hunk.txt");
    assert_blob(
        &env,
        "B:ws-remains.txt",
        b"another one-hunk file stays in workspace\n",
    )?;
    assert_missing(&env, "feat:ws-remains.txt");
    assert_eq!(
        std::fs::read(worktree_dir(&env).join("ws-hunk.txt"))?,
        b"the only hunk from workspace\n"
    );
    assert!(!worktree_dir(&env).join("ws-remains.txt").exists());
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn move_one_hunk_from_worktree_commit_to_workspace_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    std::fs::write(
        worktree.join("wt-hunk.txt"),
        "the only hunk from worktree\n",
    )?;
    std::fs::write(
        worktree.join("wt-remains.txt"),
        "another one-hunk file stays in worktree\n",
    )?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- wt-hunk.txt wt-remains.txt && git commit -qm 'worktree hunk source'",
        &worktree,
    );
    env.file("ws-target.txt", "workspace target\n");
    env.but("commit B -m 'workspace target'")
        .assert()
        .success()
        .stdout_eq(str!["✓ Created commit [..] on branch B\n\n"])
        .stderr_eq(str![]);

    let source = revision(&env, "feat")?;
    let target = revision(&env, "B")?;
    env.but(format!("move {source}:wt-hunk.txt {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved files between commits!\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "B:wt-hunk.txt", b"the only hunk from worktree\n")?;
    assert_missing(&env, "feat:wt-hunk.txt");
    assert_blob(
        &env,
        "feat:wt-remains.txt",
        b"another one-hunk file stays in worktree\n",
    )?;
    assert_missing(&env, "B:wt-remains.txt");
    assert!(!worktree.join("wt-hunk.txt").exists());
    assert_eq!(
        std::fs::read(worktree.join("wt-remains.txt"))?,
        b"another one-hunk file stays in worktree\n"
    );
    assert_eq!(but_testsupport::git_status_at_dir(worktree)?, "");
    Ok(())
}

#[test]
fn squash_workspace_uncommitted_change_into_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_empty(&worktree_dir(&env), "worktree target");
    env.file("ws-dirty.txt", "dirty in workspace\n");

    let target = revision(&env, "feat")?;
    env.but(format!("rub zz {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Amended uncommitted files → [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:ws-dirty.txt", b"dirty in workspace\n")?;
    assert_eq!(env.git_status(), "", "the workspace change was consumed");
    assert_eq!(
        std::fs::read(worktree_dir(&env).join("ws-dirty.txt"))?,
        b"dirty in workspace\n"
    );
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn squash_worktree_uncommitted_change_into_workspace_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    std::fs::write(worktree.join("wt-dirty.txt"), "dirty in worktree\n")?;

    let target = revision(&env, "B")?;
    env.but(format!("rub {} {target}", worktree_id(&env)?))
        .assert()
        .success()
        .stdout_eq(str!["Amended changes from worktree C → [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "B:wt-dirty.txt", b"dirty in worktree\n")?;
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree)?,
        "",
        "the worktree change was consumed"
    );
    Ok(())
}

#[test]
fn make_commit_in_worktree() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    let hooks = env.projects_root().join(".git/test-hooks");
    std::fs::create_dir_all(&hooks)?;
    let pre_commit = hooks.join("pre-commit");
    std::fs::write(&pre_commit, "#!/bin/sh\ntest -f wt-commit.txt\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&pre_commit, std::fs::Permissions::from_mode(0o755))?;
    }
    env.invoke_git(&format!("config core.hooksPath {}", hooks.display()));
    env.file("main-dirty.txt", "must stay in the main workspace\n");
    std::fs::write(worktree.join("wt-commit.txt"), "committed by but\n")?;

    env.but("commit -m 'commit from linked worktree'")
        .current_dir(&worktree)
        .assert()
        .success()
        .stdout_eq(str!["✓ Created commit [..] on branch feat\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:wt-commit.txt", b"committed by but\n")?;
    assert_eq!(tip_message(&env, "feat")?, "commit from linked worktree");
    assert_missing(&env, "feat:main-dirty.txt");
    assert!(
        env.git_status().contains("main-dirty.txt"),
        "the unrelated main-workspace change remains uncommitted"
    );
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree)?,
        "",
        "the committed worktree is clean"
    );
    Ok(())
}

#[test]
fn squash_worktree_uncommitted_change_into_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    commit_empty(&worktree, "worktree target");
    std::fs::write(worktree.join("wt-amend.txt"), "amended in worktree\n")?;

    let target = revision(&env, "feat")?;
    env.but(format!("rub {} {target}", worktree_id(&env)?))
        .assert()
        .success()
        .stdout_eq(str!["Amended changes from worktree C → [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:wt-amend.txt", b"amended in worktree\n")?;
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree)?,
        "",
        "the amended worktree is clean"
    );
    Ok(())
}

#[test]
fn squash_commits_in_worktree() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    commit_file(&worktree, "one.txt", "one\n", "worktree one");
    let target = revision(&env, "feat")?;
    commit_file(&worktree, "two.txt", "two\n", "worktree two");
    let source = revision(&env, "feat")?;

    env.but(format!("squash {source} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Squashed [..] → [..]\n\n"])
        .stderr_eq(str![]);

    assert_eq!(
        env.invoke_git("rev-list --count main..feat"),
        "1",
        "the two worktree commits were squashed"
    );
    assert_blob(&env, "feat:one.txt", b"one\n")?;
    assert_blob(&env, "feat:two.txt", b"two\n")?;
    assert_eq!(std::fs::read(worktree.join("one.txt"))?, b"one\n");
    assert_eq!(std::fs::read(worktree.join("two.txt"))?, b"two\n");
    assert_eq!(but_testsupport::git_status_at_dir(worktree)?, "");
    Ok(())
}

#[test]
fn squash_worktree_commit_into_workspace_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_file(
        &worktree_dir(&env),
        "wt-squash.txt",
        "squashed from worktree\n",
        "worktree squash source",
    );

    let source = revision(&env, "feat")?;
    let target = revision(&env, "B")?;
    env.but(format!("squash {source} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Squashed [..] → [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "B:wt-squash.txt", b"squashed from worktree\n")?;
    assert_missing(&env, "feat:wt-squash.txt");
    assert!(!worktree_dir(&env).join("wt-squash.txt").exists());
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn squash_workspace_commit_into_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_empty(&worktree_dir(&env), "worktree squash target");
    env.file("ws-squash.txt", "squashed from workspace\n");
    env.but("commit B -m 'workspace squash source'")
        .assert()
        .success()
        .stdout_eq(str!["✓ Created commit [..] on branch B\n\n"])
        .stderr_eq(str![]);

    let source = revision(&env, "B")?;
    let target = revision(&env, "feat")?;
    env.but(format!("squash {source} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Squashed [..] → [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:ws-squash.txt", b"squashed from workspace\n")?;
    assert_missing(&env, "B:ws-squash.txt");
    assert_eq!(
        std::fs::read(worktree_dir(&env).join("ws-squash.txt"))?,
        b"squashed from workspace\n"
    );
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

fn worktree_env() -> Sandbox {
    let mut env = Sandbox::init_scenario_with_target_and_default_settings_slow("two-worktrees");
    env.setup_metadata(&["A", "B"]);
    env.set_worktree_manipulation(true);
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    let mut settings: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&settings_path).expect("settings can be read"),
    )
    .expect("settings are JSON");
    settings["featureFlags"]["worktreeManipulation"] = serde_json::Value::Bool(true);
    std::fs::write(
        settings_path,
        serde_json::to_string_pretty(&settings).expect("settings serialize"),
    )
    .expect("settings can be written");
    env.context()
        .worktrees_with_state()
        .expect("existing worktrees can be reconciled");
    but_testsupport::invoke_bash_at_dir(
        "git worktree add .git/gitbutler/worktrees/C -b feat main",
        env.projects_root(),
    );
    env
}

fn worktree_dir(env: &Sandbox) -> PathBuf {
    env.projects_root().join(".git/gitbutler/worktrees/C")
}

fn worktree_id(env: &Sandbox) -> anyhow::Result<String> {
    let status = status_json(env)?;
    Ok(status["worktrees"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|worktree| worktree["name"] == "C")
        .and_then(|worktree| worktree["cliId"].as_str())
        .expect("active worktree C has a CLI id")
        .to_owned())
}

fn workspace_branch_id(env: &Sandbox, name: &str) -> anyhow::Result<String> {
    let status = status_json(env)?;
    Ok(status["stacks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .find(|branch| branch["name"] == name)
        .and_then(|branch| branch["cliId"].as_str())
        .expect("workspace branch has a CLI id")
        .to_owned())
}

fn status_json(env: &Sandbox) -> anyhow::Result<serde_json::Value> {
    let output = env
        .but("--format json status")
        .allow_json()
        .env("NO_BG_TASKS", "1")
        .output()?;
    assert!(output.status.success(), "status should succeed");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn revision(env: &Sandbox, revision: &str) -> anyhow::Result<gix::ObjectId> {
    Ok(env.open_repo().rev_parse_single(revision)?.detach())
}

fn tip_message(env: &Sandbox, revision: &str) -> anyhow::Result<String> {
    Ok(env
        .open_repo()
        .rev_parse_single(revision)?
        .object()?
        .peel_to_commit()?
        .message()?
        .title
        .to_string()
        .trim_end()
        .to_owned())
}

fn assert_blob(env: &Sandbox, revision: &str, expected: &[u8]) -> anyhow::Result<()> {
    assert_eq!(
        env.open_repo().rev_parse_single(revision)?.object()?.data,
        expected,
        "{revision} has the expected one-hunk file"
    );
    Ok(())
}

fn assert_missing(env: &Sandbox, revision: &str) {
    assert!(
        env.open_repo().rev_parse_single(revision).is_err(),
        "{revision} should no longer exist"
    );
}

fn commit_file(worktree: &Path, path: &str, contents: &str, message: &str) {
    std::fs::write(worktree.join(path), contents).expect("test file can be written");
    but_testsupport::invoke_bash_at_dir(
        &format!("git add -- {path} && git commit -qm '{message}'"),
        worktree,
    );
}

fn commit_empty(worktree: &Path, message: &str) {
    but_testsupport::invoke_bash_at_dir(
        &format!("git commit --allow-empty -qm '{message}'"),
        worktree,
    );
}
