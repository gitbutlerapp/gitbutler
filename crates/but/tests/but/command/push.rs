use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn push_refuses_conflicted_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    // Create a new branch for our test
    env.but("branch new branchB").assert().success();

    // Create a file with initial content and commit it
    env.file("test-file.txt", "line 1\nline 2\nline 3\n");
    env.but("commit -m 'first commit' branchB")
        .assert()
        .success();

    // Add more content that depends on the first commit and commit again
    env.file("test-file.txt", "line 1\nline 2\nline 3\nline 4\n");
    env.but("commit -m 'second commit' branchB")
        .assert()
        .success();

    // add origin as local repository
    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg(".")
        .output()?;

    // Get the first commit's CLI ID from status
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let first_commit_id = status_json["stacks"][0]["branches"][0]["commits"][1]["cliId"]
        .as_str()
        .expect("should have first commit cliId");

    // Rub the first commit to unassigned (zz) - this should create a conflict
    // in the second commit since it depends on the first
    env.but(format!("rub {} zz", first_commit_id))
        .assert()
        .success();

    // Try to push the branch - should fail with an error about conflicted commits
    env.but("push branchB")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Cannot push branch 'branchB': the branch contains 1 conflicted commit.
Conflicted commits: [..]
Please resolve conflicts before pushing using 'but resolve <commit>'.

"#]]);

    Ok(())
}
