use snapbox::IntoData;
use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn wt_alias_lists_worktrees() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.but("wt list")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
No worktrees found

"#]]);
}

/// The typical journey: create an isolated worktree off a workspace branch, do
/// work there, then squash-integrate the result back into the workspace.
#[test]
fn journey_new_list_integrate() {
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
    let remaining_worktrees = std::fs::read_dir(worktrees_dir(&env))
        .unwrap()
        .collect::<Result<Vec<_>, std::io::Error>>()
        .unwrap();
    assert_eq!(
        remaining_worktrees.len(),
        0,
        "the worktree checkout is removed after integration"
    );
    assert_eq!(
        worktree_private_branches(&env),
        Vec::<String>::new(),
        "worktree creation should not leave hidden branches in the main repository"
    );
}

#[test]
fn destroy_by_name_and_by_reference() {
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
        worktree_private_branches(&env),
        Vec::<String>::new(),
        "destroy should not have hidden branches to clean up"
    );
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
fn worktree_private_branches(env: &Sandbox) -> Vec<String> {
    let repo = env.open_repo();
    let refs = repo.references().unwrap();
    refs.prefixed(&b"refs/heads/gitbutler/worktree/"[..])
        .unwrap()
        .filter_map(Result::ok)
        .map(|r| r.name().as_bstr().to_string())
        .collect()
}
