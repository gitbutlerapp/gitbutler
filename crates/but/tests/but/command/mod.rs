//! Put command-specific tests here. They should be focused on what's important for each command.
//!
//! Ideally they *show* the initial state, and the *post* state, to validate the commands actually do what they claim.
//! **Only** test the *happy path* of a typical user journey, while keeping details in unit tests with private module access.

#[cfg(feature = "legacy")]
mod absorb;
mod agent;
mod alias;
#[cfg(feature = "legacy")]
mod amend;
#[cfg(feature = "legacy")]
mod branch;
#[cfg(feature = "legacy")]
mod clean;
#[cfg(feature = "legacy")]
mod comment;
#[cfg(feature = "legacy")]
mod commit;
mod config;
#[cfg(feature = "legacy")]
mod diff;
#[cfg(feature = "legacy")]
mod diff2;
#[cfg(feature = "legacy")]
mod discard;
#[cfg(feature = "legacy")]
mod expand;
#[cfg(unix)]
mod external;
mod format;
mod gui;
mod help;
#[cfg(feature = "legacy")]
mod land;
#[cfg(feature = "legacy")]
mod r#move;
mod onboarding;
#[cfg(feature = "legacy")]
mod open;
#[cfg(feature = "legacy")]
mod pick;
#[cfg(feature = "legacy")]
mod pull;
#[cfg(feature = "legacy")]
mod push;
#[cfg(feature = "legacy")]
mod resolve;
#[cfg(feature = "legacy")]
mod reword;
#[cfg(feature = "legacy")]
mod reword2;
#[cfg(feature = "legacy")]
mod setup;
mod skill;
#[cfg(feature = "legacy")]
mod squash;
#[cfg(feature = "legacy")]
mod status;
mod r#switch;
#[cfg(feature = "legacy")]
mod teardown;
#[cfg(feature = "legacy")]
mod uncommit;
#[cfg(feature = "legacy")]
mod undo;
#[cfg(feature = "legacy")]
mod worktree;

#[cfg(feature = "legacy")]
mod util {
    use crate::utils::{CommandExt as _, Sandbox};

    /// Create two files `filename1` and `filename2` and commit them to `branch`,
    /// each having two lines, `first_line`, then filler, and a last line that are far enough apart to
    /// ensure that they become 2 hunks when changed.
    pub fn commit_two_files_as_two_hunks_each(
        env: &Sandbox,
        branch: &str,
        filename1: &str,
        filename2: &str,
        first_line: &str,
    ) {
        let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;
        env.file(
            filename1,
            format!("{first_line}\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.file(
            filename2,
            format!("{first_line}\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.but(format!(
            "commit -b {branch} -m 'create {filename1} and {filename2}'"
        ))
        .assert()
        .success();
    }

    /// Create a file with `filename`, commit it to `branch`, then edit it once more to have two uncommitted hunks.
    pub fn commit_file_with_worktree_changes_as_two_hunks(
        env: &Sandbox,
        branch: &str,
        filename: &str,
    ) {
        let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;
        env.file(
            filename,
            format!("first\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.but(format!("commit -b {branch} -m {filename} {filename}"))
            .assert()
            .success();
        env.file(
            filename,
            format!("firsta\n{}lasta\n", "line\n".repeat(context_distance)),
        );
    }

    /// Return `but status` JSON output as a parsed value.
    pub fn status_json(env: &Sandbox) -> serde_json::Value {
        let output = env.but("--json status").allow_json().output().unwrap();
        serde_json::from_slice(&output.stdout).expect("status output should be valid JSON")
    }

    /// Return `but status -f` JSON output as a parsed value.
    pub fn status_json_with_files(env: &Sandbox) -> serde_json::Value {
        let output = env.but("--json status -f").allow_json().output().unwrap();
        serde_json::from_slice(&output.stdout).expect("status output should be valid JSON")
    }

    /// Return the CLI IDs for all commits on `branch_name` in `status` output.
    pub fn branch_commit_cli_ids(status: &serde_json::Value, branch_name: &str) -> Vec<String> {
        status["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
            .find(|branch| branch["name"].as_str().unwrap() == branch_name)
            .unwrap()["commits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|commit| commit["cliId"].as_str().unwrap().to_string())
            .collect()
    }

    /// Return the CLI ID of the commit on `branch_name` containing `file_path` in `status` output.
    pub fn branch_commit_cli_id_for_file(
        status: &serde_json::Value,
        branch_name: &str,
        file_path: &str,
    ) -> Option<String> {
        status["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
            .find(|branch| branch["name"].as_str().unwrap() == branch_name)
            .and_then(|branch| {
                branch["commits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find_map(|commit| {
                        let has_file = commit["changes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|change| change["filePath"].as_str().unwrap() == file_path);
                        has_file.then(|| commit["cliId"].as_str().unwrap().to_string())
                    })
            })
    }

    /// Build an isolated `std::process::Command` for `but` with the same environment as the Sandbox.
    pub fn but_std_cmd(env: &Sandbox, args: &str) -> std::process::Command {
        let mut cmd = std::process::Command::new(snapbox::cmd::cargo_bin!("but"));
        cmd.args(shell_words::split(args).unwrap());
        cmd.current_dir(env.projects_root());
        cmd.env("E2E_TEST_APP_DATA_DIR", env.app_data_dir());
        cmd.env("GITBUTLER_CHANGE_ID", "42");
        cmd.env("NOPAGER", "1");
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        but_testsupport::isolate_env_std_cmd_with_additional_removals(
            &mut cmd,
            but::AGENT_ENVIRONMENT_VARIABLES,
        );
        cmd
    }

    /// Find a branch by name in `status` output.
    pub fn find_branch<'a>(
        status: &'a serde_json::Value,
        branch_name: &str,
    ) -> &'a serde_json::Value {
        status["stacks"]
            .as_array()
            .expect("status.stacks should be an array")
            .iter()
            .flat_map(|stack| {
                stack["branches"]
                    .as_array()
                    .into_iter()
                    .flat_map(|branches| branches.iter())
            })
            .find(|branch| branch["name"].as_str() == Some(branch_name))
            .expect("expected branch in status output")
    }

    /// Create a sandbox where pulling the target materializes a conflicted commit on branch A.
    pub fn sandbox_with_conflicted_commit() -> Sandbox {
        let env = Sandbox::init_scenario_with_target_and_default_settings("upstream-conflicted");
        env.setup_metadata_at_target(&["A"], "refs/heads/base");
        env.invoke_git("remote set-url origin .");
        env.but("pull").assert().success();
        env
    }

    /// Create a conflicted edit-mode session by integrating upstream and entering `resolve`.
    pub fn enter_edit_mode_with_conflicted_commit() -> Sandbox {
        let env = sandbox_with_conflicted_commit();
        let status = status_json(&env);
        let branch = find_branch(&status, "A");
        let conflicted_commit_cli_id = branch["commits"]
            .as_array()
            .expect("branch commits should be an array")
            .iter()
            .find(|commit| commit["conflicted"].as_bool() == Some(true))
            .and_then(|commit| commit["cliId"].as_str())
            .expect("should find conflicted commit cli id");

        env.file("uncommitted.txt", "uncommitted work\n");
        env.but(format!("resolve {conflicted_commit_cli_id}"))
            .assert()
            .success();
        env
    }

    /// Whether `file_path` currently appears among the uncommitted changes.
    pub fn uncommitted_contains_file(status: &serde_json::Value, file_path: &str) -> bool {
        status["uncommittedChanges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["filePath"].as_str().unwrap() == file_path)
    }
}
