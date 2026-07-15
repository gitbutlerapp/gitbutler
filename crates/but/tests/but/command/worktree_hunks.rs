use std::path::PathBuf;

use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn diff_exposes_hunk_ids_and_rub_consumes_only_the_selected_hunk() -> anyhow::Result<()> {
    let env = worktree_env();
    let (baseline, first_changed, second_changed) = prepare_two_hunks(&env, true)?;
    let file_id = format!("{}:two-hunks.txt", worktree_id(&env)?);
    let first_hunk = format!("{file_id}:#0");
    let second_hunk = format!("{file_id}:#1");

    env.but(format!("diff {file_id}"))
        .assert()
        .success()
        .stdout_eq(format!(
            concat!(
                "──────────────────────────────────╮\n",
                "{first_hunk} two-hunks.txt│\n",
                "──────────────────────────────────╯\n",
                "   1  │-first\n",
                "     1│+first changed\n",
                "   2 2│ line\n",
                "   3 3│ line\n",
                "   4 4│ line\n",
                "──────────────────────────────────╮\n",
                "{second_hunk} two-hunks.txt│\n",
                "──────────────────────────────────╯\n",
                "    7  7│ line\n",
                "    8  8│ line\n",
                "    9  9│ line\n",
                "   10   │-last\n",
                "      10│+last changed\n",
            ),
            first_hunk = first_hunk,
            second_hunk = second_hunk,
        ))
        .stderr_eq(str![]);

    let target = revision(&env, "B")?;
    env.but(format!("rub {first_hunk} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Amended changes from worktree C → [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "B:two-hunks.txt", first_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(worktree_dir(&env).join("two-hunks.txt"))?,
        second_changed,
        "the selected hunk is consumed while the other remains dirty"
    );
    assert_ne!(baseline, second_changed);
    Ok(())
}

#[test]
#[cfg(feature = "but-2")]
fn squash_consumes_only_the_selected_worktree_hunk() -> anyhow::Result<()> {
    let env = worktree_env();
    let (_, first_changed, second_changed) = prepare_two_hunks(&env, true)?;
    let first_hunk = format!("{}:two-hunks.txt:#0", worktree_id(&env)?);
    let target = revision(&env, "B")?;

    env.but(format!(
        "_squash2 {first_hunk} --target {target} --use-target-message"
    ))
    .assert()
    .success()
    .stdout_eq(str!["Amended [..] to create [..]\n\n"])
    .stderr_eq(str![]);

    assert_blob(&env, "B:two-hunks.txt", first_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(worktree_dir(&env).join("two-hunks.txt"))?,
        second_changed,
        "the selected hunk is consumed while the other remains dirty"
    );
    Ok(())
}

#[test]
fn commit_from_worktree_consumes_only_the_selected_hunk() -> anyhow::Result<()> {
    let env = worktree_env();
    let (_, first_changed, _) = prepare_two_hunks(&env, false)?;
    let first_hunk = format!("{}:two-hunks.txt:#0", worktree_id(&env)?);

    env.but(format!(
        "commit -m 'selected linked-worktree hunk' --changes {first_hunk}"
    ))
    .current_dir(worktree_dir(&env))
    .assert()
    .success()
    .stdout_eq(str!["✓ Created commit [..] on branch feat\n\n"])
    .stderr_eq(str![]);

    assert_blob(&env, "feat:two-hunks.txt", first_changed.as_bytes())?;
    let diff = but_testsupport::git_at_dir(worktree_dir(&env))
        .args(["diff", "HEAD", "--unified=0", "--", "two-hunks.txt"])
        .output()?;
    assert!(diff.status.success());
    let diff = String::from_utf8(diff.stdout)?;
    assert!(diff.contains("-last\n+last changed"), "{diff}");
    assert!(!diff.contains("-first\n+first changed"), "{diff}");
    assert!(
        but_testsupport::git_status_at_dir(worktree_dir(&env))?.contains("two-hunks.txt"),
        "the unselected hunk remains dirty"
    );
    Ok(())
}

#[test]
fn commit_from_worktree_combines_two_selected_hunks_in_one_file() -> anyhow::Result<()> {
    let env = worktree_env();
    let (baseline, _, _) = prepare_two_hunks(&env, false)?;
    let file_id = format!("{}:two-hunks.txt", worktree_id(&env)?);
    let hooks = env.projects_root().join(".git/test-hooks");
    std::fs::create_dir_all(&hooks)?;
    let pre_commit = hooks.join("pre-commit");
    std::fs::write(
        &pre_commit,
        concat!(
            "#!/bin/sh\n",
            "git diff --cached --unified=0 -- two-hunks.txt | grep -q '^+first changed$' && \\\n",
            "git diff --cached --unified=0 -- two-hunks.txt | grep -q '^+last changed$'\n",
        ),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&pre_commit, std::fs::Permissions::from_mode(0o755))?;
    }
    env.invoke_git(&format!("config core.hooksPath {}", hooks.display()));

    env.but(format!(
        "commit -m 'both linked-worktree hunks' --changes {file_id}:#0 --changes {file_id}:#1"
    ))
    .current_dir(worktree_dir(&env))
    .assert()
    .success()
    .stdout_eq(str!["✓ Created commit [..] on branch feat\n\n"])
    .stderr_eq(str![]);

    let expected =
        baseline
            .replacen("first", "first changed", 1)
            .replacen("last", "last changed", 1);
    assert_blob(&env, "feat:two-hunks.txt", expected.as_bytes())?;
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
#[cfg(feature = "but-2")]
fn squash_combines_two_selected_worktree_hunks_in_one_file() -> anyhow::Result<()> {
    let env = worktree_env();
    let (baseline, _, _) = prepare_two_hunks(&env, true)?;
    let file_id = format!("{}:two-hunks.txt", worktree_id(&env)?);
    let target = revision(&env, "B")?;

    env.but(format!(
        "_squash2 {file_id}:#0 {file_id}:#1 --target {target} --use-target-message"
    ))
    .assert()
    .success()
    .stdout_eq(str!["Amended [..] to create [..]\n\n"])
    .stderr_eq(str![]);

    let expected =
        baseline
            .replacen("first", "first changed", 1)
            .replacen("last", "last changed", 1);
    assert_blob(&env, "B:two-hunks.txt", expected.as_bytes())?;
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn move_workspace_commit_hunk_to_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    let (baseline, first_changed, second_changed) = committed_move_fixture(&env, true)?;
    let source = revision(&env, "B")?;
    let target = revision(&env, "feat")?;
    let [first_hunk, _] = two_hunk_ids_from_diff(&env, &format!("{source}:two-hunks.txt"))?;

    env.but(format!("move {first_hunk} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved files between commits!\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "B:two-hunks.txt", second_changed.as_bytes())?;
    assert_blob(&env, "feat:two-hunks.txt", first_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("two-hunks.txt"))?,
        second_changed
    );
    assert_eq!(
        std::fs::read_to_string(worktree_dir(&env).join("two-hunks.txt"))?,
        first_changed
    );
    assert_eq!(env.git_status(), "");
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    assert_ne!(baseline, first_changed);
    Ok(())
}

#[test]
fn move_worktree_commit_hunk_to_workspace_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    let (_, first_changed, second_changed) = committed_move_fixture(&env, false)?;
    let source = revision(&env, "feat")?;
    let target = revision(&env, "B")?;
    let [first_hunk, _] = two_hunk_ids_from_diff(&env, &format!("{source}:two-hunks.txt"))?;

    env.but(format!("move {first_hunk} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved files between commits!\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:two-hunks.txt", second_changed.as_bytes())?;
    assert_blob(&env, "B:two-hunks.txt", first_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(worktree_dir(&env).join("two-hunks.txt"))?,
        second_changed
    );
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("two-hunks.txt"))?,
        first_changed
    );
    assert_eq!(env.git_status(), "");
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn move_committed_hunk_between_worktrees() -> anyhow::Result<()> {
    let env = worktree_env();
    let source_worktree = worktree_dir(&env);
    let target_worktree = env.projects_root().join(".git/gitbutler/worktrees/D");
    but_testsupport::invoke_bash_at_dir(
        "git worktree add .git/gitbutler/worktrees/D -b target-wt main",
        env.projects_root(),
    );
    let (baseline, both_changed, first_changed, second_changed) = two_hunk_contents(&env);
    for (worktree, message) in [
        (&source_worktree, "source worktree baseline"),
        (&target_worktree, "target worktree baseline"),
    ] {
        std::fs::write(worktree.join("two-hunks.txt"), &baseline)?;
        but_testsupport::invoke_bash_at_dir(
            &format!("git add -- two-hunks.txt && git commit -qm '{message}'"),
            worktree,
        );
    }
    std::fs::write(source_worktree.join("two-hunks.txt"), &both_changed)?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'source two-hunk commit'",
        &source_worktree,
    );
    let source = revision(&env, "feat")?;
    let target = revision(&env, "target-wt")?;
    let [first_hunk, _] = two_hunk_ids_from_diff(&env, &format!("{source}:two-hunks.txt"))?;

    env.but(format!("move {first_hunk} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved files between commits!\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:two-hunks.txt", second_changed.as_bytes())?;
    assert_blob(&env, "target-wt:two-hunks.txt", first_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(source_worktree.join("two-hunks.txt"))?,
        second_changed
    );
    assert_eq!(
        std::fs::read_to_string(target_worktree.join("two-hunks.txt"))?,
        first_changed
    );
    assert_eq!(env.git_status(), "");
    assert_eq!(but_testsupport::git_status_at_dir(source_worktree)?, "");
    assert_eq!(but_testsupport::git_status_at_dir(target_worktree)?, "");
    Ok(())
}

#[test]
fn move_committed_hunk_between_commits_on_one_worktree_branch() -> anyhow::Result<()> {
    let env = worktree_env();
    let worktree = worktree_dir(&env);
    let (baseline, both_changed, first_changed, _) = two_hunk_contents(&env);
    std::fs::write(worktree.join("two-hunks.txt"), &baseline)?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'worktree baseline' && git commit --allow-empty -qm 'hunk target'",
        &worktree,
    );
    std::fs::write(worktree.join("two-hunks.txt"), &both_changed)?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'hunk source'",
        &worktree,
    );
    let source = revision(&env, "feat")?;
    let target = revision(&env, "feat^")?;
    let [first_hunk, _] = two_hunk_ids_from_diff(&env, &format!("{source}:two-hunks.txt"))?;

    env.but(format!("move {first_hunk} {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Moved files between commits!\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat^:two-hunks.txt", first_changed.as_bytes())?;
    assert_blob(&env, "feat:two-hunks.txt", both_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(worktree.join("two-hunks.txt"))?,
        both_changed
    );
    assert_eq!(env.git_status(), "");
    assert_eq!(but_testsupport::git_status_at_dir(worktree)?, "");
    Ok(())
}

#[test]
fn uncommit_worktree_commit_hunk_into_workspace() -> anyhow::Result<()> {
    let env = worktree_env();
    let (_, first_changed, second_changed) = committed_move_fixture(&env, false)?;
    let source = revision(&env, "feat")?;
    let [first_hunk, _] = two_hunk_ids_from_diff(&env, &format!("{source}:two-hunks.txt"))?;

    env.but(format!("rub {first_hunk} zz"))
        .assert()
        .success()
        .stdout_eq(str!["Uncommitted changes\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:two-hunks.txt", second_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("two-hunks.txt"))?,
        first_changed
    );
    assert_eq!(
        std::fs::read_to_string(worktree_dir(&env).join("two-hunks.txt"))?,
        second_changed
    );
    assert_eq!(env.git_status(), " M two-hunks.txt\n");
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
#[cfg(feature = "but-2")]
fn squash_two_committed_hunks_from_worktree_into_workspace_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    let (baseline, _, _) = committed_move_fixture(&env, false)?;
    let source = revision(&env, "feat")?;
    let target = revision(&env, "B")?;
    let [first_hunk, second_hunk] =
        two_hunk_ids_from_diff(&env, &format!("{source}:two-hunks.txt"))?;

    env.but(format!(
        "_squash2 {first_hunk} {second_hunk} --target {target} --use-target-message"
    ))
    .assert()
    .success()
    .stdout_eq(str!["Amended [..] to create [..]\n\n"])
    .stderr_eq(str![]);

    let both_changed =
        baseline
            .replacen("first", "first changed", 1)
            .replacen("last", "last changed", 1);
    assert_blob(&env, "feat:two-hunks.txt", baseline.as_bytes())?;
    assert_blob(&env, "B:two-hunks.txt", both_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(worktree_dir(&env).join("two-hunks.txt"))?,
        baseline
    );
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("two-hunks.txt"))?,
        both_changed
    );
    assert_eq!(env.git_status(), "");
    assert_eq!(but_testsupport::git_status_at_dir(worktree_dir(&env))?, "");
    Ok(())
}

#[test]
fn rub_workspace_dirty_hunk_into_worktree_commit() -> anyhow::Result<()> {
    let env = worktree_env();
    let (baseline, both_changed, first_changed, second_changed) = two_hunk_contents(&env);
    env.file("two-hunks.txt", &baseline);
    env.but("commit B -m 'workspace baseline'")
        .assert()
        .success();
    let worktree = worktree_dir(&env);
    std::fs::write(worktree.join("two-hunks.txt"), &baseline)?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'worktree target'",
        &worktree,
    );
    env.file("two-hunks.txt", both_changed);
    let [first_hunk, _] = two_hunk_ids_from_diff(&env, "two-hunks.txt")?;
    let target = revision(&env, "feat")?;

    env.but(format!("rub {first_hunk} {target}"))
        .assert()
        .success()
        .stdout_eq(str![
            "Amended a hunk in two-hunks.txt in the uncommitted area → [..]\n\n"
        ])
        .stderr_eq(str![]);

    assert_blob(&env, "feat:two-hunks.txt", first_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("two-hunks.txt"))?,
        second_changed
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("two-hunks.txt"))?,
        first_changed
    );
    assert!(env.git_status().contains("two-hunks.txt"));
    assert_eq!(but_testsupport::git_status_at_dir(worktree)?, "");
    Ok(())
}

#[test]
fn rub_dirty_hunk_between_two_worktree_commits() -> anyhow::Result<()> {
    let env = worktree_env();
    but_testsupport::invoke_bash_at_dir(
        "git worktree add .git/gitbutler/worktrees/D -b target-wt main",
        env.projects_root(),
    );
    let source_worktree = worktree_dir(&env);
    let target_worktree = env.projects_root().join(".git/gitbutler/worktrees/D");
    let (baseline, both_changed, first_changed, second_changed) = two_hunk_contents(&env);
    for (worktree, message) in [
        (&source_worktree, "source worktree baseline"),
        (&target_worktree, "target worktree baseline"),
    ] {
        std::fs::write(worktree.join("two-hunks.txt"), &baseline)?;
        but_testsupport::invoke_bash_at_dir(
            &format!("git add -- two-hunks.txt && git commit -qm '{message}'"),
            worktree,
        );
    }
    std::fs::write(source_worktree.join("two-hunks.txt"), both_changed)?;
    let source_file = format!("{}:two-hunks.txt", worktree_id(&env)?);
    let target = revision(&env, "target-wt")?;

    env.but(format!("rub {source_file}:#0 {target}"))
        .assert()
        .success()
        .stdout_eq(str!["Amended changes from worktree C → [..]\n\n"])
        .stderr_eq(str![]);

    assert_blob(&env, "target-wt:two-hunks.txt", first_changed.as_bytes())?;
    assert_eq!(
        std::fs::read_to_string(source_worktree.join("two-hunks.txt"))?,
        second_changed
    );
    assert_eq!(
        std::fs::read_to_string(target_worktree.join("two-hunks.txt"))?,
        first_changed
    );
    assert!(but_testsupport::git_status_at_dir(source_worktree)?.contains("two-hunks.txt"));
    assert_eq!(but_testsupport::git_status_at_dir(target_worktree)?, "");
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

fn prepare_two_hunks(
    env: &Sandbox,
    baseline_on_workspace_target: bool,
) -> anyhow::Result<(String, String, String)> {
    let filler = "line\n".repeat((env.app_settings().context_lines * 2 + 2) as usize);
    let baseline = format!("first\n{filler}last\n");
    let both_changed = format!("first changed\n{filler}last changed\n");
    let first_changed = format!("first changed\n{filler}last\n");
    let second_changed = format!("first\n{filler}last changed\n");

    if baseline_on_workspace_target {
        env.file("two-hunks.txt", &baseline);
        env.but("commit B -m 'workspace baseline'")
            .assert()
            .success();
    }

    let worktree = worktree_dir(env);
    std::fs::write(worktree.join("two-hunks.txt"), &baseline)?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'worktree baseline'",
        &worktree,
    );
    std::fs::write(worktree.join("two-hunks.txt"), both_changed)?;
    Ok((baseline, first_changed, second_changed))
}

fn committed_move_fixture(
    env: &Sandbox,
    source_in_workspace: bool,
) -> anyhow::Result<(String, String, String)> {
    let filler = "line\n".repeat((env.app_settings().context_lines * 2 + 2) as usize);
    let baseline = format!("first\n{filler}last\n");
    let both_changed = format!("first changed\n{filler}last changed\n");
    let first_changed = format!("first changed\n{filler}last\n");
    let second_changed = format!("first\n{filler}last changed\n");
    let worktree = worktree_dir(env);

    env.file("two-hunks.txt", &baseline);
    env.but("commit B -m 'workspace baseline'")
        .assert()
        .success();
    std::fs::write(worktree.join("two-hunks.txt"), &baseline)?;
    but_testsupport::invoke_bash_at_dir(
        "git add -- two-hunks.txt && git commit -qm 'worktree baseline'",
        &worktree,
    );

    if source_in_workspace {
        env.file("two-hunks.txt", &both_changed);
        env.but("commit B -m 'workspace two-hunk source'")
            .assert()
            .success();
    } else {
        std::fs::write(worktree.join("two-hunks.txt"), both_changed)?;
        but_testsupport::invoke_bash_at_dir(
            "git add -- two-hunks.txt && git commit -qm 'worktree two-hunk source'",
            &worktree,
        );
    }
    Ok((baseline, first_changed, second_changed))
}

fn two_hunk_contents(env: &Sandbox) -> (String, String, String, String) {
    let filler = "line\n".repeat((env.app_settings().context_lines * 2 + 2) as usize);
    (
        format!("first\n{filler}last\n"),
        format!("first changed\n{filler}last changed\n"),
        format!("first changed\n{filler}last\n"),
        format!("first\n{filler}last changed\n"),
    )
}

fn two_hunk_ids_from_diff(env: &Sandbox, entity: &str) -> anyhow::Result<[String; 2]> {
    let output = env.but(format!("diff {entity}")).output()?;
    assert!(output.status.success(), "diff should succeed");
    let stdout = String::from_utf8(output.stdout)?;
    let ids = stdout
        .lines()
        .filter_map(|line| line.strip_suffix(" two-hunks.txt│"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let [first, second]: [String; 2] = ids.try_into().expect("two committed hunk IDs");
    let first_border = "─".repeat(first.len() + 1 + "two-hunks.txt".len());
    let second_border = "─".repeat(second.len() + 1 + "two-hunks.txt".len());
    env.but(format!("diff {entity}"))
        .assert()
        .success()
        .stdout_eq(format!(
            concat!(
                "{first_border}╮\n",
                "{first} two-hunks.txt│\n",
                "{first_border}╯\n",
                "   1  │-first\n",
                "     1│+first changed\n",
                "   2 2│ line\n",
                "   3 3│ line\n",
                "   4 4│ line\n",
                "{second_border}╮\n",
                "{second} two-hunks.txt│\n",
                "{second_border}╯\n",
                "    7  7│ line\n",
                "    8  8│ line\n",
                "    9  9│ line\n",
                "   10   │-last\n",
                "      10│+last changed\n",
            ),
            first_border = first_border,
            second_border = second_border,
            first = first,
            second = second,
        ))
        .stderr_eq(str![]);
    Ok([first, second])
}

fn worktree_dir(env: &Sandbox) -> PathBuf {
    env.projects_root().join(".git/gitbutler/worktrees/C")
}

fn worktree_id(env: &Sandbox) -> anyhow::Result<String> {
    let output = env
        .but("--format json status")
        .allow_json()
        .env("NO_BG_TASKS", "1")
        .output()?;
    assert!(output.status.success(), "status should succeed");
    let status: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    Ok(status["worktrees"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|worktree| worktree["name"] == "C")
        .and_then(|worktree| worktree["cliId"].as_str())
        .expect("active worktree has a CLI id")
        .to_owned())
}

fn revision(env: &Sandbox, revision: &str) -> anyhow::Result<gix::ObjectId> {
    Ok(env.open_repo().rev_parse_single(revision)?.detach())
}

fn assert_blob(env: &Sandbox, revision: &str, expected: &[u8]) -> anyhow::Result<()> {
    assert_eq!(
        env.open_repo().rev_parse_single(revision)?.object()?.data,
        expected,
        "{revision} has the expected selected hunk"
    );
    Ok(())
}
