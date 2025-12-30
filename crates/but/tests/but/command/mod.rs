//! Put command-specific tests here. They should be focused on what's important for each command.
//! Ideally they *show* the initial state, and the *post* state, to validate the commands actually do what they claim.
//! **Only** test the *happy path* of a typical user journey, while keeping details in unit tests with private module access.
#[cfg(feature = "legacy")]
mod absorb;
mod branch;
#[cfg(feature = "legacy")]
mod commit;
mod format;
mod gui;
mod help;
#[cfg(feature = "legacy")]
mod reword;
#[cfg(feature = "legacy")]
mod rub;
#[cfg(feature = "legacy")]
mod status;

mod util {
    use crate::utils::Sandbox;

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
            "commit {branch} -m 'create {filename1} and {filename2}'"
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
        env.but(format!("commit {branch} -m {filename}"))
            .assert()
            .success();
        env.file(
            filename,
            format!("firsta\n{}lasta\n", "line\n".repeat(context_distance)),
        );
    }
}
