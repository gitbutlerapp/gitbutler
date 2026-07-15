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

#[test]
fn move_workspace_commit_before_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_empty(&worktree_dir(&env), "worktree position target");
    env.file("ws-position.txt", "positioned from workspace\n");
    env.but("commit B -m 'workspace position source'")
        .assert()
        .success();

    let source = revision(&env, "B")?;
    let target = revision(&env, "feat")?;
    env.but(format!("move {source} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved [..] → before [..]\n\n"])
        .stderr_eq(str![]);

    assert_eq!(tip_message(&env, "feat")?, "worktree position target");
    assert_blob(&env, "feat:ws-position.txt", b"positioned from workspace\n")?;
    assert_missing(&env, "B:ws-position.txt");
    assert_eq!(env.invoke_git("rev-list --count main..feat"), "2");
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn move_worktree_commit_before_workspace_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_file(
        &worktree_dir(&env),
        "wt-position.txt",
        "positioned from worktree\n",
        "worktree position source",
    );

    let source = revision(&env, "feat")?;
    let target = revision(&env, "B")?;
    env.but(format!("move {source} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved [..] → before [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "B:wt-position.txt", b"positioned from worktree\n")?;
    assert_missing(&env, "feat:wt-position.txt");
    assert_eq!(env.invoke_git("rev-list --count main..B"), "2");
    assert!(!worktree_dir(&env).join("wt-position.txt").exists());
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn move_worktree_commit_after_another_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    commit_file(&worktree, "first.txt", "first\n", "first worktree commit");
    let source = revision(&env, "feat")?;
    commit_file(
        &worktree,
        "second.txt",
        "second\n",
        "second worktree commit",
    );
    commit_file(&worktree, "third.txt", "third\n", "third worktree commit");
    let target = revision(&env, "feat")?;

    env.but(format!("move {source} {target} --after"))
        .assert()
        .success()
        .stdout_eq(str!["Moved [..] → after [..]\n\n"])
        .stderr_eq(str![]);

    assert_eq!(tip_message(&env, "feat")?, "first worktree commit");
    assert_eq!(env.invoke_git("rev-list --count main..feat"), "3");
    assert_blob(&env, "feat:first.txt", b"first\n")?;
    assert_blob(&env, "feat:second.txt", b"second\n")?;
    assert_blob(&env, "feat:third.txt", b"third\n")?;
    assert_eq!(but_testsupport::git_status_at_dir(worktree)?, "");
    Ok(())
}

#[test]
fn move_mixed_workspace_and_worktree_commits_to_other_worktree() -> anyhow::Result<()> {
    let env = worktree_env();
    add_worktree(&env, "D", "other");
    env.file("mixed-ws.txt", "workspace source\n");
    env.but("commit B -m 'mixed workspace source'")
        .assert()
        .success();
    commit_file(
        &worktree_dir(&env),
        "mixed-wt.txt",
        "worktree source\n",
        "mixed worktree source",
    );

    let ws_source = revision(&env, "B")?;
    let wt_source = revision(&env, "feat")?;
    env.but(format!(
        "move {ws_source},{wt_source} {}",
        worktree_id_named(&env, "D")?
    ))
    .assert()
    .success()
    .stdout_eq(str!["Moved 2 commits → [other]\n\n"])
    .stderr_eq(str![]);

    assert_blob(&env, "other:mixed-ws.txt", b"workspace source\n")?;
    assert_blob(&env, "other:mixed-wt.txt", b"worktree source\n")?;
    assert_missing(&env, "B:mixed-ws.txt");
    assert_missing(&env, "feat:mixed-wt.txt");
    assert_eq!(env.invoke_git("rev-list --count main..other"), "2");
    assert!(!worktree_dir(&env).join("mixed-wt.txt").exists());
    assert_eq!(
        std::fs::read(worktree_dir_named(&env, "D").join("mixed-ws.txt"))?,
        b"workspace source\n"
    );
    assert_eq!(
        std::fs::read(worktree_dir_named(&env, "D").join("mixed-wt.txt"))?,
        b"worktree source\n"
    );
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree_dir_named(&env, "D"))?,
        ""
    );
    Ok(())
}

#[test]
fn squash_mixed_workspace_and_worktree_commits_into_other_worktree() -> anyhow::Result<()> {
    let env = worktree_env();
    add_worktree(&env, "D", "other");
    commit_empty(&worktree_dir_named(&env, "D"), "mixed squash target");
    env.file("squash-mixed-ws.txt", "workspace source\n");
    env.but("commit B -m 'mixed squash workspace source'")
        .assert()
        .success();
    commit_file(
        &worktree_dir(&env),
        "squash-mixed-wt.txt",
        "worktree source\n",
        "mixed squash worktree source",
    );

    let ws_source = revision(&env, "B")?;
    let wt_source = revision(&env, "feat")?;
    let target = revision(&env, "other")?;
    env.but(format!("squash {ws_source} {wt_source} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Squashed 2 commits → [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "other:squash-mixed-ws.txt", b"workspace source\n")?;
    assert_blob(&env, "other:squash-mixed-wt.txt", b"worktree source\n")?;
    assert_missing(&env, "B:squash-mixed-ws.txt");
    assert_missing(&env, "feat:squash-mixed-wt.txt");
    assert_eq!(env.invoke_git("rev-list --count main..other"), "1");
    assert_eq!(
        std::fs::read(worktree_dir_named(&env, "D").join("squash-mixed-ws.txt"))?,
        b"workspace source\n"
    );
    assert_eq!(
        std::fs::read(worktree_dir_named(&env, "D").join("squash-mixed-wt.txt"))?,
        b"worktree source\n"
    );
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree_dir_named(&env, "D"))?,
        ""
    );
    Ok(())
}

#[test]
fn amend_other_worktree_dirt_into_non_tip_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    add_worktree(&env, "D", "other");
    let target_worktree = worktree_dir(&env);
    commit_empty(&target_worktree, "non-tip amend target");
    let target = revision(&env, "feat")?;
    commit_file(
        &target_worktree,
        "descendant.txt",
        "descendant remains\n",
        "worktree descendant",
    );
    std::fs::write(
        worktree_dir_named(&env, "D").join("cross-dirty.txt"),
        "dirty from other worktree\n",
    )?;

    env.but(format!("rub {} {target}", worktree_id_named(&env, "D")?))
        .assert()
        .success()
        .stdout_eq(str!["Amended changes from worktree D → [..]\n\n"])
        .stderr_eq(str![]);

    assert_eq!(tip_message(&env, "feat")?, "worktree descendant");
    assert_blob(&env, "feat:cross-dirty.txt", b"dirty from other worktree\n")?;
    assert_blob(&env, "feat:descendant.txt", b"descendant remains\n")?;
    assert_eq!(env.invoke_git("rev-list --count main..feat"), "2");
    assert_eq!(but_testsupport::git_status_at_dir(target_worktree)?, "");
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree_dir_named(&env, "D"))?,
        ""
    );
    Ok(())
}

#[test]
fn move_committed_file_between_worktrees() -> anyhow::Result<()> {
    let env = worktree_env();
    add_worktree(&env, "D", "other");
    let source_worktree = worktree_dir(&env);
    std::fs::write(source_worktree.join("cross-file.txt"), "move me\n")?;
    std::fs::write(source_worktree.join("source-remains.txt"), "keep me\n")?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- cross-file.txt source-remains.txt && git commit -qm 'cross worktree file source'",
        &source_worktree,
    );
    commit_empty(&worktree_dir_named(&env, "D"), "cross worktree file target");

    let source = revision(&env, "feat")?;
    let target = revision(&env, "other")?;
    env.but(format!("move {source}:cross-file.txt {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved files between commits!\n\n"])
        .stderr_eq(str![]);

    assert_missing(&env, "feat:cross-file.txt");
    assert_blob(&env, "feat:source-remains.txt", b"keep me\n")?;
    assert_blob(&env, "other:cross-file.txt", b"move me\n")?;
    assert_eq!(but_testsupport::git_status_at_dir(source_worktree)?, "");
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree_dir_named(&env, "D"))?,
        ""
    );
    Ok(())
}

#[test]
fn uncommit_worktree_commit_into_workspace() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_file(
        &worktree_dir(&env),
        "uncommit-wt.txt",
        "becomes workspace dirt\n",
        "worktree uncommit source",
    );
    let source = revision(&env, "feat")?;

    env.but(format!("rub {source} zz"))
        .assert()
        .success()
        .stdout_eq(str!["Uncommitted [..]\n\n"])
        .stderr_eq(str![]);

    assert_missing(&env, "feat:uncommit-wt.txt");
    assert_eq!(
        std::fs::read(env.projects_root().join("uncommit-wt.txt"))?,
        b"becomes workspace dirt\n"
    );
    assert!(env.git_status().contains("uncommit-wt.txt"));
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn uncommit_worktree_file_into_workspace() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    std::fs::write(
        worktree.join("uncommit-file.txt"),
        "becomes workspace dirt\n",
    )?;
    std::fs::write(worktree.join("uncommit-remains.txt"), "stays committed\n")?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- uncommit-file.txt uncommit-remains.txt && git commit -qm 'worktree file uncommit source'",
        &worktree,
    );
    let source = revision(&env, "feat")?;

    env.but(format!("rub {source}:uncommit-file.txt zz"))
        .assert()
        .success()
        .stdout_eq(str!["Uncommitted changes\n\n"])
        .stderr_eq(str![]);

    assert_missing(&env, "feat:uncommit-file.txt");
    assert_blob(&env, "feat:uncommit-remains.txt", b"stays committed\n")?;
    assert_eq!(
        std::fs::read(env.projects_root().join("uncommit-file.txt"))?,
        b"becomes workspace dirt\n"
    );
    assert!(env.git_status().contains("uncommit-file.txt"));
    assert_eq!(but_testsupport::git_status_at_dir(worktree)?, "");
    Ok(())
}

#[test]
fn uncommit_worktree_commit_to_workspace_stack() -> anyhow::Result<()> {
    let env = worktree_env();
    commit_file(
        &worktree_dir(&env),
        "wt-to-stack.txt",
        "assigned to workspace stack\n",
        "worktree to stack source",
    );
    let source = revision(&env, "feat")?;

    env.but(format!("rub {source} B@{{stack}}"))
        .assert()
        .success()
        .stdout_eq(str!["Uncommitted [..] to [B]\n\n"])
        .stderr_eq(str![]);

    assert_missing(&env, "feat:wt-to-stack.txt");
    assert_eq!(
        std::fs::read(env.projects_root().join("wt-to-stack.txt"))?,
        b"assigned to workspace stack\n"
    );
    assert!(stack_assigned_contains_file(
        &status_json(&env)?,
        "B",
        "wt-to-stack.txt"
    ));
    assert!(!worktree_dir(&env).join("wt-to-stack.txt").exists());
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn uncommit_worktree_file_to_workspace_branch() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    std::fs::write(worktree.join("wt-file-to-branch.txt"), "assign me\n")?;
    std::fs::write(worktree.join("wt-file-remains.txt"), "keep committed\n")?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- wt-file-to-branch.txt wt-file-remains.txt && git commit -qm 'worktree file to branch source'",
        &worktree,
    );
    let source = revision(&env, "feat")?;

    env.but(format!("rub {source}:wt-file-to-branch.txt B"))
        .assert()
        .success()
        .stdout_eq(str!["Uncommitted changes\n\n"])
        .stderr_eq(str![]);

    assert_missing(&env, "feat:wt-file-to-branch.txt");
    assert_blob(&env, "feat:wt-file-remains.txt", b"keep committed\n")?;
    assert_eq!(
        std::fs::read(env.projects_root().join("wt-file-to-branch.txt"))?,
        b"assign me\n"
    );
    assert!(stack_assigned_contains_file(
        &status_json(&env)?,
        "B",
        "wt-file-to-branch.txt"
    ));
    assert!(!worktree.join("wt-file-to-branch.txt").exists());
    assert_eq!(
        std::fs::read(worktree.join("wt-file-remains.txt"))?,
        b"keep committed\n"
    );
    assert_eq!(but_testsupport::git_status_at_dir(worktree)?, "");
    Ok(())
}

#[test]
fn uncommit_worktree_commit_refuses_conflicting_workspace_dirt() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    commit_file(
        &worktree,
        "uncommit-conflict.txt",
        "committed in worktree\n",
        "worktree conflict source",
    );
    let source = revision(&env, "feat")?;
    env.file("uncommit-conflict.txt", "dirty in workspace\n");

    env.but(format!("rub {source} zz"))
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Rubbed the wrong way. Cannot uncommit changes into the workspace because they conflict with existing uncommitted changes

"#]]);

    assert_eq!(
        revision(&env, "feat")?,
        source,
        "the source ref does not move when preserving the change would conflict"
    );
    assert_blob(
        &env,
        "feat:uncommit-conflict.txt",
        b"committed in worktree\n",
    )?;
    assert_eq!(
        std::fs::read(env.projects_root().join("uncommit-conflict.txt"))?,
        b"dirty in workspace\n",
        "the conflicting workspace edit is preserved byte-for-byte"
    );
    assert_eq!(
        std::fs::read(worktree.join("uncommit-conflict.txt"))?,
        b"committed in worktree\n",
        "the linked checkout remains on the original source commit"
    );
    assert_eq!(
        but_testsupport::git_status_at_dir(worktree)?,
        "",
        "the linked checkout remains clean"
    );
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
    worktree_dir_named(env, "C")
}

fn worktree_id(env: &Sandbox) -> anyhow::Result<String> {
    worktree_id_named(env, "C")
}

fn worktree_dir_named(env: &Sandbox, name: &str) -> PathBuf {
    env.projects_root()
        .join(".git/gitbutler/worktrees")
        .join(name)
}

fn add_worktree(env: &Sandbox, name: &str, branch: &str) {
    but_testsupport::invoke_bash_at_dir(
        &format!("git worktree add .git/gitbutler/worktrees/{name} -b {branch} main"),
        env.projects_root(),
    );
}

fn worktree_id_named(env: &Sandbox, name: &str) -> anyhow::Result<String> {
    let status = status_json(env)?;
    Ok(status["worktrees"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|worktree| worktree["name"] == name)
        .and_then(|worktree| worktree["cliId"].as_str())
        .expect("active worktree has a CLI id")
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

fn stack_assigned_contains_file(
    status: &serde_json::Value,
    branch_name: &str,
    file_path: &str,
) -> bool {
    status["stacks"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|stack| {
            stack["branches"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|branch| branch["name"] == branch_name)
                && stack["assignedChanges"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|change| change["filePath"] == file_path)
        })
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
