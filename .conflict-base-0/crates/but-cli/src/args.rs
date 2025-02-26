use std::path::PathBuf;

#[derive(Debug, clap::Parser)]
#[clap(name = "gitbutler-cli", about = "A CLI for GitButler", version = option_env!("GIX_VERSION"))]
pub struct Args {
    /// Enable tracing for debug and performance information printed to stderr.
    #[clap(short = 'd', long)]
    pub trace: bool,
    /// Run as if gitbutler-cli was started in PATH instead of the current working directory.
    #[clap(short = 'C', long, default_value = ".", value_name = "PATH")]
    pub current_dir: PathBuf,
    /// The location of the directory to contain app data.
    ///
    /// Defaults to the standard location on this platform if unset.
    #[clap(short = 'a', long, env = "GITBUTLER_CLI_DATA_DIR")]
    pub app_data_dir: Option<PathBuf>,
    /// A suffix like `dev` to refer to projects of the development version of the application.
    ///
    /// The production version is used if unset.
    #[clap(short = 's', long)]
    pub app_suffix: Option<String>,

    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Commit or amend all worktree changes to a new commit.
    Commit {
        /// The message of the new commit.
        #[clap(long, short = 'm')]
        message: Option<String>,
        /// Amend to the current or given commit.
        #[clap(long)]
        amend: bool,
        /// The revspec to create the commit on top of, or the commit to amend to.
        #[clap(long)]
        parent: Option<String>,
    },
    /// Update the local workspace against an updated remote or target branch.
    Status {
        /// Also compute unified diffs for each tree-change.
        #[clap(long, short = 'd')]
        unified_diff: bool,
    },
    /// Calculate the changes between two commits.
    CommitChanges {
        /// Also compute unified diffs for each tree-change.
        #[clap(long, short = 'd')]
        unified_diff: bool,
        /// The revspec to the commit that the returned changes turn the previous commit into.
        current_commit: String,
        /// The revspec to the previous commit that the returned changes transform into current commit.
        previous_commit: Option<String>,
    },
    /// Return the dependencies of worktree changes with the commits that last changed them.
    #[clap(visible_alias = "dep")]
    HunkDependency,
    /// Returns the list of stacks that are currently part of the GitButler workspace.
    Stacks,
    /// Return all stack branches related to the given `id`.
    StackBranches { id: String },
    /// Returns all commits for the branch with the given `name` in the stack with the given `id`.
    StackBranchCommits { id: String, name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
