use snapbox::IntoData;
use snapbox::str;

use crate::utils::Sandbox;

/// The typical journey: create an isolated worktree off a workspace branch, do
/// work there, then squash-integrate the result back into the workspace.
#[test]
fn journey_new_list_integrate() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 9477ae7 (A) add A
* | d3e2ba3 (B) add B
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );
    env.setup_metadata(&["A", "B"]);

    env.but("worktree new A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created worktree at: [..]
Reference: refs/heads/A

"#]]);

    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Path: [..]
Reference: refs/heads/A
Base: [..]


"#]]);

    // Do some work in the worktree, like an agent would.
    let wt_id = single_worktree_id(&env);
    but_testsupport::invoke_bash_at_dir(
        r#"echo "from worktree" > wt-file.txt && git add . && git commit -qm "worktree work""#,
        &worktrees_dir(&env).join(&wt_id),
    );

    env.but(format!("worktree integrate {wt_id} --dry"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Integration status for worktree: [..]
Target: refs/heads/A
Status: Integratable
  No conflicts expected

"#]]);

    env.but(format!("worktree integrate {wt_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Successfully integrated worktree: [..]
Target: refs/heads/A

"#]]);

    let log = env.git_log();
    assert!(
        log.contains("(A) Integrated worktree"),
        "the worktree work is squashed into a commit on the branch it was created from: {log}"
    );
    assert!(
        env.projects_root().join("wt-file.txt").exists(),
        "the integrated change is checked out in the main worktree"
    );
    let remaining_worktrees =
        std::fs::read_dir(worktrees_dir(&env))?.collect::<Result<Vec<_>, std::io::Error>>()?;
    assert_eq!(
        remaining_worktrees.len(),
        0,
        "the worktree checkout is removed after integration"
    );
    assert_eq!(
        worktree_private_branches(&env)?,
        Vec::<String>::new(),
        "worktree creation should not leave hidden branches in the main repository"
    );

    Ok(())
}

#[test]
fn destroy_by_name_and_by_reference() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("worktree new A").assert().success();
    let a_id = single_worktree_id(&env);
    env.but("worktree new B").assert().success();

    env.but(format!("worktree destroy {a_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Destroyed worktree: [..]

"#]]);

    env.but("worktree destroy B --reference")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Destroyed 1 worktree(s) for reference: refs/heads/B
  - [..]

"#]]);

    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
No worktrees found

"#]]);
    assert_eq!(
        worktree_private_branches(&env)?,
        Vec::<String>::new(),
        "destroy should not have hidden branches to clean up"
    );

    Ok(())
}

#[test]
fn integrate_dry_run_reports_worktree_without_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("worktree new A").assert().success();
    let wt_id = single_worktree_id(&env);

    env.but(format!("worktree integrate {wt_id} --dry"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Integration status for worktree: [..]
Target: refs/heads/A
Status: Nothing to integrate - the worktree has no changes

"#]]);
}

/// Experimental worktree listing in `but status` plus the archive/unarchive
/// subcommands, all gated behind the `worktreeManipulation` feature flag.
#[test]
fn archive_unarchive_and_status_listing() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow("two-worktrees");
    env.setup_metadata(&["A", "B"]);

    // The flag is off by default, so archive/unarchive refuse to run.
    env.but("worktree unarchive A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: worktree manipulation is not enabled (featureFlags.worktreeManipulation)

"#]]);

    enable_worktree_manipulation(&env)?;

    // The first flag-on run adopts the pre-existing worktrees as archived,
    // so no worktree group shows up yet.
    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A 📁 gitbutler/worktrees/A]
┊┊
┊╭┄┄(upstream: on origin/A)
┊●   197ddce A-remote (no changes)
┊-
┊●   4c4624e A (no changes)
├╯
┊
┊╭┄h0 [B 📁 gitbutler/worktrees/B]
┊●   3e01e28 B (no changes)
├╯
┊
┊● 8dc508f (upstream) ⏫ 1 commit
├╯ 8dc508f (common base) 2000-01-02 M-advanced

Hint: run `but help` for all commands

"#]]);

    env.but("worktree unarchive A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Unarchived worktree: A

"#]]);

    // The active worktree is listed as a branch-style group after the stacks,
    // with its own CLI id chip and the path relative to the main worktree.
    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A 📁 gitbutler/worktrees/A]
┊┊
┊╭┄┄(upstream: on origin/A)
┊●   197ddce A-remote (no changes)
┊-
┊●   4c4624e A (no changes)
├╯
┊
┊╭┄h0 [B 📁 gitbutler/worktrees/B]
┊●   3e01e28 B (no changes)
├╯
┊
┊╭┄k0 [A] (.git/gitbutler/worktrees/A)
┊●   4c4624e A
├╯
┊
┊● 8dc508f (upstream) ⏫ 1 commit
├╯ 8dc508f (common base) 2000-01-02 M-advanced

Hint: run `but help` for all commands

"#]]);

    // The JSON output lists it, too.
    assert_eq!(active_worktree_names(&env)?, ["A"]);

    // Archiving works with the CLI id chip from the listing and hides the
    // worktree again.
    env.but("worktree archive k0")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Archived worktree: A

"#]]);
    assert_eq!(active_worktree_names(&env)?, Vec::<String>::new());

    Ok(())
}

/// Amend uncommitted changes from a linked worktree into its own branch's head
/// commit (the worktree checkout follows the rebase) and into another branch's
/// commit (the worktree tip stays, the duplicate is discarded afterwards).
#[test]
fn amend_from_worktree() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow("two-worktrees");
    env.setup_metadata(&["A", "B"]);

    // Gated on the feature flag like its siblings.
    env.but("worktree amend A 12345678 --changes some.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: worktree manipulation is not enabled (featureFlags.worktreeManipulation)

"#]]);

    enable_worktree_manipulation(&env)?;
    env.but("worktree unarchive A").assert().success();
    env.but("worktree unarchive B").assert().success();

    let wt_a = worktrees_dir(&env).join("A");
    std::fs::write(wt_a.join("own.txt"), "own\n")?;
    std::fs::write(wt_a.join("cross.txt"), "cross\n")?;

    // Paths that have no uncommitted change in the worktree are refused.
    let repo = env.open_repo();
    let a_head = repo.rev_parse_single("A")?.detach();
    env.but(format!("worktree amend A {a_head} --changes missing.txt"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Worktree A has no uncommitted change at path 'missing.txt'

"#]]);

    // Amend one of two dirty files into the head commit of the worktree's own
    // branch.
    env.but(format!("worktree amend A {a_head} --changes own.txt"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Amended changes from worktree A into [..]
The amended changes were moved out of the worktree.

"#]]);

    let repo = env.open_repo();
    let new_a_head = repo.rev_parse_single("A")?.detach();
    assert_ne!(new_a_head, a_head, "the worktree's branch moved");
    assert_eq!(
        repo.rev_parse_single("A:own.txt")?.object()?.data,
        b"own\n",
        "the amended commit contains the worktree's uncommitted content"
    );
    let status = but_testsupport::git_status_at_dir(&wt_a)?;
    assert!(
        !status.contains("own.txt"),
        "the consumed change no longer shows up as uncommitted in the worktree: {status}"
    );
    assert!(
        status.contains("cross.txt"),
        "the dirty file that wasn't amended survives: {status}"
    );

    // Amend the remaining dirty file into a commit on another worktree's branch.
    let b_commit = repo.rev_parse_single("B")?.detach();
    env.but(format!("worktree amend A {b_commit} --changes cross.txt"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Amended changes from worktree A into [..]
The amended changes were moved out of the worktree.

"#]]);

    let repo = env.open_repo();
    assert_eq!(
        repo.rev_parse_single("A")?.detach(),
        new_a_head,
        "the source worktree's branch is untouched when the target lives elsewhere"
    );
    assert_eq!(
        repo.rev_parse_single("B:cross.txt")?.object()?.data,
        b"cross\n",
        "the change landed in the other branch's commit"
    );
    assert_eq!(
        but_testsupport::git_status_at_dir(&wt_a)?,
        "",
        "the now-committed change was discarded from the source worktree"
    );
    assert!(
        worktrees_dir(&env).join("B").join("cross.txt").exists(),
        "the target worktree's checkout followed its rebased branch"
    );

    Ok(())
}

/// Turn on the `worktreeManipulation` feature flag in the sandbox settings.
fn enable_worktree_manipulation(env: &Sandbox) -> anyhow::Result<()> {
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    let mut settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path)?)?;
    settings["featureFlags"]["worktreeManipulation"] = serde_json::Value::Bool(true);
    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}

/// The names of all active worktrees according to `but status --format json`.
fn active_worktree_names(env: &Sandbox) -> anyhow::Result<Vec<String>> {
    let output = env
        .but("--format json status")
        .env_remove("BUT_OUTPUT_FORMAT")
        .env("NO_BG_TASKS", "1")
        .output()?;
    anyhow::ensure!(output.status.success(), "status --format json failed");
    let status: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    Ok(status["worktrees"]
        .as_array()
        .map(|worktrees| {
            worktrees
                .iter()
                .map(|worktree| worktree["name"].as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default())
}

fn worktrees_dir(env: &Sandbox) -> std::path::PathBuf {
    env.projects_root().join(".git/gitbutler/worktrees")
}

/// The id of the only worktree that currently exists.
fn single_worktree_id(env: &Sandbox) -> String {
    let mut entries: Vec<_> = std::fs::read_dir(worktrees_dir(env))
        .expect("worktrees directory exists")
        .map(|e| e.expect("readable directory entry"))
        .collect();
    assert_eq!(entries.len(), 1, "exactly one worktree is expected");
    entries
        .pop()
        .expect("one entry")
        .file_name()
        .to_string_lossy()
        .into_owned()
}

/// All local branches under the private worktree namespace.
fn worktree_private_branches(env: &Sandbox) -> anyhow::Result<Vec<String>> {
    let repo = env.open_repo();
    let refs = repo.references()?;
    Ok(refs
        .prefixed(b"refs/heads/gitbutler/worktree/".as_ref())?
        .filter_map(Result::ok)
        .map(|r| r.name().as_bstr().to_string())
        .collect())
}
